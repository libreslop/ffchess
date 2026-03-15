# FFchess (MMO battle chess) Project Overview

This document summarizes the current architecture, core mechanics, and implementation details for the `ffchess-server` workspace.

## 1. Project Structure
- `common/`: Shared data models, protocol messages, typed identifiers, and core logic.
- `server/`: Axum WebSocket server with per-mode `GameInstance`s, NPC logic, and configuration loading.
- `client/`: Yew WebAssembly frontend that renders the canvas, handles input, and runs the reducer.
- `config/`: JSONC configuration for pieces, shops, modes, and global defaults.
- `docs/`: Design and refactor notes.

## 2. Core Mechanics
- Board size is computed per mode using `ExprString` expressions (typically based on `player_count`).
- Movement and capture rules are defined as path lists in piece configs and validated via shared logic.
- Cooldowns are per-piece values from config; the client predicts cooldown locally and the server validates.
- Capturing removes pieces immediately. Capturing a King eliminates the owning player and all their pieces.
- Shops allow recruiting or upgrading based on per-piece shop groups and expression-based pricing.
- Kits define starting armies per mode.
- NPC spawn caps are expression-driven; NPC spawning and movement are paused if no players are viewing for ~5 seconds.
- Board contraction happens only when player pieces are inside the new bounds; otherwise size reduction is deferred.

## 3. Technical Implementation

### Shared (`common/`)
- **Typed semantics:** `Score`, `BoardSize`, `DurationMs`, `TimestampMs`, `ExprString`, and strongly typed IDs (`ModeId`, `PieceId`, `ShopId`, etc.).
- **Models:** `GameModeConfig` (server), `GameModeClientConfig` (client-safe), and `ModeSummary` (lobby list).
- **Logic:** `evaluate_expression`, `calculate_board_size`, `is_valid_move` (with `MoveValidationParams`), `calculate_cooldown`, and shop helpers (`select_shop_group`, `build_price_vars`).
- **Protocol:** `ClientMessage` and `ServerMessage` with typed fields for score and board size.

### Server (`server/`)
- **State:** `ServerState` owns `GameInstance`s keyed by `ModeId`.
- **Game instances:** Each `GameInstance` holds its `GameState`, config snapshots, channels, and utility managers.
- **Channels:** `player_channels` for bound sessions plus `connection_channels` for unbound viewers.
- **Spawning:** Dedicated helpers in `server/src/spawning.rs` (adjacent and random nearby placement).
- **Client assets:** `server/src/paths.rs` resolves `client/dist` for static serving.
- **Global config injection:** `index.html` is served with `ModeSummary` and global client config JSON embedded.

### Client (`client/`)
- **App module:** `client/src/app` splits global config loading, WebSocket connection, and the main `App` component.
- **Reducer:** Centralized `GameStateReducer` handles init, updates, premoves, and UI state.
- **Canvas:** Renderer structs with typed parameter objects for drawing; GameView split into `component.rs` and `helpers.rs`.
- **Premoves:** Client queues pending moves and reconciles with server updates.

## 4. Security & Performance
- Session secrets prevent player ID hijacking on rejoin.
- Server trims player names and validates moves/cooldowns.
- Full-board state is still broadcast to all clients (server-side fog-of-war is a planned improvement).
- Channels are currently unbounded; switching to bounded channels is recommended for backpressure safety.

## 5. Build & Test
- `cargo test --workspace`
- `cargo check --workspace`
- `cargo clippy -p server`
- `cargo clippy -p client`

## 6. Key Files
- `common/src/logic.rs`: Move validation, expression evaluation, and shop helpers.
- `common/src/types.rs`: Typed wrappers and identifiers.
- `server/src/instance/`: Core game loop, move handling, and NPC ticks.
- `server/src/handlers.rs`: WebSocket entry and mode list endpoints.
- `client/src/reducer/reducer_impl.rs`: Client-side state transitions.
- `common/src/protocol.rs`: Message schema between client and server.
