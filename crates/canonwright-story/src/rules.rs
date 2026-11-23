//! Rules content: the other kind of book this harness has to keep true.
//!
//! A novel is checked by continuity. A rulebook is checked by **referential
//! integrity and completeness**: every "see the Grapple action" resolves, every
//! class has the numbers a player needs, and no chapter refers to a rule that
//! was cut. That is a compiler-shaped problem, and it is why rulebooks get their
//! own primitives instead of being forced into scenes with a point of view.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What kind of content an entry is. Deliberately coarse: the systems people
/// write vary enormously, so the harness stores *shape* and leaves vocabulary to
/// the book.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A player-facing option: class, archetype, calling, playbook.
    Class,
    /// Ancestry, species, lineage, heritage.
    Species,
    /// A purchasable or collectable thing.
    Item,
    /// An adversary or ally with a stat block.
    Monster,
    /// A power, spell, maneuver, or talent.
    Spell,
    /// A rule the table follows: a procedure, a subsystem, a resolution.
    Procedure,
    /// A place that matters to play.
    Location,
    /// An organisation with goals and resources.
    Faction,
    #[default]
    Other,
}

impl EntityKind {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Species => "species",
            Self::Item => "item",
            Self::Monster => "monster",
            Self::Spell => "spell",
            Self::Procedure => "procedure",
            Self::Location => "location",
            Self::Faction => "faction",
            Self::Other => "other",
        }
    }

    /// Kinds that are unusable without their numbers. A class with no fields is
    /// a name and a promise.
    #[must_use]
    pub fn needs_fields(self) -> bool {
        matches!(
            self,
            Self::Class | Self::Species | Self::Item | Self::Monster | Self::Spell
        )
    }
}

/// A typed entry in the rules content.
///
/// `fields` is a string map on purpose. "Hit points: 1d8 + Constitution" and
/// "Stress: 3" and "Tier: 2" are all the same shape, and a harness that insisted
/// on one system's schema would be useless to every other system.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub kind: EntityKind,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    /// Cross-references, checked for resolution.
    #[serde(default)]
    pub references: Vec<Reference>,
}

/// A pointer to something else in the book, by id.
///
/// Ids, not page numbers: a page number is a promise the layout has to keep,
/// and layout is the last thing written.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub to: String,
    #[serde(default)]
    pub note: String,
}

impl Entity {
    /// Is this entry complete enough to be used at a table?
    #[must_use]
    pub fn is_incomplete(&self) -> bool {
        self.name.trim().is_empty()
            || (self.kind.needs_fields()
                && self.fields.is_empty()
                && self.summary.trim().is_empty())
    }
}
