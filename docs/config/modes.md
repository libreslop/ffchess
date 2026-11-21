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
| `queue_countdown_ms` | integer (`i64`) | No | `0` | Match-start countdown for queue modes. Players and pieces are spawned immediately, overlays are removed, and premoves are allowed, but actual movement waits until this timer expires. |
| `board_size` | expression string | Yes | none | Server-side expression evaluated with `player_count`. |
| `camera_pan_limit` | expression string | Yes | none | Client-side expression evaluated with `player_piece_count` and `fog_of_war_radius`. |
| `fog_of_war_radius` | expression string or `null` | No | `null` | Client-side expression evaluated with `player_piece_count`. `null` disables fog of war. |
| `show_scoreboard` | boolean | No | `true` | Shows or hides the in-game leaderboard overlay. |
| `starting_camera` | `StartingCamera` | No | `{ "position": { "piece_id": "king" }, "zoom": "1" }` | Camera pose when entering `Alive` (first spawn/rejoin), and when starting to watch a match or replay from the menu. |
| `disable_camera_movement` | boolean | No | `false` | Disables player-controlled camera drag and zoom input. Automatic camera focus and scripted panning still run. |
| `respawn_cooldown_ms` | integer (`i64`) | Yes | none | Time after elimination before the same stored player id may respawn. |
| `allow_replays` | boolean | No | `false` | Records each private match so its players and spectators can watch it back after it finishes. Replays are kept for 24 hours. Only queue modes have matches that finish, so it has no effect on public modes. See [spectating.md](../logic/spectating.md#replays). |
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

### `StartingCamera`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `position` | `StartingCameraPosition` | No | `{ "piece_id": "king" }` | Point the camera centres on. |
| `zoom` | expression string | No | `"1"` | Client-side zoom expression evaluated with `max_fit_zoom`. |
| `reset_on_resize` | boolean | No | `false` | Whenever the screen is resized, puts the camera back in the starting pose with the zoom worked out for the new size, even if the player moved it. Stops once a hook moves the camera or the player leaves. |

`StartingCameraPosition` is one of:

| Shape | Example | Notes |
| --- | --- | --- |
| Piece target | `{ "piece_id": "king" }` | Centres on the local player's first owned piece with this type id, or on the king when the player owns none. Spectators centre on the board centre. |
| Fixed point | `[0, 0]` | Centres on a board-space point `[x, y]` (`f64`) measured in tile units from the board origin. `[0, 0]` is the board origin, and square centers are half-integers such as `[0.5, 0.5]`. |

`max_fit_zoom` is the zoom at which the whole board just fits in the part of the screen the
board is shown in, so `"0.8 * max_fit_zoom"` shows the whole board with a margin on any screen.
The replay viewer counts only the room beside its analysis panel. The camera cuts to the
starting pose with a board switch (see `board_switch_regions` in the client globals); while the
switch runs, the pose keeps fitting the screen as the view lays out, and afterwards only
`reset_on_resize` refits it. Zooms are kept within the
`camera_zoom_min` / `camera_zoom_max` range of the client globals, widened to reach
`max_fit_zoom`.

### `SetCamera`

Camera move a hook makes when a piece dies. Both fields are optional; a field left out keeps
that part of the camera as it is, and a hook without `set_camera` does not move the camera.

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `position` | expression string | No | `null` | Point to centre on, as an `(x, y)` tuple in tile units from the board origin. |
| `zoom` | expression string | No | `null` | Zoom to move to. |

Both expressions are evaluated on the client with:

| Variable | Value |
| --- | --- |
| `death_position` | Centre of the square the piece died on, as an `(x, y)` tuple. |
| `death_x`, `death_y` | The components of `death_position`, for arithmetic such as `"(death_x, death_y - 2)"`; evalexpr cannot do arithmetic on tuples. |
| `max_fit_zoom` | As for `StartingCamera`. |

### `Hook`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `trigger` | string enum | Yes | none | One of `OnCapture`, `OnCapturePieceActive`, `OnPlayerLeave`, `OnPlayerJoin`, `OnPlayerDisconnect`, `OnPlayerKilled`, `OnQueueCountdown`, `OnGameStart`, `OnSpectatorJoin`, `OnSpectatorLeave`. |
| `target_piece_id` | piece id or `null` | No | `null` | Piece filter for capture hooks. `null` means "all piece types". |
| `players_left` | integer (`u32`) or `null` | No | `null` | Only used by `OnPlayerLeave` / `WinRemaining`. |
| `action` | string enum | Yes | none | One of `EliminateOwner`, `WinCapturer`, `WinRemaining`, `SystemChatMessage`. |
| `capture` | boolean | No | `false` | If this hook matches, no later hooks in the mode list are evaluated for the same event batch. |
| `title` | string or `null` | No | `null` | Optional victory title override. |
| `message` | string or `null` | No | `null` | Optional victory message override. |
| `set_camera` | `SetCamera` or `null` | No | `null` | Camera move made when the hook fires: `EliminateOwner` moves the eliminated player's camera, `WinCapturer` the winner's. Other actions ignore it. |

## Supported Hook Combinations

The loader will deserialize any `trigger` + `action` pairing, but the runtime currently only
executes these combinations:

| Trigger | Action | Runtime behavior |
| --- | --- | --- |
| `OnCapture` | `EliminateOwner` | Remove the owner of the captured piece from the match. |
| `OnCapturePieceActive` | `WinCapturer` | Send a victory payload to the capturing player. |
| `OnPlayerLeave` | `WinRemaining` | If the remaining player count matches, send a victory payload to the survivor. |
| `OnPlayerJoin` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each joined player event. |
| `OnPlayerDisconnect` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each player who left the board, by leaving or by losing the connection. |
| `OnPlayerKilled` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each king-kill event; supports `{player}` and `{killer}` variables. |
| `OnQueueCountdown` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each countdown second; supports `{seconds}` variable. |
| `OnSpectatorJoin` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each named spectator who started watching; supports `{player}`. Default message: `{player} is spectating`. |
| `OnSpectatorLeave` | `SystemChatMessage` | Broadcast one system chat line (yellow) for each named spectator who stopped watching; supports `{player}`. Default message: `{player} stopped spectating`. |

Any other pairing deserializes successfully but is ignored by hook resolution.

## Expression Contexts

Different expressions run in different places:

| Field | Variables available |
| --- | --- |
| `board_size` | `player_count` |
| `npc_limits[*].max_expr` | `player_count` |
| `fog_of_war_radius` | `player_piece_count` |
| `camera_pan_limit` | `player_piece_count`, `fog_of_war_radius` |
| `starting_camera.zoom` | `max_fit_zoom` |
| `hooks[*].set_camera.position`, `hooks[*].set_camera.zoom` | `death_position`, `death_x`, `death_y`, `max_fit_zoom` |

Expressions use [evalexpr](https://docs.rs/evalexpr) syntax. Counts such as `player_count` are
integers, so dividing two integers rounds down (`player_count / 3`); write `player_count / 3.0`
to keep the fraction. Most math functions are namespaced, for example `math::sqrt`, while
`min`, `max`, `floor`, `ceil`, and `round` are not. A malformed expression evaluates to `0`;
camera expressions are checked when the server loads the config, and a malformed one stops it.

## Queue Mode Rules

If `queue_players >= 2`, the mode behaves differently:

- players join a matchmaking queue instead of entering the public instance directly,
- the public instance is reused as a preview board,
- when the queue fills, the server spawns a private `GameInstance`,
- the private instance copies the mode config but zeroes `queue_players`,
- if `queue_countdown_ms > 0`, the private instance publishes `move_unlock_at` and the client shows a center-screen countdown before moves unlock,
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
  "queue_countdown_ms": 5000,
  "board_size": "8",
  "camera_pan_limit": "8",
  "fog_of_war_radius": null,
  "show_scoreboard": false,
  "starting_camera": { "position": [0, 0], "zoom": "0.8 * max_fit_zoom", "reset_on_resize": true },
  "disable_camera_movement": true,
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
      "trigger": "OnCapture",
      "target_piece_id": "bullet_king",
      "action": "EliminateOwner",
      // The loser's camera zooms in on the square their king fell on.
      "set_camera": { "position": "death_position", "zoom": "max_fit_zoom" }
    },
    {
      "trigger": "OnCapturePieceActive",
      "target_piece_id": "bullet_king",
      "action": "WinCapturer",
      // So does the winner's.
      "set_camera": { "position": "death_position", "zoom": "max_fit_zoom" }
    }
  ]
}
```
