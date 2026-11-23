use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::delta::{Change, Harm};
use crate::ids::{CharacterId, FactId, FactionId, SceneId, TargetRef, ThreadId};
use crate::model::{Axis, Posture, Relationship, Scene, SceneStatus, Story, ThreadKind};

/// The materialized world at one point in the timeline.
///
/// This is what an agent is allowed to see. It contains no future, by
/// construction: it was folded from scenes that precede the point of view, so
/// there is nothing later in it to leak.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StoryState {
    /// The scene this state sits after, if it was cut at a scene boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneId>,
    /// The highest scene index folded in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    pub characters: BTreeMap<CharacterId, CharacterState>,
    pub factions: BTreeMap<FactionId, FactionState>,
    /// A flat list rather than a map keyed by `(CharacterId, TargetRef)`,
    /// because JSON object keys must be strings and a synthesized `"a->b"` key
    /// is the kind of thing that ends up in a prompt and confuses a model.
    pub relationships: Vec<RelationshipRecord>,
    pub threads: BTreeMap<ThreadId, ThreadState>,
    /// Facts the reader has been told. Characters may still be ignorant of
    /// these, which is where dramatic irony lives.
    pub known_to_reader: BTreeSet<FactId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterState {
    pub knowledge: BTreeSet<FactId>,
    pub injuries: Vec<InjuryRecord>,
    pub alive: bool,
    pub location: Option<String>,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            knowledge: BTreeSet::new(),
            injuries: Vec::new(),
            alive: true,
            location: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InjuryRecord {
    pub description: String,
    pub severity: Harm,
    pub scene: SceneId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipRecord {
    pub from: CharacterId,
    pub to: TargetRef,
    pub value: Relationship,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactionState {
    pub posture: Option<Posture>,
    pub resources: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    /// Declared, but its opening scene has not happened yet.
    #[default]
    Unopened,
    Planted,
    Advanced,
    PaidOff,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThreadState {
    pub status: ThreadStatus,
    /// The last scene that touched this thread, and the line if it had one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_touched: Option<SceneId>,
    pub kind: ThreadKindValue,
}

/// A copy of the thread's kind so a state snapshot stands alone without
/// reaching back into the story document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadKindValue {
    #[default]
    Mystery,
    Promise,
    Gun,
    Relationship,
}

impl From<ThreadKind> for ThreadKindValue {
    fn from(kind: ThreadKind) -> Self {
        match kind {
            ThreadKind::Mystery => Self::Mystery,
            ThreadKind::Promise => Self::Promise,
            ThreadKind::Gun => Self::Gun,
            ThreadKind::Relationship => Self::Relationship,
        }
    }
}

impl StoryState {
    /// A character's state, or the default (alive, present, ignorant) if the
    /// character has never appeared. Returning a default keeps reviewers from
    /// having to distinguish "unknown character" from "character with no
    /// interesting state", which they are bad at.
    #[must_use]
    pub fn character(&self, id: &CharacterId) -> CharacterState {
        self.characters.get(id).cloned().unwrap_or_default()
    }

    #[must_use]
    pub fn knows(&self, character: &CharacterId, fact: &FactId) -> bool {
        self.characters
            .get(character)
            .is_some_and(|state| state.knowledge.contains(fact))
    }

    #[must_use]
    pub fn relationship(&self, from: &CharacterId, to: &TargetRef) -> Relationship {
        self.relationships
            .iter()
            .find(|record| &record.from == from && &record.to == to)
            .map(|record| record.value.clone())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn thread(&self, id: &ThreadId) -> ThreadState {
        self.threads.get(id).cloned().unwrap_or_default()
    }

    #[must_use]
    pub fn is_open_thread(&self, id: &ThreadId) -> bool {
        matches!(
            self.thread(id).status,
            ThreadStatus::Planted | ThreadStatus::Advanced
        )
    }
}

impl Story {
    /// State after every contributing scene strictly before `scene`.
    ///
    /// "Contributing" means drafted or later. A planned scene has not happened,
    /// so it cannot be a premise.
    #[must_use]
    pub fn state_before(&self, scene: &SceneId) -> StoryState {
        match self.scene(scene) {
            Some(target) => self.fold_until(target.index, false),
            None => self.base_state(),
        }
    }

    /// State after every contributing scene up to and including `scene`.
    #[must_use]
    pub fn state_at(&self, scene: &SceneId) -> StoryState {
        match self.scene(scene) {
            Some(target) => {
                let mut state = self.fold_until(target.index, true);
                state.scene = Some(scene.clone());
                state
            }
            None => self.base_state(),
        }
    }

    /// State as it would stand if `scene`'s planned changes landed: the state
    /// before, folded with the scene's own delta. Used to preview the
    /// consequences of a scene before committing it to canon.
    #[must_use]
    pub fn state_projecting(&self, scene: &SceneId) -> StoryState {
        let mut state = self.state_before(scene);
        if let Some(scene) = self.scene(scene) {
            self.fold_scene(&mut state, scene);
        }
        state
    }

    /// A character's knowledge as of one line of one scene.
    ///
    /// Changes placed earlier in the scene count; changes placed later do not;
    /// changes with no line count only after the scene is over. This is the
    /// strict reading, and it is the right one: an accusation of a knowledge
    /// leak should have to clear a bar.
    #[must_use]
    pub fn knowledge_at(
        &self,
        scene: &SceneId,
        line: Option<u32>,
        character: &CharacterId,
    ) -> BTreeSet<FactId> {
        let mut state = self.state_before(scene);
        if let (Some(target), Some(line)) = (self.scene(scene), line) {
            for change in &target.delta.changes {
                if change.at_line.is_some_and(|at| at < line) {
                    apply_change(&mut state, &change.change, &target.id);
                }
            }
        }
        state.character(character).knowledge
    }

    fn fold_until(&self, index: u32, inclusive: bool) -> StoryState {
        let mut state = self.base_state();
        let mut folded: Option<u32> = None;
        for scene in self.ordered_scenes() {
            if !scene.status.contributes_state() {
                continue;
            }
            let in_range = if inclusive {
                scene.index <= index
            } else {
                scene.index < index
            };
            if !in_range {
                continue;
            }
            self.fold_scene(&mut state, scene);
            folded = Some(folded.map_or(scene.index, |high: u32| high.max(scene.index)));
        }
        state.index = folded;
        state.scene = None;
        state
    }

    pub(crate) fn base_state(&self) -> StoryState {
        let mut state = StoryState {
            characters: self
                .characters
                .keys()
                .map(|id| (id.clone(), CharacterState::default()))
                .collect(),
            factions: self
                .factions
                .iter()
                .map(|(id, faction)| {
                    (
                        id.clone(),
                        FactionState {
                            posture: faction.posture,
                            resources: faction.resources.clone(),
                        },
                    )
                })
                .collect(),
            threads: self
                .threads
                .iter()
                .map(|(id, thread)| {
                    (
                        id.clone(),
                        ThreadState {
                            status: ThreadStatus::Unopened,
                            last_touched: None,
                            kind: thread.kind.into(),
                        },
                    )
                })
                .collect(),
            ..StoryState::default()
        };
        for (from, character) in &self.characters {
            for edge in &character.relationships {
                state.relationships.push(RelationshipRecord {
                    from: from.clone(),
                    to: edge.to.clone(),
                    value: edge.value.clone(),
                });
            }
        }
        state
    }

    fn fold_scene(&self, state: &mut StoryState, scene: &Scene) {
        for change in &scene.delta.changes {
            apply_change(state, &change.change, &scene.id);
        }
        for (id, thread) in &self.threads {
            if thread.opened_in == scene.id {
                let entry = state.threads.entry(id.clone()).or_default();
                entry.status = if entry.status == ThreadStatus::Unopened {
                    ThreadStatus::Planted
                } else {
                    entry.status
                };
                entry.last_touched = Some(scene.id.clone());
            }
        }
    }

    /// The scenes a revision would invalidate, nearest first.
    ///
    /// Transitive: if scene 2 changes, and 4 depends on 2, and 7 depends on 4,
    /// then both 4 and 7 are dirty. Only scenes that already exist as prose are
    /// returned — a dirty planned scene is just a plan.
    #[must_use]
    pub fn dirty_after(&self, revised: &SceneId) -> Vec<SceneId> {
        self.dependents(revised)
            .into_iter()
            .filter(|id| {
                self.scene(id)
                    .is_some_and(|scene| scene.status.contributes_state())
            })
            .collect()
    }

    /// Every scene downstream of `scene_id`, transitively, in index order.
    #[must_use]
    pub fn dependents(&self, scene_id: &SceneId) -> Vec<SceneId> {
        let mut found: BTreeSet<SceneId> = BTreeSet::new();
        let mut queue: Vec<SceneId> = vec![scene_id.clone()];
        while let Some(current) = queue.pop() {
            for scene in &self.scenes {
                if scene.dependencies.iter().any(|link| link.scene == current)
                    && found.insert(scene.id.clone())
                {
                    queue.push(scene.id.clone());
                }
            }
        }
        let mut ordered: Vec<SceneId> = found.into_iter().collect();
        ordered.sort_by_key(|id| self.scene(id).map_or(u32::MAX, |scene| scene.index));
        ordered
    }

    /// Scenes whose declared status outruns their dependencies.
    #[must_use]
    pub fn unwritten_premises(&self) -> Vec<(SceneId, SceneId)> {
        let mut result = Vec::new();
        for scene in &self.scenes {
            if !scene.status.contributes_state() {
                continue;
            }
            for link in &scene.dependencies {
                if self
                    .scene(&link.scene)
                    .is_some_and(|dependency| dependency.status == SceneStatus::Planned)
                {
                    result.push((scene.id.clone(), link.scene.clone()));
                }
            }
        }
        result
    }
}

pub(crate) fn apply_change(state: &mut StoryState, change: &Change, scene: &SceneId) {
    match change {
        Change::Learned { character, fact } => {
            state
                .characters
                .entry(character.clone())
                .or_default()
                .knowledge
                .insert(fact.clone());
        }
        Change::Revealed { fact } => {
            state.known_to_reader.insert(fact.clone());
        }
        Change::Relationship {
            from,
            to,
            axis,
            value,
            note,
        } => {
            let position = state
                .relationships
                .iter()
                .position(|record| &record.from == from && &record.to == to);
            let position = match position {
                Some(position) => position,
                None => {
                    state.relationships.push(RelationshipRecord {
                        from: from.clone(),
                        to: to.clone(),
                        value: Relationship::default(),
                    });
                    state.relationships.len() - 1
                }
            };
            let record = &mut state.relationships[position];
            Axis::set(*axis, &mut record.value, *value);
            if !note.is_empty() {
                record.value.note = note.clone();
            }
        }
        Change::Resource {
            faction,
            resource,
            delta,
        } => {
            let entry = state
                .factions
                .entry(faction.clone())
                .or_default()
                .resources
                .entry(resource.clone())
                .or_insert(0);
            *entry += delta;
        }
        Change::Posture {
            faction, posture, ..
        } => {
            state.factions.entry(faction.clone()).or_default().posture = Some(*posture);
        }
        Change::Injury {
            character,
            description,
            severity,
        } => {
            state
                .characters
                .entry(character.clone())
                .or_default()
                .injuries
                .push(InjuryRecord {
                    description: description.clone(),
                    severity: *severity,
                    scene: scene.clone(),
                });
        }
        Change::ThreadPaidOff { thread } => {
            let entry = state.threads.entry(thread.clone()).or_default();
            entry.status = ThreadStatus::PaidOff;
            entry.last_touched = Some(scene.clone());
        }
        Change::ThreadAbandoned { thread, .. } => {
            let entry = state.threads.entry(thread.clone()).or_default();
            entry.status = ThreadStatus::Abandoned;
            entry.last_touched = Some(scene.clone());
        }
        Change::Arrival {
            character,
            location,
        } => {
            state
                .characters
                .entry(character.clone())
                .or_default()
                .location = Some(location.clone());
        }
        Change::Death { character } => {
            let entry = state.characters.entry(character.clone()).or_default();
            entry.alive = false;
        }
        Change::CanonNote { .. } => {}
    }
}
