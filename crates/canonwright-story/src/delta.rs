use serde::{Deserialize, Serialize};

use crate::ids::TargetRef;
use crate::ids::{CharacterId, FactId, FactionId, ThreadId};
use crate::model::{Axis, Posture};

/// Everything one scene changes about the world.
///
/// A delta is the only way state moves. Nothing downstream reads a scene's prose
/// to decide what happened; it reads this. That is the whole trick: the prose can
/// be rewritten freely as long as the delta still describes it, and when the
/// delta changes, [`crate::Story::dirty_after`] knows exactly which later scenes
/// need a continuity pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SceneDelta {
    #[serde(default)]
    pub changes: Vec<SceneChange>,
}

impl SceneDelta {
    #[must_use]
    pub fn new(changes: Vec<SceneChange>) -> Self {
        Self { changes }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// A change plus where in the scene it lands.
///
/// `at_line` is what makes a knowledge leak checkable. "Mara learns the duke
/// poisoned the well" on line 12 cannot justify Mara acting on it on line 9.
/// A change without a line is treated as happening at the end of the scene, so
/// it can never excuse an earlier citation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneChange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_line: Option<u32>,
    #[serde(flatten)]
    pub change: Change,
}

impl SceneChange {
    #[must_use]
    pub fn at(line: u32, change: Change) -> Self {
        Self {
            at_line: Some(line),
            change,
        }
    }

    #[must_use]
    pub fn unplaced(change: Change) -> Self {
        Self {
            at_line: None,
            change,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// A character comes to know a fact.
    Learned {
        character: CharacterId,
        fact: FactId,
    },
    /// The reader learns a fact that was previously hidden.
    Revealed {
        fact: FactId,
    },
    /// An opinion moves to an absolute value, not by a step. Authors reason
    /// about where a relationship *stands*, not about arithmetic on it.
    Relationship {
        from: CharacterId,
        to: TargetRef,
        axis: Axis,
        value: i8,
        #[serde(default)]
        note: String,
    },
    /// A faction's named resource moves by a delta.
    Resource {
        faction: FactionId,
        resource: String,
        delta: i64,
    },
    Posture {
        faction: FactionId,
        posture: Posture,
        #[serde(default)]
        note: String,
    },
    Injury {
        character: CharacterId,
        description: String,
        #[serde(default)]
        severity: Harm,
    },
    ThreadPaidOff {
        thread: ThreadId,
    },
    ThreadAbandoned {
        thread: ThreadId,
        #[serde(default)]
        note: String,
    },
    Arrival {
        character: CharacterId,
        location: String,
    },
    Death {
        character: CharacterId,
    },
    /// A note that changes nothing mechanical. Useful for the state extractor to
    /// record a judgement for the author without inventing structure.
    CanonNote {
        note: String,
    },
}

impl Change {
    /// A short, stable phrase for logs, diffs, and prompt context.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Learned { character, fact } => format!("{character} learns {fact}"),
            Self::Revealed { fact } => format!("the reader learns {fact}"),
            Self::Relationship {
                from,
                to,
                axis,
                value,
                ..
            } => format!("{from} → {to} {} becomes {value}", axis.name()),
            Self::Resource {
                faction,
                resource,
                delta,
            } => format!("{faction} {resource} {delta:+}"),
            Self::Posture {
                faction, posture, ..
            } => {
                format!("{faction} posture becomes {posture:?}")
            }
            Self::Injury {
                character,
                description,
                ..
            } => format!("{character} injured: {description}"),
            Self::ThreadPaidOff { thread } => format!("thread {thread} pays off"),
            Self::ThreadAbandoned { thread, .. } => format!("thread {thread} abandoned"),
            Self::Arrival {
                character,
                location,
            } => format!("{character} arrives at {location}"),
            Self::Death { character } => format!("{character} dies"),
            Self::CanonNote { note } => note.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harm {
    #[default]
    Light,
    Serious,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_round_trips_with_its_line() {
        let change = SceneChange::at(
            12,
            Change::Learned {
                character: CharacterId::new("mara").unwrap(),
                fact: FactId::new("duke-poisoned-the-well").unwrap(),
            },
        );
        let encoded = serde_json::to_value(&change).unwrap();
        assert_eq!(encoded["at_line"], 12);
        assert_eq!(encoded["kind"], "learned");
        assert_eq!(encoded["character"], "mara");
        assert_eq!(
            serde_json::from_value::<SceneChange>(encoded).unwrap(),
            change
        );
    }

    #[test]
    fn an_unplaced_change_omits_the_line() {
        let change = SceneChange::unplaced(Change::Death {
            character: CharacterId::new("harek").unwrap(),
        });
        let encoded = serde_json::to_value(&change).unwrap();
        assert!(encoded.get("at_line").is_none());
        assert_eq!(encoded["kind"], "death");
    }

    #[test]
    fn axis_values_are_clamped_so_one_scene_cannot_rewrite_a_bond() {
        let mut relationship = crate::model::Relationship::default();
        Axis::Trust.set(&mut relationship, 99);
        assert_eq!(relationship.trust, crate::model::AXIS_MAX);
        Axis::Resentment.set(&mut relationship, -99);
        assert_eq!(relationship.resentment, crate::model::AXIS_MIN);
    }
}
