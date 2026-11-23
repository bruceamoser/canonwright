use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ids::{CharacterId, FactId, RuleId, SceneId, ThreadId, TicketId};

/// The critique vocabulary. Codes are stable strings because they are quoted in
/// prompts, in the UI, and in an author's notes; renaming one breaks all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TicketCode {
    /// A character acts on a fact they have not learned yet.
    KnowledgeLeak,
    /// A character speaks or thinks outside their established voice profile.
    VoiceDrift,
    /// A character behaves against their motivations or established traits.
    OutOfCharacter,
    /// Events happen to the point-of-view character instead of because of them.
    PassiveProtagonist,
    /// The scene does not follow from anything, or a premise is unwritten.
    CausalityBreak,
    /// A plant with no payoff in sight.
    DanglingSetup,
    /// A payoff the story did not earn.
    UnearnedResolution,
    /// A hard world rule was broken.
    RuleViolation,
    /// The scene changes nothing: the value at the end equals the value at the start.
    NoValueShift,
    /// Characters state their feelings or the plot directly.
    OnTheNose,
    /// A physical or timeline impossibility carrying across scenes.
    ContinuityBreak,
    /// A thread paid off twice, or paid off before it is planted.
    ThreadAlreadyPaidOff,
    /// A rules entry without the numbers a table would need.
    IncompleteEntity,
}

impl TicketCode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KnowledgeLeak => "KNOWLEDGE_LEAK",
            Self::VoiceDrift => "VOICE_DRIFT",
            Self::OutOfCharacter => "OUT_OF_CHARACTER",
            Self::PassiveProtagonist => "PASSIVE_PROTAGONIST",
            Self::CausalityBreak => "CAUSALITY_BREAK",
            Self::DanglingSetup => "DANGLING_SETUP",
            Self::UnearnedResolution => "UNEARNED_RESOLUTION",
            Self::RuleViolation => "RULE_VIOLATION",
            Self::NoValueShift => "NO_VALUE_SHIFT",
            Self::OnTheNose => "ON_THE_NOSE",
            Self::ContinuityBreak => "CONTINUITY_BREAK",
            Self::ThreadAlreadyPaidOff => "THREAD_ALREADY_PAID_OFF",
            Self::IncompleteEntity => "INCOMPLETE_ENTITY",
        }
    }

    /// Which reviewer is allowed to raise this code. Function and domain are
    /// orthogonal, and an agent that can raise everything dilutes its prompt.
    #[must_use]
    pub fn reviewer(self) -> &'static str {
        match self {
            Self::KnowledgeLeak | Self::OutOfCharacter | Self::PassiveProtagonist => "character",
            Self::CausalityBreak
            | Self::DanglingSetup
            | Self::UnearnedResolution
            | Self::NoValueShift => "plot",
            Self::RuleViolation | Self::ContinuityBreak => "world",
            Self::OnTheNose | Self::VoiceDrift => "subtext",
            Self::ThreadAlreadyPaidOff => "plot",
            Self::IncompleteEntity => "rules",
        }
    }

    #[must_use]
    pub fn default_severity(self) -> Severity {
        match self {
            Self::VoiceDrift | Self::NoValueShift | Self::OnTheNose => Severity::Advisory,
            Self::IncompleteEntity => Severity::Warning,
            Self::DanglingSetup | Self::PassiveProtagonist | Self::ContinuityBreak => {
                Severity::Warning
            }
            Self::KnowledgeLeak
            | Self::OutOfCharacter
            | Self::CausalityBreak
            | Self::UnearnedResolution
            | Self::RuleViolation
            | Self::ThreadAlreadyPaidOff => Severity::Error,
        }
    }
}

impl fmt::Display for TicketCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[{}]", self.as_str())
    }
}

/// Nothing here is a build blocker. `Error` means "this is almost certainly
/// wrong", not "the tool refuses to continue": fiction is allowed to break its
/// own rules on purpose, and a harness that cannot be overruled will be turned
/// off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Advisory,
    Warning,
    Error,
}

/// The machine-checkable part of a ticket.
///
/// A reviewer that only says "Mara could not know that" is unfalsifiable. A
/// reviewer that names the character and the fact is answering a question the
/// harness can settle by folding the timeline. Claims are how critique becomes
/// evidence instead of opinion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "claim", rename_all = "snake_case")]
pub enum Claim {
    Knowledge {
        character: CharacterId,
        fact: FactId,
    },
    Thread {
        thread: ThreadId,
    },
    Rule {
        rule: RuleId,
    },
    ValueShift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: TicketId,
    pub code: TicketCode,
    pub severity: Severity,
    pub scene: SceneId,
    /// Line in the scene file. Optional because structural tickets have no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Who or what the ticket is about, for grouping in a triage list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    /// Always false in this version. The field exists so the policy is explicit
    /// and so a future gate has something to read rather than a convention.
    #[serde(default)]
    pub blocking: bool,
}

impl Ticket {
    #[must_use]
    pub fn new(code: TicketCode, scene: SceneId, message: impl Into<String>) -> Self {
        let message = message.into();
        let id = stable_ticket_id(code, &scene, None, &message);
        Self {
            id,
            code,
            severity: code.default_severity(),
            scene,
            line: None,
            subject: None,
            message,
            claim: None,
            blocking: false,
        }
    }

    #[must_use]
    pub fn at(mut self, line: u32) -> Self {
        self.line = Some(line);
        self.id = stable_ticket_id(self.code, &self.scene, self.line, &self.message);
        self
    }

    #[must_use]
    pub fn about(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    #[must_use]
    pub fn claiming(mut self, claim: Claim) -> Self {
        self.claim = Some(claim);
        self
    }

    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Stable across runs, so a dismissed ticket stays dismissed instead of
    /// reappearing with a new id every time the reviewer is asked again.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.id.to_string()
    }
}

fn stable_ticket_id(
    code: TicketCode,
    scene: &SceneId,
    line: Option<u32>,
    message: &str,
) -> TicketId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(code.as_str().as_bytes());
    hasher.update(b"\x00");
    hasher.update(scene.as_str().as_bytes());
    hasher.update(b"\x00");
    hasher.update(line.unwrap_or(0).to_le_bytes().as_slice());
    hasher.update(b"\x00");
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();
    let short = digest.to_hex();
    // A hash that starts with a digit is still a valid slug, so no prefix is
    // needed; the id is opaque either way.
    TicketId::new(&short.as_str()[..16]).expect("hex digest is a valid slug")
}

impl fmt::Display for Ticket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.code, self.message)?;
        if let Some(line) = self.line {
            write!(formatter, " (line {line})")?;
        }
        Ok(())
    }
}

/// The verdict when a claim meets the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Substantiation {
    /// The timeline agrees: the character really did not know, the thread
    /// really is unplanted, the rule really is hard.
    Confirmed,
    /// The timeline contradicts the ticket. Keep the ticket, fix the reviewer.
    Refuted(String),
    /// Not a checkable claim. Only the author can settle it.
    Uncheckable,
}
