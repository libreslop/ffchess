# Piece Configuration

Piece configuration files live in `config/pieces/` and define how one piece type moves, captures,
scores, and renders.

**Path pattern:** `config/pieces/<piece_id>.jsonc`

The filename stem becomes the runtime `PieceConfig.id`. For example:

- `config/pieces/queen.jsonc` -> `id = "queen"`
- `config/pieces/blitz_pawn_northbound_not_moved.jsonc` -> `id = "blitz_pawn_northbound_not_moved"`

## Schema

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `display_name` | string | Yes | UI-facing name. |
| `svg_path` | string | Yes | Asset filename under `assets/pieces/`. The runtime prepends `/assets/pieces/` when loading it in the client. |
| `score_value` | integer (`u64`) | Yes | Score awarded to the capturer when this piece is removed. |
| `cooldown_ms` | integer (`i64`) | Yes | Move cooldown applied after a successful move. Also used for NPC move cadence for that piece type. |
| `move_paths` | path[] | Yes | Legal non-capture movement paths. |
| `capture_paths` | path[] | Yes | Legal capture movement paths. There is no fallback to `move_paths` when this array is empty. |
| `special_moves` | special move[] | No | Moves beyond the paths, such as castling. See [Special Moves](#special-moves). |
| `destination_requirements` | requirement[] | No | Conditions every move of this piece must meet on the board as it would be after the move. See [Destination Requirements](#destination-requirements). |
| `shading` | shading rule[] | No | Visual cues drawn on pieces of this type. See [Shading](#shading). |

### Path Encoding

A **path** is an array of coordinate steps, and each step is serialized as `[x, y]`.

Examples:

- `[[1, 2]]`: a single knight jump.
- `[[0, 1], [0, 2], [0, 3]]`: one ray that may stop at 1, 2, or 3 tiles along the same file.
- `[[0, -1], [0, -2]]`: a two-step forward pawn path used by the "not moved yet" blitz and rapid pawns.

### How Paths Are Interpreted

The validator `PieceConfig::check_move` in `common/src/rules/movement.rs` works like this:

1. Pick `move_paths` for quiet moves or `capture_paths` for captures.
2. Compute the displacement from the start square to the target square.
3. For each path, compare the target displacement to every step in that path.
4. If the target matches step `n`, every earlier step in that same path must be empty.
   When several paths reach the target, one clear path is enough; when all are blocked the
   move is rejected with `PathBlocked`.

That means long sliding pieces are represented as one path per direction, not one path per final square.

## Special Moves

A special move is chosen by its `type` field. The only type today is `partneredmove`: the mover
lands on `target` while any number of **partner** pieces act alongside it. Castling, en passant,
and swapping places with an ally are all partnered moves.

All offsets (`target`, `at`, `move_to`) are relative to the mover's square before the
move, in the same coordinates as paths.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `type` | `"partneredmove"` | Yes | Selects the kind of special move. |
| `target` | `[x, y]` | Yes | Where the mover lands. Selecting this square triggers the move. |
| `condition` | condition | No | Must hold for the mover. Sees the `self_*` variables. |
| `requirements` | requirement[] | No | Conditions on board squares, each `{ "at": [x, y], "condition": "..." }`. Sees the `self_*` and `square_*` variables. |
| `becomes` | piece id | No | Piece type the mover turns into. |
| `partners` | partner[] | No | Pieces that must be present, and what happens to them. |

Each partner:

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `at` | `[x, y]` | Yes | Square the partner stands on. |
| `owner` | `"self"`, `"enemy"`, `"npc"`, `"any"` | Yes | Whose piece it must be, relative to the mover. `enemy` means any piece the mover could capture. |
| `types` | piece id[] | No | Accepted piece types; any type when empty. |
| `condition` | condition | No | Must hold for this partner. Sees the `self_*` and `partner_*` variables. |
| `action` | action | No | `{ "move_to": [x, y] }`, `"capture"`, or `{ "becomes": "<piece id>" }`. The partner stays untouched when omitted, which makes it a pure requirement. |

The move is legal only when the mover's condition holds, every requirement holds, and every
partner is present, distinct, and satisfies its owner, types, and condition. Every condition is
evaluated on the board as it is before the move. All pieces then move
at the same instant: a piece may land on a square another piece of the same move leaves, but two
pieces may never land on the same square, and every destination must be on the board.

Other rules:

- path moves win: a special move is only tried when no `move_paths` / `capture_paths` entry
  reaches the target,
- a partner moved by `move_to` counts a move (`partner_moves`), but only the mover's cooldown
  restarts: castling is the king's move, so the rook stays ready,
- a captured partner credits the mover's owner, like a normal capture,
- the server cancels queued premoves of every partner the move changed, since they were planned
  from where the partner stood before,
- a special move that affects other pieces is hinted with a capture frame around those pieces
  rather than a dot on the mover's target, and the player taps one of them to make it (tap the
  rook to castle); a special move that affects no other piece is hinted and chosen on its target,
- with a piece selected, tapping one of your own pieces that selects one of the selected piece's
  moves makes that move instead of selecting the tapped piece.

### Conditions

Conditions are [evalexpr](https://docs.rs/evalexpr) expressions that must produce `true` or
`false`, for example `self_moves == 0 && !self_targeted`. Variables:

| Variable | Available in | Meaning |
| --- | --- | --- |
| `self_moves` | all | Moves the mover has made since it was placed. |
| `self_targeted` | all | Whether an enemy of the mover could capture it. |
| `partner_moves` | partner conditions | Moves the partner has made since it was placed. |
| `partner_targeted` | partner conditions | Whether an enemy of the partner could capture it. |
| `square_empty` | requirements | Whether no piece stands on the square. |
| `square_targeted` | requirements | Whether an enemy of the mover could capture onto the square. |

A piece or square is **targeted** when a piece of another owner could reach it along one of its
`capture_paths` over empty squares. Cooldowns are ignored, the square does not have to be
occupied, special moves do not count, and NPCs never target each other. It is the closest this
game has to "check".

The server checks every condition (including destination requirements and shading rules) when
it loads the config and refuses to start on a malformed condition, a misspelled variable, or one that does not produce a boolean. It also refuses unknown
piece ids in `types`, `becomes`, and `action`.

### Example: Castling King

This is the castling of the blitz and rapid kings. Unlike standard chess, the king may cross an
attacked square; adding `&& !square_targeted` to the `[1, 0]` and `[-1, 0]` requirements
forbids that too.

```jsonc
{
  "display_name": "King",
  "svg_path": "king.svg",
  "score_value": 500,
  "cooldown_ms": 1000,
  "move_paths": [[[1, 0]], [[-1, 0]], [[0, 1]], [[0, -1]], [[1, 1]], [[1, -1]], [[-1, 1]], [[-1, -1]]],
  "capture_paths": [[[1, 0]], [[-1, 0]], [[0, 1]], [[0, -1]], [[1, 1]], [[1, -1]], [[-1, 1]], [[-1, -1]]],
  "special_moves": [
    {
      // Short castle: the king steps two squares right, the rook jumps over it.
      "type": "partneredmove",
      "target": [2, 0],
      "condition": "self_moves == 0 && !self_targeted",
      // The king may not land on an attacked square. The square it crosses is where the rook
      // lands, so it only has to be empty.
      "requirements": [
        { "at": [1, 0], "condition": "square_empty" },
        { "at": [2, 0], "condition": "square_empty && !square_targeted" }
      ],
      "partners": [{
        "at": [3, 0],
        "owner": "self",
        "types": ["rook"],
        "condition": "partner_moves == 0",
        "action": { "move_to": [1, 0] }
      }]
    },
    {
      // Long castle.
      "type": "partneredmove",
      "target": [-2, 0],
      "condition": "self_moves == 0 && !self_targeted",
      "requirements": [
        { "at": [-1, 0], "condition": "square_empty" },
        { "at": [-2, 0], "condition": "square_empty && !square_targeted" },
        // Only the rook crosses this square, so it may be attacked.
        { "at": [-3, 0], "condition": "square_empty" }
      ],
      "partners": [{
        "at": [-4, 0],
        "owner": "self",
        "types": ["rook"],
        "condition": "partner_moves == 0",
        "action": { "move_to": [-1, 0] }
      }]
    }
  ]
}
```

Because conditions see the board before the move, a square behind the king on an attacking ray
is not treated as targeted while the king itself still blocks that ray.

## Destination Requirements

`destination_requirements` lists requirements that every move of the piece must meet, path moves
and special moves alike. They are evaluated on the board **as it would be after
the move**: captured pieces are gone and every moved piece stands on its new square. A move
breaking one is rejected with `DestinationRestricted` and is never offered as a legal move.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `condition` | condition | Yes | The [condition](#conditions) the move must meet. |
| `ignore_for_premove` | bool | No | Defaults to `false`. When `true`, clients skip the requirement when predicting queued moves, so a queued chain is not shown as cancelled while a threat it would pass through comes and goes. The server still checks it when the move runs, and cancels the move and the rest of its chain if it fails then. |

Conditions see these variables:

| Variable | Meaning |
| --- | --- |
| `self_moves` | Moves the mover had made before this move. |
| `move_capture` | Whether the move captures a piece. |
| `square_targeted` | Whether a piece of another owner could capture onto the square the mover lands on, after the move. |

Every king in this repository uses it to forbid moving into check:

```jsonc
"destination_requirements": [
    { "condition": "!square_targeted", "ignore_for_premove": true }
]
```

Because the board after the move is used, a king cannot step back along the ray of an attacking
rook (the king no longer blocks the ray) or capture a defended piece.

## Shading

`shading` lists rules that draw visual cues on pieces of this type. The client resolves them
against the authoritative board for the local player on every board change.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `condition` | condition | No | Must hold for the piece. Sees the `self_*` variables. Always holds when omitted. |
| `visible_to` | condition | No | Must hold for the viewer. Sees the `self_*` and `viewer_*` variables. Everyone sees the rule when omitted. |
| `effects` | effect[] | Yes | What is drawn. |

Effects, chosen by their `type` field:

| `type` | Fields | Draws |
| --- | --- | --- |
| `background` | `color`, `opacity` (percent, default `35`) | Fills the piece's square behind the piece. |
| `line_towards` | `towards` (`"attackers"` or `"targets"`), `filter` (condition, optional), `color`, `opacity` (percent, default `100`) | A short line segment on the rim of the piece's square pointing straight at each related piece. `attackers` are the pieces that could capture this piece (its threat sources); `targets` are the pieces it could capture. `filter` sees the `self_*`, `viewer_*`, and `other_*` variables and keeps only matching related pieces. |

A `color` is `"owner"` (team color of the shaded piece's owner), `"viewer"` (the local player's
color), `"other"` (the related piece's owner, for `line_towards`), `{ "most_common": "attackers" }`
or `{ "most_common": "targets" }` (the team color appearing most among those pieces' owners, ties
broken consistently; the owner's color when there are none), or a literal `"#rrggbb"`. NPCs use
the neutral NPC color.

Threats follow the same rule as `targeted`: a piece of another owner on one of the piece's
`capture_paths` with every earlier square empty. Cooldowns are ignored and NPCs never threaten
each other.

| Variable | Available in | Meaning |
| --- | --- | --- |
| `self_is_king` | all | Whether the piece is a king. |
| `self_is_npc` | all | Whether no player owns the piece. |
| `self_moves` | all | Moves the piece has made since it was placed. |
| `self_targeted` | all | Whether any piece could capture the piece. |
| `self_attackers` | all | How many pieces could capture the piece. |
| `self_targets` | all | How many pieces the piece could capture. |
| `self_targets_king` | all | Whether the piece could capture any king. |
| `viewer_is_player` | `visible_to`, `filter` | Whether the viewer is a player in the game (not a spectator). |
| `viewer_is_owner` | `visible_to`, `filter` | Whether the viewer owns the piece. |
| `viewer_is_opponent` | `visible_to`, `filter` | Whether the viewer is a player who does not own the piece. |
| `viewer_attackers` | `visible_to`, `filter` | How many of the viewer's pieces could capture the piece. |
| `viewer_targeted` | `visible_to`, `filter` | How many of the viewer's pieces the piece could capture. |
| `viewer_king_targeted` | `visible_to`, `filter` | Whether the piece could capture the viewer's king. |
| `other_is_king` | `filter` | Whether the related piece is a king. |
| `other_is_npc` | `filter` | Whether no player owns the related piece. |
| `other_is_viewers` | `filter` | Whether the viewer owns the related piece. |
| `other_moves` | `filter` | Moves the related piece has made. |

Every piece in this repository is filled in its team color for the players whose king it
threatens. While in check, every king shows its owner its square filled in the most common
attacker team color, and a line towards each attacker:

```jsonc
"shading": [
    {
        // Seen only by the player whose king this piece threatens.
        "visible_to": "viewer_king_targeted",
        "effects": [{ "type": "background", "color": "owner" }]
    },
    {
        // Kings only: while in check, the owner sees marks towards the attackers.
        "condition": "self_targeted",
        "visible_to": "viewer_is_owner",
        "effects": [
            { "type": "background", "color": { "most_common": "attackers" } },
            { "type": "line_towards", "towards": "attackers", "color": "other" }
        ]
    }
]
```

Other cues are a config change, for example `"condition": "self_targeted", "visible_to":
"viewer_is_owner"` with a `background` effect to show players which of their pieces hang.

The server validates every shading condition and destination requirement when it loads the
config, like special move conditions.

## Naming Conventions That Matter

`PieceTypeId::is_king()` is name-based. A piece is treated as a king if its id:

- equals `"king"`, or
- ends with `"_king"`

That affects:

- player spawn validation (`kits` and `queue_layout` must include a king),
- kill tracking,
- hook targeting in common configs such as `target_piece_id = "king"` or `"blitz_king"`.

## Practical Patterns In This Repository

- `king`, `knight`, and `pawn` are direct-move pieces with short paths.
- `bishop`, `rook`, and `queen` use one long path per direction for sliding.
- Blitz and rapid pawns encode state transitions as different piece ids:
  - `blitz_pawn_*_not_moved` / `rapid_pawn_*_not_moved`
  - `blitz_pawn_*_moved` / `rapid_pawn_*_moved`
- Blitz and rapid promotion is not built into piece configs; it is implemented through shops.
- Blitz and rapid kings castle through `special_moves` (see `config/pieces/blitz_king.jsonc`).

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

## Example: Blitz Pawn Before Its First Move

```jsonc
{
  "display_name": "Blitz Pawn (Northbound, Not Moved)",
  "svg_path": "pawn.svg",
  "score_value": 10,
  "cooldown_ms": 3000,

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
