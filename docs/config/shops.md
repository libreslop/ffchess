# Shop Configuration

Shop configs live in `config/shops/` and describe what a board shop can sell, who sees each item
group, and what happens when an item is purchased.

**Path pattern:** `config/shops/<shop_id>.jsonc`

The filename stem becomes the runtime `ShopConfig.id`.

## Schema

### Top-Level Shop Fields

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `display_name` | string | Yes | Label shown in the client UI. |
| `default_uses` | integer (`u32`) | Yes | Number of successful purchases before the shop depletes. When it reaches zero, the server removes the shop and respawns a fresh copy elsewhere. |
| `color` | string or `null` | No | `null` | CSS color string used when drawing the shop tile. Transparent RGBA values are valid and used in bullet mode. |
| `focus_color` | string or `null` | No | `null` | CSS color string used when highlighting the active/focused shop square. If omitted, the client falls back to `color`. |
| `auto_upgrade_single_item` | boolean | No | `false` | If `true` and the selected group contains exactly one item, landing on the shop auto-purchases that item after a move succeeds. |
| `groups` | `ShopGroup[]` | Yes | none | Piece-specific groups checked before `default_group`. |
| `default_group` | `ShopGroup` or `null` | No | `null` | Fallback group used when no specific group matches. |

### `ShopGroup`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `applies_to` | piece id[] | No | `[]` | Piece types that should use this group. For `default_group`, the loader normalizes a missing value to `[]`. |
| `items` | `ShopItem[]` | Yes | none | Purchase options shown for that group. |

### `ShopItem`

| Field | Type | Required | Default | Notes |
| --- | --- | --- | --- | --- |
| `display_name` | string | Yes | none | Button label in the shop UI. |
| `price_expr` | string or `null` | No | `null` | Pricing formula. `null` means free. |
| `replace_with` | piece id or `null` | No | `null` | If set, the piece standing on the shop is transformed into this piece type. |
| `add_pieces` | piece id[] | No | `[]` | Extra pieces spawned adjacent to the shop tile for the buyer. |

## Group Selection Rules

The shared helper `common::logic::select_shop_group` applies these rules:

1. If a piece is on the shop, scan `groups` in order and pick the first group whose `applies_to`
   contains that piece type.
2. If no explicit group matches, fall back to `default_group`.
3. If there is no piece and no `default_group`, the result is `None`.

The normal client UI only opens a shop when the local player has a piece on the shop square.

## Pricing Expression Variables

`price_expr` is evaluated with these variables:

- `player_piece_count`: total pieces owned by the buyer
- `<piece_id>_count`: per-piece ownership counts, for example `pawn_count` or `bullet_queen_count`

Values are computed server-side during purchase. The client uses the same formula inputs for its
shop UI preview.

## Purchase Effects

A successful purchase can do three things:

1. Deduct score from the buyer.
2. Replace the shop-standing piece with a different `piece_type`.
3. Spawn extra pieces on adjacent free squares.

Important runtime details:

- Added pieces must fit in one of the eight adjacent offsets checked by the server.
- If there is no adjacent space for an `add_pieces` spawn, the purchase fails with `NoSpaceNearby`.
- Replacement updates the piece's cooldown to the replacement piece's configured cooldown.
- When `default_uses` reaches zero, the shop is removed and a fresh copy respawns at a new random free location.

## Example: Standard Recruit Shop

```jsonc
{
  "display_name": "Mercenary Outpost",
  "default_uses": 3,
  "color": "#3b82f6",
  "groups": [],
  "default_group": {
    "items": [
      {
        "display_name": "Hire Pawn",
        "price_expr": "10 + player_piece_count * 2",
        "replace_with": null,
        "add_pieces": ["pawn"]
      },
      {
        "display_name": "Hire Knight",
        "price_expr": "50 + player_piece_count * 5",
        "replace_with": null,
        "add_pieces": ["knight"]
      }
    ]
  }
}
```

## Example: Bullet Promotion Shop

```jsonc
{
  "display_name": "Pawn Promotion",
  "default_uses": 9999,
  "color": "rgba(253, 224, 71, 0)",
  "focus_color": "#fde047",
  "auto_upgrade_single_item": false,
  "groups": [
    {
      "applies_to": ["bullet_pawn_northbound_moved", "bullet_pawn_southbound_moved"],
      "items": [
        { "display_name": "Queen",  "price_expr": null, "replace_with": "bullet_queen",  "add_pieces": [] },
        { "display_name": "Rook",   "price_expr": null, "replace_with": "bullet_rook",   "add_pieces": [] },
        { "display_name": "Bishop", "price_expr": null, "replace_with": "bullet_bishop", "add_pieces": [] },
        { "display_name": "Knight", "price_expr": null, "replace_with": "bullet_knight", "add_pieces": [] }
      ]
    }
  ],
  "default_group": null
}
```

## Validation Checklist

When adding a shop config, verify that:

- the filename matches the intended `shop_id`,
- every referenced `piece_id` exists,
- formulas only use the supported variables,
- `auto_upgrade_single_item` is only enabled when that behavior is intentional,
- `default_group` is `null` when you want "no valid purchase here" rather than a fallback menu.
