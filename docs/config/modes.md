# Mode Configuration

Mode configs live in `config/modes/` and define the rules for one playable ruleset.

**Path pattern:** `config/modes/<mode_id>.jsonc`

The filename stem becomes the runtime `GameModeConfig.id`.

## Top-Level Fields

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `display_name` | string | Yes | none | UI-facing mode name. |
| `max_players` | integer (`u32`) | Yes | none | Maximum simultaneous players in one instance. |
| `queue_players` | integer (`u32`) | No | `0` | Queue threshold for private-match modes. Only values `>= 2` create matchmaking behavior. |
| `preview_switch_delay_ms` | integer (`i64`) | No | `5000` | Delay before queue preview boards switch away from an ended private match. Only relevant for queue modes. |
| `board_size` | expression string | Yes | none | Server-side expression evaluated with `player_count`. |
| `camera_pan_limit` | expression string | Yes | none | Client-side expression evaluated with `player_piece_count` and `fog_of_war_radius`. |
| `fog_of_war_radius` | expression string or `null` | No | `null` | Client-side expression evaluated with `player_piece_count`. `null` disables fog of war. |
| `show_scoreboard` | boolean | No | `true` | Shows or hides the in-game leaderboard overlay. |
| `join_camera_center` | `JoinCameraCenter` | No | `{ "piece_id": "king" }` | Initial camera focus when entering `Alive` (first spawn/rejoin). |
| `disable_screen_panning` | boolean | No | `false` | Disables drag/touch panning by the player. Camera can still auto-focus/follow. |
| `respawn_cooldown_ms` | integer (`i64`) | Yes | none | Time after elimination before the same stored player id may respawn. |
| `npc_limits` | `NpcLimit[]` | Yes | none | Per-piece NPC cap formulas. |
| `shop_counts` | `ShopCount[]` | Yes | none | Randomly spawned shops for the mode. |
| `fixed_shops` | `FixedShop[]` | No | `[]` | Absolute shop placements that always spawn at the configured coordinates. |
| `kits` | `Kit[]` | Yes | none | Selectable starting armies. |
| `queue_layout` | `QueuePresetLayout` or `null` | No | `null` | Fixed per-slot spawn layout for queue matches. |
| `hooks` | `Hook[]` | Yes | none | Trigger/action rules evaluated during the tick hook phase. |

## Nested Types

### `NpcLimit`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `piece_id` | piece id | Yes | NPC piece type to count and spawn. |
| `max_expr` | expression string | Yes | Server-side formula evaluated with `player_count`. |

### `ShopCount`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `shop_id` | shop id | Yes | Shop type to spawn randomly. |
| `count` | integer (`u32`) | Yes | Number of copies spawned during instance initialization. |

### `FixedShop`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `shop_id` | shop id | Yes | Shop type to place. |
| `position` | `[i32, i32]` | Yes | Absolute board coordinate. |

### `Kit`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | Yes | Identifier shown in the join screen. |
| `description` | string | Yes | Short UI description. |
| `pieces` | piece id[] | Yes | Starting pieces. At least one entry must satisfy king naming rules. |

### `QueuePresetLayout`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `players` | `QueuePresetPlayer[]` | Yes | Spawn slots in join order. |

### `QueuePresetPlayer`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `board_rotation_deg` | integer (`i32`) | No | `0` | The client only gives special meaning to `180`; other values are preserved but not interpreted differently. |
| `pieces` | `QueuePresetPiece[]` | Yes | none | Exact pieces placed for this player slot. |

### `QueuePresetPiece`

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `piece_id` | piece id | Yes | Piece type to spawn. |
| `position` | `[i32, i32]` | Yes | Absolute board coordinate. |

### `JoinCameraCenter`

Untagged union: use exactly one of these objects.

| Shape | Required fields | Notes |
| --- | --- | --- |
| Piece target | `piece_id` | Focuses the local player's first owned piece with this type id (falls back to king if missing). |
| Position target | `position` | Focuses a board-space position `[x, y]` (`f64`); integer values target a tile center and fractions are allowed. |

### `Hook`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `trigger` | string enum | Yes | none | One of `OnCapture`, `OnCapturePieceActive`, `OnPlayerLeave`. |
| `target_piece_id` | piece id or `null` | No | `null` | Piece filter for capture hooks. `null` means "all piece types". |
| `players_left` | integer (`u32`) or `null` | No | `null` | Only used by `OnPlayerLeave` / `WinRemaining`. |
| `action` | string enum | Yes | none | One of `EliminateOwner`, `WinCapturer`, `WinRemaining`. |
| `title` | string or `null` | No | `null` | Optional victory title override. |
| `message` | string or `null` | No | `null` | Optional victory message override. |
| `victory_focus` | string enum or `null` | No | runtime-specific | One of `CaptureSquare` or `KeepCurrent`. |

## Supported Hook Combinations

The loader will deserialize any `trigger` + `action` pairing, but the runtime currently only
executes these combinations:

| Trigger | Action | Runtime behavior |
| --- | --- | --- |
| `OnCapture` | `EliminateOwner` | Remove the owner of the captured piece from the match. |
| `OnCapturePieceActive` | `WinCapturer` | Send a victory payload to the capturing player. |
| `OnPlayerLeave` | `WinRemaining` | If the remaining player count matches, send a victory payload to the survivor. |

Any other pairing deserializes successfully but is ignored by hook resolution.

## Expression Contexts

Different expressions run in different places:

| Field | Variables available |
| --- | --- |
| `board_size` | `player_count` |
| `npc_limits[*].max_expr` | `player_count` |
| `fog_of_war_radius` | `player_piece_count` |
| `camera_pan_limit` | `player_piece_count`, `fog_of_war_radius` |

## Queue Mode Rules

If `queue_players >= 2`, the mode behaves differently:

- players join a matchmaking queue instead of entering the public instance directly,
- the public instance is reused as a preview board,
- when the queue fills, the server spawns a private `GameInstance`,
- the private instance copies the mode config but zeroes `queue_players`,
- `queue_layout`, if present, controls the exact spawn positions for the private match.

If `queue_layout.players.len()` is smaller than the number of queued players admitted into a match,
joining the extra slot fails at runtime.

## Example: FFA

```jsonc
{
  "display_name": "FFA",
  "max_players": 20,

  // Grows with active player count on the authoritative server.
  "board_size": "max(40, 25 + sqrt(floor(player_count/3)) * 15)",

  // Evaluated on the client using the local player's piece count and fog radius.
  "camera_pan_limit": "fog_of_war_radius + 2",
  "fog_of_war_radius": "20",

  "respawn_cooldown_ms": 3000,
  "npc_limits": [
    { "piece_id": "pawn", "max_expr": "max(5, player_count * 2)" }
  ],
  "shop_counts": [
    { "shop_id": "spawn_shop", "count": 10 },
    { "shop_id": "upgrade_shop", "count": 10 }
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

## Example: Queue Mode With Fixed Layout

```jsonc
{
  "display_name": "Bullet",
  "max_players": 2,
  "queue_players": 2,
  "preview_switch_delay_ms": 5000,
  "board_size": "8",
  "camera_pan_limit": "8",
  "fog_of_war_radius": null,
  "respawn_cooldown_ms": 0,
  "npc_limits": [],
  "shop_counts": [],
  "fixed_shops": [
    { "shop_id": "bullet_pawn_northbound_step_shop", "position": [-4, 1] }
  ],
  "kits": [
    {
      "name": "Classic",
      "description": "Standard chess pieces",
      "pieces": ["bullet_king", "bullet_queen"]
    }
  ],
  "queue_layout": {
    "players": [
      {
        "board_rotation_deg": 0,
        "pieces": [
          { "piece_id": "bullet_king", "position": [0, 3] }
        ]
      },
      {
        "board_rotation_deg": 180,
        "pieces": [
          { "piece_id": "bullet_king", "position": [0, -4] }
        ]
      }
    ]
  },
  "hooks": [
    {
      "trigger": "OnCapturePieceActive",
      "target_piece_id": "bullet_king",
      "action": "WinCapturer",
      "victory_focus": "CaptureSquare"
    }
  ]
}
```
