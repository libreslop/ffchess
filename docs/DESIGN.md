# FFChess (Free-For-All Chess) Design Document

## 1. Technical Stack
- **Language:** Rust (Full-stack)
- **Backend:** Axum (Web server & WebSocket handling)
- **Frontend:** Yew (WebAssembly-based UI)
- **Communication:** WebSockets for real-time, event-driven updates.
- **Project Structure:** Cargo Workspace with a `common` crate for shared types and logic.
- **State Management:** In-memory board state (no persistent DB for now).

## 2. Gameplay Mechanics
- **World:** A dynamic grid that starts at 25x25 and scales with player count (up to 200x200).
- **Multiplayer:** Many players coexist on the same board simultaneously.
- **Move Interference:** First-come-first-served. Pieces block movement (except Knights).
- **Pieces:** 
    - Each piece has an independent cooldown (Type-based + Distance-based).
    - **Pawn Movement:** Pawns can move and capture in 4 cardinal directions (standard chess style adapted for 4-way FFA).
    - **King Mobility:** Standard chess movement (1 square in any direction). Cooldown is relatively slow.
- **Capture Logic:**
    - **Immediate Delete:** Capturing a piece immediately removes it from the board.
- **Elimination:**
    - **King Capture:** A player is eliminated when their King is captured. All owned pieces are removed.
- **Economy & Upgrades:**
    - **Score:** Gained immediately upon capturing an enemy or NPC piece.
    - **Shop Areas:** Pieces on shop squares can be used to spawn new pieces by spending Score.
    - **Shop Movement:** Shop squares are single-use and reappear at a random location after being used.
- **Kits:** Players choose a specialized starting set (Standard, Shield, Scout, Tank).
- **NPCs:** Roaming pieces that can be captured for score.

## 3. Visuals & UI
- **Style:** Minimalist 2D ".io game" aesthetic with SVG chess pieces.
- **Viewport (Dynamic):**
    - **Focus:** The camera is strictly constrained to the player's King (cannot pan away from it).
    - **Smooth Zoom:** Continuous zooming (0.2x to 2.0x) centered on the cursor position with exponential smoothing.
- **Overlay:**
    - **Stats Bar:** A semi-transparent top bar showing FPS, Ping, Coordinates, and Player count.
    - **Leaderboard:** Top 10 players by score shown in the top-right corner.
- **Interaction:**
    - **Pmoves (Pre-moves):** Players can queue multiple moves. These are executed sequentially as cooldowns expire.
    - **Move Highlighting:** Valid moves for the selected piece are shown.
    - **Cooldown Indicators:** Visual progress on each piece.

## 4. Technical Implementation
- **Concurrency:** The server uses `tokio::sync::RwLock` for the `GameState`.
- **WASM Client:** The client utilizes `web-sys` for high-performance Canvas rendering.
- **Protocol:** JSON-serialized messages over WebSockets.
- **Scalability:** 
    - Dynamic board resizing manages player density.
    - Aggressive cleanup of pre-move queues ensures client-server synchronization.
- **Shared Logic:** The `common` crate contains the source of truth for movement rules and cooldowns.

## 5. Running the Project
- **Server:** `cargo run -p server` (runs on port 8080)
- **Client:** `cd client && trunk serve`
