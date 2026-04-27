# ffchess

`ffchess` is a real-time, data-driven multiplayer chess sandbox built as a Rust workspace.
Players control whole armies instead of a single piece, pieces move on independent cooldowns,
NPCs wander the board for score, shops upgrade armies in place, and queue-based modes can spin
up isolated private matches from shared public lobbies.

## What It Includes

- Real-time movement instead of alternating turns.
- Config-defined pieces, shops, kits, hooks, and modes.
- Room-scoped chat for both public mode lobbies and private queue matches.
- Public sandbox modes plus queue-based private match modes.
- A WebSocket server, a Yew/WebAssembly client, and a shared `common` crate.
- Client-side premove prediction backed by server-side queued move execution.

## Workspace Layout

- `common/`: shared domain models, protocol types, strong primitive wrappers, and rule helpers.
- `server/`: Axum server, live game instances, matchmaking, previews, hooks, NPCs, and config loading.
- `client/`: Yew application, canvas renderer, camera/input logic, and UI overlays.
- `config/`: JSONC content that defines runtime behavior.
- `docs/`: configuration reference and logic chapters for the project.

## Running Locally

### Prerequisites

- Rust stable with `cargo`
- `trunk` for the WebAssembly client

### Development Flow

1. Run the test suite:

   ```bash
   cargo test
   ```

2. Build the client bundle:

   ```bash
   cd client
   NO_COLOR=true trunk build --release
   cd ..
   ```

3. Build or run the server:

   ```bash
   cargo build --release -p server
   cargo run --release -p server
   ```

The server serves the built client from `client/dist/` and static assets from `assets/`.
By default it listens on `http://localhost:8080`. Set `PORT` to override that.

## Configuration Overview

The runtime is intentionally driven by files under `config/`:

- `config/global/`: server and client globals.
- `config/pieces/`: movement rules, cooldowns, score values, and SVG asset names.
- `config/shops/`: recruit/upgrade behavior and price formulas.
- `config/modes/`: board sizing, NPC caps, kits, hooks, queue behavior, and layouts.

Detailed field-by-field reference lives in [docs/README.md](docs/README.md).

## Architecture Notes

- The server owns the authoritative `GameState` for each `GameInstance`.
- The client renders a predicted view by applying local premoves on top of the latest server snapshot.
- Queue modes keep a public preview board alive while private match instances are created on demand.
- Queue matches can publish a pre-move countdown so players see the full starting position before moves unlock.
- Chat follows the currently viewed room, with mode-room chat in lobbies and game-room chat inside private matches.
- Hook handling is tick-buffered so captures and leave events resolve in a predictable order.

## Documentation

Start with [docs/README.md](docs/README.md) for the full
documentation index.
