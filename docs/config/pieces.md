# Piece Configuration

Piece configuration files live in `config/pieces/` and define how one piece type moves, captures,
scores, and renders.

**Path pattern:** `config/pieces/<piece_id>.jsonc`

The filename stem becomes the runtime `PieceConfig.id`. For example:

- `config/pieces/queen.jsonc` -> `id = "queen"`
- `config/pieces/bullet_pawn_northbound_not_moved.jsonc` -> `id = "bullet_pawn_northbound_not_moved"`

## Schema

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `display_name` | string | Yes | UI-facing name. |
| `svg_path` | string | Yes | Asset filename under `assets/pieces/`. The runtime prepends `/assets/pieces/` when loading it in the client. |
| `score_value` | integer (`u64`) | Yes | Score awarded to the capturer when this piece is removed. |
| `cooldown_ms` | integer (`i64`) | Yes | Move cooldown applied after a successful move. Also used for NPC move cadence for that piece type. |
| `move_paths` | path[] | Yes | Legal non-capture movement paths. |
| `capture_paths` | path[] | Yes | Legal capture movement paths. There is no fallback to `move_paths` when this array is empty. |

### Path Encoding

A **path** is an array of coordinate steps, and each step is serialized as `[x, y]`.

Examples:

- `[[1, 2]]`: a single knight jump.
- `[[0, 1], [0, 2], [0, 3]]`: one ray that may stop at 1, 2, or 3 tiles along the same file.
- `[[0, -1], [0, -2]]`: a two-step forward pawn path used by the "not moved yet" bullet pawns.

### How Paths Are Interpreted

The validator in `common/src/logic.rs` works like this:

1. Pick `move_paths` for quiet moves or `capture_paths` for captures.
2. Compute the displacement from the start square to the target square.
3. For each path, compare the target displacement to every step in that path.
4. If the target matches step `n`, every earlier step in that same path must be empty.

That means long sliding pieces are represented as one path per direction, not one path per final square.

## Naming Conventions That Matter

`PieceTypeId::is_king()` is name-based. A piece is treated as a king if its id:

- equals `"king"`, or
- ends with `"_king"`

That affects:

- player spawn validation (`kits` and `queue_layout` must include a king),
- kill tracking,
- hook targeting in common configs such as `target_piece_id = "king"` or `"bullet_king"`.

## Practical Patterns In This Repository

- `king`, `knight`, and `pawn` are direct-move pieces with short paths.
- `bishop`, `rook`, and `queen` use one long path per direction for sliding.
- Bullet-mode pawns encode state transitions as different piece ids:
  - `bullet_pawn_*_not_moved`
  - `bullet_pawn_*_moved`
- Bullet-mode promotion is not built into piece configs; it is implemented through shops.

## Example: Standard Pawn

```jsonc
{
  "display_name": "Pawn",
  "svg_path": "pawn.svg",
  "score_value": 10,
  "cooldown_ms": 1000,

  // Orthogonal quiet movement
  "move_paths": [
    [[1, 0]],
    [[-1, 0]],
    [[0, 1]],
    [[0, -1]]
  ],

  // Diagonal captures only
  "capture_paths": [
    [[1, 1]],
    [[1, -1]],
    [[-1, 1]],
    [[-1, -1]]
  ]
}
```

## Example: Bullet Pawn Before Its First Move

```jsonc
{
  "display_name": "Bullet Pawn (Northbound, Not Moved)",
  "svg_path": "pawn.svg",
  "score_value": 10,
  "cooldown_ms": 1000,

  // One path, two possible landing squares. The intermediate square must be free.
  "move_paths": [
    [[0, -1], [0, -2]]
  ],

  // Captures stay single-step diagonals.
  "capture_paths": [
    [[1, -1]],
    [[-1, -1]]
  ]
}
```

## Validation Checklist

When adding a new piece config, verify that:

- the filename matches the intended `piece_id`,
- `svg_path` exists under `assets/pieces/`,
- every path uses integer coordinates,
- quiet moves and captures are both defined explicitly,
- the piece id naming is intentional if you want king semantics.
