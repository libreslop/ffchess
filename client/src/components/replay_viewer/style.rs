//! Styles of the replay viewer, which need classes for layout breakpoints, hover states and
//! the scrub bar thumb that inline styles cannot reach.

/// Fills the screen with the board like a match does, shifted left to make room for the move
/// history floating on the right, or shifted up for it on narrow and tall screens.
///
/// The board view reaches past the left edge of the screen by the room the panel takes, so
/// the board, which sits in the middle of the view, lands in the middle of the space left.
/// The panel is drawn in the wood colors of the shop menu.
pub(super) const REPLAY_CSS: &str = "
    .replay-layout {
        --replay-panel: 320px;
        --replay-gap: 12px;
        --replay-room: calc(var(--replay-panel) + 2 * var(--replay-gap));
        --wood-frame: #6b4a2b;
        --wood-plate: #a87545;
        --wood-text: #fff1e0;
        --wood-shade: rgba(43, 30, 18, 0.88);
        position: fixed; inset: 0; z-index: 300; overflow: hidden; background: #413931;
    }
    .replay-board { position: absolute; top: 0; bottom: 0; right: 0; left: calc(-1 * var(--replay-room)); }
    .replay-hud { position: absolute; top: 0; bottom: 0; left: 0; right: var(--replay-room); pointer-events: none; }
    .replay-hud button { pointer-events: auto; }
    .replay-panel {
        position: absolute; top: var(--replay-gap); bottom: var(--replay-gap); right: var(--replay-gap);
        width: var(--replay-panel); box-sizing: border-box; display: flex; flex-direction: column;
        background: var(--wood-shade); color: var(--wood-text);
        border: 3px solid var(--wood-frame); box-shadow: 4px 4px 0 rgba(0, 0, 0, 0.35);
    }
    @media (max-width: 760px), (max-aspect-ratio: 1/1) {
        .replay-layout { --replay-panel: 40dvh; }
        .replay-board { left: 0; top: calc(-1 * var(--replay-room)); }
        .replay-hud { right: 0; bottom: var(--replay-room); }
        .replay-panel { top: auto; left: var(--replay-gap); width: auto; height: var(--replay-panel); }
    }

    .replay-title {
        flex: none; display: flex; align-items: center; gap: 4px; padding: 2px 2px 2px 12px;
        background: var(--wood-frame); font-size: 20px; letter-spacing: 2px; text-transform: uppercase;
    }
    .replay-title-text { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

    .replay-moves {
        position: relative; flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none;
    }
    .replay-moves::-webkit-scrollbar { display: none; }
    .replay-row { display: flex; border-bottom: 3px solid var(--wood-frame); }
    .replay-row-time {
        flex: none; width: 44px; box-sizing: border-box; padding: 6px 8px; text-align: right;
        background: rgba(0, 0, 0, 0.2); color: rgba(255, 241, 224, 0.5); font-size: 20px;
    }
    .replay-row-moves {
        flex: 1; min-width: 0; display: flex; flex-wrap: wrap; align-items: flex-start;
        gap: 2px 4px; padding: 3px 4px;
    }
    .replay-entry {
        display: inline-flex; flex-wrap: wrap; max-width: 100%; box-sizing: border-box;
        padding: 0 2px; border: 2px solid transparent; border-radius: 0;
        background: transparent; color: inherit; font: inherit; font-size: 20px; cursor: pointer;
    }
    .replay-entry:hover { background: rgba(255, 241, 224, 0.12); }
    .replay-entry.current { background: var(--wood-plate); border-color: var(--wood-frame); }
    .replay-move { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; padding: 2px 4px; }
    .replay-piece { flex: none; display: block; width: 16px; height: 16px; image-rendering: pixelated; }
    .replay-swatch { flex: none; width: 8px; height: 8px; border: 1px solid var(--wood-text); }
    .replay-marker { opacity: 0.6; }
    .replay-entry.current .replay-marker { opacity: 1; }

    .replay-nav { flex: none; display: flex; gap: 4px; padding: 0 4px 4px; }
    .replay-nav button, .replay-button {
        display: flex; align-items: center; justify-content: center;
        height: 36px; background: var(--wood-plate); color: var(--wood-text);
        border: 2px solid var(--wood-frame); border-radius: 0; font-size: 20px; cursor: pointer;
    }
    .replay-nav button { flex: 1; touch-action: none; -webkit-user-select: none; user-select: none; }
    .replay-nav button:hover, .replay-button:hover { filter: brightness(1.1); }
    .replay-nav button:active, .replay-button:active,
    .replay-nav button.pressed, .replay-button.pressed { filter: brightness(1.25); }
    .replay-icon {
        flex: none; display: block; width: 24px; height: 24px; background-color: currentColor;
        -webkit-mask: var(--icon) center / contain no-repeat; mask: var(--icon) center / contain no-repeat;
    }
    .replay-icon-arrow { --icon: url(/assets/static/img/arrow-up.svg); }
    .replay-icon-play { --icon: url(/assets/static/img/play.svg); }
    .replay-icon-pause { --icon: url(/assets/static/img/pause.svg); }
    .replay-icon-flip { --icon: url(/assets/static/img/flip.svg); }
    .replay-icon-share { --icon: url(/assets/static/img/share.svg); }
    .replay-icon-link { --icon: url(/assets/static/img/link.svg); }
    .replay-icon-download { --icon: url(/assets/static/img/download.svg); }
    .replay-icon-video { --icon: url(/assets/static/img/video.svg); }
    .replay-icon-button { flex: none; width: 32px; height: 32px; padding: 0; }

    .replay-share { position: relative; flex: none; }
    .replay-menu {
        position: absolute; top: calc(100% + 4px); right: 0; z-index: 10; min-width: 220px;
        display: flex; flex-direction: column; background: var(--wood-plate);
        border: 3px solid var(--wood-frame); box-shadow: 4px 4px 0 rgba(0, 0, 0, 0.35);
        letter-spacing: 0; text-transform: none;
    }
    .replay-menu-item {
        display: flex; align-items: center; gap: 8px; padding: 6px 10px; background: transparent;
        color: var(--wood-text); border: none; border-radius: 0; font: inherit; font-size: 20px;
        text-align: left; white-space: nowrap; cursor: pointer;
    }
    .replay-menu-item + .replay-menu-item { border-top: 2px solid var(--wood-frame); }
    .replay-menu-item:hover { background: rgba(255, 241, 224, 0.15); }
    .replay-menu-item:disabled { opacity: 0.5; cursor: wait; }

    .replay-scrubber { -webkit-appearance: none; appearance: none; }
    .replay-scrubber::-webkit-slider-runnable-track { background: transparent; height: 36px; }
    .replay-scrubber::-moz-range-track { background: transparent; height: 36px; }
    .replay-scrubber::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 24px; margin-top: 6px; background: var(--wood-text); border: 3px solid var(--wood-frame); border-radius: 0; }
    .replay-scrubber::-moz-range-thumb { width: 8px; height: 18px; background: var(--wood-text); border: 3px solid var(--wood-frame); border-radius: 0; }
";
