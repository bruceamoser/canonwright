//! The Story State Graph.
//!
//! A story is not a pile of Markdown that a retriever samples from. It is a
//! typed, temporal state machine: characters with motivations and knowledge
//! boundaries, factions with resources and internal friction, hard world rules,
//! and scenes whose deltas move all of it.
//!
//! Three properties are load-bearing, and everything else is negotiable:
//!
//! 1. **State is folded, not stored.** [`Story::state_before`] reconstructs the
//!    world at a scene boundary by replaying deltas. There is no second copy of
//!    the truth to drift out of sync.
//! 2. **The future is not addressable.** A snapshot taken before scene *N* has
//!    no representation of scene *N+1*, so no prompt built from it can spoil
//!    one. Temporal isolation is a type-level property here, not a prompt rule.
//! 3. **Critique is evidence.** A [`Ticket`] that claims a knowledge leak names
//!    the character and the fact, and [`Story::substantiate`] settles it against
//!    the timeline. Reviewers may be wrong about taste; they are not allowed to
//!    be wrong about arithmetic.
//!
//! Everything a reviewer says is advisory. Fiction improves by breaking its own
//! rules on purpose, and a harness that can refuse to save a chapter is a
//! harness that gets uninstalled.
//!
//! # Example
//!
//! ```
//! use canonwright_story::prelude::*;
//! use canonwright_story::delta::{Change, SceneChange};
//!
//! let mut story = Story::default();
//! story.meta.title = "The Salt Road".to_owned();
//! story.characters.insert(
//!     CharacterId::new("mara").unwrap(),
//!     Character::default(),
//! );
//! story.facts.insert(
//!     FactId::new("duke-poisoned-the-well").unwrap(),
//!     Fact { statement: "The duke poisoned the well.".to_owned(), secret: true },
//! );
//! story.scenes.push(Scene {
//!     id: SceneId::new("s1").unwrap(),
//!     index: 1,
//!     pov: CharacterId::new("mara").unwrap(),
//!     status: SceneStatus::Canon,
//!     ..Scene::default()
//! });
//!
//! let state = story.state_before(&SceneId::new("s2").unwrap());
//! assert!(!state.knows(
//!     &CharacterId::new("mara").unwrap(),
//!     &FactId::new("duke-poisoned-the-well").unwrap()
//! ));
//! ```

pub mod context;
pub mod delta;
pub mod error;
pub mod ids;
pub mod lint;
pub mod model;
pub mod state;
pub mod ticket;

use std::path::Path;

pub use context::{SceneContext, Withheld};
pub use delta::{Change, Harm, SceneChange, SceneDelta};
pub use error::StoryError;
pub use ids::{CharacterId, FactId, FactionId, RuleId, SceneId, TargetRef, ThreadId, TicketId};
pub use lint::COLD_THREAD_GAP;
pub use model::{
    AXIS_MAX, AXIS_MIN, Axis, CausalKind, CausalLink, Character, Fact, Faction, Posture,
    Relationship, Rule, RuleKind, SCHEMA_VERSION, Scene, SceneStatus, Story, StoryMeta, Thread,
    ThreadKind, VoiceProfile,
};
pub use state::{
    CharacterState, FactionState, InjuryRecord, RelationshipRecord, StoryState, ThreadState,
    ThreadStatus,
};
pub use ticket::{Claim, Severity, Substantiation, Ticket, TicketCode};

/// The types a caller needs to talk about a story.
///
/// Separate from the crate root because a tool or an agent should not have to
/// list nine identifiers to read one scene.
pub mod prelude {
    pub use crate::context::SceneContext;
    pub use crate::delta::{Change, SceneChange, SceneDelta};
    pub use crate::error::StoryError;
    pub use crate::ids::{
        CharacterId, FactId, FactionId, RuleId, SceneId, TargetRef, ThreadId, TicketId,
    };
    pub use crate::model::{
        Axis, CausalKind, CausalLink, Character, Fact, Faction, Posture, Relationship, Rule,
        RuleKind, Scene, SceneStatus, Story, StoryMeta, Thread, ThreadKind, VoiceProfile,
    };
    pub use crate::state::{StoryState, ThreadStatus};
    pub use crate::ticket::{Claim, Severity, Substantiation, Ticket, TicketCode};
}

impl Default for Story {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            meta: StoryMeta::default(),
            characters: Default::default(),
            factions: Default::default(),
            rules: Default::default(),
            facts: Default::default(),
            threads: Default::default(),
            scenes: Vec::new(),
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            id: SceneId::new("scene").expect("literal is a valid slug"),
            index: 0,
            title: String::new(),
            summary: String::new(),
            pov: CharacterId::new("nobody").expect("literal is a valid slug"),
            location: String::new(),
            present: Vec::new(),
            status: SceneStatus::Planned,
            value_start: String::new(),
            value_end: String::new(),
            dependencies: Vec::new(),
            delta: SceneDelta::default(),
        }
    }
}

impl Story {
    #[must_use]
    pub fn scene(&self, id: &SceneId) -> Option<&Scene> {
        self.scenes.iter().find(|scene| &scene.id == id)
    }

    pub fn scene_mut(&mut self, id: &SceneId) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|scene| &scene.id == id)
    }

    /// Scenes in narrative order, which is by index and not by array position.
    #[must_use]
    pub fn ordered_scenes(&self) -> Vec<&Scene> {
        let mut scenes: Vec<&Scene> = self.scenes.iter().collect();
        scenes.sort_by_key(|scene| scene.index);
        scenes
    }

    /// The index a new scene should take: the end of the book.
    #[must_use]
    pub fn next_index(&self) -> u32 {
        self.scenes
            .iter()
            .map(|scene| scene.index)
            .max()
            .map_or(1, |index| index + 1)
    }

    pub fn from_json(source: &str) -> Result<Self, StoryError> {
        Ok(serde_json::from_str(source)?)
    }

    pub fn to_json(&self) -> Result<String, StoryError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn load(path: &Path) -> Result<Self, StoryError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }

    /// Write the story so that a crash mid-write cannot leave a half-file.
    ///
    /// Authors lose work to this exactly once and never forgive it.
    pub fn save(&self, path: &Path) -> Result<(), StoryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, self.to_json()?)?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}
