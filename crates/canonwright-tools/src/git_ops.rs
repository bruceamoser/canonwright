//! The git operations the assistant was missing.
//!
//! The registry already had `git_status`, `git_diff`, `git_log`, `git_commit`,
//! `git_branch` and `git_checkout` — enough to read a repository and write one
//! commit, and not enough to *work* in one. A writer whose manuscript lives in a
//! repository needs to stage, push, pull, tag a draft, and occasionally undo a
//! commit. This module is the rest of that surface.
//!
//! Everything here shells out through the same `run_tool_shell` path as
//! `git_commit`, so it inherits the workspace confinement, the approval gate and
//! the audit trail rather than reimplementing them.

use async_trait::async_trait;
use canonwright_governance::RiskLevel;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolDefinition, ToolError, ToolResult, run_tool_shell, sh_quote};

/// Quote a list of paths for the shell, refusing anything that is not a path.
fn quoted_paths(paths: &[String]) -> Result<String, ToolError> {
    if paths.is_empty() {
        return Err(ToolError::InvalidArgument(
            "provide at least one path, or all=true".to_owned(),
        ));
    }
    Ok(paths
        .iter()
        .map(|path| sh_quote(path))
        .collect::<Vec<_>>()
        .join(" "))
}

fn shell(
    context: &ToolContext,
    command: String,
) -> impl std::future::Future<Output = Result<ToolResult, ToolError>> + '_ {
    run_tool_shell(context, command)
}

/// Stage paths, or everything.
pub struct GitStageTool;

#[async_trait]
impl Tool for GitStageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_stage",
            "Stage paths for the next commit. `all: true` stages every change, including \
             deletions and new files. Requires Full access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "paths": {"type": "array", "items": {"type": "string"}},
                "all": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let all = crate::optional_bool(&arguments, "all").unwrap_or(false);
        let paths = crate::string_array_arg_or_empty(&arguments, "paths")?;
        let command = if all {
            "git add -A".to_owned()
        } else {
            format!("git add -- {}", quoted_paths(&paths)?)
        };
        shell(context, command).await
    }
}

/// Unstage paths, or everything.
pub struct GitUnstageTool;

#[async_trait]
impl Tool for GitUnstageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_unstage",
            "Take paths back out of the index. `all: true` unstages everything. \
             The working tree is not touched. Requires Full access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "paths": {"type": "array", "items": {"type": "string"}},
                "all": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let all = crate::optional_bool(&arguments, "all").unwrap_or(false);
        let paths = crate::string_array_arg_or_empty(&arguments, "paths")?;
        let command = if all {
            "git reset".to_owned()
        } else {
            format!("git restore --staged -- {}", quoted_paths(&paths)?)
        };
        shell(context, command).await
    }
}

/// Send commits to a remote.
pub struct GitPushTool;

#[async_trait]
impl Tool for GitPushTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_push",
            "Push the current branch. Pass `set_upstream: true` the first time a branch is \
             pushed. Requires Full access and network access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "remote": {"type": "string"},
                "branch": {"type": "string"},
                "set_upstream": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let mut command = String::from("git push");
        if crate::optional_bool(&arguments, "set_upstream").unwrap_or(false) {
            command.push_str(" -u");
        }
        if let Some(remote) = arguments.get("remote").and_then(Value::as_str) {
            command.push(' ');
            command.push_str(&sh_quote(remote));
        }
        if let Some(branch) = arguments.get("branch").and_then(Value::as_str) {
            command.push(' ');
            command.push_str(&sh_quote(branch));
        }
        shell(context, command).await
    }
}

/// Bring commits down from a remote.
pub struct GitPullTool;

#[async_trait]
impl Tool for GitPullTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_pull",
            "Pull the current branch. Defaults to `--ff-only`, which refuses to invent a merge \
             commit in someone's manuscript; pass `rebase: false, ff_only: false` to allow a \
             real merge. Requires Full access and network access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "remote": {"type": "string"},
                "branch": {"type": "string"},
                "rebase": {"type": "boolean"},
                "ff_only": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let rebase = crate::optional_bool(&arguments, "rebase").unwrap_or(false);
        let ff_only = crate::optional_bool(&arguments, "ff_only").unwrap_or(!rebase);
        let mut command = String::from("git pull");
        if rebase {
            command.push_str(" --rebase");
        } else if ff_only {
            command.push_str(" --ff-only");
        }
        if let Some(remote) = arguments.get("remote").and_then(Value::as_str) {
            command.push(' ');
            command.push_str(&sh_quote(remote));
        }
        if let Some(branch) = arguments.get("branch").and_then(Value::as_str) {
            command.push(' ');
            command.push_str(&sh_quote(branch));
        }
        shell(context, command).await
    }
}

/// Update remote-tracking refs without touching the working tree.
pub struct GitFetchTool;

#[async_trait]
impl Tool for GitFetchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_fetch",
            "Fetch from a remote without changing the working tree. Use before deciding whether \
             to pull. Requires Full access and network access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "remote": {"type": "string"},
                "prune": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let mut command = String::from("git fetch");
        if crate::optional_bool(&arguments, "prune").unwrap_or(false) {
            command.push_str(" --prune");
        }
        if let Some(remote) = arguments.get("remote").and_then(Value::as_str) {
            command.push(' ');
            command.push_str(&sh_quote(remote));
        }
        shell(context, command).await
    }
}

/// Merge a branch into the current one.
pub struct GitMergeTool;

#[async_trait]
impl Tool for GitMergeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_merge",
            "Merge a branch into the current one. `ff_only: true` refuses anything that would \
             need a merge commit. A conflict is reported, not resolved. Requires Full access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "branch": {"type": "string"},
                "ff_only": {"type": "boolean"},
                "no_ff": {"type": "boolean"},
                "message": {"type": "string"}
            },
            "required": ["branch"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let branch = crate::string_arg(&arguments, "branch")?;
        let mut command = String::from("git merge");
        if crate::optional_bool(&arguments, "ff_only").unwrap_or(false) {
            command.push_str(" --ff-only");
        }
        if crate::optional_bool(&arguments, "no_ff").unwrap_or(false) {
            command.push_str(" --no-ff");
        }
        if let Some(message) = arguments.get("message").and_then(Value::as_str) {
            command.push_str(" -m ");
            command.push_str(&sh_quote(message));
        }
        command.push(' ');
        command.push_str(&sh_quote(&branch));
        shell(context, command).await
    }
}

/// Tag a revision, and optionally push the tag.
pub struct GitTagTool;

#[async_trait]
impl Tool for GitTagTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_tag",
            "Create an annotated tag — a draft milestone, a released version — and optionally \
             push it. `list: true` lists existing tags instead. Requires Full access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "message": {"type": "string"},
                "list": {"type": "boolean"},
                "push": {"type": "boolean"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if crate::optional_bool(&arguments, "list").unwrap_or(false) {
            return shell(context, "git tag --list --sort=-creatordate".to_owned()).await;
        }
        let name = crate::string_arg(&arguments, "name")?;
        let message = arguments
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(&name);
        let tagged = shell(
            context,
            format!("git tag -a {} -m {}", sh_quote(&name), sh_quote(message)),
        )
        .await?;
        if !tagged.metadata["success"].as_bool().unwrap_or(false)
            || !crate::optional_bool(&arguments, "push").unwrap_or(false)
        {
            return Ok(tagged);
        }
        let pushed = shell(context, format!("git push origin {}", sh_quote(&name))).await?;
        let push_metadata = pushed.metadata.clone();
        Ok(after_tag_metadata(tagged, push_metadata))
    }
}

fn after_tag_metadata(tagged: ToolResult, pushed: Value) -> ToolResult {
    let mut result = tagged;
    let existing = result.metadata.take();
    result.metadata = json!({"tag": existing, "push": pushed});
    result
}

/// Inspect or edit remotes.
pub struct GitRemoteTool;

#[async_trait]
impl Tool for GitRemoteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_remote",
            "List remotes, add one, or remove one. Use this to attach a manuscript to its \
             GitHub repository. Requires Full access for changes.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "add", "remove"]},
                "name": {"type": "string"},
                "url": {"type": "string"}
            },
            "required": ["action"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        match crate::string_arg(&arguments, "action")?.as_str() {
            "list" => shell(context, "git remote -v".to_owned()).await,
            "add" => {
                let name = crate::string_arg(&arguments, "name")?;
                let url = crate::string_arg(&arguments, "url")?;
                shell(
                    context,
                    format!("git remote add {} {}", sh_quote(&name), sh_quote(&url)),
                )
                .await
            }
            "remove" => {
                let name = crate::string_arg(&arguments, "name")?;
                shell(context, format!("git remote remove {}", sh_quote(&name))).await
            }
            other => Err(ToolError::InvalidArgument(format!(
                "action must be list, add or remove, not {other:?}"
            ))),
        }
    }
}

/// Start a repository in the project directory.
pub struct GitInitTool;

#[async_trait]
impl Tool for GitInitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "git_init",
            "Initialise a git repository in the project directory, with `main` as the initial \
             branch. Does nothing if one already exists. Requires Full access.",
            RiskLevel::Execute,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "initial_branch": {"type": "string"}
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let branch = arguments
            .get("initial_branch")
            .and_then(Value::as_str)
            .unwrap_or("main");
        let command = format!(
            "if [ -d .git ]; then echo 'already a repository'; else git init -b {}; fi",
            sh_quote(branch)
        );
        shell(context, command).await
    }
}
