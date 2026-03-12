# FFchess (MMO battle chess) Project Overview

This document provides a comprehensive overview of the `ffchess-server` project, its current state, architecture, and core mechanics to assist future development.

## 1. Project Structure
The project is organized as a Cargo Workspace:
- `common/`: Shared data models (`models.rs`), game logic (`logic.rs`), and network protocol definitions (`protocol.rs`).
- `server/`: Axum-based WebSocket server. Manages game state, NPC behavior, and player connections.
- `client/`: Yew-based WebAssembly frontend. Handles rendering (Canvas), user input, and state synchronization.

## 2. Core Mechanics
- **Board:** A dynamic grid (starts at 25x25, scales with player count using a square-root formula up to 200x200).
- **Movement:** Standard chess moves (King, Queen, Rook, Bishop, Knight). Pawns move/capture in 4 directions (adjacent/diagonal).
- **Cooldowns:** Every move triggers a cooldown based on piece type and distance moved.
- **Combat:** Capturing a piece immediately removes it. Capturing a King eliminates the player and all their pieces.
- **Economy:** Players gain score by capturing pieces. Score can be spent at **Shops** to upgrade pieces or spawn new ones. Shops are single-use and reappear at a random location after being used.
- **Kits:** Players choose a starting kit (Standard, Shield, Scout, Tank) which determines their initial pieces.
- **NPCs:** Non-player pieces that roam the board and can be captured for score.

## 3. Technical Implementation
### Server (`server/`)
- **State Management:** `ServerState` holds the `GameState` behind an `RwLock`. It tracks active player channels and handles broadcasting.
- **Concurrency:** Uses `tokio` for asynchronous tasks (game loop, NPC logic, WebSocket handling).
- **Networking:** `axum` for HTTP and WebSocket routing. Messages are JSON-serialized `ClientMessage` and `ServerMessage`.

### Client (`client/`)
- **Framework:** `Yew` with a `GameStateReducer` for state management.
- **Rendering:** `web-sys` Canvas API for drawing the board and pieces.
### Synchronization & Security
- **Pmoves (Pre-moves):** The client supports queuing multiple moves. These are executed sequentially as cooldowns expire.
- **Session Security:** Implemented a `session_secret` (UUID) system. When a player joins, they receive a secret token stored in local storage. Subsequent re-joins must provide this secret to prevent UUID hijacking.
- **Synchronization:** The client receives periodic `UpdateState` messages and performs "aggressive cleanup" of the pre-move queue when the server confirms a piece's position.

## 4. Security & Performance Evaluation (March 2026)
A comprehensive security audit identified several key areas for improvement:
- **Session Integrity:** (Fixed) Added session secrets to prevent impersonation.
- **Information Leakage:** The server currently broadcasts the entire game state to all players. A server-side "Fog of War" (spatial partitioning) is planned to limit data sent to the player's immediate vicinity.
- **Protocol Robustness:** Potential vulnerabilities to JSON-based DoS and message spamming have been identified. Future work includes implementing message size limits and per-connection rate limiting.
- **Memory Safety:** Transitioning from `unbounded_channel` to bounded channels is recommended to prevent memory exhaustion from slow/malicious clients.

## 5. Current Status & Handoff (March 2026)
### Build & Test Status
- `cargo check`: **PASSED**.
- `cargo test`: **PASSED** (all 17 tests pass, including new session hijack prevention tests).
- **WASM Build:** `cargo check -p client --target wasm32-unknown-unknown` **PASSED**.

### Recent Progress
- Implemented robust session secret validation to prevent player ID hijacking.
- Updated all server test suites to support the new secure join protocol.
- Performed a security audit and architectural evaluation.
- Implemented robust pre-move queue handling with aggressive cleanup on server confirmation.
### Known Issues / Future Work
- **Unused Warning:** `server/src/main.rs` has an unused `Html` import.
- **NPC Intelligence:** NPCs currently move randomly. Future work could include basic AI/puzzles.
- **UI Polish:** The Canvas rendering is functional but basic.
- **Fog of War:** While planned in the design, server-side validation of viewport visibility is still in progress.

## 5. Development Workflow
- **Run Server:** `cargo run -p server` (Port 8080)
- **Run Client:** `cd client && trunk serve`
- **Run Tests:** `cargo test --workspace`
- **WASM Check:** `cargo check -p client --target wasm32-unknown-unknown`

## 6. Key Files for Agents
- `common/src/logic.rs`: The "source of truth" for chess rules and physics.
- `server/src/state.rs`: Core server-side state transition logic.
- `client/src/reducer.rs`: Client-side state and pre-move queue management.
- `common/src/protocol.rs`: Communication interface between client and server.
