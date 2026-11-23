use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::delta::SceneDelta;
use crate::ids::{CharacterId, FactId, FactionId, RuleId, SceneId, TargetRef, ThreadId};

/// Bumped whenever a field is removed or reinterpreted. Loaders refuse to guess.
pub const SCHEMA_VERSION: u32 = 1;

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

/// The whole story: static primitives plus the ordered scene list that carries
/// every change.
///
/// The split is deliberate. `characters`, `factions`, `rules`, `facts`, and
/// `threads` describe what is true *before the story starts*. Everything that
/// happens afterwards lives in a scene's [`SceneDelta`]. That is what makes a
/// temporal projection possible: state at scene *N* is the base state folded
/// over the deltas of the scenes that precede it, and nothing else.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Story {
    #[serde(default = "schema_version")]
    pub schema: u32,
    pub meta: StoryMeta,
    #[serde(default)]
    pub characters: BTreeMap<CharacterId, Character>,
    #[serde(default)]
    pub factions: BTreeMap<FactionId, Faction>,
    #[serde(default)]
    pub rules: BTreeMap<RuleId, Rule>,
    #[serde(default)]
    pub facts: BTreeMap<FactId, Fact>,
    #[serde(default)]
    pub threads: BTreeMap<ThreadId, Thread>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoryMeta {
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// One sentence, present tense, no character names if it can be helped.
    #[serde(default)]
    pub logline: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// The default point of view, used when a scene omits one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pov: Option<CharacterId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Character {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
    /// The arc: what the character needs and cannot admit wanting.
    #[serde(default)]
    pub need: String,
    /// The active goal in this part of the story.
    #[serde(default)]
    pub want: String,
    #[serde(default)]
    pub voice: VoiceProfile,
    #[serde(default)]
    pub traits: Vec<String>,
    /// Starting opinions, before any scene moves them.
    #[serde(default)]
    pub relationships: BTreeMap<TargetRef, Relationship>,
}

/// What a character sounds like, in terms a line editor can act on.
///
/// These are constraints on prose, not descriptions of personality. "Terse"
/// is a mood; "no subordinate clauses, never uses a name where a pronoun will
/// do" is an instruction.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VoiceProfile {
    #[serde(default)]
    pub register: String,
    #[serde(default)]
    pub rhythm: String,
    #[serde(default)]
    pub favored: Vec<String>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    #[serde(default)]
    pub tics: Vec<String>,
    #[serde(default)]
    pub tells: Vec<String>,
}

impl VoiceProfile {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.register.is_empty()
            && self.rhythm.is_empty()
            && self.favored.is_empty()
            && self.forbidden.is_empty()
            && self.tics.is_empty()
            && self.tells.is_empty()
    }
}

/// A directed opinion. Axes are signed and clamped to [-5, 5] so that one scene
/// cannot rewrite a relationship, and so the numbers mean something stable
/// across a long book: 0 is neutral, 5 is unconditional, -5 is irreconcilable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    #[serde(default)]
    pub trust: i8,
    #[serde(default)]
    pub fear: i8,
    #[serde(default)]
    pub debt: i8,
    #[serde(default)]
    pub resentment: i8,
    #[serde(default)]
    pub note: String,
}

pub const AXIS_MIN: i8 = -5;
pub const AXIS_MAX: i8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Trust,
    Fear,
    Debt,
    Resentment,
}

impl Axis {
    #[must_use]
    pub fn get(self, relationship: &Relationship) -> i8 {
        match self {
            Self::Trust => relationship.trust,
            Self::Fear => relationship.fear,
            Self::Debt => relationship.debt,
            Self::Resentment => relationship.resentment,
        }
    }

    pub fn set(self, relationship: &mut Relationship, value: i8) {
        let clamped = value.clamp(AXIS_MIN, AXIS_MAX);
        match self {
            Self::Trust => relationship.trust = clamped,
            Self::Fear => relationship.fear = clamped,
            Self::Debt => relationship.debt = clamped,
            Self::Resentment => relationship.resentment = clamped,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Trust => "trust",
            Self::Fear => "fear",
            Self::Debt => "debt",
            Self::Resentment => "resentment",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Faction {
    #[serde(default)]
    pub name: String,
    /// Named quantities a scene can move. Integer levels, not currency: the
    /// point is that a blockade is felt three chapters later, not that the
    /// ledger balances.
    #[serde(default)]
    pub resources: BTreeMap<String, i64>,
    #[serde(default)]
    pub territory: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<Posture>,
    /// Sub-factions, rivals, and fault lines. A faction with none of these is a
    /// monolith, and monoliths make for dull antagonists.
    #[serde(default)]
    pub friction: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    Expansionist,
    Fractured,
    Covert,
    Defensive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Non-negotiable. Breaking it is a continuity bug, not a choice.
    Hard,
    /// Cultural, mythical, customary. May be believed wrongly on purpose.
    Soft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub kind: RuleKind,
    pub statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A proposition that can be true, hidden, or learned.
///
/// Facts are the unit of dramatic irony. A fact marked `secret` is true but
/// not yet revealed to the reader; a character learns one through a
/// [`crate::Change::Learned`]. Together they are the only mechanism that can
/// catch a knowledge leak.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    pub statement: String,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKind {
    /// A question the reader wants answered.
    Mystery,
    /// A promise the narrative made to the reader.
    Promise,
    /// An object or capability introduced for later use.
    Gun,
    /// A bond that will be tested.
    Relationship,
}

/// A planted obligation. The thread knows where it was planted and where it
/// promised to pay off; whether it actually did is a fact about scenes, not
/// about the thread, so it lives in the delta stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thread {
    pub kind: ThreadKind,
    pub label: String,
    pub opened_in: SceneId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promised_payoff: Option<SceneId>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneStatus {
    /// A beat on the board. Contributes no state.
    #[default]
    Planned,
    /// Written, not yet accepted. Contributes state for drafting what follows.
    Drafted,
    /// Linted and triaged. Contributes state.
    Reviewed,
    /// Accepted as canon. Contributes state, permanently.
    Canon,
}

impl SceneStatus {
    /// Planned scenes have not happened yet, so they cast no state forward.
    #[must_use]
    pub fn contributes_state(self) -> bool {
        self != Self::Planned
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Drafted => "drafted",
            Self::Reviewed => "reviewed",
            Self::Canon => "canon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalKind {
    /// This happens because that happened.
    Because,
    /// This happens, but that complication intervenes.
    But,
    /// therefore this follows.
    Therefore,
}

impl CausalKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Because => "because",
            Self::But => "but",
            Self::Therefore => "therefore",
        }
    }
}

/// A scene linked by causality, not adjacency. Sequence is free; consequence is
/// not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalLink {
    pub kind: CausalKind,
    pub scene: SceneId,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub id: SceneId,
    /// Dense, ordered, author-assigned. Not derived from array position, so a
    /// scene can be inserted without renumbering the book.
    pub index: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub pov: CharacterId,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub present: Vec<CharacterId>,
    #[serde(default)]
    pub status: SceneStatus,
    /// The value in play at the top of the scene, and the one it has become.
    /// Equal values mean nothing happened, which is a lint warning rather than
    /// an error: a quiet scene is allowed, an unchanged one usually is not.
    #[serde(default)]
    pub value_start: String,
    #[serde(default)]
    pub value_end: String,
    #[serde(default)]
    pub dependencies: Vec<CausalLink>,
    #[serde(default)]
    pub delta: SceneDelta,
}

impl Scene {
    #[must_use]
    pub fn shifts_value(&self) -> bool {
        !self.value_start.trim().is_empty()
            && !self.value_end.trim().is_empty()
            && !self
                .value_start
                .trim()
                .eq_ignore_ascii_case(self.value_end.trim())
    }

    #[must_use]
    pub fn has_value_axis(&self) -> bool {
        !self.value_start.trim().is_empty() || !self.value_end.trim().is_empty()
    }
}
