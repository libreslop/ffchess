# FFChess Data-Driven Engine Design

FFChess is a data-driven, multi-mode game engine. Mode behavior, pieces, and shops are defined in JSONC, while shared Rust types enforce semantics across server and client.

## Core Architecture

### 1. Multi-Instance Server
- Each mode runs as its own `GameInstance` keyed by `ModeId`.
- WebSocket connections use `/api/ws/:mode_id`.
- A `ModeSummary` list is served at `/api/modes` and embedded into `index.html` for instant boot.

### 2. Configuration System
- `config/pieces/*.jsonc`: Piece movement/capture paths, cooldowns, score values, glyphs.
- `config/shops/*.jsonc`: Shop groups and items with expression-driven prices.
- `config/modes/*.jsonc`: Board size formulas, fog radius, respawn cooldowns, NPC limits, shop counts, kits.
- `config/global/server.jsonc`: Default name pool (server-only).
- `config/global/client.jsonc`: Client timing and camera tuning (injected into HTML).

### 3. Expression Engine (`meval`)
Expressions are stored as `ExprString` and evaluated at runtime using `evaluate_expression`.
- Board size uses `player_count`.
- Shop pricing uses `player_piece_count` and per-piece counts like `pawn_count`.
- NPC limits use expressions in `npc_limits` per mode.

### 4. Path-Based Movement
Movement and capture are defined as lists of finite paths per piece.
- Blocking is enforced for multi-step paths.
- Jumping (e.g., Knight-style) is a single-step path.
- Shared move validation lives in `common::logic::is_valid_move`.

### 5. Shop Group Selection
Shop groups are resolved by the acting piece type.
- `select_shop_group` picks a matching group by `applies_to` or falls back to `default_group`.
- `build_price_vars` constructs the expression variables used in `price_expr`.

## Client Integration
- The server sends `GameModeClientConfig` plus relevant piece/shop configs in `Init`.
- The client renders shop UI and move hints from config data.
- Mode list and global client config are injected into `index.html` to avoid extra round trips on boot.

## Data Schema Summary

### Piece Configuration
```json
{
  "id": "knight",
  "display_name": "Knight",
  "char": "N",
  "cooldown_ms": 2000,
  "score_value": 30,
  "move_paths": [ [[1, 2]], [[2, 1]] ],
  "capture_paths": [ [[1, 2]], [[2, 1]] ]
}
```

### Shop Configuration
```json
{
  "id": "upgrade_shop",
  "display_name": "Upgrade Forge",
  "default_uses": 3,
  "color": "#1e293b",
  "groups": [
    {
      "applies_to": ["pawn"],
      "items": [
        { "display_name": "Knight", "price_expr": "50 + knight_count * 25", "replace_with": "knight", "add_pieces": [] }
      ]
    }
  ],
  "default_group": {
    "applies_to": [],
    "items": [
      { "display_name": "Pawn", "price_expr": "10", "replace_with": null, "add_pieces": ["pawn"] }
    ]
  }
}
```

### Game Mode Configuration
```json
{
  "id": "standard",
  "display_name": "Standard FFA",
  "max_players": 32,
  "board_size": "max(40, 25 + sqrt(player_count) * 17.5)",
  "camera_pan_limit": "20",
  "fog_of_war_radius": "12",
  "respawn_cooldown_ms": 3000,
  "npc_limits": [
    { "piece_id": "pawn", "max_expr": "player_count * 2" }
  ],
  "shop_counts": [
    { "shop_id": "upgrade_shop", "count": 4 }
  ],
  "kits": [
    { "name": "standard", "description": "Classic", "pieces": ["king", "pawn", "pawn"] }
  ],
  "hooks": [
    { "trigger": "OnCapture", "target_piece_id": "king", "action": "EliminateOwner" }
  ]
}
```

## Expression Variables
- `player_count`: Active players in the mode.
- `player_piece_count`: Pieces owned by the acting player (for shop pricing).
- `[piece_id]_count`: Per-piece counts (e.g., `pawn_count`).
- Standard math functions from `meval` (e.g., `sqrt`, `max`, `min`, `floor`).
