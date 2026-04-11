# Global Configuration

Global configuration files define system-wide settings for both the server and the client.

## Server Global Configuration

**Path:** `config/global/server.jsonc`

This file contains settings that affect the overall server behavior, such as the sync interval and the pool of names used for players.

### Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `sync_interval_ms` | `u32` | The interval in milliseconds at which the server synchronizes state with clients. | Yes | `10000` |
| `default_name` | `Object` | Configuration for generating default player names. | Yes | `{}` |
| `default_name.adjectives` | `Array<String>` | A list of adjectives to use for name generation. | Yes | `[]` |
| `default_name.nouns` | `Array<String>` | A list of nouns to use for name generation. | Yes | `[]` |

### Example

```jsonc
{
  "sync_interval_ms": 10000,
  "default_name": {
    "adjectives": ["Swift", "Brave", "Silent"],
    "nouns": ["Knight", "King", "Rook"]
  }
}
```

---

## Client Global Configuration

**Path:** `config/global/client.jsonc`

This file contains settings for the client-side application, including UI timing, camera behavior, and rendering parameters.

### Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `game_order` | `Array<String>` | The preferred order of game modes in the UI. | Yes | `[]` |
| `modes_refresh_ms` | `u32` | Interval for refreshing the list of available game modes. | Yes | `5000` |
| `ping_interval_ms` | `u32` | Interval for sending ping messages to the server. | Yes | `2000` |
| `tick_interval_ms` | `u32` | The logic tick interval for the client. | Yes | `50` |
| `render_interval_ms` | `u32` | The target rendering interval (e.g., 16ms for 60fps). | Yes | `16` |
| `disconnected_hide_ms` | `u32` | Delay before hiding the disconnected screen after reconnecting. | Yes | `300` |
| `fatal_auto_hide_ms` | `u32` | Time after which fatal error notifications automatically hide. | Yes | `5000` |
| `camera_zoom_min` | `f64` | Minimum allowed camera zoom level. | Yes | `0.2` |
| `camera_zoom_max` | `f64` | Maximum allowed camera zoom level. | Yes | `2.0` |
| `zoom_lerp` | `f64` | Interpolation factor for smooth zooming. | Yes | `0.15` |
| `inertia_decay` | `f64` | Decay factor for camera movement inertia. | Yes | `0.94` |
| `velocity_cutoff` | `f64` | Velocity threshold below which camera movement stops. | Yes | `0.1` |
| `pan_lerp_alive` | `f64` | Pan interpolation factor when the player is alive. | Yes | `0.15` |
| `pan_lerp_dead` | `f64` | Pan interpolation factor when the player is dead/spectating. | Yes | `0.08` |
| `tile_size_px` | `f64` | Visual size of a single board tile in pixels. | Yes | `40.0` |
| `death_zoom` | `f64` | Target zoom level when the player dies. | Yes | `1.3` |
| `scroll_zoom_base` | `f64` | Base factor for scroll-based zooming. | Yes | `1.2` |

### Example

```jsonc
{
  "game_order": ["ffa", "arena", "duel"],
  "modes_refresh_ms": 5000,
  "ping_interval_ms": 2000,
  "camera_zoom_min": 0.2,
  "camera_zoom_max": 2.0,
  "tile_size_px": 40.0
}
```
