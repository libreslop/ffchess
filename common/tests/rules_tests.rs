//! Tests for shared movement, shop, and simulation rules.

use common::models::{
    GameState, Piece, PieceAction, PieceConfig, Player, Shop, ShopConfig, ShopGroupConfig,
    ShopItemConfig, ShopItemIndex,
};
use common::protocol::GameError;
use common::rules::{Catalog, LegalMove, MoveKind, Simulation};
use common::types::{
    BoardCoord, BoardSize, ColorHex, DurationMs, ExprString, ExprValue, PieceId, PieceTypeId,
    PlayerId, Score, ShopId, TimestampMs,
};
use glam::IVec2;

/// Builds a piece config with the given single-step move and capture offsets.
fn piece_config(id: &str, moves: &[[i32; 2]], captures: &[[i32; 2]]) -> PieceConfig {
    let paths = |steps: &[[i32; 2]]| {
        steps
            .iter()
            .map(|[x, y]| vec![IVec2::new(*x, *y)])
            .collect()
    };
    PieceConfig {
        id: PieceTypeId::from(id),
        display_name: id.to_string(),
        svg_path: format!("{id}.svg"),
        score_value: Score::from(10),
        cooldown_ms: DurationMs::from_millis(1000),
        move_paths: paths(moves),
        capture_paths: paths(captures),
        special_moves: vec![],
    }
}

/// Builds a pawn-like piece config: orthogonal moves and diagonal captures.
fn pawn_config() -> PieceConfig {
    piece_config(
        "pawn",
        &[[0, 1], [0, -1], [1, 0], [-1, 0]],
        &[[1, 1], [1, -1], [-1, 1], [-1, -1]],
    )
}

/// Builds a rook config sliding up to three squares upwards.
fn rook_config() -> PieceConfig {
    let path = vec![IVec2::new(0, 1), IVec2::new(0, 2), IVec2::new(0, 3)];
    PieceConfig {
        move_paths: vec![path.clone()],
        capture_paths: vec![path],
        ..piece_config("rook", &[], &[])
    }
}

/// Checks a move of a `config` piece owned by `owner` from `start` to `end`.
///
/// The moving piece is not placed on the board.
fn check(
    config: &PieceConfig,
    state: &GameState,
    start: BoardCoord,
    end: BoardCoord,
    owner: Option<PlayerId>,
) -> Result<MoveKind, GameError> {
    let mover = Piece::new(owner, config.id.clone(), start);
    catalog(vec![config.clone()], vec![]).check_move(state, &mover, end)
}

/// Builds an empty 100x100 board.
fn empty_state() -> GameState {
    GameState {
        board_size: BoardSize::from(100),
        ..GameState::default()
    }
}

/// Adds a player with `score` to the state.
fn add_player(state: &mut GameState, score: u64) -> PlayerId {
    let id = PlayerId::new();
    state.players.insert(
        id,
        Player {
            id,
            name: "P".to_string(),
            score: Score::from(score),
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

/// Builds a shop item replacing the piece type.
fn upgrade_item(name: &str, price: &str, replace_with: &str) -> ShopItemConfig {
    ShopItemConfig {
        display_name: name.to_string(),
        price_expr: Some(ExprString::from(price)),
        replace_with: Some(PieceTypeId::from(replace_with)),
        add_pieces: vec![],
    }
}

/// Builds a shop config with one group for `applies_to`.
fn shop_config(id: &str, auto: bool, applies_to: &str, items: Vec<ShopItemConfig>) -> ShopConfig {
    ShopConfig {
        id: ShopId::from(id),
        display_name: id.to_string(),
        default_uses: 2,
        color: None,
        focus_color: None,
        auto_upgrade_single_item: auto,
        groups: vec![ShopGroupConfig {
            applies_to: vec![PieceTypeId::from(applies_to)],
            items,
        }],
        default_group: None,
    }
}

/// Builds a catalog from piece and shop configs.
fn catalog(pieces: Vec<PieceConfig>, shops: Vec<ShopConfig>) -> Catalog {
    Catalog {
        pieces: pieces.into_iter().map(|p| (p.id.clone(), p)).collect(),
        shops: shops.into_iter().map(|s| (s.id.clone(), s)).collect(),
    }
}

/// Places a shop on the board.
fn place_shop(state: &mut GameState, id: &str, pos: [i32; 2], uses: u32) {
    state.shops.push(Shop {
        position: BoardCoord::new(pos[0], pos[1]),
        uses_remaining: uses,
        shop_id: ShopId::from(id),
    });
}

#[test]
/// Verifies quiet moves follow move paths and need empty targets.
fn pawn_quiet_moves() {
    let state = empty_state();
    let config = pawn_config();
    let start = BoardCoord::new(0, 0);

    for target in [[0, 1], [0, -1], [1, 0], [-1, 0]] {
        let end = BoardCoord::new(target[0], target[1]);
        assert_eq!(
            check(&config, &state, start, end, None),
            Ok(MoveKind::Quiet)
        );
    }
    assert_eq!(
        check(&config, &state, start, BoardCoord::new(1, 1), None),
        Err(GameError::InvalidMove)
    );
    assert_eq!(
        check(&config, &state, start, start, None),
        Err(GameError::InvalidMove)
    );
}

#[test]
/// Verifies captures follow capture paths and respect ownership.
fn pawn_captures() {
    let mut state = empty_state();
    let config = pawn_config();
    let me = PlayerId::new();
    let enemy = PlayerId::new();
    add_piece(&mut state, Some(enemy), "pawn", [1, 1]);
    add_piece(&mut state, Some(me), "pawn", [-1, 1]);
    add_piece(&mut state, Some(enemy), "pawn", [0, 1]);
    let start = BoardCoord::new(0, 0);

    assert_eq!(
        check(&config, &state, start, BoardCoord::new(1, 1), Some(me)),
        Ok(MoveKind::Capture)
    );
    assert_eq!(
        check(&config, &state, start, BoardCoord::new(-1, 1), Some(me)),
        Err(GameError::TargetFriendly)
    );
    // Orthogonal squares are only quiet moves, never captures.
    assert_eq!(
        check(&config, &state, start, BoardCoord::new(0, 1), Some(me)),
        Err(GameError::InvalidMove)
    );
}

#[test]
/// Verifies moves off the board are rejected.
fn moves_off_board_are_invalid() {
    let state = GameState {
        board_size: BoardSize::from(4),
        ..GameState::default()
    };
    assert_eq!(
        check(
            &pawn_config(),
            &state,
            BoardCoord::new(1, 0),
            BoardCoord::new(2, 0),
            None
        ),
        Err(GameError::InvalidMove)
    );
}

#[test]
/// Verifies sliding moves are blocked by pieces on earlier path squares.
fn sliding_path_blocking() {
    let mut state = empty_state();
    add_piece(&mut state, Some(PlayerId::new()), "pawn", [0, 1]);
    let config = rook_config();
    let start = BoardCoord::new(0, 0);

    assert_eq!(
        check(&config, &state, start, BoardCoord::new(0, 2), None),
        Err(GameError::PathBlocked)
    );
    assert_eq!(
        check(&config, &state, start, BoardCoord::new(0, 1), None),
        Ok(MoveKind::Capture)
    );
}

#[test]
/// Verifies NPCs never capture other NPCs.
fn npcs_do_not_capture_each_other() {
    let mut state = empty_state();
    add_piece(&mut state, None, "pawn", [1, 1]);
    assert_eq!(
        check(
            &pawn_config(),
            &state,
            BoardCoord::new(0, 0),
            BoardCoord::new(1, 1),
            None
        ),
        Err(GameError::TargetFriendly)
    );
}

#[test]
/// Verifies legal move listing contains quiet moves and captures only.
fn legal_moves_lists_quiet_and_capture_targets() {
    let mut state = empty_state();
    let me = PlayerId::new();
    add_piece(&mut state, Some(PlayerId::new()), "pawn", [1, 1]);
    add_piece(&mut state, Some(me), "pawn", [0, 1]);
    let moves = catalog(vec![pawn_config()], vec![]).legal_moves(
        &state,
        &Piece::new(Some(me), PieceTypeId::from("pawn"), BoardCoord::new(0, 0)),
    );

    let capture = BoardCoord::new(1, 1);
    let quiet = BoardCoord::new(0, -1);
    assert_eq!(moves.len(), 4);
    assert!(moves.contains(&LegalMove {
        target: capture,
        kind: MoveKind::Capture,
        affected: vec![capture],
    }));
    assert!(moves.contains(&LegalMove {
        target: quiet,
        kind: MoveKind::Quiet,
        affected: vec![],
    }));
    assert!(
        !moves
            .iter()
            .any(|legal| legal.target == BoardCoord::new(0, 1))
    );
    // Captures are chosen on the captured piece, quiet moves on their target.
    let handles: Vec<_> = moves.iter().flat_map(LegalMove::handles).collect();
    assert!(handles.contains(&&capture) && handles.contains(&&quiet));
}

#[test]
/// Verifies shop group selection matches piece type or defaults.
fn shop_group_by_piece_type() {
    let mut shop = shop_config("shop", false, "pawn", vec![upgrade_item("Up", "1", "rook")]);
    shop.default_group = Some(ShopGroupConfig {
        applies_to: vec![],
        items: vec![],
    });
    let pawn = Piece::new(None, PieceTypeId::from("pawn"), BoardCoord::new(0, 0));
    let rook = Piece::new(None, PieceTypeId::from("rook"), BoardCoord::new(0, 0));

    assert_eq!(shop.group_for(Some(&pawn)).unwrap().items.len(), 1);
    assert_eq!(shop.group_for(Some(&rook)).unwrap().items.len(), 0);
    assert_eq!(shop.group_for(None).unwrap().items.len(), 0);
    assert!(shop.manual_items_for(&rook).is_none());
}

#[test]
/// Verifies price variables count the buyer's pieces per type.
fn price_vars_count_player_pieces() {
    let mut state = empty_state();
    let me = add_player(&mut state, 0);
    add_piece(&mut state, Some(me), "pawn", [0, 0]);
    add_piece(&mut state, Some(me), "pawn", [1, 0]);
    add_piece(&mut state, Some(me), "rook", [2, 0]);
    add_piece(&mut state, Some(PlayerId::new()), "rook", [3, 0]);
    let vars = catalog(vec![pawn_config(), rook_config()], vec![]).price_vars(&state, me);

    assert_eq!(vars.get("player_piece_count"), Some(ExprValue::Int(3)));
    assert_eq!(vars.get("pawn_count"), Some(ExprValue::Int(2)));
    assert_eq!(vars.get("rook_count"), Some(ExprValue::Int(1)));
}

#[test]
/// Verifies captures reward the capturer with score, takes and kills.
fn capture_rewards_owner() {
    let mut state = empty_state();
    let me = add_player(&mut state, 0);
    let pawn = add_piece(&mut state, Some(me), "pawn", [0, 0]);
    add_piece(&mut state, None, "pawn", [1, 1]);
    let catalog = catalog(vec![pawn_config()], vec![]);

    let effects = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(5))
        .apply_move(pawn, BoardCoord::new(1, 1))
        .expect("capture is legal");

    assert_eq!(effects.captured.len(), 1);
    assert_eq!(state.pieces.len(), 1);
    let player = &state.players[&me];
    assert_eq!(player.score, Score::from(10));
    assert_eq!(player.pieces_captured, 1);
    let moved = &state.pieces[&pawn];
    assert_eq!(moved.position, BoardCoord::new(1, 1));
    assert_eq!(moved.last_move_time, TimestampMs::from_millis(5));
    assert_eq!(moved.cooldown_ms, DurationMs::from_millis(1000));
}

#[test]
/// Verifies landing on an auto-upgrade shop replaces the piece type and uses the shop.
fn auto_upgrade_on_landing() {
    let mut state = empty_state();
    let me = add_player(&mut state, 0);
    let pawn = add_piece(&mut state, Some(me), "pawn", [0, 0]);
    place_shop(&mut state, "step", [0, 1], 2);
    let catalog = catalog(
        vec![pawn_config(), rook_config()],
        vec![shop_config(
            "step",
            true,
            "pawn",
            vec![upgrade_item("Step", "0", "rook")],
        )],
    );

    Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply(
            pawn,
            PieceAction::Move {
                target: BoardCoord::new(0, 1),
            },
        )
        .expect("move is legal");

    assert_eq!(state.pieces[&pawn].piece_type, PieceTypeId::from("rook"));
    assert_eq!(state.shops[0].uses_remaining, 1);
}

#[test]
/// Verifies a purchase that cannot place its pieces changes nothing.
fn failed_purchase_is_atomic() {
    let mut state = GameState {
        board_size: BoardSize::from(1),
        ..GameState::default()
    };
    let me = add_player(&mut state, 100);
    let king = add_piece(&mut state, Some(me), "pawn", [0, 0]);
    place_shop(&mut state, "hire", [0, 0], 1);
    let mut hire = shop_config("hire", false, "pawn", vec![]);
    hire.groups[0].items.push(ShopItemConfig {
        display_name: "Hire".to_string(),
        price_expr: Some(ExprString::from("10")),
        replace_with: Some(PieceTypeId::from("rook")),
        add_pieces: vec![PieceTypeId::from("pawn")],
    });
    let catalog = catalog(vec![pawn_config(), rook_config()], vec![hire]);
    let before = state.clone();

    let result = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0)).apply(
        king,
        PieceAction::Buy {
            item: ShopItemIndex(0),
        },
    );

    assert_eq!(result, Err(GameError::NoSpaceNearby));
    assert_eq!(state, before);
}

#[test]
/// Verifies purchases charge the price and remove shops on their last use.
fn purchase_charges_and_depletes_shop() {
    let mut state = empty_state();
    let me = add_player(&mut state, 100);
    let pawn = add_piece(&mut state, Some(me), "pawn", [0, 0]);
    place_shop(&mut state, "forge", [0, 0], 1);
    let catalog = catalog(
        vec![pawn_config(), rook_config()],
        vec![shop_config(
            "forge",
            false,
            "pawn",
            vec![upgrade_item("Rook", "30", "rook")],
        )],
    );

    let effects = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0))
        .apply(
            pawn,
            PieceAction::Buy {
                item: ShopItemIndex(0),
            },
        )
        .expect("purchase succeeds");

    assert_eq!(effects.depleted_shops, vec![ShopId::from("forge")]);
    assert!(state.shops.is_empty());
    assert_eq!(state.players[&me].score, Score::from(70));
    assert_eq!(state.pieces[&pawn].piece_type, PieceTypeId::from("rook"));
}

#[test]
/// Verifies unaffordable purchases are rejected without changes.
fn purchase_requires_score() {
    let mut state = empty_state();
    let me = add_player(&mut state, 5);
    let pawn = add_piece(&mut state, Some(me), "pawn", [0, 0]);
    place_shop(&mut state, "forge", [0, 0], 1);
    let catalog = catalog(
        vec![pawn_config(), rook_config()],
        vec![shop_config(
            "forge",
            false,
            "pawn",
            vec![upgrade_item("Rook", "30", "rook")],
        )],
    );

    let result = Simulation::new(&catalog, &mut state, TimestampMs::from_millis(0)).apply(
        pawn,
        PieceAction::Buy {
            item: ShopItemIndex(0),
        },
    );

    assert_eq!(
        result,
        Err(GameError::InsufficientScore {
            needed: Score::from(30),
            have: Score::from(5)
        })
    );
    assert_eq!(state.shops[0].uses_remaining, 1);
}

#[test]
/// Verifies rejections caused by a changed board are silent while real mistakes are shown.
fn stale_premove_errors_are_silent() {
    for error in [
        GameError::PieceNotFound,
        GameError::TargetFriendly,
        GameError::InvalidMove,
        GameError::PathBlocked,
        GameError::ShopNotFound,
        GameError::ActionCancelled,
    ] {
        assert!(!error.is_user_visible(), "{error:?} should be silent");
    }
    assert!(
        GameError::InsufficientScore {
            needed: Score::from(10),
            have: Score::from(5),
        }
        .is_user_visible()
    );
}
