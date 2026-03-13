# Data-Driven Game Engine Refactor Design

## Overview
This document outlines the architectural changes required to transition `ffchess-server` from a hardcoded chess variant to a generic, data-driven tile-based game engine.

## 1. Configuration System
The system will rely on JSON configuration files located in a root `config/` directory.

### 1.1 Directory Structure
```
config/
├── pieces/
│   ├── pawn.json
│   ├── knight.json
│   └── ...
├── shops/
│   ├── spawn_shop.json
│   └── upgrade_shop.json
└── modes/
    └── standard.json
```

### 1.2 Schemas

#### Piece Configuration (`config/pieces/*.json`)
```json
{
  "id": "string",
  "display_name": "string",
  "char": "char", // Single character for board representation
  "score_value": "u64",
  "cooldown_ms": "u64",
  "move_paths": [ [[x, y], [x, y]], ... ], // List of paths. Each path is a list of steps.
  "capture_paths": [ [[x, y], [x, y]], ... ] // Separate paths for capturing.
}
```

#### Shop Configuration (`config/shops/*.json`)
```json
{
  "id": "string",
  "display_name": "string",
  "default_uses": "u32", // -1 for infinite
  "groups": [
    {
      "applies_to": ["piece_id", ...],
      "entries": [
        {
          "display_name": "string",
          "price_expr": "string", // Evaluated expression (e.g. "10 + pawn_count * 2")
          "replace_with": "piece_id" | null,
          "add_pieces": ["piece_id", ...]
        }
      ]
    }
  ],
  "default_group": {
    "entries": [...]
  }
}
```

#### Game Mode Configuration (`config/modes/*.json`)
```json
{
  "id": "string",
  "display_name": "string",
  "max_players": "u32",
  "board_size_expr": "string", // e.g. "max(40, 25 + sqrt(player_count) * 17.5)"
  "npc_limits": [
    { "piece_id": "string", "max_expr": "string" }
  ],
  "shop_counts": [
    { "shop_id": "string", "count_expr": "string" }
  ],
  "kits": [
    { "name": "string", "pieces": ["piece_id", ...] }
  ],
  "win_conditions": [
    { "type": "CapturePiece", "target_id": "string", "reward": "EliminateOwner" }
  ]
}
```

## 2. Server Architecture

### 2.1 Config Manager
A singleton or shared resource `ConfigManager` will load and validate all JSON files at startup. It will panic if any references (e.g., a mode referencing a missing piece) are invalid.

### 2.2 Multi-Tenancy
The `ServerState` will no longer hold a single `GameState`. Instead, it will manage a collection of `GameInstance`s.
- `games: RwLock<HashMap<String, Arc<RwLock<GameInstance>>>>`
- Keys are `mode_id` (or `instance_id` if we scale later).
- `ws_handler` will extract the mode from the URL path (e.g., `/ws/{mode_id}`) and connect the player to the appropriate instance.

### 2.3 Game Logic Refactor
- **Movement**: The `is_valid_chess_move` function will be replaced by a path-checking algorithm that iterates through the `move_paths` or `capture_paths` defined in the piece config.
- **Spawning**: NPC spawning logic will evaluate `npc_limits` expressions.
- **Shop**: Shop interactions will evaluate `price_expr` using `meval`.

## 3. Client Architecture

### 3.1 Dynamic Initialization
The `Init` message from the server will now include the `GameModeConfig`, `PieceConfig`s (for all pieces used in that mode), and `ShopConfig`s.
The client will use this data to:
- Render the correct character/SVG for pieces.
- Validate moves locally (for UI feedback).
- Render the Shop UI dynamically.

### 3.2 Asset Mapping
The client will map piece IDs to assets. For the MVP, we will stick to the existing mapping where possible (e.g., "pawn" -> pawn SVG) or fallback to the character defined in JSON.

## 4. Dependencies
- `meval`: For expression evaluation.
- `serde`, `serde_json`: For config parsing.
- `walkdir` (optional): For convenient directory traversal.

## 5. Migration Strategy
1. Add dependencies.
2. Define new Config structs in `common`.
3. Create the configuration files for the "Standard" mode (replicating current behavior).
4. Implement `ConfigManager` in `server`.
5. Refactor `GameState` and Logic in `server` to use Configs.
6. Refactor `Client` to consume Configs.
7. Update Tests.
