//! Tests for destination requirements, capture threats, and piece shading.

use common::models::{DestinationRequirement, GameState, MoveTiming, Piece, PieceConfig, Player};
use common::protocol::GameError;
use common::rules::{Catalog, PaintColor, Shade, ShadeMark};
use common::types::{
    BoardCoord, BoardSize, ColorHex, OpacityPercent, PieceId, PieceTypeId, PlayerId, Score,
    TimestampMs,
};
use serde_json::json;

/// Builds a piece config from JSON, filling in the fields these tests do not care about.
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

/// Returns one single-step path per neighbouring square.
fn king_steps() -> serde_json::Value {
    json!([
        [[0, 1]],
        [[0, -1]],
        [[1, 0]],
        [[-1, 0]],
        [[1, 1]],
        [[1, -1]],
        [[-1, 1]],
        [[-1, -1]]
    ])
}

/// Builds a king that may not move into check, filled and marked towards its attackers for
/// its owner.
fn king() -> PieceConfig {
    piece_config(
        "king",
        json!({
            "move_paths": king_steps(),
            "capture_paths": king_steps(),
            "destination_requirements": [
                { "condition": "!square_targeted", "ignore_for_premove": true }
            ],
            "shading": [
                {
                    "condition": "self_targeted",
                    "visible_to": "viewer_is_owner",
                    "effects": [
                        { "type": "background", "color": { "most_common": "attackers" } },
                        { "type": "line_towards", "towards": "attackers", "color": "other" }
                    ]
                }
            ]
        }),
    )
}

/// Builds a rook sliding up to five squares along each axis, filled for players it checks.
fn rook() -> PieceConfig {
    let ray = |dx: i32, dy: i32| (1..=5).map(|i| [dx * i, dy * i]).collect::<Vec<_>>();
    let paths = json!([ray(0, 1), ray(0, -1), ray(1, 0), ray(-1, 0)]);
    piece_config(
        "rook",
        json!({
            "move_paths": paths,
            "capture_paths": paths,
            "shading": [
                {
                    "visible_to": "viewer_king_targeted",
                    "effects": [{ "type": "background", "color": "owner" }]
                }
            ]
        }),
    )
}

/// Builds a catalog of the test king and rook.
fn catalog() -> Catalog {
    Catalog {
        pieces: [king(), rook()]
            .into_iter()
            .map(|config| (config.id.clone(), config))
            .collect(),
        shops: Default::default(),
    }
}

/// Builds an empty 20x20 board.
fn empty_state() -> GameState {
    GameState {
        board_size: BoardSize::from(20),
        ..GameState::default()
    }
}

/// Adds a player to the state.
fn add_player(state: &mut GameState) -> PlayerId {
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
    id
}

/// Adds a piece to the state and returns its id.
fn add_piece(state: &mut GameState, owner: Option<PlayerId>, kind: &str, pos: [i32; 2]) -> PieceId {
    let piece = Piece::new(
        owner,
        PieceTypeId::from(kind),
        BoardCoord::new(pos[0], pos[1]),
    );
    let id = piece.id;
    state.pieces.insert(id, piece);
    id
}

/// Checks a move of the piece `mover` to `end`, made at once.
fn check(state: &GameState, mover: PieceId, end: [i32; 2]) -> Result<(), GameError> {
    check_with(catalog(), state, mover, end, MoveTiming::Now)
}

/// Checks a move of the piece `mover` to `end` with `catalog`, made with `timing`.
fn check_with(
    catalog: Catalog,
    state: &GameState,
    mover: PieceId,
    end: [i32; 2],
    timing: MoveTiming,
) -> Result<(), GameError> {
    catalog
        .check_move(
            state,
            &state.pieces[&mover],
            BoardCoord::new(end[0], end[1]),
            timing,
        )
        .map(drop)
}

#[test]
/// Premoves skip destination requirements marked `ignore_for_premove` and keep the others.
fn premoves_skip_ignored_destination_requirements() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(enemy), "rook", [1, 4]);

    assert_eq!(
        check_with(catalog(), &state, king, [1, 0], MoveTiming::Premove),
        Ok(())
    );

    let mut strict = catalog();
    for config in strict.pieces.values_mut() {
        for requirement in &mut config.destination_requirements {
            requirement.ignore_for_premove = false;
        }
    }
    assert_eq!(
        check_with(strict, &state, king, [1, 0], MoveTiming::Premove),
        Err(GameError::DestinationRestricted)
    );
}

#[test]
/// A king may not step onto a square an enemy could capture onto.
fn king_cannot_move_into_check() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(enemy), "rook", [1, 4]);

    assert_eq!(
        check(&state, king, [1, 0]),
        Err(GameError::DestinationRestricted)
    );
    assert_eq!(check(&state, king, [-1, 0]), Ok(()));
}

#[test]
/// A king stepping back along an attacking ray is still in check, although it blocks that ray
/// before the move.
fn king_cannot_retreat_along_attacking_ray() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(enemy), "rook", [0, 3]);

    assert_eq!(
        check(&state, king, [0, -1]),
        Err(GameError::DestinationRestricted)
    );
}

#[test]
/// A king may not capture a defended piece, but may capture an undefended one.
fn king_cannot_capture_defended_piece() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(enemy), "rook", [1, 0]);
    add_piece(&mut state, Some(enemy), "rook", [5, 0]);
    assert_eq!(
        check(&state, king, [1, 0]),
        Err(GameError::DestinationRestricted)
    );

    let mut lone = empty_state();
    let (me, enemy) = (add_player(&mut lone), add_player(&mut lone));
    let king = add_piece(&mut lone, Some(me), "king", [0, 0]);
    add_piece(&mut lone, Some(enemy), "rook", [1, 1]);
    assert_eq!(check(&lone, king, [1, 1]), Ok(()));
}

#[test]
/// Destination-restricted moves are not offered as legal moves.
fn legal_moves_skip_restricted_destinations() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(enemy), "rook", [1, 4]);

    let targets: Vec<BoardCoord> = catalog()
        .legal_moves(&state, &state.pieces[&king], MoveTiming::Now)
        .into_iter()
        .map(|legal| legal.target)
        .collect();
    assert_eq!(targets.len(), 5);
    assert!(targets.iter().all(|target| target.0.x != 1));
}

#[test]
/// Threats stop at the first piece on a path and never include own pieces.
fn threats_stop_at_first_piece() {
    let mut state = empty_state();
    let (me, enemy) = (add_player(&mut state), add_player(&mut state));
    let rook = add_piece(&mut state, Some(enemy), "rook", [0, 4]);
    let shield = add_piece(&mut state, Some(me), "rook", [0, 2]);
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    let ally = add_piece(&mut state, Some(enemy), "rook", [2, 4]);

    let threats = catalog().threats(&state);
    assert_eq!(threats.targets_of(rook), &[shield]);
    assert!(threats.attackers_of(king).is_empty());
    assert!(threats.attackers_of(ally).is_empty());
}

#[test]
/// A rook checking a king is filled for the checked player only, and the king is marked
/// towards the rook for its owner only.
fn check_shading_is_seen_by_the_checked_player() {
    let mut state = empty_state();
    let (me, enemy, bystander) = (
        add_player(&mut state),
        add_player(&mut state),
        add_player(&mut state),
    );
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    let rook = add_piece(&mut state, Some(enemy), "rook", [0, 4]);
    add_piece(&mut state, Some(bystander), "king", [9, 9]);
    let catalog = catalog();

    let seen = catalog.shading(&state, me);
    let expected = [
        Shade {
            piece_id: rook,
            mark: ShadeMark::Background {
                color: PaintColor::Team(Some(enemy)),
                opacity: OpacityPercent::translucent(),
            },
        },
        Shade {
            piece_id: king,
            mark: ShadeMark::Background {
                color: PaintColor::Team(Some(enemy)),
                opacity: OpacityPercent::translucent(),
            },
        },
        Shade {
            piece_id: king,
            mark: ShadeMark::Line {
                towards: rook,
                color: PaintColor::Team(Some(enemy)),
                opacity: OpacityPercent::opaque(),
            },
        },
    ];
    assert_eq!(seen.len(), expected.len());
    assert!(expected.iter().all(|shade| seen.contains(shade)));
    assert!(catalog.shading(&state, enemy).is_empty());
    assert!(catalog.shading(&state, bystander).is_empty());
}

#[test]
/// A checked king is filled in the color of the team with the most attackers.
fn checked_king_takes_most_common_attacker_color() {
    let mut state = empty_state();
    let (me, many, few) = (
        add_player(&mut state),
        add_player(&mut state),
        add_player(&mut state),
    );
    let king = add_piece(&mut state, Some(me), "king", [0, 0]);
    add_piece(&mut state, Some(many), "rook", [0, 4]);
    add_piece(&mut state, Some(many), "rook", [4, 0]);
    add_piece(&mut state, Some(few), "rook", [-4, 0]);

    let king_fill: Vec<PaintColor> = catalog()
        .shading(&state, me)
        .into_iter()
        .filter(|shade| shade.piece_id == king)
        .filter_map(|shade| match shade.mark {
            ShadeMark::Background { color, .. } => Some(color),
            ShadeMark::Line { .. } => None,
        })
        .collect();
    assert_eq!(king_fill, vec![PaintColor::Team(Some(many))]);
}

#[test]
/// Misspelled shading variables and destination variables fail validation.
fn invalid_conditions_fail_validation() {
    let mut broken_shading = rook();
    broken_shading.shading[0].visible_to = Some("viewer_kng_targeted".into());
    let mut broken_destination = king();
    broken_destination.destination_requirements = vec![DestinationRequirement {
        condition: "!square_attacked".into(),
        ignore_for_premove: false,
    }];

    for config in [broken_shading, broken_destination] {
        let catalog = Catalog {
            pieces: [(config.id.clone(), config)].into_iter().collect(),
            shops: Default::default(),
        };
        assert!(catalog.validate().is_err());
    }
    assert!(catalog().validate().is_ok());
}
