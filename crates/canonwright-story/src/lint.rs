use std::collections::BTreeMap;

use crate::delta::Change;
use crate::error::StoryError;
use crate::ids::{CharacterId, FactId, SceneId, TargetRef, ThreadId};
use crate::model::{RuleKind, SCHEMA_VERSION, Scene, SceneStatus, Story};
use crate::state::{StoryState, ThreadStatus, apply_change};
use crate::ticket::{Claim, Severity, Substantiation, Ticket, TicketCode};

/// How many scenes a plant may lie dormant before it is called a dangling setup.
///
/// Six is a judgement call, not a law: a Chekhov's gun can sit in a drawer for
/// half a book, and the ticket is advisory precisely because only the author
/// knows whether the silence is suspense or an oversight.
pub const COLD_THREAD_GAP: u32 = 6;

#[derive(Debug, Clone, Copy, Default)]
struct ThreadTrace {
    touched: u32,
    payoffs: u32,
}

impl Story {
    /// Structural faults: a document that cannot be true.
    ///
    /// These are the compiler. They are returned as errors because there is no
    /// reading of the story in which they are correct, and because a tool that
    /// silently repairs them teaches the author to distrust it.
    #[must_use]
    pub fn check(&self) -> Vec<StoryError> {
        let mut errors = Vec::new();

        if self.schema != SCHEMA_VERSION {
            errors.push(StoryError::SchemaVersion {
                found: self.schema,
                expected: SCHEMA_VERSION,
            });
        }

        let mut seen_index: BTreeMap<u32, &Scene> = BTreeMap::new();
        for scene in &self.scenes {
            if seen_index.insert(scene.index, scene).is_some() {
                errors.push(StoryError::DuplicateSceneIndex(scene.index));
            }
            if let Some(pov) = &scene.pov {
                if !self.characters.contains_key(pov) {
                    errors.push(unknown("character", pov.as_str()));
                }
            }
            for character in &scene.present {
                if !self.characters.contains_key(character) {
                    errors.push(unknown("character", character.as_str()));
                }
            }
            for link in &scene.dependencies {
                match self.scene(&link.scene) {
                    None => errors.push(unknown("scene", link.scene.as_str())),
                    Some(dependency) if dependency.index >= scene.index => {
                        errors.push(StoryError::CausalityOrder {
                            scene: scene.id.to_string(),
                            dependency: link.scene.to_string(),
                        });
                    }
                    Some(_) => {}
                }
            }
            self.check_delta(scene, &mut errors);
        }

        for entity in self.entities.values() {
            for reference in &entity.references {
                if !self.resolves(&reference.to) {
                    errors.push(unknown("reference", &reference.to));
                }
            }
        }

        for (id, thread) in &self.threads {
            if self.scene(&thread.opened_in).is_none() {
                errors.push(unknown("scene", thread.opened_in.as_str()));
            }
            if let Some(payoff) = &thread.promised_payoff {
                let opened = self.scene(&thread.opened_in).map(|scene| scene.index);
                match (opened, self.scene(payoff)) {
                    (Some(opened), Some(payoff)) if payoff.index <= opened => {
                        errors.push(StoryError::ThreadPayoffBeforeOpen {
                            thread: id.to_string(),
                            opened: thread.opened_in.to_string(),
                            payoff: payoff.id.to_string(),
                        });
                    }
                    (_, None) => errors.push(unknown("scene", payoff.as_str())),
                    _ => {}
                }
            }
        }

        errors
    }

    fn check_delta(&self, scene: &Scene, errors: &mut Vec<StoryError>) {
        for change in &scene.delta.changes {
            match &change.change {
                Change::Learned { character, fact } => {
                    self.check_character(character, errors);
                    self.check_fact(fact, errors);
                }
                Change::Revealed { fact } => self.check_fact(fact, errors),
                Change::Relationship { from, to, .. } => {
                    self.check_character(from, errors);
                    match to {
                        TargetRef::Character(id) => self.check_character(id, errors),
                        TargetRef::Faction(id) => self.check_faction(id, errors),
                    }
                }
                Change::Resource { faction, .. } | Change::Posture { faction, .. } => {
                    self.check_faction(faction, errors);
                }
                Change::Injury { character, .. }
                | Change::Death { character }
                | Change::Arrival { character, .. } => self.check_character(character, errors),
                Change::ThreadPaidOff { thread } | Change::ThreadAbandoned { thread, .. } => {
                    if !self.threads.contains_key(thread) {
                        errors.push(unknown("thread", thread.as_str()));
                    }
                }
                Change::CanonNote { .. } => {}
            }
        }
    }

    fn check_character(&self, id: &CharacterId, errors: &mut Vec<StoryError>) {
        if !self.characters.contains_key(id) {
            errors.push(unknown("character", id.as_str()));
        }
    }

    fn check_faction(&self, id: &crate::ids::FactionId, errors: &mut Vec<StoryError>) {
        if !self.factions.contains_key(id) {
            errors.push(unknown("faction", id.as_str()));
        }
    }

    fn check_fact(&self, id: &FactId, errors: &mut Vec<StoryError>) {
        if !self.facts.contains_key(id) {
            errors.push(unknown("fact", id.as_str()));
        }
    }

    /// Narrative judgement: things that may be deliberate.
    ///
    /// Everything here is advisory. A reviewer that can block a commit is a
    /// reviewer the author learns to route around, and the interesting fiction
    /// is usually the scene that trips a warning.
    /// Does a cross-reference point at something that exists?
    ///
    /// Ids can name an entity, a rule, a thread or a scene — the four things a
    /// rulebook or a novel actually cites.
    #[must_use]
    pub fn resolves(&self, id: &str) -> bool {
        if let Ok(entity) = crate::ids::EntityId::new(id) {
            if self.entities.contains_key(&entity) {
                return true;
            }
        }
        if let Ok(rule) = crate::ids::RuleId::new(id) {
            if self.rules.contains_key(&rule) {
                return true;
            }
        }
        if let Ok(thread) = crate::ids::ThreadId::new(id) {
            if self.threads.contains_key(&thread) {
                return true;
            }
        }
        self.scenes.iter().any(|scene| scene.id.as_str() == id)
    }

    #[must_use]
    pub fn lint(&self) -> Vec<Ticket> {
        let mut tickets = Vec::new();
        if !self.meta.kind.lints_as_narrative() {
            // A rulebook is not a story. Narrative checks here would be noise,
            // and noise is how a linter gets switched off.
            tickets.extend(self.entity_tickets());
            return tickets;
        }
        let mut traces: BTreeMap<ThreadId, ThreadTrace> = BTreeMap::new();
        let mut state = self.base_state();
        let mut highest: Option<u32> = None;
        let mut first_index: Option<u32> = None;

        for scene in self.ordered_scenes() {
            if !scene.status.contributes_state() {
                continue;
            }
            first_index.get_or_insert(scene.index);
            highest = Some(highest.map_or(scene.index, |high| high.max(scene.index)));

            // Check against the world as it stood *before* this scene.
            for character in &scene.present {
                let alive = state.characters.get(character).is_none_or(|c| c.alive);
                if !alive {
                    tickets.push(
                        Ticket::new(
                            TicketCode::ContinuityBreak,
                            scene.id.clone(),
                            format!(
                                "{character} appears in this scene after their death was \
                                 established earlier in the story"
                            ),
                        )
                        .about(character.to_string()),
                    );
                }
            }

            if !scene.has_value_axis() {
                tickets.push(
                    Ticket::new(
                        TicketCode::NoValueShift,
                        scene.id.clone(),
                        "the scene declares no value at stake, so there is nothing to change",
                    )
                    .claiming(Claim::ValueShift),
                );
            } else if !scene.shifts_value() {
                tickets.push(
                    Ticket::new(
                        TicketCode::NoValueShift,
                        scene.id.clone(),
                        format!(
                            "the scene starts and ends on the same value ({:?}); something \
                             should be different for someone",
                            scene.value_start.trim()
                        ),
                    )
                    .claiming(Claim::ValueShift),
                );
            }

            let is_first = first_index == Some(scene.index);
            if !is_first && scene.dependencies.is_empty() {
                tickets.push(
                    Ticket::new(
                        TicketCode::CausalityBreak,
                        scene.id.clone(),
                        "the scene declares no causal link to an earlier scene",
                    )
                    .with_severity(Severity::Warning),
                );
            }

            // Fold the scene, then trace the threads it moved.
            apply_scene(&mut state, self, scene);
            for change in &scene.delta.changes {
                if let Some(thread) = thread_of(&change.change) {
                    let trace = traces.entry(thread.clone()).or_default();
                    trace.touched += 1;
                    if matches!(change.change, Change::ThreadPaidOff { .. }) {
                        trace.payoffs += 1;
                    }
                }
            }
        }

        for (id, thread) in &self.threads {
            let opened = self.scene(&thread.opened_in).map(|scene| scene.index);
            let trace = traces.get(id).copied().unwrap_or_default();
            let status = state.thread(id).status;

            if trace.payoffs > 1 {
                tickets.push(
                    Ticket::new(
                        TicketCode::ThreadAlreadyPaidOff,
                        thread.opened_in.clone(),
                        format!(
                            "thread {id:?} is paid off {} times; a payoff only lands once",
                            trace.payoffs
                        ),
                    )
                    .about(id.to_string())
                    .claiming(Claim::Thread { thread: id.clone() }),
                );
            }

            let Some((opened, highest)) = opened.zip(highest) else {
                continue;
            };

            if let Some(promised) = &thread.promised_payoff {
                let promised_index = self.scene(promised).map(|scene| scene.index);
                if promised_index.is_some_and(|index| index <= highest)
                    && status != ThreadStatus::PaidOff
                {
                    tickets.push(
                        Ticket::new(
                            TicketCode::UnearnedResolution,
                            promised.clone(),
                            format!(
                                "thread {id:?} was promised a payoff here and none has \
                                 happened by scene {highest}"
                            ),
                        )
                        .about(id.to_string())
                        .claiming(Claim::Thread { thread: id.clone() }),
                    );
                }
            }

            let dormant = status == ThreadStatus::Planted
                && trace.touched <= 1
                && highest.saturating_sub(opened) >= COLD_THREAD_GAP;
            if dormant {
                tickets.push(
                    Ticket::new(
                        TicketCode::DanglingSetup,
                        thread.opened_in.clone(),
                        format!(
                            "thread {id:?} was planted in scene {opened} and has not been \
                             touched in the {} scenes since",
                            highest - opened
                        ),
                    )
                    .about(id.to_string())
                    .claiming(Claim::Thread { thread: id.clone() }),
                );
            }
        }

        tickets.extend(self.entity_tickets());
        tickets.sort_by(|a, b| {
            self.scene(&a.scene)
                .map(|scene| scene.index)
                .cmp(&self.scene(&b.scene).map(|scene| scene.index))
                .then_with(|| a.severity.cmp(&b.severity))
                .then_with(|| a.code.cmp(&b.code))
        });
        tickets
    }

    /// Findings that apply to rules content in any kind of book.
    #[must_use]
    pub fn entity_tickets(&self) -> Vec<Ticket> {
        let mut tickets = Vec::new();
        for (id, entity) in &self.entities {
            if entity.is_incomplete() {
                tickets.push(
                    Ticket::new(
                        TicketCode::IncompleteEntity,
                        self.ordered_scenes().first().map_or_else(
                            || crate::ids::SceneId::new("s1").expect("literal"),
                            |scene| scene.id.clone(),
                        ),
                        format!(
                            "{} `{id}` has no fields and no summary; a table cannot use it",
                            entity.kind.name()
                        ),
                    )
                    .about(id.to_string()),
                );
            }
        }
        tickets
    }

    /// Does the timeline actually support this ticket?
    ///
    /// This is the evidence gate. A reviewer may still be wrong in ways only a
    /// human can see — subtext, voice, whether a chapter earns its twist — but
    /// it may not be wrong about arithmetic.
    #[must_use]
    pub fn substantiate(&self, ticket: &Ticket) -> Substantiation {
        let Some(claim) = &ticket.claim else {
            return Substantiation::Uncheckable;
        };
        match claim {
            Claim::Knowledge { character, fact } => {
                if !self.characters.contains_key(character) {
                    return Substantiation::Refuted(format!("unknown character {character}"));
                }
                if !self.facts.contains_key(fact) {
                    return Substantiation::Refuted(format!("unknown fact {fact}"));
                }
                if self
                    .knowledge_at(&ticket.scene, ticket.line, character)
                    .contains(fact)
                {
                    Substantiation::Refuted(format!(
                        "{character} learned {fact} before this point in the timeline"
                    ))
                } else {
                    Substantiation::Confirmed
                }
            }
            Claim::Thread { thread } => {
                if self.threads.contains_key(thread) {
                    Substantiation::Confirmed
                } else {
                    Substantiation::Refuted(format!("unknown thread {thread}"))
                }
            }
            Claim::Rule { rule } => match self.rules.get(rule) {
                None => Substantiation::Refuted(format!("unknown rule {rule}")),
                Some(found) if found.kind != RuleKind::Hard => Substantiation::Refuted(format!(
                    "rule {rule} is soft lore; breaking it is a choice, not a bug"
                )),
                Some(_) => Substantiation::Confirmed,
            },
            Claim::ValueShift => match self.scene(&ticket.scene) {
                None => Substantiation::Refuted("unknown scene".to_owned()),
                Some(scene) if scene.shifts_value() => Substantiation::Refuted(
                    "the scene does declare a value shift; quote the unchanged value to \
                     raise this"
                        .to_owned(),
                ),
                Some(_) => Substantiation::Confirmed,
            },
        }
    }

    /// The scenes that would need a continuity pass if this scene were revised.
    #[must_use]
    pub fn revision_blast_radius(&self, scene: &SceneId) -> usize {
        self.dirty_after(scene).len()
    }

    /// Scenes marked canon whose premise was never written.
    #[must_use]
    pub fn canon_with_planned_premises(&self) -> Vec<(SceneId, SceneId)> {
        self.unwritten_premises()
            .into_iter()
            .filter(|(scene, _)| {
                self.scene(scene)
                    .is_some_and(|scene| scene.status == SceneStatus::Canon)
            })
            .collect()
    }
}

fn apply_scene(state: &mut StoryState, story: &Story, scene: &Scene) {
    for change in &scene.delta.changes {
        apply_change(state, &change.change, &scene.id);
    }
    for (id, thread) in &story.threads {
        if thread.opened_in == scene.id {
            let entry = state.threads.entry(id.clone()).or_default();
            if entry.status == ThreadStatus::Unopened {
                entry.status = ThreadStatus::Planted;
            }
            entry.last_touched = Some(scene.id.clone());
        }
    }
}

fn thread_of(change: &Change) -> Option<&ThreadId> {
    match change {
        Change::ThreadPaidOff { thread } | Change::ThreadAbandoned { thread, .. } => Some(thread),
        _ => None,
    }
}

fn unknown(kind: &'static str, id: &str) -> StoryError {
    StoryError::Unknown {
        kind,
        id: id.to_owned(),
    }
}
