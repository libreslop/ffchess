# Global Configuration

Global configuration lives under `config/global/` and is split into two fixed files:

- `config/global/server.jsonc`
- `config/global/client.jsonc`

Unlike modes, pieces, and shops, these files do not derive an `id` from the filename. Their
paths are fixed and the server/client loaders expect exactly those locations.

## Server Global File

**Path:** `config/global/server.jsonc`

This file controls server-wide behavior that is not specific to one mode instance.

### Fields

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `sync_interval_ms` | integer (`u32`) | No | `10000` | Included in the `ServerMessage::Init` payload so the client knows the intended sync cadence. |
| `default_name` | object | No | `{}` | Source pool for generated player names when the join request name is blank. |
| `default_name.adjectives` | string[] | No | `[]` | Random first half of an auto-generated name. |
| `default_name.nouns` | string[] | No | `[]` | Random second half of an auto-generated name. |

### Runtime Notes

- Blank names are normalized server-side. The server picks one adjective and one noun.
- If a generated adjective and noun are identical, the server tries to swap the noun once.
- Missing arrays fall back to `"Unnamed"` and `"Player"` at runtime.

### Example

```jsonc
{
  "sync_interval_ms": 10000,
  "default_name": {
    // Used when a player submits an empty name.
    "adjectives": ["Swift", "Frosty", "Hidden"],
    "nouns": ["Knight", "Raven", "Sentinel"]
  }
}
```

## Client Global File

**Path:** `config/global/client.jsonc`

This file is injected into the landing page and deserialized by the Yew client at startup.

### Fields

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `game_order` | mode id[] | No | `[]` | Preferred ordering for the mode list. Unknown ids are ignored by normal sorting behavior. |
| `modes_refresh_ms` | integer (`u32`) | No | `5000` | Poll interval for `/api/modes`. The client clamps this to at least `500`. |
| `ping_interval_ms` | integer (`u32`) | No | `2000` | WebSocket ping cadence. The client clamps this to at least `500`. |
| `tick_interval_ms` | integer (`u32`) | No | `50` | Loaded into config, but currently unused by the runtime. |
| `render_interval_ms` | integer (`u32`) | No | `16` | Interval used by the render timer that drives camera updates and FPS reporting. |
| `disconnected_hide_ms` | integer (`u32`) | No | `300` | Loaded into config, but currently unused. The disconnect overlay hide delay is hard-coded separately. |
| `fatal_auto_hide_ms` | integer (`u32`) | No | `5000` | Loaded into config, but currently unused. Fatal reset timing is hard-coded separately. |
| `camera_zoom_min` | float | No | `0.2` | Minimum allowed zoom after wheel and pinch input. |
| `camera_zoom_max` | float | No | `2.0` | Maximum allowed zoom after wheel and pinch input. |
| `zoom_lerp` | float | No | `0.15` | Smoothing factor used when interpolating toward `target_zoom`. |
| `inertia_decay` | float | No | `0.94` | Per-frame decay applied to camera velocity after panning stops. |
| `velocity_cutoff` | float | No | `0.1` | Velocity threshold below which inertial camera panning stops. |
| `pan_lerp_alive` | float | No | `0.15` | Camera interpolation speed while the local player is alive. |
| `pan_lerp_dead` | float | No | `0.08` | Camera interpolation speed in menu/dead/victory states. |
| `tile_size_px` | float | No | `40.0` | Base world-tile size before zoom is applied. |
| `death_zoom` | float | No | `1.3` | Target zoom when the death or victory camera takes over. |
| `scroll_zoom_base` | float | No | `1.2` | Exponential base used for wheel zoom scaling. Values above `1.0` make zoom responsive without going unstable. |

### Runtime Notes

- `game_order` should contain mode ids from `config/modes/*.jsonc`.
- The client still works if this file is missing; it falls back to hard-coded defaults.
- The document is injected by `server/src/handlers/http.rs` into the `index.html` template.

### Example

```jsonc
{
  "game_order": ["ffa", "arena", "bullet", "duel"],
  "modes_refresh_ms": 5000,
  "ping_interval_ms": 2000,
  "render_interval_ms": 16,

  // Camera tuning
  "camera_zoom_min": 0.2,
  "camera_zoom_max": 2.0,
  "zoom_lerp": 0.15,
  "inertia_decay": 0.94,
  "velocity_cutoff": 0.1,
  "pan_lerp_alive": 0.15,
  "pan_lerp_dead": 0.08,
  "tile_size_px": 40.0,
  "death_zoom": 1.3,
  "scroll_zoom_base": 1.2
}
```
