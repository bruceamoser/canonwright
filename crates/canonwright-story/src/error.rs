/// Errors raised while reading, validating, or querying a story.
///
/// These are *structural* faults — a document that cannot be true — as opposed
/// to [`crate::Ticket`]s, which are narrative judgements that a human may
/// overrule.
#[derive(Debug, thiserror::Error)]
pub enum StoryError {
    #[error("invalid identifier {value:?}: {reason}")]
    InvalidId { value: String, reason: &'static str },

    #[error("unknown {kind} {id:?}")]
    Unknown { kind: &'static str, id: String },

    #[error("story schema is version {found}, but this build reads version {expected}")]
    SchemaVersion { found: u32, expected: u32 },

    #[error("scene index {0} is used twice")]
    DuplicateSceneIndex(u32),

    #[error("scene {scene:?} cannot depend on scene {dependency:?}, which is not earlier")]
    CausalityOrder { scene: String, dependency: String },

    #[error("thread {thread:?} pays off in {payoff:?}, which does not follow {opened:?}")]
    ThreadPayoffBeforeOpen {
        thread: String,
        opened: String,
        payoff: String,
    },

    #[error("a scene must declare at least one causal dependency once the story has begun")]
    MissingCausalLink { scene: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
