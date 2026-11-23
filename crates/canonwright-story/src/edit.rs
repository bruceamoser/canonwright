use std::collections::BTreeSet;

use crate::error::StoryError;
use crate::ids::SceneId;
use crate::model::{Scene, SceneStatus, Story};
use crate::ticket::Ticket;

/// What happened when a scene was written.
///
/// Rejection is not an exception: it is the normal outcome of writing a scene
/// that contradicts the document, and the caller needs the errors, not a panic.
#[derive(Debug)]
pub struct WriteOutcome {
    /// False when the story was left untouched because the write was invalid.
    pub accepted: bool,
    pub created: bool,
    pub errors: Vec<StoryError>,
    /// Canon scenes downstream of this scene that now need a continuity pass.
    pub dirty: Vec<SceneId>,
    /// Tickets for this scene alone, so a caller does not have to lint the book.
    pub tickets: Vec<Ticket>,
}

impl WriteOutcome {
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.accepted
    }
}

/// A scene is stale when the premises it was reviewed against have moved.
///
/// This is the build-system half of the design: `reviewed_against` stores a
/// digest of the transitive dependencies' deltas at the moment a human accepted
/// the scene. If an upstream delta changes, the digest no longer matches and the
/// scene is dirty again — no timestamps, no manual invalidation, and nothing to
/// remember across a restart.
impl Story {
    /// Write a scene, all of it: status, value shift, causal links, and delta.
    ///
    /// The whole story is validated before the write is kept. A document that
    /// cannot be true is rejected wholesale rather than accepted with a warning,
    /// because the warnings would then be everybody's problem forever.
    pub fn write_scene(&mut self, scene: Scene) -> WriteOutcome {
        let id = scene.id.clone();
        let created = self.scene(&id).is_none();

        let mut candidate = self.clone();
        match candidate.scene_mut(&id) {
            Some(existing) => *existing = scene,
            None => candidate.scenes.push(scene),
        }

        let errors = candidate.check();
        if !errors.is_empty() {
            return WriteOutcome {
                accepted: false,
                created,
                errors,
                dirty: Vec::new(),
                tickets: Vec::new(),
            };
        }

        *self = candidate;
        let dirty = self.dirty_dependents(&id);
        let tickets = self
            .lint()
            .into_iter()
            .filter(|ticket| ticket.scene == id)
            .collect();
        WriteOutcome {
            accepted: true,
            created,
            errors: Vec::new(),
            dirty,
            tickets,
        }
    }

    /// Move a scene between statuses without rewriting its body.
    pub fn set_scene_status(&mut self, scene: &SceneId, status: SceneStatus) -> WriteOutcome {
        let Some(mut target) = self.scene(scene).cloned() else {
            return WriteOutcome {
                accepted: false,
                created: false,
                errors: vec![StoryError::Unknown {
                    kind: "scene",
                    id: scene.to_string(),
                }],
                dirty: Vec::new(),
                tickets: Vec::new(),
            };
        };
        target.status = status;
        self.write_scene(target)
    }

    /// Record that a human accepted this scene against its current premises.
    pub fn mark_reviewed(&mut self, scene: &SceneId) -> Result<(), StoryError> {
        let digest = self
            .premises_digest(scene)
            .ok_or_else(|| StoryError::Unknown {
                kind: "scene",
                id: scene.to_string(),
            })?;
        match self.scene_mut(scene) {
            Some(target) => {
                target.reviewed_against = Some(digest);
                Ok(())
            }
            None => Err(StoryError::Unknown {
                kind: "scene",
                id: scene.to_string(),
            }),
        }
    }

    /// Has a premise moved since this scene was accepted?
    ///
    /// Only canon scenes can be dirty: a drafted scene is not stale, it is
    /// unfinished.
    #[must_use]
    pub fn is_dirty(&self, scene: &SceneId) -> bool {
        let Some(target) = self.scene(scene) else {
            return false;
        };
        if target.status != SceneStatus::Canon {
            return false;
        }
        match (&target.reviewed_against, self.premises_digest(scene)) {
            (Some(recorded), Some(current)) => recorded != &current,
            // Accepted without ever recording what it was accepted against.
            (None, _) => true,
            (_, None) => false,
        }
    }

    /// Every canon scene that needs a continuity pass, in narrative order.
    #[must_use]
    pub fn dirty_scenes(&self) -> Vec<SceneId> {
        self.ordered_scenes()
            .into_iter()
            .filter(|scene| self.is_dirty(&scene.id))
            .map(|scene| scene.id.clone())
            .collect()
    }

    /// Canon scenes downstream of `scene` that this write invalidated.
    #[must_use]
    pub fn dirty_dependents(&self, scene: &SceneId) -> Vec<SceneId> {
        self.dependents(scene)
            .into_iter()
            .filter(|id| self.is_dirty(id))
            .collect()
    }

    /// A digest of everything this scene's truth rests on.
    ///
    /// Transitive, and over deltas rather than prose: rewriting a scene's
    /// sentences does not invalidate its dependents, but changing what the scene
    /// *did* does.
    #[must_use]
    pub fn premises_digest(&self, scene: &SceneId) -> Option<String> {
        self.scene(scene)?;
        let mut hasher = blake3::Hasher::new();
        let mut premises: Vec<SceneId> = self.premises(scene);
        premises.sort();
        for id in premises {
            if let Some(premise) = self.scene(&id) {
                hasher.update(id.as_str().as_bytes());
                hasher.update(b"\x00");
                hasher.update(delta_digest(premise).as_bytes());
                hasher.update(b"\x01");
            }
        }
        Some(hasher.finalize().to_hex().to_string())
    }

    /// Every scene this one transitively depends on.
    #[must_use]
    pub fn premises(&self, scene: &SceneId) -> Vec<SceneId> {
        let mut found: BTreeSet<SceneId> = BTreeSet::new();
        let mut queue: Vec<SceneId> = vec![scene.clone()];
        while let Some(current) = queue.pop() {
            let Some(current) = self.scene(&current) else {
                continue;
            };
            for link in &current.dependencies {
                if found.insert(link.scene.clone()) {
                    queue.push(link.scene.clone());
                }
            }
        }
        found.into_iter().collect()
    }
}

fn delta_digest(scene: &Scene) -> String {
    let bytes = serde_json::to_vec(&scene.delta).unwrap_or_default();
    blake3::hash(&bytes).to_hex().to_string()
}
