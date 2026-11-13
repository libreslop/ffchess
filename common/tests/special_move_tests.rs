//! Tests for special moves configured as `partneredmove`.

use common::models::{GameState, Piece, PieceConfig, Player};
use common::protocol::GameError;
use common::rules::{Catalog, LegalMove, MoveKind, PieceMove, Simulation};
use common::types::{
    BoardCoord, BoardSize, ColorHex, DurationMs, MoveCount, PieceId, PieceTypeId, PlayerId, Score,
    TimestampMs,
};
use serde_json::json;

/// Builds a piece config from JSON, filling in the fields special moves do not care about.
fn piece_config(id: &str, mut fields: serde_json::Value) -> PieceConfig {
    let defaults = json!({
        "id": id,
        "display_name": id,
        "svg_path": format!("{id}.svg"),
        "score_value": 10,
        "cooldown_ms": 1000,
        "move_paths": [],
        "capture_paths": [],
    });
    for (key, value) in defaults.as_object().unwrap() {
        fields
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert(value.clone());
    }
    serde_json::from_value(fields).expect("piece config deserializes")
}

/// Builds a king that steps one square sideways and castles both ways while unmoved.
///
/// The king may not castle out of, through, or into an attacked square.
fn castling_king() -> PieceConfig {
    let castle = |dir: i32, rook_at: i32| {
        // The king crosses two squares that must be safe; any square beyond them up to the
        // rook only has to be empty.
        let requirements: Vec<_> = (1..rook_at.abs())
            .map(|step| {
                let condition = if step <= 2 {
                    "square_empty && !square_targeted"
                } else {
                    "square_empty"
                };
                json!({ "at": [step * dir, 0], "condition": condition })
            })
            .collect();
        json!({
            "type": "partneredmove",
            "target": [2 * dir, 0],
            "condition": "self_moves == 0 && !self_targeted",
            "requirements": requirements,
            "partners": [{
                "at": [rook_at, 0],
                "owner": "self",
                "types": ["rook"],
                "condition": "partner_moves == 0",
                "action": { "move_to": [dir, 0] }
            }]
        })
    };
    piece_config(
        "king",
        json!({
            "move_paths": [[[1, 0]], [[-1, 0]]],
            "capture_paths": [[[1, 0]], [[-1, 0]]],
            "special_moves": [
                castle(1, 3),
                castle(-1, -4),
            ]
        }),
    )
}

/// Builds an enemy piece that captures up to three squares towards row 0.
fn lance() -> PieceConfig {
    piece_config(
        "lance",
        json!({ "capture_paths": [[[0, -1], [0, -2], [0, -3]]] }),
    )
}

/// Builds a rook config with a cooldown distinct from the king's.
fn rook() -> PieceConfig {
    piece_config("rook", json!({ "cooldown_ms": 3000 }))
}

/// Builds a catalog from piece configs.
fn catalog(pieces: Vec<PieceConfig>) -> Catalog {
    Catalog {
        pieces: pieces.into_iter().map(|p| (p.id.clone(), p)).collect(),
        shops: Default::default(),
    }
}

/// Builds an empty 16x16 board with one player.
fn board_with_player() -> (GameState, PlayerId) {
    let mut state = GameState {
        board_size: BoardSize::from(16),
        ..GameState::default()
    };
    let id = PlayerId::new();
    state.players.insert(
        id,
        Player {
            id,
            name: "P".to_string(),
            score: Score::from(0),
            kills: 0,
            pieces_captured: 0,
            join_time: TimestampMs::from_millis(0),
            king_id: PieceId::nil(),
            color: ColorHex::from("#000000"),
            board_rotation_deg: 0,
        },
    );
    (state, id)
}

/// Adds a piece to the state and returns its id.
fn add_piece(state: &mut GameState, owner: Option<PlayerId>, kind: &str, pos: [i32; 2]) -> PieceId {
    let piece = Piece::new(owner, PieceTypeId::from(kind), coord(pos));
    let id = piece.id;
    state.pieces.insert(id, piece);
    id
}

/// Shorthand for a board coordinate.
fn coord([x, y]: [i32; 2]) -> BoardCoord {
    BoardCoord::new(x, y)
}

/// Board with a player king on (4, 0) and rooks on both corners.
struct CastlingBoard {
    state: GameState,
    king: PieceId,
    short_rook: PieceId,
    long_rook: PieceId,
}

impl CastlingBoard {
    fn new() -> Self {
        let (mut state, me) = board_with_player();
        let king = add_piece(&mut state, Some(me), "king", [4, 0]);
        let short_rook = add_piece(&mut state, Some(me), "rook", [7, 0]);
        let long_rook = add_piece(&mut state, Some(me), "rook", [0, 0]);
        Self {
            state,
            king,
            short_rook,
            long_rook,
        }
    }

    /// Plays a king move to `target` at time 5.
    fn move_king(&mut self, target: [i32; 2]) -> Result<common::rules::ActionEffects, GameError> {
        let catalog = catalog(vec![castling_king(), rook(), lance()]);
        Simulation::new(&catalog, &mut self.state, TimestampMs::from_millis(5))
            .apply_move(self.king, coord(target))
    }

    fn position(&self, piece: PieceId) -> BoardCoord {
        self.state.pieces[&piece].position
    }
}

#[test]
/// Verifies castling moves the king and its rook together and counts both moves.
fn castling_moves_king_and_rook() {
    let mut board = CastlingBoard::new();
    let effects = board.move_king([6, 0]).expect("short castle is legal");

    assert_eq!(board.position(board.king), coord([6, 0]));
    assert_eq!(board.position(board.short_rook), coord([5, 0]));
    assert_eq!(board.position(board.long_rook), coord([0, 0]));
    let rook = &board.state.pieces[&board.short_rook];
    assert_eq!(rook.moves_made, MoveCount(1));
    // Castling is the king's move: the rook keeps its cooldown untouched.
    assert_eq!(rook.last_move_time, TimestampMs::from_millis(0));
    assert_eq!(rook.cooldown_ms, DurationMs::zero());
    assert_eq!(board.state.pieces[&board.king].moves_made, MoveCount(1));
    assert_eq!(effects.partners, vec![board.short_rook]);
    assert!(effects.moved.contains(&PieceMove {
        piece_id: board.short_rook,
        from: coord([7, 0]),
        to: coord([5, 0]),
    }));
    assert_eq!(effects.moved.len(), 2);
}

#[test]
/// Verifies the long castle uses its own empty squares and partner.
fn long_castling_moves_the_other_rook() {
    let mut board = CastlingBoard::new();
    board.move_king([2, 0]).expect("long castle is legal");
    assert_eq!(board.position(board.long_rook), coord([3, 0]));
    assert_eq!(board.position(board.short_rook), coord([7, 0]));
}

#[test]
/// Verifies castling needs every listed square empty.
fn castling_through_a_piece_is_rejected() {
    let mut board = CastlingBoard::new();
    add_piece(&mut board.state, None, "rook", [5, 0]);
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
    assert_eq!(board.position(board.king), coord([4, 0]));
}

#[test]
/// Verifies square requirements see squares attacked by enemies.
fn castling_through_an_attacked_square_is_rejected() {
    let mut board = CastlingBoard::new();
    add_piece(&mut board.state, Some(PlayerId::new()), "lance", [5, 3]);
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
    board
        .move_king([2, 0])
        .expect("the long side is still safe");
}

#[test]
/// Verifies an attack only counts when its path is clear.
fn blocked_attacks_do_not_target_squares() {
    let mut board = CastlingBoard::new();
    add_piece(&mut board.state, Some(PlayerId::new()), "lance", [5, 3]);
    add_piece(&mut board.state, None, "rook", [5, 2]);
    board.move_king([6, 0]).expect("the attack is blocked");
}

#[test]
/// Verifies squares only the rook crosses may be attacked.
fn attacked_squares_beyond_the_king_path_are_allowed() {
    let mut board = CastlingBoard::new();
    add_piece(&mut board.state, Some(PlayerId::new()), "lance", [1, 3]);
    board
        .move_king([2, 0])
        .expect("only the rook crosses the attacked square");
}

#[test]
/// Verifies the mover's condition sees whether the mover is attacked.
fn castling_out_of_an_attack_is_rejected() {
    let mut board = CastlingBoard::new();
    add_piece(&mut board.state, Some(PlayerId::new()), "lance", [4, 3]);
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
    assert_eq!(board.move_king([2, 0]), Err(GameError::InvalidMove));
}

#[test]
/// Verifies own pieces never target squares and capture paths, not move paths, count.
fn only_enemy_captures_target_squares() {
    let (mut state, me) = board_with_player();
    let walker = piece_config("walker", json!({ "move_paths": [[[0, -1]]] }));
    let catalog = catalog(vec![lance(), walker]);
    add_piece(&mut state, Some(me), "lance", [1, 3]);
    add_piece(&mut state, Some(PlayerId::new()), "walker", [2, 1]);
    add_piece(&mut state, Some(PlayerId::new()), "lance", [3, 3]);

    assert!(!catalog.is_targeted(&state, coord([1, 0]), Some(me)));
    assert!(catalog.is_targeted(&state, coord([1, 0]), Some(PlayerId::new())));
    assert!(!catalog.is_targeted(&state, coord([2, 0]), Some(me)));
    assert!(catalog.is_targeted(&state, coord([3, 0]), Some(me)));
}

#[test]
/// Verifies partner conditions see whether the partner is attacked.
fn partner_conditions_see_attacks_on_the_partner() {
    let guard = piece_config(
        "guard",
        json!({
            "special_moves": [{
                "type": "partneredmove",
                "target": [0, 1],
                "partners": [{
                    "at": [1, 0],
                    "owner": "self",
                    "condition": "partner_targeted",
                    "action": { "move_to": [1, 1] }
                }]
            }]
        }),
    );
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "guard", [4, 4]);
    let ally = add_piece(&mut state, Some(me), "guard", [5, 4]);
    let catalog = catalog(vec![guard, lance()]);
    let target = coord([4, 5]);

    assert!(
        catalog
            .check_move(&state, &state.pieces[&mover], target)
            .is_err()
    );
    add_piece(&mut state, Some(PlayerId::new()), "lance", [5, 6]);
    assert_eq!(
        catalog.check_move(&state, &state.pieces[&mover], target),
        Ok(MoveKind::Quiet)
    );
    Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply_move(mover, target)
        .expect("the attacked ally is pulled away");
    assert_eq!(state.pieces[&ally].position, coord([5, 5]));
}

#[test]
/// Verifies the mover's condition sees its own move count.
fn castling_after_the_king_moved_is_rejected() {
    let mut board = CastlingBoard::new();
    board.move_king([5, 0]).expect("king steps right");
    board.move_king([4, 0]).expect("king steps back");
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
}

#[test]
/// Verifies partner conditions see the partner's move count.
fn castling_with_a_moved_rook_is_rejected() {
    let mut board = CastlingBoard::new();
    board
        .state
        .pieces
        .get_mut(&board.short_rook)
        .unwrap()
        .moves_made = MoveCount(1);
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
}

#[test]
/// Verifies partners must have the configured owner.
fn castling_with_an_enemy_rook_is_rejected() {
    let mut board = CastlingBoard::new();
    board
        .state
        .pieces
        .get_mut(&board.short_rook)
        .unwrap()
        .owner_id = Some(PlayerId::new());
    assert_eq!(board.move_king([6, 0]), Err(GameError::InvalidMove));
}

#[test]
/// Verifies castling is listed with the king's landing square and offered on its rook.
fn legal_moves_include_castling() {
    let board = CastlingBoard::new();
    let king = &board.state.pieces[&board.king];
    let moves = catalog(vec![castling_king(), rook()]).legal_moves(&board.state, king);
    assert!(moves.contains(&LegalMove {
        target: coord([6, 0]),
        kind: MoveKind::Quiet,
        affected: vec![coord([7, 0])],
    }));
    let long = moves
        .iter()
        .find(|legal| legal.target == coord([2, 0]))
        .expect("long castle is listed");
    assert_eq!(long.handles(), [coord([0, 0])]);
    assert_eq!(moves.len(), 4);
}

#[test]
/// Verifies a captured partner is removed and credited to the mover's owner.
fn partner_capture_credits_the_mover() {
    let pawn = piece_config(
        "pawn",
        json!({
            "special_moves": [{
                "type": "partneredmove",
                "target": [1, 1],
                "partners": [{ "at": [1, 0], "owner": "enemy", "action": "capture" }]
            }]
        }),
    );
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "pawn", [2, 2]);
    let victim = add_piece(&mut state, None, "pawn", [3, 2]);
    let catalog = catalog(vec![pawn]);

    let effects = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply_move(mover, coord([3, 3]))
        .expect("en passant is legal");

    assert_eq!(effects.captured.len(), 1);
    assert_eq!(effects.captured[0].id, victim);
    assert!(!state.pieces.contains_key(&victim));
    assert_eq!(state.pieces[&mover].position, coord([3, 3]));
    assert_eq!(state.players[&me].score, Score::from(10));
}

/// Builds a piece that swaps places with a friendly piece on its right, which then becomes
/// `promoted`.
fn swapper() -> PieceConfig {
    piece_config(
        "swapper",
        json!({
            "special_moves": [{
                "type": "partneredmove",
                "target": [1, 0],
                "becomes": "promoted",
                "partners": [
                    { "at": [1, 0], "owner": "self", "action": { "move_to": [0, 0] } },
                    { "at": [0, 1], "owner": "any", "action": { "becomes": "promoted" } }
                ]
            }]
        }),
    )
}

#[test]
/// Verifies pieces may land on squares other pieces of the same move leave.
fn swapping_places_with_a_partner_is_legal() {
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "swapper", [2, 2]);
    let ally = add_piece(&mut state, Some(me), "pawn", [3, 2]);
    let below = add_piece(&mut state, None, "pawn", [2, 3]);
    let catalog = catalog(vec![
        swapper(),
        piece_config("pawn", json!({})),
        piece_config("promoted", json!({ "cooldown_ms": 4000 })),
    ]);

    Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply_move(mover, coord([3, 2]))
        .expect("swap is legal");

    assert_eq!(state.pieces[&mover].position, coord([3, 2]));
    assert_eq!(state.pieces[&ally].position, coord([2, 2]));
    assert_eq!(
        state.pieces[&mover].piece_type,
        PieceTypeId::from("promoted")
    );
    assert_eq!(
        state.pieces[&below].piece_type,
        PieceTypeId::from("promoted")
    );
    assert_eq!(
        state.pieces[&below].cooldown_ms,
        DurationMs::from_millis(4000)
    );
    assert_eq!(state.pieces[&below].position, coord([2, 3]));
}

#[test]
/// Verifies a special move is rejected when a required partner is missing.
fn missing_partner_rejects_the_move() {
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "swapper", [2, 2]);
    add_piece(&mut state, Some(me), "pawn", [3, 2]);
    let catalog = catalog(vec![swapper()]);
    assert_eq!(
        Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
            .apply_move(mover, coord([3, 2])),
        Err(GameError::TargetFriendly)
    );
}

#[test]
/// Verifies two pieces may not end on the same square.
fn colliding_destinations_reject_the_move() {
    let jumper = piece_config(
        "jumper",
        json!({
            "special_moves": [{
                "type": "partneredmove",
                "target": [0, 2],
                "partners": [{ "at": [0, 1], "owner": "any", "action": { "move_to": [0, 2] } }]
            }]
        }),
    );
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "jumper", [2, 2]);
    add_piece(&mut state, None, "pawn", [2, 3]);
    assert!(
        catalog(vec![jumper])
            .legal_moves(&state, &state.pieces[&mover])
            .is_empty()
    );
}

#[test]
/// Verifies path moves win over special moves sharing a target.
fn path_moves_take_precedence() {
    let mut king = castling_king();
    king.move_paths.push(vec![glam::IVec2::new(2, 0)]);
    let (mut state, me) = board_with_player();
    let mover = add_piece(&mut state, Some(me), "king", [4, 0]);
    let rook_id = add_piece(&mut state, Some(me), "rook", [7, 0]);
    let catalog = catalog(vec![king, rook()]);

    let effects = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply_move(mover, coord([6, 0]))
        .expect("path move is legal");

    assert!(effects.partners.is_empty());
    assert_eq!(state.pieces[&rook_id].position, coord([7, 0]));
}

#[test]
/// Verifies catalog validation reports unknown piece types and malformed conditions.
fn validation_reports_config_mistakes() {
    assert_eq!(catalog(vec![castling_king(), rook()]).validate(), Ok(()));

    let unknown_type = catalog(vec![castling_king()]).validate();
    assert!(
        unknown_type
            .unwrap_err()
            .contains("unknown piece type `rook`")
    );

    let mut bad_condition = castling_king();
    bad_condition.special_moves = vec![
        serde_json::from_value(json!({
            "type": "partneredmove",
            "target": [2, 0],
            "condition": "self_moves ==",
        }))
        .unwrap(),
    ];
    let error = catalog(vec![bad_condition, rook()]).validate().unwrap_err();
    assert!(error.contains("invalid condition"), "{error}");
}
