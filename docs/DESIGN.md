# FFchess Design Document

## 1. Technical Stack
- Language: Rust (full stack).
- Backend: Axum + WebSockets + Tokio.
- Frontend: Yew + WebAssembly.
- Shared: Cargo workspace with `common` crate for types, protocol, and logic.
- State: In-memory board state (no persistence yet).

## 2. Gameplay Mechanics
- World: Dynamic grid sized per mode configuration.
- Multiplayer: Many players on the same board at once.
- Movement: Path-based rules per piece config; pieces block multi-step paths.
- Cooldowns: Per-piece cooldown values from config; predicted on client.
- Capture: Immediate removal of the captured piece.
- Elimination: Capturing a King eliminates the owner and all their pieces.
- Economy: Score earned on captures, spent at shops.
- Shops: Single-use squares that can upgrade or spawn pieces, then respawn elsewhere.
- Kits: Starting armies defined per mode.
- NPCs: Roaming pieces with expression-driven spawn caps.

## 3. Visuals & UI
- Style: Minimalist canvas-based rendering with per-piece glyphs from config.
- Viewport: Smooth pan and zoom with momentum and mode-configured pan limits.
- Overlay: Leaderboard, ping, FPS, and board size in lightweight HUD elements.
- Interaction: Premoves (queued moves) with local move highlighting and cooldown bars.

## 4. Technical Implementation Notes
- Server runs per-mode `GameInstance`s and broadcasts snapshots.
- Client uses a reducer to reconcile server snapshots with predicted state.
- Shared logic in `common` is the authoritative source for move validation.

## 5. Running the Project
- Server: `cargo run -p server`
- Client: `cd client && trunk serve`
