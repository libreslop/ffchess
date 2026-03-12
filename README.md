# FFchess (MMO battle chess)

Command your army in a real-time, multiplayer chess world. Scale your board, capture territories, and outfox your opponents in a dynamic, expanding battlefield.

## 🚀 Features
- **Dynamic Board:** The board size scales based on the number of active players.
- **Independent Cooldowns:** Every piece has its own cooldown timer based on its type and travel distance.
- **Physics-Based Camera:** Smooth panning with momentum and exponential zoom smoothing.
- **King-Centric Strategy:** Lose your King, lose your entire army.
- **Dynamic Viewport:** Your vision (Fog of War) expands as your army grows.
- **Recruitment & Upgrades:** 
    - **Recruit:** Buy a Pawn to spawn a new unit in the nearest free square.
    - **Upgrade:** Use a shop square to transform an existing piece into a Knight, Bishop, Rook, or Queen.
    - **Note:** The King cannot be upgraded, but it can use shop squares to recruit new Pawns!
- **Starting Kits:** Choose between Standard, Shield, Scout, or Tank starting sets.
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

## 🧪 Testing
Run the test suite for shared logic, server state, and piece removal:
```bash
cargo test
```
