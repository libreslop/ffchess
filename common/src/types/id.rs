use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Declares a random UUID-backed identifier newtype.
macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generates a new random identifier.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Returns the nil identifier, used as a placeholder.
            pub fn nil() -> Self {
                Self(Uuid::nil())
            }

            /// Returns true when this is the nil placeholder identifier.
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

/// Declares a string-backed config identifier newtype.
macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

uuid_id!(
    /// Unique identifier for a player across sessions.
    PlayerId
);
uuid_id!(
    /// Unique identifier for a piece instance.
    PieceId
);
uuid_id!(
    /// Private session token used to authenticate re-joins.
    SessionSecret
);

string_id!(
    /// Identifier for a piece type defined in config (e.g., "pawn").
    PieceTypeId
);
string_id!(
    /// Identifier for a shop config entry.
    ShopId
);
string_id!(
    /// Identifier for a starting kit configuration.
    KitId
);
string_id!(
    /// Identifier for a game mode configuration.
    ModeId
);

impl PieceTypeId {
    /// Returns true when the piece type is a king variant.
    pub fn is_king(&self) -> bool {
        self.0 == "king" || self.0.ends_with("_king")
    }
}

/// Client-assigned identifier of one queued piece action, unique per connection.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ActionId(pub u64);

impl ActionId {
    /// Returns the identifier following this one.
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}
