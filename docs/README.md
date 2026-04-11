# Project Documentation

This folder contains the technical documentation for the **ffchess** project.

## Configuration Guide

The following documents describe the various configuration files used to define game pieces, shops, and game modes.

-   [**Global Configuration**](config/global.md): System-wide settings for server and client.
-   [**Piece Configuration**](config/pieces.md): Define properties, movement, and scoring for pieces.
-   [**Shop Configuration**](config/shops.md): Configure in-game shops, items, and pricing formulas.
-   [**Game Mode Configuration**](config/modes.md): Rules, board parameters, and kits for different modes.

## Logic and Flow

These documents explain the core logic and detailed workflows of the game's systems.

-   [**Game Loop and Ticking**](logic/game_loop.md): The heart of the server's periodic processing.
-   [**Movement and Validation**](logic/movement.md): How moves are validated, queued, and executed.
-   [**Combat, Captures, and Hooks**](logic/combat.md): Capture side-effects and the custom hook system.
-   [**NPC Behavior and Spawning**](logic/npcs.md): AI logic for autonomous pieces on the board.
-   [**Board and World Logic**](logic/world.md): Dynamic board resizing and interactive shop systems.

## Overview

The project is a data-driven chess variant where multiple players can compete on a dynamic, resizing board. Players can capture both each other's pieces and NPCs to earn scores, which can then be spent at shops to expand or upgrade their army.

The server is built in Rust and uses a tick-based system for state management, while the client (also in Rust via WebAssembly) provides a real-time, interactive experience with smooth animations and camera controls.
