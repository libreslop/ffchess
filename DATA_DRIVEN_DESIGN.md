# FFChess Data-Driven Engine Design

FFChess has been refactored from a hardcoded game into a flexible, data-driven engine. This allows for rapid iteration of game modes, piece types, and shop configurations without recompiling the server or client.

## Core Architecture

### 1. Multi-Instance Server
The server now supports multiple encapsulated game instances. Each instance runs its own game loop and is identified by a `mode_id`.
- **Routing:** WebSocket connections are routed via `/api/ws/:mode_id`.
- **Encapsulation:** Every `GameInstance` has its own `GameState`, configurations, and player channels.

### 2. Configuration System
Configurations are stored as JSON files in the `config/` directory:
- `config/pieces/*.json`: Individual piece definitions (move paths, capture paths, cooldowns, score values).
- `config/shops/*.json`: Shop definitions (entry groups, price expressions, pieces to recruit/upgrade).
- `config/modes/*.json`: Game mode definitions (board size formulas, NPC limits, win conditions, starting kits).

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
      "entries": [
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
  "win_conditions": [
    { "type": "CapturePiece", "target_id": "king", "reward": "EliminateOwner" }
  ]
}
```
