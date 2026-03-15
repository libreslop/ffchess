# FFchess - Technical Design Document

## 1. Vision
FFchess is a real-time, multiplayer chess-inspired arena with data-driven rules. The system emphasizes fast iteration via config files and shared, typed logic.

## 2. Architecture
- Workspace crates: `common`, `server`, `client`.
- Shared types and logic in `common` enforce semantics across the stack.
- Server runs per-mode `GameInstance`s, each with its own state and configs.
- Client is a Yew/WASM app rendering the game via HTML5 Canvas.

## 3. Core Mechanics

### 3.1 Dynamic World Scaling
- Board size is computed from `GameModeConfig.board_size` expressions.
- Coordinates are centered at `(0, 0)`; valid positions satisfy `-half <= x < limit_pos`.
- Board contraction only occurs when no player-owned pieces are outside the new bounds.

### 3.2 Movement & Cooldowns
- Movement and capture rules are defined as path lists in piece config.
- `common::logic::is_valid_move` enforces path blocking and capture rules.
- Cooldowns are per-piece values from config and are predicted locally by the client.
- Invalid move recovery resets the piece cooldown state to the server-confirmed values.

### 3.3 NPC Logic
- Spawn caps are expression-based (`npc_limits`).
- NPC spawn positions avoid other pieces and shops via spawn helpers.
- AI hunts the nearest player piece within ~12 tiles; otherwise it performs random valid moves.
- NPC ticks are paused if no players are viewing for ~5 seconds.

### 3.4 Economy & Kits
- Score is earned by captures and spent at shops.
- Shops are grouped by applicable piece types and fall back to `default_group`.
- Kits are defined per mode and delivered as `KitSummary` to clients.

## 4. Security & Robustness
- Session secrets prevent player ID hijacking on rejoin.
- Server validates moves, cooldowns, and shop purchases.
- Full-state broadcasts remain in place; server-side fog-of-war is a planned improvement.
- Unbounded channels are still used; bounded channels are recommended for backpressure.

## 5. Client Experience

### 5.1 Rendering
- Canvas renderer draws board, pieces, labels, and UI overlays.
- Piece glyphs are taken from config and rendered as text on pieces.
- Cooldown bars are drawn on owned pieces only.

### 5.2 Camera & Interaction
- Smooth pan/zoom with momentum and per-mode pan limits.
- Death camera focuses on the last king position with a zoom ramp.
- Premoves allow players to queue actions; the reducer reconciles against server updates.

### 5.3 Networking
- WebSocket reconnect loop with disconnected overlay.
- Mode list and global client config are injected into `index.html` for fast boot.
