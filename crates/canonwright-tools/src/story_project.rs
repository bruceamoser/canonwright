//! Starting a project: a blank manuscript, or one that already exists.
//!
//! This is the backend half of the first-run prompt. The UI asks the writer one
//! question — "are you starting fresh, or do you have a draft?" — and this tool
//! answers both branches, so the two paths cannot drift apart.

use async_trait::async_trait;
use canonwright_governance::RiskLevel;
use canonwright_manuscript::{Scaffold, read, scaffold, segment};
use canonwright_story::prelude::*;
use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolDefinition, ToolError, ToolResult};

const DEFAULT_STORY_PATH: &str = ".canonwright/story.json";

/// Create a story, from nothing or from a document.
pub struct StoryStartTool;

#[async_trait]
impl Tool for StoryStartTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "story_start",
            "Start a story project. mode=\"new\" creates an empty manuscript to write into. \
             mode=\"existing\" imports a draft — .md, .markdown, .txt, .docx, .epub or .pdf — \
             splitting it into scenes by headings and scene breaks and keeping the prose. \
             Imported scenes arrive drafted, with no state extracted yet.",
            RiskLevel::Write,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["new", "existing"],
                    "description": "new: begin a blank manuscript. existing: import a draft."
                },
                "title": {
                    "type": "string",
                    "description": "Story title. Falls back to the document's own title, then the file name."
                },
                "path": {
                    "type": "string",
                    "description": "The manuscript to import. Required when mode is existing."
                },
                "pov": {
                    "type": "string",
                    "description": "Optional point-of-view character id for every imported scene."
                },
                "out": {
                    "type": "string",
                    "description": format!("Where to write the story. Defaults to {DEFAULT_STORY_PATH}.")
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Replace an existing story file. Defaults to false: an import never silently clobbers a story."
                }
            },
            "required": ["mode"],
            "additionalProperties": false
        }))
    }

    async fn execute(
        &self,
        arguments: Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if !context.allow_writes {
            return Err(ToolError::ApprovalRequired(
                "story_start (switch the composer access mode to Can edit or Full access)"
                    .to_owned(),
            ));
        }

        let mode = arguments
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("new");
        let out = resolve(
            context,
            arguments
                .get("out")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_STORY_PATH),
        )?;
        let overwrite = arguments
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if out.exists() && !overwrite {
            return Err(ToolError::Failed(format!(
                "{} already holds a story. Pass overwrite:true to replace it, or choose another \
                 out path. An import should never be able to destroy a manuscript by accident.",
                out.display()
            )));
        }

        let title = arguments
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let pov = arguments
            .get("pov")
            .and_then(Value::as_str)
            .map(CharacterId::new)
            .transpose()
            .map_err(|error| ToolError::InvalidArgument(error.to_string()))?;

        match mode {
            "new" => start_blank(&arguments, context, &out, title, pov),
            "existing" => start_from_document(&arguments, context, &out, title, pov),
            other => Err(ToolError::InvalidArgument(format!(
                "mode must be \"new\" or \"existing\", not {other:?}"
            ))),
        }
    }
}

fn start_blank(
    _arguments: &Value,
    _context: &ToolContext,
    out: &std::path::Path,
    title: Option<String>,
    pov: Option<CharacterId>,
) -> Result<ToolResult, ToolError> {
    let mut story = Story::default();
    story.meta.title = title.unwrap_or_else(|| "Untitled".to_owned());
    let mut cast_note = String::new();
    if let Some(pov) = pov {
        story.characters.insert(
            pov.clone(),
            Character {
                name: pov.to_string(),
                role: "placeholder — rename me, then write their want and need".to_owned(),
                ..Character::default()
            },
        );
        story.meta.pov = Some(pov.clone());
        cast_note = format!(
            "\nA placeholder character `{pov}` was created as the default point of view.\n"
        );
    }
    story.save(out).map_err(|error| {
        ToolError::Failed(format!("could not write {}: {error}", out.display()))
    })?;

    Ok(ToolResult::ok(format!(
        "# {} — new manuscript\n\nNothing written yet.{cast_note}\n## Next\n\n\
         1. Decide the cast and what each of them wants (`story_apply` a scene, or edit the story file).\n\
         2. Draft the first scene, then run `story_lint`.\n\
         3. Commit it with `story_commit` so it becomes canon for the scene after it.\n",
        story.meta.title
    ))
    .with_metadata(json!({
        "mode": "new",
        "title": story.meta.title,
        "scenes": 0,
        "out": out.display().to_string(),
    })))
}

fn start_from_document(
    arguments: &Value,
    context: &ToolContext,
    out: &std::path::Path,
    title: Option<String>,
    pov: Option<CharacterId>,
) -> Result<ToolResult, ToolError> {
    let raw = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::InvalidArgument(
                "mode is \"existing\", so path must name the manuscript to import".to_owned(),
            )
        })?;
    let manuscript = resolve(context, raw)?;
    if !manuscript.exists() {
        return Err(ToolError::InvalidArgument(format!(
            "no manuscript at {}",
            manuscript.display()
        )));
    }

    let document = read(&manuscript).map_err(|error| ToolError::Failed(error.to_string()))?;
    if document.is_empty() {
        return Err(ToolError::Failed(format!(
            "{} contains no prose we could read",
            manuscript.display()
        )));
    }
    let segments = segment(&document);
    let words: usize = segments
        .iter()
        .map(canonwright_manuscript::Segment::word_count)
        .sum();
    let guessed = segments.iter().filter(|s| s.kind.is_guess()).count();

    let mut story = Story::default();
    story.meta.title = title.or_else(|| document.title.clone()).unwrap_or_else(|| {
        manuscript
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    });
    story.meta.source = Some(raw.to_owned());

    let mut cast_note = String::new();
    if let Some(pov) = pov {
        story.characters.insert(
            pov.clone(),
            Character {
                name: pov.to_string(),
                role: "placeholder — the import assigned every scene this point of view".to_owned(),
                ..Character::default()
            },
        );
        story.meta.pov = Some(pov.clone());
    }

    let Scaffold { scenes, narrator } = scaffold(&mut story, &segments);
    let errors = story.check();
    if !errors.is_empty() {
        let details: Vec<String> = errors.iter().map(|error| error.to_string()).collect();
        return Err(ToolError::Failed(format!(
            "the imported structure is not sound, so nothing was written:\n- {}",
            details.join("\n- ")
        )));
    }
    story.save(out).map_err(|error| {
        ToolError::Failed(format!("could not write {}: {error}", out.display()))
    })?;

    if narrator.is_some() {
        cast_note = "\nThe draft names no cast, so every scene was given a placeholder \
                     `narrator` point of view. Replace it once the real characters exist.\n"
            .to_owned();
    }
    let guess_note = if guessed > 0 {
        format!(
            "\n{guessed} of {} scenes were split by word count because the document had no \
             headings or scene breaks. Check those boundaries.\n",
            segments.len()
        )
    } else {
        String::new()
    };

    Ok(ToolResult::ok(format!(
        "# {} — imported from {}\n\n\
         - Format: {}\n- Scenes: {}\n- Words: {words}\n{cast_note}{guess_note}\n\
         ## What this is and is not\n\n\
         The prose and the scene boundaries are real. The **state is not extracted yet**: \
         every scene is drafted with an empty delta, so the cast knows nothing and no thread \
         is planted. `story_brief` will be thin until that pass runs.\n\n\
         ## Next\n\n\
         1. Run `story_lint` to see the continuity queue — imported scenes have no value shift yet.\n\
         2. Fill in the cast: who is present, what they want, how they sound.\n\
         3. Commit scenes with `story_commit` as you accept them.\n",
        story.meta.title,
        manuscript.display(),
        document.format,
        scenes.len()
    ))
    .with_metadata(json!({
        "mode": "existing",
        "format": document.format.name(),
        "title": story.meta.title,
        "scenes": scenes.len(),
        "words": words,
        "guessed_segments": guessed,
        "placeholder_narrator": narrator.is_some(),
        "out": out.display().to_string(),
    })))
}

fn resolve(context: &ToolContext, raw: &str) -> Result<std::path::PathBuf, ToolError> {
    context
        .workspace
        .resolve(std::path::Path::new(raw))
        .map_err(|error| match error {
            canonwright_workspace::WorkspaceError::OutsideRoot(_) => {
                ToolError::OutsideRoot(raw.to_owned())
            }
            other => ToolError::Failed(other.to_string()),
        })
}
