//! Which way up the client keeps the board.
//!
//! The server always describes the board the right way up. A player seated on another side
//! of it sees it turned by a number of quarter turns, so the client state turns everything on
//! the board about the board centre as it arrives, and turns moves back as they leave for the
//! server. The renderer, the camera, and input handling only ever see the board as shown.

use crate::math::Vec2;
use common::models::{
    CameraCue, GameModeClientConfig, GameState, PartnerAction, Piece, PieceAction, PieceConfig,
    Player, Shop, SpecialMove, StartingCameraPosition,
};
use common::rules::Catalog;
use common::types::{BoardCoord, BoardSize};
use glam::IVec2;
use std::rc::Rc;

/// Number of quarter turns making a whole turn.
const QUARTERS_PER_TURN: u8 = 4;

/// How far the board is shown turned clockwise from the way the server describes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Orientation {
    /// Clockwise quarter turns, below a whole turn.
    quarters: u8,
}

impl Orientation {
    /// Returns the orientation `quarters` clockwise quarter turns from upright.
    pub fn clockwise_quarters(quarters: u8) -> Self {
        Self {
            quarters: quarters % QUARTERS_PER_TURN,
        }
    }

    /// Returns the orientation `player` views the board in.
    pub fn of_player(player: &Player) -> Self {
        Self::clockwise_quarters(player.board_quarter_turns())
    }

    /// Returns the orientation half a turn from this one, as seen from across the board.
    pub fn flipped(self) -> Self {
        Self::clockwise_quarters(self.quarters + QUARTERS_PER_TURN / 2)
    }

    /// Returns the orientation turning the board back from this one to upright.
    fn inverse(self) -> Self {
        Self::clockwise_quarters(QUARTERS_PER_TURN - self.quarters)
    }

    /// Returns the orientation turning the board from this one to `to`.
    pub fn towards(self, to: Self) -> Self {
        Self::clockwise_quarters(to.quarters + self.inverse().quarters)
    }

    /// Returns `value` of a board of `size`, given the right way up, as shown in this
    /// orientation; turns are relative, so a value already turned gets turned further.
    pub fn show<T: Turnable>(self, value: T, size: BoardSize) -> T {
        match self.quarters {
            0 => value,
            quarters => value.turned(Turn::of(quarters, size)),
        }
    }

    /// Returns `value` of a board of `size`, given as shown in this orientation, the right
    /// way up.
    pub fn upright<T: Turnable>(self, value: T, size: BoardSize) -> T {
        self.inverse().show(value, size)
    }
}

/// A clockwise turn of a board about its centre by whole quarter turns, which takes the
/// board onto itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    /// Clockwise quarter turns.
    quarters: u8,
    /// Twice the coordinate of the board centre on either axis, so it stays whole.
    doubled_center: i32,
}

impl Turn {
    /// Returns the turn by `quarters` clockwise quarter turns of a board of `size`.
    fn of(quarters: u8, size: BoardSize) -> Self {
        // The board spans `-half..limit_pos` on both axes.
        Self {
            quarters: quarters % QUARTERS_PER_TURN,
            doubled_center: size.limit_pos() - size.half(),
        }
    }

    /// Returns `offset` turned, such as a step of a move; offsets turn about no point.
    pub fn offset(self, offset: IVec2) -> IVec2 {
        // With the y axis pointing down the screen, a clockwise quarter turn takes right to
        // down and down to left.
        (0..self.quarters).fold(offset, |offset, _| IVec2::new(-offset.y, offset.x))
    }

    /// Returns the square `square` turns onto.
    pub fn square(self, square: BoardCoord) -> BoardCoord {
        // Square `s` spans `s..s + 1`, so its centre, doubled, is `2s + 1`.
        let center = IVec2::splat(self.doubled_center);
        let doubled = center + self.offset(square.0 * 2 + IVec2::ONE - center);
        BoardCoord((doubled - IVec2::ONE) / 2)
    }

    /// Returns the point, in squares from the board origin, `point` turns onto.
    pub fn point(self, point: Vec2) -> Vec2 {
        let center = Vec2::splat(f64::from(self.doubled_center) / 2.0);
        let relative =
            (0..self.quarters).fold(point - center, |offset, _| Vec2::new(-offset.y, offset.x));
        center + relative
    }
}

/// Something on, or about, the board that can be turned.
pub trait Turnable: Sized {
    /// Returns the value turned by `turn`.
    fn turned(self, turn: Turn) -> Self;
}

impl Turnable for BoardCoord {
    fn turned(self, turn: Turn) -> Self {
        turn.square(self)
    }
}

impl<T: Turnable> Turnable for Vec<T> {
    fn turned(self, turn: Turn) -> Self {
        self.into_iter().map(|value| value.turned(turn)).collect()
    }
}

impl<T: Turnable> Turnable for Option<T> {
    fn turned(self, turn: Turn) -> Self {
        self.map(|value| value.turned(turn))
    }
}

impl<T: Turnable + Clone> Turnable for Rc<T> {
    fn turned(self, turn: Turn) -> Self {
        Rc::new(Rc::unwrap_or_clone(self).turned(turn))
    }
}

impl Turnable for Piece {
    fn turned(self, turn: Turn) -> Self {
        Self {
            position: self.position.turned(turn),
            ..self
        }
    }
}

impl Turnable for Shop {
    fn turned(self, turn: Turn) -> Self {
        Self {
            position: self.position.turned(turn),
            ..self
        }
    }
}

impl Turnable for GameState {
    fn turned(self, turn: Turn) -> Self {
        Self {
            pieces: self
                .pieces
                .into_iter()
                .map(|(id, piece)| (id, piece.turned(turn)))
                .collect(),
            shops: self.shops.turned(turn),
            ..self
        }
    }
}

impl Turnable for PieceAction {
    fn turned(self, turn: Turn) -> Self {
        match self {
            Self::Move { target } => Self::Move {
                target: target.turned(turn),
            },
            Self::Guard { target } => Self::Guard {
                target: target.turned(turn),
            },
            Self::Buy { item } => Self::Buy { item },
        }
    }
}

impl Turnable for CameraCue {
    fn turned(self, turn: Turn) -> Self {
        Self {
            death_square: self.death_square.turned(turn),
            ..self
        }
    }
}

impl Turnable for GameModeClientConfig {
    fn turned(mut self, turn: Turn) -> Self {
        if let StartingCameraPosition::Point(point) = &mut self.starting_camera.position {
            let turned = turn.point(Vec2::from(*point));
            *point = [turned.x, turned.y];
        }
        self
    }
}

/// Moves and captures are offsets from the mover, which turn along with the board.
impl Turnable for Catalog {
    fn turned(self, turn: Turn) -> Self {
        Self {
            pieces: self
                .pieces
                .into_iter()
                .map(|(id, piece)| (id, piece.turned(turn)))
                .collect(),
            ..self
        }
    }
}

impl Turnable for PieceConfig {
    fn turned(self, turn: Turn) -> Self {
        let turn_paths = |paths: Vec<Vec<IVec2>>| -> Vec<Vec<IVec2>> {
            paths
                .into_iter()
                .map(|path| path.into_iter().map(|offset| turn.offset(offset)).collect())
                .collect()
        };
        Self {
            move_paths: turn_paths(self.move_paths),
            capture_paths: turn_paths(self.capture_paths),
            special_moves: self.special_moves.turned(turn),
            ..self
        }
    }
}

impl Turnable for SpecialMove {
    fn turned(self, turn: Turn) -> Self {
        match self {
            Self::PartneredMove(mut special) => {
                special.target = turn.offset(special.target);
                for requirement in &mut special.requirements {
                    requirement.at = turn.offset(requirement.at);
                }
                for partner in &mut special.partners {
                    partner.at = turn.offset(partner.at);
                    if let Some(PartnerAction::MoveTo(to)) = &mut partner.action {
                        *to = turn.offset(*to);
                    }
                }
                Self::PartneredMove(special)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_take_even_and_odd_boards_onto_themselves() {
        for size in [8, 9, 40, 41] {
            let size = BoardSize::new(size);
            let (low, high) = (-size.half(), size.limit_pos() - 1);
            let quarter = Turn::of(1, size);
            // Clockwise: the top-left corner goes to the top-right.
            assert_eq!(
                quarter.square(BoardCoord::new(low, low)),
                BoardCoord::new(high, low)
            );
            assert_eq!(
                quarter.square(BoardCoord::new(high, low)),
                BoardCoord::new(high, high)
            );
            assert_eq!(
                Turn::of(2, size).square(BoardCoord::new(low, high)),
                BoardCoord::new(high, low)
            );
            let center = crate::camera::board_center(size);
            assert_eq!(quarter.point(center), center);
            let square = BoardCoord::new(3, -2);
            for quarters in 0..QUARTERS_PER_TURN {
                let orientation = Orientation::clockwise_quarters(quarters);
                let shown = orientation.show(square, size);
                assert_eq!(orientation.upright(shown, size), square);
                let corner = Vec2::new(f64::from(square.0.x), f64::from(square.0.y));
                let shown_center = Turn::of(quarters, size).point(corner + 0.5);
                assert_eq!(shown_center - 0.5, shown.0.as_dvec2());
            }
        }
    }

    #[test]
    fn orientations_compose() {
        let size = BoardSize::new(9);
        let square = BoardCoord::new(1, 3);
        for from in 0..QUARTERS_PER_TURN {
            for to in 0..QUARTERS_PER_TURN {
                let (from, to) = (
                    Orientation::clockwise_quarters(from),
                    Orientation::clockwise_quarters(to),
                );
                let reoriented = from.towards(to).show(from.show(square, size), size);
                assert_eq!(reoriented, to.show(square, size));
            }
        }
    }
}
