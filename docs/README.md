# ffchess Documentation

This directory documents the runtime configuration and the major logic flows in `ffchess`.
The documents below are written from the current code and config files in this repository.

## Configuration Reference

- [config/global.md](config/global.md): global client and server settings.
- [config/pieces.md](config/pieces.md): piece schema, movement path encoding, and piece-family examples.
- [config/shops.md](config/shops.md): shop groups, pricing, auto-upgrades, and purchase effects.
- [config/modes.md](config/modes.md): mode schema, queue behavior, layouts, hooks, and expression contexts.

## Logic Chapters

- [logic/game_loop.md](logic/game_loop.md): the per-instance server tick and maintenance flow.
- [logic/movement.md](logic/movement.md): validation, cooldowns, client premoves, and server queued moves.
- [logic/combat.md](logic/combat.md): captures, rewards, eliminations, and victory hooks.
- [logic/npcs.md](logic/npcs.md): spawn rules and autonomous NPC movement.
- [logic/world.md](logic/world.md): board sizing, spawn heuristics, shop spawning, and pruning.
- [logic/matchmaking.md](logic/matchmaking.md): queue modes, preview boards, private matches, and connection binding.
- [logic/chat.md](logic/chat.md): room selection, sender identity, retention, fade rules, and client rendering behavior.

## Reading Order

If you are new to the codebase, a practical order is:

1. Read [README.md](../README.md).
2. Read the four config reference files to understand the data model.
3. Read `game_loop`, then `movement`, `combat`, `world`, `npcs`, `matchmaking`, and `chat`.
