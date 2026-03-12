# FFchess (MMO battle chess) - Technical Design Document

## 1. Vision & Branding
**FFchess** is a real-time, multiplayer chess-based world designed for high-density, strategic gameplay on a massive, dynamic board.
- **Tagline:** MMO battle chess
- **Aesthetic:** Minimalist, high-contrast UI with a focus on smooth, responsive interaction.

## 2. Technical Architecture
The system is built using a modern Rust stack for both performance and type safety.
- **Workspace:** Cargo workspace with three primary crates:
    - `common`: Shared types, models, protocol definitions, and game logic (movement validation, cooldown calculation).
    - `server`: Axum-based server managing game state, NPC logic, and WebSocket communication.
    - `client`: Yew-based WASM application utilizing HTML5 Canvas for high-performance rendering.

## 3. Core Mechanics

### 3.1 Dynamic World Scaling
- **Coordinate System:** Centered at `(0,0)`. Range expands from `-(size/2)` to `(size+1)/2 - 1`.
- **Scaling Formula:** `(25.0 + (player_count as f32).sqrt() * 17.5).clamp(25.0, 200.0)`.
- **Expansion:** Instant when player count increases.
- **Contraction:** Deferred until all player pieces have cleared the region to be removed, preventing unfair eliminations.

### 3.2 Movement & Cooldowns
- **Zero-Latency Prediction:** The client predicts its own moves and manages cooldown bars locally at 60 FPS.
- **Server Authority:** The server validates all moves and broadcasts state updates, but **never** sends individual piece cooldown timers, saving bandwidth and preventing jitter.
- **Cooldown Logic:** Calculated using server-defined parameters (`CooldownConfig`).
    - Fixed base values for Pawns, Knights, and Kings.
    - Distance-based multipliers for Bishops, Rooks, and Queens.
- **Illegal Move Recovery:** If a predicted move is rejected by the server, the client reverts the piece's cooldown state to its original values instead of resetting them.

### 3.3 NPC Intelligence
- **NPC Density:** Scales with board size (approx. 1 per 250 squares).
- **Behavioral Modes:**
    - **Hunt:** Prioritizes King captures, then other player pieces within a 12-square radius.
    - **Roam:** Moves randomly when no players are nearby.
- **Spawn Buffer:** NPCs are strictly prohibited from spawning within a 10-square grid radius (Chebyshev distance) of any player piece.

### 3.4 Economy & Kits
- **Score:** Earned by capturing pieces (King = 500, Queen = 90, etc.).
- **Kits:** Players choose from Standard, Shield, Scout, or Tank starting armies.
- **Shops:** Single-use squares for spawning or upgrading pieces. Relocate randomly after use.

### 4.4 Security Architecture (Added March 2026)
- **Session Management:** Prevents `player_id` hijacking by requiring a `session_secret` (UUIDv4) for all re-joins.
    - **First Join:** Server generates a random secret and returns it to the client.
    - **Re-Join:** Client provides both its public `player_id` and private `session_secret`.
    - **Validation:** Server rejects any join where the secret does not match the stored secret for that player ID.
- **Protocol Protections:** 
    - **Input Sanitization:** Player names are truncated to 32 characters on the server.
    - **Cooldown Validation:** Strict server-side cooldown checks with a minimal 100ms tolerance.
- **Planned Mitigations:**
    - **Spatial Filtering:** Transitioning to a server-side "Fog of War" to prevent information leakage of the entire board.
    - **Rate Limiting:** Per-connection token-bucket rate limiting for movement and shop commands.
    - **Memory Safety:** Switching to bounded channels to mitigate DoS from slow consumers.

## 5. Client Experience

### 4.1 Rendering Engine
- **Layering:** Background -> Checkerboard -> Grid -> Border -> Pieces -> Names.
- **Text:** Player names feature high-contrast white outlines for visibility against any background.
- **FPS:** Locked at 60 FPS via a continuous internal heartbeat, ensuring fluid animations even during idle periods.

### 4.2 Camera & Interaction
- **Smooth Interpolation:** Pan and zoom use exponential smoothing for a "cinematic" feel.
- **Death Camera:** Pans to the location of the King's defeat and dims the background with a 300ms fade.
- **Fog of War (Client-Side Only):** The client provides radial vision centered on the King. Currently, the server broadcasts the **full** board state; a server-side implementation is planned to prevent data scraping of the entire world.

### 4.3 Network Stability
- **Auto-Reconnection:** Client attempts to reconnect every 2 seconds if the WebSocket is lost.
- **Disconnected Overlay:** A full-screen red overlay appears when connection is lost, blocking input until recovery is complete. Does not show on the home screen.

## 5. Deployment & Tooling
- **Build System:** `trunk` for WebAssembly bundling.
- **Server:** Axum serving both the API/WebSocket and the static WASM assets.
- **Optimization:** Release builds utilize full LTO and stripped symbols for minimal bundle size.
