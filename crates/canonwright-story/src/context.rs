use serde::{Deserialize, Serialize};

use crate::ids::{CharacterId, SceneId, ThreadId};
use crate::model::{Scene, Story};
use crate::state::{StoryState, ThreadStatus};

/// The prompt payload for one scene, assembled from the timeline.
///
/// This is the Context Assembler from the architecture: the point of view's
/// knowledge, the people actually present, the causal lead-in, and the threads
/// still owed — and nothing else. The `withheld` counts exist so the author can
/// see what was filtered rather than trusting that the filter did something.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneContext {
    pub scene: SceneId,
    pub index: u32,
    pub markdown: String,
    pub withheld: Withheld,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Withheld {
    /// Scenes after this one. They exist in the document and not in the prompt.
    pub later_scenes: usize,
    /// Facts the reader has not been told yet.
    pub unrevealed_facts: usize,
    /// Facts this scene's point of view has not learned.
    pub unknown_to_pov: usize,
    /// Threads planned but not yet planted.
    pub unopened_threads: usize,
    /// Threads planted but not paid off, truncated if there were many.
    pub open_threads_omitted: usize,
}

/// How many open threads are worth putting in front of a drafter before it
/// becomes a list nobody reads.
const MAX_OPEN_THREADS: usize = 8;

impl Story {
    /// Assemble the scene brief.
    ///
    /// Returns `None` only when the scene does not exist; a scene with no canon
    /// before it still gets a brief, because chapter one is a legitimate place
    /// to start.
    #[must_use]
    pub fn context_for(&self, scene: &SceneId) -> Option<SceneContext> {
        let target = self.scene(scene)?;
        let state = self.state_before(scene);
        let markdown = self.render_context(target, &state);
        Some(SceneContext {
            scene: scene.clone(),
            index: target.index,
            markdown,
            withheld: self.withheld(target, &state),
        })
    }

    fn withheld(&self, target: &Scene, state: &StoryState) -> Withheld {
        let later_scenes = self
            .scenes
            .iter()
            .filter(|scene| scene.index > target.index)
            .count();
        let unrevealed = self
            .facts
            .keys()
            .filter(|fact| !state.known_to_reader.contains(fact))
            .count();
        let pov_knowledge = state.character(&target.pov).knowledge;
        let unknown_to_pov = self
            .facts
            .keys()
            .filter(|fact| !pov_knowledge.contains(fact))
            .count();
        let unopened = self
            .threads
            .iter()
            .filter(|(_, thread)| {
                self.scene(&thread.opened_in)
                    .is_some_and(|scene| scene.index > target.index)
            })
            .count();
        let open = self
            .threads
            .keys()
            .filter(|id| state.is_open_thread(id))
            .count();
        Withheld {
            later_scenes,
            unrevealed_facts: unrevealed,
            unknown_to_pov,
            unopened_threads: unopened,
            open_threads_omitted: open.saturating_sub(MAX_OPEN_THREADS),
        }
    }

    fn render_context(&self, target: &Scene, state: &StoryState) -> String {
        let mut out = String::new();
        let title = if target.title.is_empty() {
            "(untitled)"
        } else {
            target.title.as_str()
        };
        out.push_str(&format!("# Scene {} — {title}\n\n", target.index));
        out.push_str(&format!(
            "- Point of view: {}\n- Location: {}\n- Status: {}\n",
            self.character_label(&target.pov),
            or_unknown(&target.location),
            target.status.name()
        ));
        if !target.summary.is_empty() {
            out.push_str(&format!("\n**Brief.** {}\n", target.summary));
        }
        if target.has_value_axis() {
            out.push_str(&format!(
                "\n**Value at stake.** {} → {}\n",
                or_dash(&target.value_start),
                or_dash(&target.value_end)
            ));
        }

        if !target.dependencies.is_empty() {
            out.push_str("\n## How we got here\n\n");
            for link in &target.dependencies {
                let source = self
                    .scene(&link.scene)
                    .map(|scene| format!("Scene {} — {}", scene.index, scene.title))
                    .unwrap_or_else(|| link.scene.to_string());
                let note = if link.note.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", link.note)
                };
                out.push_str(&format!(
                    "- {} {source}{note}\n",
                    capitalise(link.kind.name())
                ));
            }
        }

        out.push_str("\n## Present\n\n");
        let mut present: Vec<CharacterId> = target.present.clone();
        if !present.contains(&target.pov) {
            present.insert(0, target.pov.clone());
        }
        for id in &present {
            self.render_character(&mut out, id, &target.pov, state);
        }

        let open: Vec<(&ThreadId, &crate::model::Thread)> = self
            .threads
            .iter()
            .filter(|(id, _)| state.is_open_thread(id))
            .take(MAX_OPEN_THREADS)
            .collect();
        if !open.is_empty() {
            out.push_str("\n## Owed to the reader\n\n");
            for (id, thread) in open {
                let status = state.thread(id).status;
                let label = match status {
                    ThreadStatus::Advanced => "advanced",
                    ThreadStatus::PaidOff | ThreadStatus::Abandoned => continue,
                    _ => "planted",
                };
                out.push_str(&format!(
                    "- {id} ({:?}, {label}): {}\n",
                    thread.kind, thread.label
                ));
            }
        }

        if !state.known_to_reader.is_empty() {
            out.push_str("\n## The reader knows\n\n");
            for fact in &state.known_to_reader {
                if let Some(fact) = self.facts.get(fact) {
                    out.push_str(&format!("- {}\n", fact.statement));
                }
            }
        }

        out
    }

    fn render_character(
        &self,
        out: &mut String,
        id: &CharacterId,
        pov: &CharacterId,
        state: &StoryState,
    ) {
        let Some(character) = self.characters.get(id) else {
            out.push_str(&format!("- {id} (not in the cast list)\n"));
            return;
        };
        let current = state.character(id);
        let marker = if id == pov { " (POV)" } else { "" };
        out.push_str(&format!(
            "\n### {}{marker}\n\n",
            or_unknown(&character.name)
        ));
        if !character.role.is_empty() {
            out.push_str(&format!("- Role: {}\n", character.role));
        }
        if !character.want.is_empty() {
            out.push_str(&format!("- Wants: {}\n", character.want));
        }
        if !character.need.is_empty() {
            out.push_str(&format!("- Needs, and cannot say so: {}\n", character.need));
        }
        if !current.alive {
            out.push_str("- Not alive at this point. Do not show them acting.\n");
        }
        if !current.injuries.is_empty() {
            let wounds: Vec<&str> = current
                .injuries
                .iter()
                .map(|injury| injury.description.as_str())
                .collect();
            out.push_str(&format!("- Carrying: {}\n", wounds.join("; ")));
        }
        if !character.voice.is_empty() {
            out.push_str(&format!("- Voice: {}\n", render_voice(&character.voice)));
        }
        let knowledge: Vec<&str> = current
            .knowledge
            .iter()
            .filter_map(|fact| self.facts.get(fact))
            .map(|fact| fact.statement.as_str())
            .collect();
        if knowledge.is_empty() {
            out.push_str("- Knows nothing they were not born knowing.\n");
        } else {
            out.push_str(&format!("- Knows: {}\n", knowledge.join("; ")));
        }
        for edge in &character.relationships {
            let now = state.relationship(id, &edge.to);
            if now == edge.value {
                continue;
            }
            out.push_str(&format!(
                "- Feels toward {}: {}\n",
                edge.to,
                render_relationship(&now)
            ));
        }
    }

    #[must_use]
    pub fn character_label(&self, id: &CharacterId) -> String {
        self.characters
            .get(id)
            .map(|character| {
                if character.name.is_empty() {
                    id.to_string()
                } else {
                    format!("{} ({id})", character.name)
                }
            })
            .unwrap_or_else(|| id.to_string())
    }
}

fn render_voice(voice: &crate::model::VoiceProfile) -> String {
    let mut parts = Vec::new();
    if !voice.register.is_empty() {
        parts.push(voice.register.clone());
    }
    if !voice.rhythm.is_empty() {
        parts.push(voice.rhythm.clone());
    }
    if !voice.favored.is_empty() {
        parts.push(format!("reaches for {}", voice.favored.join(", ")));
    }
    if !voice.forbidden.is_empty() {
        parts.push(format!("never says {}", voice.forbidden.join(", ")));
    }
    if !voice.tics.is_empty() {
        parts.push(format!("tics: {}", voice.tics.join(", ")));
    }
    if !voice.tells.is_empty() {
        parts.push(format!("tells: {}", voice.tells.join(", ")));
    }
    parts.join(". ")
}

fn render_relationship(relationship: &crate::model::Relationship) -> String {
    let mut parts = Vec::new();
    for axis in [
        crate::model::Axis::Trust,
        crate::model::Axis::Fear,
        crate::model::Axis::Debt,
        crate::model::Axis::Resentment,
    ] {
        let value = axis.get(relationship);
        if value != 0 {
            parts.push(format!("{} {value:+}", axis.name()));
        }
    }
    if !relationship.note.is_empty() {
        parts.push(relationship.note.clone());
    }
    if parts.is_empty() {
        "neutral".to_owned()
    } else {
        parts.join(", ")
    }
}

fn or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "(unspecified)"
    } else {
        value
    }
}

fn or_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

fn capitalise(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
