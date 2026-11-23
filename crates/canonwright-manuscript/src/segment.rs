use canonwright_story::prelude::*;

use crate::{BlockKind, Document};

/// How a segment boundary was decided, so the author can see which cuts were
/// read from the document and which were guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    /// A heading began it.
    Heading,
    /// A thematic break separated it.
    SceneBreak,
    /// The document had no structure, so it was split on word count.
    Heuristic,
    /// The whole document is one scene.
    Whole,
}

impl SegmentKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::SceneBreak => "scene break",
            Self::Heuristic => "heuristic split",
            Self::Whole => "whole document",
        }
    }

    #[must_use]
    pub fn is_guess(self) -> bool {
        matches!(self, Self::Heuristic)
    }
}

/// One scene-sized piece of a manuscript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub title: Option<String>,
    pub body: String,
    pub kind: SegmentKind,
}

impl Segment {
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.body.split_whitespace().count()
    }
}

/// Word count above which a structureless document is split anyway.
///
/// A single 90,000-word scene is not a scene; it is the whole book wearing a
/// costume. 1,500 words is roughly a long chapter beat, and the split is flagged
/// as a guess so nobody mistakes it for a fact about the text.
pub const HEURISTIC_WORDS: usize = 1_500;

/// Turn a document into scene-sized segments.
#[must_use]
pub fn segment(document: &Document) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut title: Option<String> = None;
    let mut body = String::new();
    let mut opened_by_heading = false;
    let mut saw_break = false;

    for block in &document.blocks {
        match block.kind {
            BlockKind::Heading(_) => {
                push(
                    &mut segments,
                    &mut title,
                    &mut body,
                    opened_by_heading,
                    saw_break,
                );
                title = Some(block.text.clone());
                opened_by_heading = true;
                saw_break = false;
            }
            BlockKind::Break => {
                push(
                    &mut segments,
                    &mut title,
                    &mut body,
                    opened_by_heading,
                    saw_break,
                );
                title = None;
                opened_by_heading = false;
                saw_break = true;
            }
            BlockKind::Paragraph => {
                if !body.is_empty() {
                    body.push_str("\n\n");
                }
                body.push_str(block.text.trim());
            }
        }
    }
    push(
        &mut segments,
        &mut title,
        &mut body,
        opened_by_heading,
        saw_break,
    );

    if segments.len() == 1 && segments[0].word_count() > HEURISTIC_WORDS {
        return split_heuristically(&segments[0]);
    }
    segments
}

fn push(
    segments: &mut Vec<Segment>,
    title: &mut Option<String>,
    body: &mut String,
    opened_by_heading: bool,
    saw_break: bool,
) {
    let text = body.trim();
    if text.is_empty() {
        *title = None;
        body.clear();
        return;
    }
    let kind = if opened_by_heading {
        SegmentKind::Heading
    } else if saw_break {
        SegmentKind::SceneBreak
    } else if segments.is_empty() {
        SegmentKind::Whole
    } else {
        SegmentKind::SceneBreak
    };
    segments.push(Segment {
        title: title.take(),
        body: text.to_owned(),
        kind,
    });
    body.clear();
}

fn split_heuristically(single: &Segment) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut body = String::new();
    let mut words = 0;
    for paragraph in single.body.split("\n\n") {
        let count = paragraph.split_whitespace().count();
        if words > 0 && words + count > HEURISTIC_WORDS {
            segments.push(Segment {
                title: None,
                body: body.trim().to_owned(),
                kind: SegmentKind::Heuristic,
            });
            body.clear();
            words = 0;
        }
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(paragraph);
        words += count;
    }
    if !body.trim().is_empty() {
        segments.push(Segment {
            title: None,
            body: body.trim().to_owned(),
            kind: SegmentKind::Heuristic,
        });
    }
    if segments.is_empty() {
        vec![single.clone()]
    } else {
        segments
    }
}

/// What a scaffold created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scaffold {
    pub scenes: Vec<SceneId>,
    /// Set when the manuscript named no cast, so a placeholder narrator had to
    /// be invented to satisfy the schema.
    pub narrator: Option<CharacterId>,
}

/// Add a scene to the story for each segment.
///
/// Every scene arrives `Drafted` with an empty delta: the prose is real, the
/// state is not yet known. That is deliberate — it is the difference between
/// "imported" and "understood", and the review queue is where the gap gets
/// closed.
pub fn scaffold(story: &mut Story, segments: &[Segment]) -> Scaffold {
    let mut narrator = None;
    let pov = if let Some(pov) = story.meta.pov.clone() {
        pov
    } else if let Some(existing) = story.characters.keys().next().cloned() {
        existing
    } else {
        let id = CharacterId::new("narrator").expect("literal is a valid slug");
        story.characters.insert(
            id.clone(),
            Character {
                name: "Unattributed narrator".to_owned(),
                role: "placeholder — replace with the real point of view".to_owned(),
                ..Character::default()
            },
        );
        narrator = Some(id.clone());
        id
    };

    let mut index = story.next_index();
    let mut scenes = Vec::new();
    for segment in segments {
        let id = SceneId::new(format!("s{index}")).expect("generated slug is valid");
        story.scenes.push(Scene {
            id: id.clone(),
            index,
            title: segment.title.clone().unwrap_or_default(),
            summary: opening(&segment.body, 240),
            pov: Some(pov.clone()),
            location: String::new(),
            present: vec![pov.clone()],
            status: SceneStatus::Drafted,
            value_start: String::new(),
            value_end: String::new(),
            dependencies: Vec::new(),
            delta: SceneDelta::default(),
            ..Scene::default()
        });
        scenes.push(id);
        index += 1;
    }

    Scaffold { scenes, narrator }
}

/// The first `limit` characters, cut at a word boundary.
fn opening(body: &str, limit: usize) -> String {
    let text = body.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let cut: String = text.chars().take(limit).collect();
    let trimmed = match cut.rfind(' ') {
        Some(position) => &cut[..position],
        None => cut.as_str(),
    };
    format!("{}…", trimmed.trim_end())
}
