# FFchess (MMO battle chess)

Command your army in a real-time, multiplayer chess world. Scale your board, capture territories, and outfox your opponents in a dynamic, expanding battlefield.

## 🚀 Features
- **Dynamic Board:** Board size is defined per mode using expressions (often scaling with `player_count`).
- **Independent Cooldowns:** Every piece has its own cooldown timer based on its type and travel distance.
- **Physics-Based Camera:** Smooth panning with momentum, exponential zoom smoothing, and a mode-configured pan limit.
- **King-Centric Strategy:** Lose your King, lose your entire army.
- **Dynamic Viewport:** Fog-of-war radius is mode-configured and can scale with your army.
- **Recruitment & Upgrades:** 
    - **Recruit:** Buy a Pawn to spawn a new unit in the nearest free square.
    - **Upgrade:** Use a shop square to transform an existing piece into a Knight, Bishop, Rook, or Queen.
    - **Note:** The King cannot be upgraded, but it can use shop squares to recruit new Pawns!
- **Starting Kits:** Choose between Standard, Scout, or Tank starting sets (per mode config).
- **Unique Pawn Movement:** Pawns move in 4 adjacent directions and capture in 4 diagonal directions.
- **Resource Optimized:** Server automatically suspends NPC logic when no players are active to minimize CPU load.
- **Persistent NPCs:** Automated pieces populate the board, hunting players or roaming based on proximity.

## 🛠️ Tech Stack
- **Backend:** Rust, Axum, WebSockets, Tokio.
- **Frontend:** Rust, Yew, WebAssembly, HTML5 Canvas.
- **Shared:** `common` crate for types and game logic verification.

## 📦 How to Run

### Prerequisites
- [Rust](https://rustup.rs/) (latest stable)
- [Trunk](https://trunkrs.dev/) (for the frontend)
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
The server will start on `0.0.0.0:8080` by default. It serves the frontend assets from `client/dist`.

**Configuration:**
- You can override the default port using the `PORT` environment variable:
  ```bash
  PORT=3000 cargo run -p server
  ```

### 3. Alternative (Development)
For hot-reloading on the frontend:
```bash
cd client
trunk serve
```
Then run the server in a separate terminal. Note that the client will be on `localhost:8081` and proxy API requests to `localhost:8080`.

## ⚙️ Configuration Overview
- **Global (server):** `config/global/server.jsonc` holds server-side defaults such as the adjective/noun name pool that is used when a player does not provide a name.
- **Global (client):** `config/global/client.jsonc` tunes render/heartbeat intervals, reconnect timing, camera limits, and scroll/drag smoothing. Values are injected into the HTML `<script>` tag so the client boots without an extra request.
- **Modes:** `config/modes/*.jsonc` define each game mode (board sizing formulas, fog-of-war radius, respawn cooldown, shops, kits, hooks). Mode IDs are derived from the filename. The home screen dropdown is built from these definitions and switching modes does **not** refresh the page.
- **Pieces & Shops:** `config/pieces/*.jsonc` describe movement, capture, cooldowns, and glyphs. `config/shops/*.jsonc` drive spawn/upgrade prices and options using expressions.

## 🧪 Testing
Run the test suite for shared logic, server state, and piece removal:
```bash
cargo test
```
