//! Cleanup of user-entered text that other players see.

use std::ops::RangeInclusive;

/// Characters that render as nothing, or as blank space that is not whitespace.
///
/// Covers soft hyphens, zero-width spaces and joiners, direction marks, embeddings, overrides
/// and isolates, invisible operators, blank fillers (Hangul and Braille), variation selectors,
/// and tag characters. Names made of these could look identical to another player's name.
const INVISIBLE: &[RangeInclusive<char>] = &[
    '\u{00AD}'..='\u{00AD}',
    '\u{034F}'..='\u{034F}',
    '\u{061C}'..='\u{061C}',
    '\u{115F}'..='\u{1160}',
    '\u{17B4}'..='\u{17B5}',
    '\u{180B}'..='\u{180F}',
    '\u{200B}'..='\u{200F}',
    '\u{202A}'..='\u{202E}',
    '\u{2060}'..='\u{206F}',
    '\u{2800}'..='\u{2800}',
    '\u{3164}'..='\u{3164}',
    '\u{FE00}'..='\u{FE0F}',
    '\u{FEFF}'..='\u{FEFF}',
    '\u{FFA0}'..='\u{FFA0}',
    '\u{FFF0}'..='\u{FFFB}',
    '\u{1BCA0}'..='\u{1BCA3}',
    '\u{1D173}'..='\u{1D17A}',
    '\u{E0000}'..='\u{E0FFF}',
];

/// Bidirectional controls: direction marks, embeddings, overrides, and isolates.
///
/// They can flip the order of the text around them, such as the name and the message of a
/// chat line.
const BIDI_CONTROLS: &[RangeInclusive<char>] = &[
    '\u{061C}'..='\u{061C}',
    '\u{200E}'..='\u{200F}',
    '\u{202A}'..='\u{202E}',
    '\u{2066}'..='\u{2069}',
];

/// Classification of characters that should not reach other players.
trait CharClass {
    /// Returns true when the character is in one of `ranges`.
    fn within(self, ranges: &[RangeInclusive<char>]) -> bool;
}

impl CharClass for char {
    fn within(self, ranges: &[RangeInclusive<char>]) -> bool {
        ranges.iter().any(|range| range.contains(&self))
    }
}

/// Cleanup applied to text before it is shown to other players.
pub trait UserText {
    /// Cleans a player or spectator name.
    ///
    /// Control and invisible characters are removed, and each run of whitespace becomes one
    /// space, so a name cannot pass for another name or for a different layout. The result is
    /// trimmed and may be empty.
    fn clean_name(&self) -> String;

    /// Cleans a chat message.
    ///
    /// Control characters and bidirectional controls are removed so the message cannot reorder
    /// its chat line. Joiners and variation selectors stay, because emoji sequences use them.
    /// The result is trimmed and may be empty.
    fn clean_message(&self) -> String;
}

impl UserText for str {
    fn clean_name(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                word.chars()
                    .filter(|c| !c.is_control() && !c.within(INVISIBLE))
                    .collect::<String>()
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn clean_message(&self) -> String {
        self.chars()
            .filter(|c| !c.is_control() && !c.within(BIDI_CONTROLS))
            .collect::<String>()
            .trim()
            .to_string()
    }
}
