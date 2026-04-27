# Shop Configuration

Shop configuration files define the items available for purchase, pricing formulas, and appearance of in-game shop structures.

**Path:** `config/shops/`
**Naming:** `<shop_id>.jsonc`

The `shop_id` is automatically derived from the filename stem (e.g., `spawn_shop.jsonc` becomes `spawn_shop`).

### Shop Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `display_name` | `String` | The name of the shop displayed in the UI. | No | - |
| `default_uses` | `u32` | How many times the shop can be used by any player before it vanishes. | No | - |
| `color` | `String` | Any valid CSS color string (e.g. `#3b82f6`, `rgba(59,130,246,0.2)`). | Yes | - |
| `auto_upgrade_single_item` | `bool` | If `true`, automatically buys the only item when exactly one item applies to the piece on the shop. | Yes | `false` |
| `groups` | `Array<ShopGroup>` | Specific item groups that only apply to certain piece types. | Yes | `[]` |
| `default_group` | `ShopGroup \| null` | Optional fallback group; if `null`, pieces outside configured groups see no shop menu. | Yes | `null` |

### ShopGroup Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `applies_to` | `Array<String>` | List of `piece_id`s that this group applies to. | Yes | `[]` (if in `default_group`, it applies to all) |
| `items` | `Array<ShopItem>` | The list of items available in this group. | No | - |

### ShopItem Attributes

| Attribute | Type | Description | Optional | Default |
|-----------|------|-------------|----------|---------|
| `display_name` | `String` | The name of the item. | No | - |
| `price_expr` | `String` | A mathematical expression for the price. Can use variables (see below). | No | - |
| `replace_with` | `String (piece_id)` | If set, the current piece at the shop is replaced with this type. | Yes | `null` |
| `add_pieces` | `Array<String>` | List of `piece_id`s to add to the player's collection. | Yes | `[]` |

### Pricing Expressions

The `price_expr` is evaluated using a simple math expression parser. Available variables:
- `player_piece_count`: Total number of pieces owned by the player.
- `<piece_id>_count`: Number of pieces of a specific type owned by the player (e.g., `pawn_count`).

### Example (`spawn_shop.jsonc`)

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
