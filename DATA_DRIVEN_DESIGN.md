# FFChess Data-Driven Engine Design

FFChess has been refactored from a hardcoded game into a flexible, data-driven engine. This allows for rapid iteration of game modes, piece types, and shop configurations without recompiling the server or client. Core identifiers are strongly typed in the `common` crate (e.g., `ModeId`, `PieceTypeId`, `ShopId`) to enforce semantics.

## Core Architecture

### 1. Multi-Instance Server
The server now supports multiple encapsulated game instances. Each instance runs its own game loop and is identified by a `mode_id`.
- **Routing:** WebSocket connections are routed via `/api/ws/:mode_id`.
- **Encapsulation:** Every `GameInstance` has its own `GameState`, configurations, and player channels.

### 2. Configuration System
Configurations are stored as JSONC files in the `config/` directory. Mode IDs are derived from filenames (e.g., `ffa.jsonc` → `ModeId("ffa")`).
- `config/pieces/*.jsonc`: Individual piece definitions (move paths, capture paths, cooldowns, score values).
- `config/shops/*.jsonc`: Shop definitions (item groups, price expressions, pieces to recruit/upgrade).
- `config/modes/*.jsonc`: Game mode definitions (board size formulas, NPC limits, hooks, starting kits).

### 3. Expression Engine (`meval`)
Dynamic values such as shop prices, board sizes, and NPC spawn limits are calculated using mathematical expressions.
- **Variables:** `player_count`, `board_size`, and `[piece_id]_count` (e.g., `pawn_count`).
- **Use Case:** Scaling difficulty and economy based on the number of active players.

### 4. Path-Based Movement
Movements and captures are no longer hardcoded enum types. They are defined as lists of finite paths.
- **Blocking:** If any square in a path is occupied, subsequent squares in that path are inaccessible.
- **Knight-style moves:** Defined as single-step paths to allow jumping over pieces.

### 5. Win Conditions & Triggers
Game modes define win conditions through triggers.
- **Example:** `CapturePiece` with target `king` and reward `EliminateOwner`. This generalizes the "capture king to win" logic.

## Client-Side Integration

The Yew client has been updated to be fully configuration-aware:
- **Initialization:** Upon connection, the server sends all relevant configs (Mode, Pieces, Shops) to the client.
- **Dynamic UI:** `JoinScreen` fetches available kits from the mode config; `ShopUI` resolves available upgrades based on the piece currently on the shop square.
- **Rendering:** The canvas renderer uses the character and color definitions from the piece configurations.

## Data Schema Summary

### Piece Configuration
```json
{
  "id": "knight",
  "display_name": "Knight",
  "char": "N",
  "cooldown_ms": 2000,
  "score_value": 30,
  "move_paths": [ [[1, 2]], [[2, 1]], ... ],
  "capture_paths": [ [[1, 2]], [[2, 1]], ... ]
}
```

### Shop Configuration
```json
{
  "id": "upgrade_shop",
  "display_name": "Upgrade Forge",
  "default_uses": 3,
  "groups": [
    {
      "applies_to": ["pawn"],
      "items": [
        { "replace_with": "knight", "price_expr": "50 + knight_count * 25" }
      ]
    }
  ]
}
```

### Game Mode Configuration
```json
{
  "id": "standard",
  "display_name": "Standard FFA",
  "board_size": "max(40, 25 + sqrt(player_count) * 17.5)",
  "kits": [
    { "name": "standard", "pieces": ["king", "pawn", "pawn", "knight", "knight"] }
  ],
  "hooks": [
    { "trigger": "OnCapture", "target_piece_id": "king", "action": "EliminateOwner" }
  ]
}
```

## Configuration Reference

### Global (Server) – `config/global/server.jsonc`
- `default_name.adjectives` / `default_name.nouns`: Word lists the server combines to generate a name when the player leaves the name field empty. Lives server-side only (client no longer ships a name pool).

### Global (Client) – `config/global/client.jsonc`
- `game_order`: Ordered list of mode ids; first entry is the default selection on first load.
- `modes_refresh_ms`: How often the client refreshes the mode list metadata.
- `ping_interval_ms`: Heartbeat interval for latency/presence pings.
- `tick_interval_ms`: Client-side simulation tick spacing (ms).
- `render_interval_ms`: Target frame interval (ms) for the canvas render loop.
- `disconnected_hide_ms`: Delay before hiding the disconnected overlay after recovery.
- `fatal_auto_hide_ms`: How long fatal error banners stay visible.
- Camera tuning: `camera_zoom_min`, `camera_zoom_max`, `zoom_lerp`, `inertia_decay`, `velocity_cutoff`, `pan_lerp_alive`, `pan_lerp_dead`, `tile_size_px`, `death_zoom`, `scroll_zoom_base`.

### Modes – `config/modes/*.jsonc`
- `id`, `display_name`, `max_players`: Identity and lobby limits.
- `board_size`: Expression controlling the side length (supports `player_count`, `player_piece_count`, etc.).
- `camera_pan_limit`: Expression that caps how far the camera can drift from the player’s pieces.
- `fog_of_war_radius`: Expression for visible radius around owned pieces.
- `respawn_cooldown_ms`: Per-mode respawn delay.
- `npc_limits`: Array of `{ piece_id, max_expr }` giving per-piece NPC caps via expressions.
- `shop_counts`: Array of `{ shop_id, count }` controlling how many of each shop type spawn.
- `kits`: Starting armies; each kit has `name`, `description`, and `pieces`.
- `hooks`: Trigger/action pairs (e.g., `OnCapture` + `target_piece_id: king` → `EliminateOwner`).

### Pieces – `config/pieces/*.jsonc`
- `id`, `display_name`, `char`: Piece identity and rendered glyph.
- `score_value`: Points granted on capture.
- `cooldown_ms`: Base cooldown per move.
- `move_paths` / `capture_paths`: Lists of finite step paths. Blocking ends the path; Knights are encoded as one-step paths to allow jumps.

### Shops – `config/shops/*.jsonc`
- `id`, `display_name`, `default_uses`, `color`: Identity, display info, and how many times a shop can be used before despawning.
- `groups`: Per-piece applicability. Each `group` has `applies_to` (piece ids) and `items`.
- Shop `items`: `{ display_name, price_expr, replace_with?, add_pieces[] }`. `replace_with` upgrades the acting piece; `add_pieces` spawns new ones.
- `default_group`: Fallback `items` applied when no `group` matches the acting piece.

### Expression Variables
Expressions in mode/shop configs can reference:
- `player_count`, `player_piece_count` (for the acting player), and `[piece_id]_count` (e.g., `pawn_count`).
- Standard math functions supported by `meval` (e.g., `sqrt`, `max`, `min`, `floor`).
