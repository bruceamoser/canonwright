//! Tools that let the agent read the story the way a reviewer reads it.
//!
//! These are deliberately read-only. The interesting thing about the harness is
//! not that a model can edit a JSON file; it is that the file has a timeline, and
//! every question the agent asks about it ("what does Mara know here?", "does
//! this review hold up?") is answered by folding that timeline rather than by
//! trusting the model's summary of it.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use canonwright_governance::RiskLevel;
use canonwright_story::prelude::*;
use canonwright_story::{SceneContext, Story};
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolDefinition, ToolError, ToolResult};

/// Where a story lives when the caller does not say otherwise.
const DEFAULT_STORY_PATH: &str = ".canonwright/story.json";

fn story_path(arguments: &Value, context: &ToolContext) -> PathBuf {
    let raw = arguments
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_STORY_PATH);
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        context.workspace.root().join(candidate)
    }
}

fn load(arguments: &Value, context: &ToolContext) -> Result<Story, ToolError> {
    let path = story_path(arguments, context);
    Story::load(&path).map_err(|error| {
        ToolError::Failed(format!(
            "could not read the story at {}: {error}. A story file is written by the \
             harness; create one before asking continuity questions.",
            path.display()
        ))
    })
}

fn scene_argument(arguments: &Value) -> Result<SceneId, ToolError> {
    let raw = arguments
        .get("scene")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArgument("scene is required".to_owned()))?;
    SceneId::new(raw).map_err(|error| ToolError::InvalidArgument(error.to_string()))
}

fn path_schema() -> Value {
    json!({
        "type": "string",
        "description": format!("Story file, relative to the workspace. Defaults to {DEFAULT_STORY_PATH}.")
    })
}

/// A one-screen orientation: what the story contains and whether it is sound.
pub struct StoryReadTool;

#[async_trait]
impl Tool for StoryReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_read",
            "Summarize the story state graph: cast, scenes, planted threads, and any \
             structural faults. Structural faults are real errors, not opinions.",
            RiskLevel::Read,
        )
        .with_schema(json!({
            "type": "object",
            "properties": { "path": path_schema() },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let story = load(&arguments, context)?;
        let errors = story.check();
        let mut out = String::new();
        let title = if story.meta.title.is_empty() {
            "(untitled)"
        } else {
            story.meta.title.as_str()
        };
        out.push_str(&format!("# {title}\n\n"));
        if !story.meta.logline.is_empty() {
            out.push_str(&format!("{}\n\n", story.meta.logline));
        }
        out.push_str(&format!(
            "{} scenes · {} characters · {} factions · {} facts · {} threads · {} rules\n\n",
            story.scenes.len(),
            story.characters.len(),
            story.factions.len(),
            story.facts.len(),
            story.threads.len(),
            story.rules.len(),
        ));

        if story.scenes.is_empty() {
            out.push_str("No scenes yet.\n");
        } else {
            out.push_str("## Scenes\n\n");
            for scene in story.ordered_scenes() {
                let status = scene.status.name();
                let value = if scene.has_value_axis() {
                    format!(
                        " — {} → {}",
                        if scene.value_start.trim().is_empty() {
                            "—"
                        } else {
                            scene.value_start.trim()
                        },
                        if scene.value_end.trim().is_empty() {
                            "—"
                        } else {
                            scene.value_end.trim()
                        }
                    )
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "- {} `{}` {}{value} ({status})\n",
                    scene.index,
                    scene.id,
                    if scene.title.is_empty() {
                        String::new()
                    } else {
                        format!("— {}", scene.title)
                    }
                ));
            }
        }

        out.push_str("\n## Structure\n\n");
        if errors.is_empty() {
            out.push_str("No structural faults.\n");
        } else {
            for error in &errors {
                out.push_str(&format!("- {error}\n"));
            }
        }

        Ok(ToolResult::ok(out).with_metadata(json!({
            "scenes": story.scenes.len(),
            "structural_errors": errors.len(),
        })))
    }
}

/// The scene brief: what the drafter is allowed to know.
pub struct StoryBriefTool;

#[async_trait]
impl Tool for StoryBriefTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_brief",
            "Assemble the prompt context for one scene from the timeline: who is present, \
             what the point of view knows at that exact moment, the causal lead-in, and the \
             threads still owed. Later scenes are never included.",
            RiskLevel::Read,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": { "type": "string", "description": "Scene id, e.g. `s12`." },
                "path": path_schema()
            },
            "required": ["scene"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let story = load(&arguments, context)?;
        let scene = scene_argument(&arguments)?;
        let Some(SceneContext {
            markdown, withheld, ..
        }) = story.context_for(&scene)
        else {
            return Err(ToolError::InvalidArgument(format!(
                "no scene {scene} in this story"
            )));
        };
        let footer = format!(
            "\n---\n\n_Withheld by the timeline: {} later scenes, {} facts the reader does \
             not know yet, {} facts this point of view has not learned, {} threads not yet \
             planted._\n",
            withheld.later_scenes,
            withheld.unrevealed_facts,
            withheld.unknown_to_pov,
            withheld.unopened_threads,
        );
        Ok(
            ToolResult::ok(format!("{markdown}{footer}")).with_metadata(json!({
                "scene": scene.to_string(),
                "withheld": withheld,
            })),
        )
    }
}

/// Deterministic lint: the checks that do not need an opinion.
pub struct StoryLintTool;

#[async_trait]
impl Tool for StoryLintTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_lint",
            "Run the deterministic narrative checks (value shift, causality, dangling setups, \
             unearned payoffs, continuity). Every finding is advisory: fiction is allowed to \
             break its own rules on purpose.",
            RiskLevel::Read,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": {
                    "type": "string",
                    "description": "Optional: only report tickets for this scene."
                },
                "path": path_schema()
            },
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let story = load(&arguments, context)?;
        let filter = arguments
            .get("scene")
            .and_then(Value::as_str)
            .map(SceneId::new)
            .transpose()
            .map_err(|error| ToolError::InvalidArgument(error.to_string()))?;

        let mut tickets = story.lint();
        if let Some(filter) = &filter {
            tickets.retain(|ticket| &ticket.scene == filter);
        }

        if tickets.is_empty() {
            return Ok(ToolResult::ok("No findings.")
                .with_metadata(json!({"tickets": 0, "advisory": true})));
        }

        let mut out = String::new();
        out.push_str(&format!(
            "{} finding(s). All advisory: a warning is a question, not a verdict.\n\n",
            tickets.len()
        ));
        for ticket in &tickets {
            let scene = story
                .scene(&ticket.scene)
                .map(|scene| scene.index)
                .unwrap_or_default();
            let line = ticket.line.map_or(String::new(), |line| format!(":{line}"));
            let verdict = match story.substantiate(ticket) {
                Substantiation::Confirmed => " · confirmed against the timeline",
                Substantiation::Refuted(_) => " · REFUTED by the timeline",
                Substantiation::Uncheckable => "",
            };
            out.push_str(&format!(
                "- {} scene {scene} `{}`{line}{verdict}\n  {}\n",
                ticket.code, ticket.scene, ticket.message
            ));
        }
        Ok(ToolResult::ok(out).with_metadata(json!({
            "tickets": tickets.len(),
            "advisory": true,
        })))
    }
}

/// The evidence gate: does the timeline support this claim?
pub struct StorySubstantiateTool;

#[async_trait]
impl Tool for StorySubstantiateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_substantiate",
            "Check a reviewer's structured claim against the timeline before acting on it. \
             Answers confirmed, refuted, or uncheckable. Use this instead of trusting a \
             critique that cites no evidence.",
            RiskLevel::Read,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": { "type": "string" },
                "line": { "type": "integer", "minimum": 1 },
                "code": {
                    "type": "string",
                    "description": "Ticket code, e.g. KNOWLEDGE_LEAK. Cosmetic; the claim decides."
                },
                "message": { "type": "string" },
                "claim": {
                    "type": "object",
                    "description": "Structured claim: {claim:'knowledge',character,fact} | {claim:'thread',thread} | {claim:'rule',rule} | {claim:'value_shift'}",
                    "additionalProperties": true
                },
                "path": path_schema()
            },
            "required": ["scene", "claim"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let story = load(&arguments, context)?;
        let scene = scene_argument(&arguments)?;
        let claim: Claim = serde_json::from_value(
            arguments
                .get("claim")
                .cloned()
                .ok_or_else(|| ToolError::InvalidArgument("claim is required".to_owned()))?,
        )
        .map_err(|error| {
            ToolError::InvalidArgument(format!("claim is not a recognised shape: {error}"))
        })?;
        let code: TicketCode = arguments
            .get("code")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or(TicketCode::KnowledgeLeak);
        let message = arguments
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("reviewer claim");

        let ticket = Ticket::new(code, scene.clone(), message).claiming(claim);
        let ticket = match arguments.get("line").and_then(Value::as_u64) {
            Some(line) => ticket.at(u32::try_from(line).unwrap_or(u32::MAX)),
            None => ticket,
        };

        let (verdict, detail) = match story.substantiate(&ticket) {
            Substantiation::Confirmed => ("confirmed", String::new()),
            Substantiation::Refuted(reason) => ("refuted", reason),
            Substantiation::Uncheckable => (
                "uncheckable",
                "this claim is a judgement call; only the author can settle it".to_owned(),
            ),
        };
        let body = if detail.is_empty() {
            format!("{verdict}: the timeline supports this.\n")
        } else {
            format!("{verdict}: {detail}\n")
        };
        Ok(ToolResult::ok(body).with_metadata(json!({
            "verdict": verdict,
            "scene": scene.to_string(),
            "ticket": ticket.id.to_string(),
        })))
    }
}
use canonwright_story::WriteOutcome;

fn require_writes(context: &ToolContext, tool: &str) -> Result<(), ToolError> {
    if context.allow_writes {
        return Ok(());
    }
    Err(ToolError::ApprovalRequired(format!(
        "{tool} (switch the composer access mode to Can edit or Full access)"
    )))
}

fn render_outcome(story: &Story, scene: &SceneId, outcome: &WriteOutcome) -> String {
    if !outcome.accepted {
        let mut out = String::from(
            "Rejected: the story would be structurally invalid, and was left unchanged.\n\n",
        );
        for error in &outcome.errors {
            out.push_str(&format!("- {error}\n"));
        }
        return out;
    }

    let verb = if outcome.created { "Added" } else { "Updated" };
    let index = story
        .scene(scene)
        .map(|scene| scene.index)
        .unwrap_or_default();
    let mut out = format!("{verb} scene {index} `{scene}`.\n");

    if outcome.dirty.is_empty() {
        out.push_str("\nNo canon scene downstream needs rechecking.\n");
    } else {
        out.push_str("\n## Needs a continuity pass\n\n");
        out.push_str(
            "These scenes are canon and were accepted against premises that just changed:\n\n",
        );
        for id in &outcome.dirty {
            let index = story
                .scene(id)
                .map(|scene| scene.index.to_string())
                .unwrap_or_else(|| "?".to_owned());
            out.push_str(&format!("- scene {index} `{id}`\n"));
        }
    }

    if !outcome.tickets.is_empty() {
        out.push_str(&format!(
            "\n## Findings ({}), all advisory\n\n",
            outcome.tickets.len()
        ));
        for ticket in &outcome.tickets {
            out.push_str(&format!("- {} {}\n", ticket.code, ticket.message));
        }
    }
    out
}

/// Write a whole scene: body, status, causal links, and delta.
pub struct StoryApplyTool;

#[async_trait]
impl Tool for StoryApplyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_apply",
            "Add or replace one scene in the story state graph. The whole scene is validated \
             before anything is kept: if the document would become structurally invalid the \
             write is rejected and the story is left untouched. Changing a scene's delta marks \
             canon scenes downstream as needing a continuity pass.",
            RiskLevel::Write,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": {
                    "type": "object",
                    "description": "The scene object. Fields: id, index, pov, title, summary, location, present[], status, value_start, value_end, dependencies[], delta{changes[]}.",
                    "additionalProperties": true
                },
                "path": path_schema()
            },
            "required": ["scene"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        require_writes(context, "story_apply")?;
        let path = story_path(&arguments, context);
        let mut story = load(&arguments, context)?;
        let scene: Scene = serde_json::from_value(
            arguments
                .get("scene")
                .cloned()
                .ok_or_else(|| ToolError::InvalidArgument("scene is required".to_owned()))?,
        )
        .map_err(|error| {
            ToolError::InvalidArgument(format!("scene is not a valid scene object: {error}"))
        })?;
        let id = scene.id.clone();
        let status = scene.status;

        let outcome = story.write_scene(scene);
        if !outcome.accepted {
            return Ok(ToolResult::error(render_outcome(&story, &id, &outcome))
                .with_metadata(json!({"accepted": false})));
        }
        // Accepting a scene as canon *is* the review; the record exists so a
        // later upstream change can invalidate it.
        if status == SceneStatus::Canon {
            story
                .mark_reviewed(&id)
                .map_err(|error| ToolError::Failed(error.to_string()))?;
        }
        story.save(&path).map_err(|error| {
            ToolError::Failed(format!("could not write {}: {error}", path.display()))
        })?;

        Ok(
            ToolResult::ok(render_outcome(&story, &id, &outcome)).with_metadata(json!({
                "accepted": true,
                "created": outcome.created,
                "dirty": outcome.dirty.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "tickets": outcome.tickets.len(),
            })),
        )
    }
}

/// Promote a scene to canon.
pub struct StoryCommitTool;

#[async_trait]
impl Tool for StoryCommitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_commit",
            "Accept an existing scene as canon. Records what the scene was accepted against, so \
             a later change upstream will mark it as needing a continuity pass. Reports the canon \
             scenes this commit invalidated and any findings, which are advisory.",
            RiskLevel::Write,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": { "type": "string", "description": "Scene id to accept as canon." },
                "path": path_schema()
            },
            "required": ["scene"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        require_writes(context, "story_commit")?;
        let path = story_path(&arguments, context);
        let mut story = load(&arguments, context)?;
        let id = scene_argument(&arguments)?;

        let outcome = story.set_scene_status(&id, SceneStatus::Canon);
        if !outcome.accepted {
            return Ok(ToolResult::error(render_outcome(&story, &id, &outcome))
                .with_metadata(json!({"accepted": false})));
        }
        story
            .mark_reviewed(&id)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        story.save(&path).map_err(|error| {
            ToolError::Failed(format!("could not write {}: {error}", path.display()))
        })?;

        let mut out = render_outcome(&story, &id, &outcome);
        let still_dirty = story.dirty_scenes();
        if !still_dirty.is_empty() {
            out.push_str(&format!(
                "\n{} canon scene(s) still need a continuity pass: {}\n",
                still_dirty.len(),
                still_dirty
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(ToolResult::ok(out).with_metadata(json!({
            "accepted": true,
            "dirty": outcome.dirty.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "still_dirty": still_dirty.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })))
    }
}

/// Sign off a scene after a continuity pass.
pub struct StoryReviewTool;

#[async_trait]
impl Tool for StoryReviewTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_review",
            "Record that a canon scene has been re-read against its current premises and is no \
             longer stale. Use after a downstream scene shows up as needing a continuity pass.",
            RiskLevel::Write,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "scene": { "type": "string", "description": "Scene id that was rechecked." },
                "path": path_schema()
            },
            "required": ["scene"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        require_writes(context, "story_review")?;
        let path = story_path(&arguments, context);
        let mut story = load(&arguments, context)?;
        let id = scene_argument(&arguments)?;

        story
            .mark_reviewed(&id)
            .map_err(|error| ToolError::InvalidArgument(format!("{error}")))?;
        story.save(&path).map_err(|error| {
            ToolError::Failed(format!("could not write {}: {error}", path.display()))
        })?;

        let still_dirty = story.dirty_scenes();
        let mut out = format!("Rechecked `{id}`; it is up to date.\n");
        if still_dirty.is_empty() {
            out.push_str("No canon scene needs a continuity pass.\n");
        } else {
            out.push_str(&format!(
                "Still dirty: {}\n",
                still_dirty
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(ToolResult::ok(out).with_metadata(json!({
            "still_dirty": still_dirty.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })))
    }
}
