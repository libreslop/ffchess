# ffchess - Multi-Player Multi-Piece Chess

**ffchess** is a fast-paced, multi-player, data-driven chess variant. Unlike traditional chess, this game features a dynamic, resizing board where players can bring their own starting kits, capture NPCs to earn score, and buy new pieces or upgrades from interactive shops.

## Key Features

-   **Multi-Player Sandbox**: Play with up to 20+ players on a single, shared board.
-   **Dynamic Board Size**: The world expands and contracts based on the number of active players.
-   **Data-Driven Design**: Pieces, shops, and game modes are fully configurable via JSONC files.
-   **Real-Time Combat**: Pieces move on independent cooldowns—no more waiting for turns!
-   **In-Game Economy**: Capture pieces to gain score, then visit shops to recruit new units or upgrade existing ones.
-   **Customizable Hooks**: Game modes can define custom triggers, such as "Eliminate owner on King capture."
-   **NPC AI**: Autonomous pieces roam the board, providing constant interaction even in low-population games.

## Technology Stack

-   **Server**: Built with **Rust** using `tokio` for high-performance asynchronous networking and state management.
-   **Client**: A high-performance **WebAssembly** application built with Rust and `yew`/`gloo`.
-   **Common**: Shared logic and protocols between client and server, ensuring a single source of truth for movement rules and data models.
-   **Communication**: Real-time communication via **WebSockets** with a custom JSON-based protocol.

## Getting Started

### Prerequisites

-   [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
-   [Trunk](https://trunkrs.dev/) (for building the WebAssembly client)

### Build and Run

1.  **Build the Client**:
    ```bash
    cd client
    trunk build --release
    cd ..
    ```
    This compiles the Rust client to WebAssembly and places the static assets in `client/dist`.

2.  **Run the Server**:
    ```bash
    cargo run -p server --release
    ```
    The server will start at `http://localhost:3000` (by default), serving both the WebSocket API and the static client files from the `dist` directory.

## Documentation

Comprehensive documentation for all project systems can be found in the [`docs/`](docs/README.md) folder:

-   [**Configuration Guide**](docs/README.md#configuration-guide): How to define pieces, shops, and modes.
-   [**Logic and Flow**](docs/README.md#logic-and-flow): Detailed technical descriptions of the game loop, movement, and NPC AI.

## License

AGPLv3
