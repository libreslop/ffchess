# Game Mode Configuration

Game mode configuration files define the rules, board parameters, spawn limits, and player kits for different game modes.

**Path:** `config/modes/`
**Naming:** `<mode_id>.jsonc`

The `mode_id` is automatically derived from the filename stem (e.g., `ffa.jsonc` becomes `ffa`).

### Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `display_name` | `String` | The name of the mode shown in the UI. | No | - |
| `max_players` | `u32` | Maximum number of players allowed in a single instance. | No | - |
| `queue_players` | `u32` | Minimum number of players required in the queue before starting. | Yes | `0` |
| `preview_switch_delay_ms` | `u32` | Delay before switching the queue preview when a game ends. | Yes | `5000` |
| `board_size` | `String` | Expression for the board size based on `player_count`. | No | - |
| `camera_pan_limit` | `String` | Expression for the camera's pan limit. | No | - |
| `fog_of_war_radius` | `String` | Expression for the fog of war radius. | Yes | `null` |
| `respawn_cooldown_ms` | `u32` | Cooldown period before a player can respawn. | No | - |
| `npc_limits` | `Array<NpcLimit>` | List of limits for different NPC types. | No | - |
| `shop_counts` | `Array<ShopCount>` | Number of shops of each type to spawn. | No | - |
| `kits` | `Array<Kit>` | Starting piece kits available for selection. | No | - |
| `queue_layout` | `QueueLayout` | Fixed spawn layout for queued matches; if set, players spawn from this preset instead of kit-random spawn. | Yes | `null` |
| `hooks` | `Array<Hook>` | Custom gameplay event hooks. | No | - |

### NPC Limit (`NpcLimit`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `piece_id` | `String` | The ID of the piece type. |
| `max_expr` | `String` | Expression for the maximum number of NPCs based on `player_count`. |

### Shop Count (`ShopCount`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `shop_id` | `String` | The ID of the shop type. |
| `count` | `u32` | Number of shops to spawn. |

### Kit (`Kit`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `name` | `String` | The name of the kit. |
| `description` | `String` | A brief description of the kit. |
| `pieces` | `Array<String>` | List of `piece_id`s included in the kit. |

### Hook (`Hook`)

| Attribute | Type | Description | Optional |
|-----------|------|-------------|----------|
| `trigger` | `String` | The event that triggers the hook (e.g., `OnCapture`). | No |
| `target_piece_id` | `String` | The piece type that must be involved in the trigger. | Yes |
| `action` | `String` | The action to perform when triggered (e.g., `EliminateOwner`). | No |
| `victory_title` | `String` | Custom title shown when this hook leads to victory. | Yes |
| `victory_message` | `String` | Custom message shown when this hook leads to victory. | Yes |

### Queue Layout (`QueueLayout`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `players` | `Array<QueueLayoutPlayer>` | Spawn slots in join order for a queue match. |

### Queue Layout Player (`QueueLayoutPlayer`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `pieces` | `Array<QueueLayoutPiece>` | Pieces to spawn for this player slot. |

### Queue Layout Piece (`QueueLayoutPiece`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `piece_id` | `String` | Piece type ID to spawn. |
| `position` | `[i32, i32]` | Absolute board coordinate to place the piece at. |

### Expressions

Common variables available in expressions:
- `player_count`: Current number of active players.

### Example (`ffa.jsonc`)

```jsonc
{
    "display_name": "FFA",
    "max_players": 20,
    "board_size": "max(40, 25 + sqrt(floor(player_count/3)) * 15)",
    "camera_pan_limit": "fog_of_war_radius + 2",
    "fog_of_war_radius": "20",
    "respawn_cooldown_ms": 3000,
    "npc_limits": [
        { "piece_id": "pawn", "max_expr": "max(5, player_count * 2)" },
        { "piece_id": "knight", "max_expr": "max(3, player_count * 1)" }
    ],
    "shop_counts": [
        { "shop_id": "spawn_shop", "count": 10 }
    ],
    "kits": [
        {
            "name": "Standard",
            "description": "2 Pawns, 2 Knights",
            "pieces": ["king", "pawn", "pawn", "knight", "knight"]
        }
    ],
    "hooks": [
        {
            "trigger": "OnCapture",
            "target_piece_id": "king",
            "action": "EliminateOwner"
        }
    ]
}
```
