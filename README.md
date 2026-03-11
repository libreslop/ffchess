# FFChess (Free-For-All Chess)

A real-time, multiplayer, massive-scale chess game where players compete on a shared board that scales dynamically.

## 🚀 Features
- **Dynamic Board:** The board size scales from 30x30 up to 200x200 based on the number of active players.
- **Independent Cooldowns:** Every piece has its own cooldown timer based on its type and travel distance.
- **King-Centric Strategy:** Lose your King, lose your entire army.
- **Dynamic Viewport:** Your vision (Fog of War) expands as your army grows.
- **Economy & Upgrades:** Capture pieces to gain score and use Shop squares to recruit or upgrade units.
- **Starting Kits:** Choose between Standard, Shield, Scout, or Tank starting sets.
- **Unique Pawn Movement:** Pawns move in 4 adjacent directions (Up, Down, Left, Right) and capture in 4 diagonal directions. Orientation is ignored.
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
The server will start on `0.0.0.0:8080`. It serves the frontend assets from `client/dist`.

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
