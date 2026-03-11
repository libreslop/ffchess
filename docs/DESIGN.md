# FFChess (Free-For-All Chess) Design Document

## 1. Technical Stack
- **Language:** Rust (Full-stack)
- **Backend:** Axum (Web server & WebSocket handling)
- **Frontend:** Yew (WebAssembly-based UI)
- **Communication:** WebSockets for real-time, event-driven updates.
- **Project Structure:** Cargo Workspace with a `common` crate for shared types and logic.
- **State Management:** In-memory board state (no persistent DB for now).

## 2. Gameplay Mechanics
- **World:** A large 100x100 grid.
- **Multiplayer:** Many players coexist on the same board simultaneously.
- **Move Interference:** First-come-first-served.
- **Pieces:** 
    - Each piece has an independent cooldown (Type-based + Distance-based).
    - **Pawn Rotation:** Pawns have a specific orientation. Changing this orientation costs 1 full action/cooldown.
    - **No Promotion:** Pawns do not promote.
    - **King Mobility:** Standard chess movement (1 square in any direction). Cooldown is relatively slow to ensure the game can reach a conclusion.
- **Capture Logic:**
    - **Immediate Delete:** Capturing a piece immediately removes it from the board (standard chess style).
- **Control & Viewport:**
    - **King-Centric Control:** You can only control pieces within your King's current viewport.
- **Elimination:**
    - **King Capture:** A player is eliminated when their King is captured. All owned pieces are removed.
- **Economy & Upgrades:**
    - **Score:** Gained immediately upon capturing an enemy or NPC piece.
    - **Shop Areas:** Pieces on shop squares can be upgraded or used to spawn new pieces by spending Score.
    - **Shop Movement:** Shop squares move to a new random location after 5-10 uses.
    - **Scaling Costs:** Costs increase as the player's total piece count increases.
- **Kits:** Players choose a specialized starting set (approx. 5-7 pieces).
- **NPCs & Puzzles:**
    - **Behavior:** NPCs roam or hold positions, becoming active/aggressive when a player is nearby.

## 3. Visuals & UI
- **Style:** Minimalist 2D ".io game" aesthetic.
- **Viewport (Dynamic):**
    - **Focus:** The camera is strictly centered on the player's King.
    - **Fog of War:** Pieces outside the viewport are invisible and uncontrollable.
    - **Scaling (Zoom):** The viewport zoom level scales with the **square root** of the player's total piece count (diminishing returns for larger armies).
- **Interaction:**
    - **Move Highlighting:** Valid moves for the selected piece are shown.
    - **Cooldown Indicators:** Visual progress on each piece.

## 4. Technical Implementation
- **Concurrency:** The server uses `tokio::sync::RwLock` for the `GameState` and `tokio::sync::mpsc` for per-player communication channels.
- **WASM Client:** The client utilizes `gloo-net` for WebSockets and `web-sys` for high-performance Canvas rendering.
- **Protocol:** JSON-serialized messages over WebSockets.
- **Scalability:** 
    - Full-state broadcasting every 100ms is efficient for this board size and piece density.
    - Viewport logic is handled client-side for rendering but validated server-side for control.
- **Shared Logic:** The `common` crate contains `is_valid_chess_move` which is used by the server to validate moves and by the client to (optionally) show valid move previews.

## 5. Running the Project
- **Server:** `cargo run -p server` (runs on port 8080)
- **Client:** `cd client && trunk serve` (requires [Trunk](https://trunkrs.dev/))
