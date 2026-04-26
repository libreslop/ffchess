use crate::state::ServerState;
use common::protocol::GameError;
use rand::seq::SliceRandom;

/// Player display name validated by server-side constraints.
pub(super) struct ValidPlayerName(String);

impl ValidPlayerName {
    /// Validates and normalizes a name, generating a fallback when empty.
    pub(super) fn from_input(input: String, state: &ServerState) -> Result<Self, GameError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(Self(generate_name(state)));
        }
        if trimmed.len() > 32 {
            return Err(GameError::Custom {
                title: "Invalid Name".to_string(),
                message: "Name must be 32 characters or less".to_string(),
            });
        }
        Ok(Self(trimmed.to_string()))
    }

    pub(super) fn into_inner(self) -> String {
        self.0
    }
}

/// Generates a human-friendly fallback player name.
fn generate_name(state: &ServerState) -> String {
    let pool = state.name_pool();
    let mut rng = rand::thread_rng();
    let adjective = pool
        .adjectives
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| "Unnamed".to_string());
    let mut noun = pool
        .nouns
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| "Player".to_string());

    if noun == adjective
        && let Some(another) = pool.nouns.choose(&mut rng)
    {
        noun = another.clone();
    }

    format!("{adjective} {noun}")
}
