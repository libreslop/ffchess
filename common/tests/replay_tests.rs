//! Tests for replay frames and keyframes.

use common::models::{
    GameModeClientConfig, GameState, Piece, Replay, ReplayFrame, ReplayHeader, Shop,
};
use common::rules::Catalog;
use common::types::{BoardCoord, BoardSize, PieceTypeId, ShopId, TimestampMs};

/// Returns a board with two pieces and a shop.
fn board() -> GameState {
    let mut state = GameState {
        board_size: BoardSize::from(8),
        ..GameState::default()
    };
    for x in 0..2 {
        let piece = Piece::new(None, PieceTypeId::from("pawn"), BoardCoord::new(x, 0));
        state.pieces.insert(piece.id, piece);
    }
    state.shops.push(Shop {
        position: BoardCoord::new(3, 3),
        uses_remaining: 1,
        shop_id: ShopId::from("spawn_shop"),
    });
    state
}

#[test]
/// Verifies an unchanged board produces no frame.
fn unchanged_board_has_no_frame() {
    let state = board();
    assert_eq!(ReplayFrame::between(&state, &state, TimestampMs(1)), None);
}

#[test]
/// Verifies a frame only carries what changed, and applying it reproduces the new board.
fn frame_carries_only_changes() {
    let before = board();
    let mut after = before.clone();
    let mut ids = after.pieces.keys().copied();
    let (moved, removed) = (ids.next().unwrap(), ids.next().unwrap());
    after.pieces.get_mut(&moved).unwrap().position = BoardCoord::new(0, 1);
    after.pieces.remove(&removed);

    let frame = ReplayFrame::between(&before, &after, TimestampMs(5)).expect("board changed");
    assert_eq!(frame.pieces.len(), 1);
    assert_eq!(frame.removed, vec![removed]);
    assert_eq!(frame.shops, None);
    assert_eq!(frame.board_size, None);

    let mut replayed = before.clone();
    frame.apply(&mut replayed);
    assert_eq!(replayed, after);
}

#[test]
/// Verifies keyframes rebuild every recorded board from an empty one.
fn keyframes_rebuild_each_board() {
    let empty = GameState::default();
    let first = board();
    let mut second = first.clone();
    second.board_size = BoardSize::from(6);
    second.shops.clear();
    let frames = vec![
        ReplayFrame::between(&empty, &first, TimestampMs(10)).unwrap(),
        ReplayFrame::between(&first, &second, TimestampMs(20)).unwrap(),
    ];
    let replay = Replay {
        header: ReplayHeader {
            mode: GameModeClientConfig::default(),
            catalog: Catalog::default(),
        },
        started_at: TimestampMs(10),
        ended_at: TimestampMs(30),
        frames,
    };

    let keyframes = replay.keyframes();
    assert_eq!(keyframes.len(), 2);
    assert_eq!(keyframes[0].at, TimestampMs(10));
    assert_eq!(keyframes[0].state.pieces, first.pieces);
    assert_eq!(keyframes[1].state.board_size, BoardSize::from(6));
    assert!(keyframes[1].state.shops.is_empty());
}
