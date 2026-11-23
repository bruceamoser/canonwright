use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::StoryError;
use crate::ids::{CharacterId, RuleId, ThreadId};
use crate::model::{Character, Rule, RuleKind, Story, Thread, ThreadKind};

/// Which door the writer came in by. It decides the question list, and nothing
/// else: a story is a story whether it was typed here or imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// A blank page. The interview is the only source of truth.
    New,
    /// A draft exists. The document answers what it can; the rest is asked.
    Imported,
}

impl Mode {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Imported => "imported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Identity,
    Voice,
    Cast,
    Stakes,
}

impl Section {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Voice => "voice",
            Self::Cast => "cast",
            Self::Stakes => "stakes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnswerKind {
    Text,
    Choice { options: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub section: Section,
    pub prompt: String,
    pub help: String,
    pub kind: AnswerKind,
    /// Only `title` is required. Everything else can be skipped, because a
    /// writer who is asked nine questions before they can type is a writer who
    /// closes the app.
    pub required: bool,
    /// True when the answer was read out of the imported document rather than
    /// typed, so the UI can show it as a filled field to confirm.
    pub inferred: bool,
}

impl Question {
    #[must_use]
    pub fn skippable(&self) -> bool {
        !self.required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerSource {
    Author,
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub value: String,
    pub source: AnswerSource,
}

/// A fact read out of the import, shown to the writer but never asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derived {
    pub label: String,
    pub value: String,
}

/// The deterministic intake script.
///
/// Same mode in, same questions out, in the same order, with the same ids —
/// always. The UI renders it; the tests assert it; a resumed interview is a
/// lookup, not a re-prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intake {
    pub mode: Mode,
    pub questions: Vec<Question>,
    pub answers: BTreeMap<String, Answer>,
    pub derived: Vec<Derived>,
}

/// The fixed interview. Order is part of the contract.
const SCRIPT: &[(&str, Section, &str, &str, &str, bool)] = &[
    (
        "title",
        Section::Identity,
        "What is this story called?",
        "You can rename it later; the title is only a handle for the workspace.",
        "text",
        true,
    ),
    (
        "author",
        Section::Identity,
        "Who is writing it?",
        "Goes on the title page, and nowhere near the model's idea of your voice.",
        "text",
        false,
    ),
    (
        "genre",
        Section::Identity,
        "What kind of story is it?",
        "A shelf, not a cage: fantasy, crime, literary, romance, science fiction, horror, historical.",
        "choice:fantasy,crime,literary,romance,science fiction,horror,historical,other",
        false,
    ),
    (
        "logline",
        Section::Identity,
        "One sentence: who wants what, and what is in the way?",
        "If you cannot write it yet, skip it. The harness will keep asking.",
        "text",
        false,
    ),
    (
        "pov_voice",
        Section::Voice,
        "Whose head are we in, and how close?",
        "First person, third limited, third omniscient, or multiple viewpoints.",
        "choice:first person,third limited,third omniscient,multiple",
        false,
    ),
    (
        "tense",
        Section::Voice,
        "Past or present tense?",
        "Pick the one you will still be able to hold for eighty thousand words.",
        "choice:past,present",
        false,
    ),
    (
        "protagonist",
        Section::Cast,
        "Who is the story about?",
        "A name is enough. It becomes a character in the story state.",
        "text",
        false,
    ),
    (
        "want",
        Section::Cast,
        "What do they want, in this stretch of the story?",
        "An active goal, not a wish. Something that can be thwarted on the page.",
        "text",
        false,
    ),
    (
        "need",
        Section::Cast,
        "What do they need that they would deny?",
        "The arc. The thing the story is actually about, which they cannot say out loud.",
        "text",
        false,
    ),
    (
        "central_question",
        Section::Stakes,
        "What question should keep the reader turning pages?",
        "It becomes the first open thread, owed to the reader until you pay it off.",
        "text",
        false,
    ),
    (
        "hard_rule",
        Section::Stakes,
        "What is one rule of this world that cannot be broken?",
        "Magic has a cost, the dead stay dead, the law is the law. It becomes a hard rule the reviewers check against.",
        "text",
        false,
    ),
];

impl Intake {
    /// The interview for a blank manuscript.
    #[must_use]
    pub fn new_manuscript() -> Self {
        Self::build(Mode::New, BTreeMap::new(), Vec::new())
    }

    /// The interview for a draft that already exists.
    ///
    /// Whatever the text can answer, it answers: the writer confirms rather than
    /// types. Everything a document cannot know — genre, the shape of the want,
    /// the rule of the world — is still asked.
    #[must_use]
    pub fn from_document(text: &str, title: Option<&str>) -> Self {
        let mut answers = BTreeMap::new();
        let mut derived = Vec::new();

        let words = text.split_whitespace().count();
        derived.push(Derived {
            label: "words in the draft".to_owned(),
            value: words.to_string(),
        });

        if let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) {
            answers.insert(
                "title".to_owned(),
                Answer {
                    value: title.to_owned(),
                    source: AnswerSource::Document,
                },
            );
        }
        if let Some(pov) = infer_pov(text) {
            answers.insert(
                "pov_voice".to_owned(),
                Answer {
                    value: pov.to_owned(),
                    source: AnswerSource::Document,
                },
            );
            derived.push(Derived {
                label: "point of view".to_owned(),
                value: format!("{pov} (inferred from pronouns — confirm it)"),
            });
        }
        if let Some(tense) = infer_tense(text) {
            answers.insert(
                "tense".to_owned(),
                Answer {
                    value: tense.to_owned(),
                    source: AnswerSource::Document,
                },
            );
            derived.push(Derived {
                label: "tense".to_owned(),
                value: format!("{tense} (inferred from verbs — confirm it)"),
            });
        }

        Self::build(Mode::Imported, answers, derived)
    }

    fn build(mode: Mode, answers: BTreeMap<String, Answer>, derived: Vec<Derived>) -> Self {
        let questions = SCRIPT
            .iter()
            .map(|(id, section, prompt, help, kind, required)| Question {
                id: (*id).to_owned(),
                section: *section,
                prompt: (*prompt).to_owned(),
                help: (*help).to_owned(),
                kind: parse_kind(kind),
                required: *required,
                inferred: answers.contains_key(*id),
            })
            .collect();
        Self {
            mode,
            questions,
            answers,
            derived,
        }
    }

    #[must_use]
    pub fn question(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|question| question.id == id)
    }

    /// Record an answer. An unknown id is an error rather than a silently
    /// dropped field: a typo in a UI form should not become a mystery.
    pub fn answer(&mut self, id: &str, value: impl Into<String>) -> Result<(), StoryError> {
        if self.question(id).is_none() {
            return Err(StoryError::Unknown {
                kind: "question",
                id: id.to_owned(),
            });
        }
        let value = value.into();
        if value.trim().is_empty() {
            self.answers.remove(id);
        } else {
            self.answers.insert(
                id.to_owned(),
                Answer {
                    value: value.trim().to_owned(),
                    source: AnswerSource::Author,
                },
            );
            if let Some(question) = self.questions.iter_mut().find(|q| q.id == id) {
                // An author's answer overrides an inferred one.
                question.inferred = false;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn value(&self, id: &str) -> Option<&str> {
        self.answers.get(id).map(|answer| answer.value.as_str())
    }

    /// Required questions still unanswered. Only the title, in practice.
    #[must_use]
    pub fn outstanding(&self) -> Vec<&Question> {
        self.questions
            .iter()
            .filter(|question| question.required && !self.answers.contains_key(&question.id))
            .collect()
    }

    /// How much of the interview a writer could skip.
    #[must_use]
    pub fn skippable_count(&self) -> usize {
        self.questions
            .iter()
            .filter(|question| question.skippable())
            .count()
    }

    /// Write the answers into a story.
    ///
    /// Missing answers are not an error: an intake that can only be completed
    /// would be a form, and a writer with half an idea must be able to start.
    pub fn apply(&self, story: &mut Story) {
        if let Some(title) = self.value("title") {
            story.meta.title = title.to_owned();
        }
        if let Some(author) = self.value("author") {
            story.meta.author = Some(author.to_owned());
        }
        if let Some(genre) = self.value("genre") {
            story.meta.genre = Some(genre.to_owned());
        }
        if let Some(logline) = self.value("logline") {
            story.meta.logline = logline.to_owned();
        }
        if let Some(voice) = self.value("pov_voice") {
            story.meta.pov_voice = Some(voice.to_owned());
        }
        if let Some(tense) = self.value("tense") {
            story.meta.tense = Some(tense.to_owned());
        }

        let protagonist = self.value("protagonist").and_then(|name| {
            let id = slug(name);
            if id.is_empty() {
                return None;
            }
            let id = CharacterId::new(id).ok()?;
            let want = self.value("want").unwrap_or_default().to_owned();
            let need = self.value("need").unwrap_or_default().to_owned();
            let entry = story.characters.entry(id.clone()).or_insert(Character {
                name: name.to_owned(),
                ..Character::default()
            });
            if entry.name.trim().is_empty() {
                entry.name = name.to_owned();
            }
            if !want.is_empty() {
                entry.want = want;
            }
            if !need.is_empty() {
                entry.need = need;
            }
            Some(id)
        });

        if let Some(protagonist) = &protagonist {
            if story.meta.pov.is_none() {
                story.meta.pov = Some(protagonist.clone());
            }
        }

        if let Some(question) = self.value("central_question") {
            story.meta.premise = Some(question.to_owned());
            // A thread must be planted in a real scene. A blank manuscript has
            // none yet, so the premise stays in the meta until there is one;
            // an import already has scene one and gets the thread immediately.
            if let Some(first) = story.ordered_scenes().first() {
                let id = ThreadId::new("central-question").expect("literal is a valid slug");
                let opened_in = first.id.clone();
                story.threads.entry(id).or_insert(Thread {
                    kind: ThreadKind::Mystery,
                    label: question.to_owned(),
                    opened_in,
                    promised_payoff: None,
                    note: "asked during intake".to_owned(),
                });
            }
        }

        if let Some(statement) = self.value("hard_rule") {
            let id = RuleId::new("world-rule-1").expect("literal is a valid slug");
            story.rules.entry(id).or_insert(Rule {
                kind: RuleKind::Hard,
                statement: statement.to_owned(),
                scope: None,
            });
        }
    }
}

fn parse_kind(raw: &str) -> AnswerKind {
    match raw.strip_prefix("choice:") {
        Some(options) => AnswerKind::Choice {
            options: options
                .split(',')
                .map(|option| option.trim().to_owned())
                .collect(),
        },
        None => AnswerKind::Text,
    }
}

fn slug(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

/// Guess the narrative voice from pronoun frequency.
///
/// A heuristic, and it says so: it returns `None` when the text is genuinely
/// mixed, so the question stays open instead of the harness quietly deciding
/// an omniscient novel is first person.
#[must_use]
pub fn infer_pov(text: &str) -> Option<&'static str> {
    let (mut first, mut third) = (0_u32, 0_u32);
    for word in text.split_whitespace() {
        let word = word
            .trim_matches(|character: char| !character.is_ascii_alphabetic())
            .to_ascii_lowercase();
        match word.as_str() {
            "i" | "me" | "my" | "mine" | "we" | "us" | "our" | "ours" => first += 1,
            "he" | "she" | "they" | "him" | "her" | "them" | "his" | "hers" | "their"
            | "theirs" => {
                third += 1;
            }
            _ => {}
        }
    }
    decide(first, third)
}

fn decide(first: u32, third: u32) -> Option<&'static str> {
    if first + third < 20 {
        return None;
    }
    if first * 2 > third * 3 {
        Some("first person")
    } else if third * 2 > first * 3 {
        Some("third limited")
    } else {
        None
    }
}

/// Guess the tense from the verbs that carry it.
#[must_use]
pub fn infer_tense(text: &str) -> Option<&'static str> {
    let (mut past, mut present) = (0_u32, 0_u32);
    for word in text.split_whitespace() {
        let word = word
            .trim_matches(|character: char| !character.is_ascii_alphabetic())
            .to_ascii_lowercase();
        match word.as_str() {
            "was" | "were" | "had" | "did" | "said" | "went" | "could" | "would" => past += 1,
            "is" | "are" | "has" | "does" | "am" | "cannot" => present += 1,
            _ => {
                if word.len() > 4 && word.ends_with("ed") {
                    past += 1;
                }
            }
        }
    }
    if past + present < 20 {
        return None;
    }
    if past * 2 > present * 3 {
        Some("past")
    } else if present * 2 > past * 3 {
        Some("present")
    } else {
        None
    }
}
