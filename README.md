# FFchess (MMO battle chess)

Command your army in a real-time, multiplayer chess world. Scale the board, capture territory, and outmaneuver opponents in a data-driven battlefield.

## Features
- Dynamic board sizing per mode using expressions (typically keyed off `player_count`).
- Config-driven pieces, shops, and kits (JSONC in `config/`).
- Per-piece cooldowns defined by piece config; client predicts cooldown locally and server validates.
- King-centric elimination: lose your King, lose your army.
- Fog-of-war radius and camera limits driven by mode config (client-side rendering today).
- Shop economy for recruiting and upgrading pieces, with per-piece shop groups.
- Mode list and mode switching without a page reload (hash-based mode selection).
- NPCs with expression-based spawn limits; server pauses NPC ticks when no players are viewing for ~5 seconds.
- Session secrets to prevent player ID hijacking on rejoin.

## Tech Stack
- Backend: Rust, Axum, WebSockets, Tokio.
- Frontend: Rust, Yew, WebAssembly, HTML5 Canvas.
- Shared: `common` crate with types, protocol, and gameplay logic.

## How to Run

### Prerequisites
- Rust (latest stable)
- Trunk (for the frontend)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

### 1. Build the Client
```bash
cd client
trunk build
cd ..
```

### 2. Start the Server
```bash
cargo run -p server
```
The server starts on `0.0.0.0:8080` by default and serves `client/dist` (resolved via a small path helper).

Override the port:
```bash
PORT=3000 cargo run -p server
```

### 3. Development Mode (Hot Reload)
```bash
cd client
trunk serve
```
Then run the server in a separate terminal. The client runs on `localhost:8081` and proxies API requests to `localhost:8080`.

## Configuration Overview
- Global server config: `config/global/server.jsonc` (default name pool).
- Global client config: `config/global/client.jsonc` (render/heartbeat intervals, camera tuning, UI timings).
- Modes: `config/modes/*.jsonc` (board size, fog radius, respawn cooldown, kits, shop counts, NPC limits).
- Pieces: `config/pieces/*.jsonc` (move/capture paths, cooldowns, score values, glyphs).
- Shops: `config/shops/*.jsonc` (item groups, price expressions, add/replace pieces).

## Testing
- Full workspace tests:
  ```bash
  cargo test --workspace
  ```
- Linting:
  ```bash
  cargo check --workspace
  cargo clippy -p server
  cargo clippy -p client
  ```
