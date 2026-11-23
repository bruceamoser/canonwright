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
