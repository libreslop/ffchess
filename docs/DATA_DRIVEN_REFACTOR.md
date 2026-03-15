# Data-Driven Game Engine Refactor

## Overview
This document captures the current data-driven architecture of `ffchess-server`. The project has completed the refactor from hardcoded rules to JSONC-driven configuration with shared, typed Rust models.

## 1. Configuration System
The game is configured via JSONC files under `config/`. Mode IDs are derived from filenames.

### 1.1 Directory Structure
```
config/
├── global/
│   ├── client.jsonc
│   └── server.jsonc
├── pieces/
├── shops/
└── modes/
```

### 1.2 Schemas (Implemented)

#### Piece Configuration (`config/pieces/*.jsonc`)
```json
{
  "id": "string",
  "display_name": "string",
  "char": "char",
  "score_value": "u64",
  "cooldown_ms": "u64",
  "move_paths": [ [[x, y], [x, y]], ... ],
  "capture_paths": [ [[x, y], [x, y]], ... ]
}
```

#### Shop Configuration (`config/shops/*.jsonc`)
```json
{
  "id": "string",
  "display_name": "string",
  "default_uses": "u32",
  "color": "#RRGGBB | null",
  "groups": [
    {
      "applies_to": ["piece_id", ...],
      "items": [
        {
          "display_name": "string",
          "price_expr": "string",
          "replace_with": "piece_id" | null,
          "add_pieces": ["piece_id", ...]
        }
      ]
    }
  ],
  "default_group": {
    "applies_to": [],
    "items": [ ... ]
  }
}
```

#### Game Mode Configuration (`config/modes/*.jsonc`)
```json
{
  "id": "string",
  "display_name": "string",
  "max_players": "u32",
  "board_size": "string",
  "camera_pan_limit": "string",
  "fog_of_war_radius": "string",
  "respawn_cooldown_ms": "u64",
  "npc_limits": [
    { "piece_id": "string", "max_expr": "string" }
  ],
  "shop_counts": [
    { "shop_id": "string", "count": "u32" }
  ],
  "kits": [
    { "name": "string", "description": "string", "pieces": ["piece_id", ...] }
  ],
  "hooks": [
    { "trigger": "OnCapture", "target_piece_id": "string", "action": "EliminateOwner" }
  ]
}
```

## 2. Server Architecture
- `ConfigManager` loads and validates config at startup.
- `ServerState` owns a map of `GameInstance`s, one per mode.
- WebSocket routing uses `/api/ws/:mode_id` and provides an initial `Init` payload.
- Mode list data (`ModeSummary`) is served at `/api/modes` and embedded in `index.html`.

## 3. Client Architecture
- The client consumes `GameModeClientConfig`, piece configs, and shop configs from `Init`.
- Global client config is injected into the HTML at boot and hydrated by the app.
- The reducer maintains premoves and reconciles with server updates.

## 4. Shared Logic
- Movement validation and expression evaluation live in `common::logic`.
- Types like `Score`, `BoardSize`, `DurationMs`, `TimestampMs`, and `ExprString` enforce semantics.

## 5. Dependencies
- `meval` for expression evaluation.
- `serde` and `serde_json` for config parsing.
- `walkdir` for config discovery.
