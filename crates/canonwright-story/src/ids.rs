use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::StoryError;

/// Identifiers are slugs, not UUIDs.
///
/// A story is a document people edit by hand and quote to each other, so
/// `mara-vale` is worth more than `0192f3a1-…`: it survives a merge conflict,
/// it reads in a critique ticket, and a model can reproduce it from memory.
/// The cost is that ids are constrained — lowercase ASCII, digits, `-`, `_`.
const MAX_ID_LEN: usize = 64;

fn validate_slug(value: &str) -> Result<(), StoryError> {
    let invalid = |reason: &'static str| StoryError::InvalidId {
        value: value.to_owned(),
        reason,
    };
    if value.is_empty() {
        return Err(invalid("it is empty"));
    }
    if value.len() > MAX_ID_LEN {
        return Err(invalid("it is longer than 64 characters"));
    }
    if !value
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
    {
        return Err(invalid("it must start with a lowercase letter or a digit"));
    }
    if let Some(bad) = value
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_'))
    {
        return Err(invalid(match bad {
            'A'..='Z' => "uppercase letters are not allowed",
            ' ' => "spaces are not allowed; use a hyphen",
            _ => "only lowercase ASCII letters, digits, '-' and '_' may appear",
        }));
    }
    Ok(())
}

macro_rules! story_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);

        impl $name {
            /// Build an identifier, rejecting anything that is not a slug.
            pub fn new(value: impl Into<String>) -> Result<Self, StoryError> {
                let value = value.into();
                validate_slug(&value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = StoryError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = StoryError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = StoryError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

story_id!(CharacterId);
story_id!(FactionId);
story_id!(RuleId);
story_id!(FactId);
story_id!(SceneId);
story_id!(ThreadId);
story_id!(EntityId);
story_id!(TicketId);

/// Anything a directed relationship edge can point at.
///
/// Characters hold opinions about people *and* about institutions, and the
/// distinction matters: trust in a faction outlives trust in its current
/// leader. One edge type over two targets keeps the fold in [`crate::state`]
/// simple.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum TargetRef {
    Character(CharacterId),
    Faction(FactionId),
}

impl TargetRef {
    #[must_use]
    pub fn character(id: CharacterId) -> Self {
        Self::Character(id)
    }

    #[must_use]
    pub fn faction(id: FactionId) -> Self {
        Self::Faction(id)
    }

    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Character(_) => "character",
            Self::Faction(_) => "faction",
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Character(id) => id.as_str(),
            Self::Faction(id) => id.as_str(),
        }
    }
}

impl fmt::Display for TargetRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind(), self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_accept_the_documented_alphabet() {
        assert!(CharacterId::new("mara").is_ok());
        assert!(CharacterId::new("mara-vale").is_ok());
        assert!(CharacterId::new("the_order_2").is_ok());
        assert!(CharacterId::new("7th-sword").is_ok());
    }

    #[test]
    fn slugs_reject_anything_a_model_could_drift_on() {
        assert!(CharacterId::new("").is_err());
        assert!(CharacterId::new("Mara").is_err());
        assert!(CharacterId::new("mara vale").is_err());
        assert!(CharacterId::new("mara.vale").is_err());
        assert!(CharacterId::new("-mara").is_err());
        assert!(CharacterId::new("x".repeat(MAX_ID_LEN + 1)).is_err());
    }

    #[test]
    fn deserializing_a_bad_id_fails_instead_of_producing_a_poisoned_story() {
        assert!(serde_json::from_str::<CharacterId>("\"Mara\"").is_err());
        assert_eq!(
            serde_json::from_str::<CharacterId>("\"mara\"").unwrap(),
            CharacterId::new("mara").unwrap()
        );
    }

    #[test]
    fn target_refs_round_trip_through_json() {
        let target = TargetRef::faction(FactionId::new("ash-guild").unwrap());
        let encoded = serde_json::to_string(&target).unwrap();
        assert_eq!(encoded, r#"{"kind":"faction","id":"ash-guild"}"#);
        assert_eq!(serde_json::from_str::<TargetRef>(&encoded).unwrap(), target);
        assert_eq!(target.to_string(), "faction:ash-guild");
    }
}
