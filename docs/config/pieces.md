# Piece Configuration

Piece configuration files define the properties, movement rules, and scoring for individual chess piece types.

**Path:** `config/pieces/`
**Naming:** `<piece_id>.jsonc`

The `piece_id` is automatically derived from the filename stem (e.g., `pawn.jsonc` becomes `pawn`).

### Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `display_name` | `String` | The name shown in the UI. | No | - |
| `svg_path` | `String` | Path to the SVG asset for this piece relative to `assets/pieces/`. | No | - |
| `score_value` | `u32` | Score awarded for capturing this piece. | No | - |
| `cooldown_ms` | `u32` | Cooldown period between moves in milliseconds. | No | - |
| `move_paths` | `Array<Array<IVec2>>` | List of possible movement paths. Each path is a list of steps. | No | - |
| `capture_paths` | `Array<Array<IVec2>>` | List of possible capture paths. If empty, the piece uses `move_paths` for captures. | No | - |

**IVec2** is an object with `x` and `y` integer fields, but in the JSON it's represented as a simple array `[x, y]`.

### Example (`pawn.jsonc`)

```jsonc
{
    "display_name": "Pawn",
    "svg_path": "pawn.svg",
    "score_value": 10,
    "cooldown_ms": 1000,
    // Pawns move orthogonally by 1 tile
    "move_paths": [
        [[1, 0]], [[-1, 0]], [[0, 1]], [[0, -1]]
    ],
    // Pawns capture diagonally by 1 tile
    "capture_paths": [
        [[1, 1]], [[1, -1]], [[-1, 1]], [[-1, -1]]
    ]
}
```

### Path Mechanics

-   A path is a list of relative coordinates `[x, y]`.
-   If a path has multiple steps (e.g., `[[0, 1], [0, 2]]`), the piece must move to the last step, but the intermediate steps must be empty (sliding movement).
-   If a path has only one step (e.g., `[[1, 1]]`), it's a direct jump or a single-tile move.
-   The server checks all paths to see if the target square matches any of the steps and ensures the path isn't blocked.
