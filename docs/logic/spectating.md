# Spectating

Besides the anonymous board shown behind the menu, a connection can watch a board as a **named
spectator**. Named spectators are announced in chat, listed under the leaderboard of everyone on
that board, and can pan the whole board freely without fog of war.

## Starting To Spectate

The client sends `ClientMessage::Spectate { name, target, player_id }`, where `player_id` is the
player id the client stored for the mode and `target` is a `SpectateTarget`:

| Target | Resolves to |
| --- | --- |
| `Own` | The match the connection last played in (its binding), or the mode's public instance when it has none. Queue modes without a binding resolve to nothing. |
| `Match(id)` | A private match of the connection's queue mode, by instance id. |

The client offers three entry points:

- **Menu, public mode**: the Spectate button next to Play sends `Own`, watching the shared world.
- **Menu, queue mode**: the Spectate button opens the match browser. While it is shown, the client
  sends `ClientMessage::ListMatches` every two seconds and the server answers with
  `ServerMessage::MatchList`, the running private matches of that mode, newest first. Picking a
  match sends `Match(id)`.
- **End screen**: the Spectate button next to Play Again always sends `Own`, so the player keeps
  watching the board they just played on.

Players still competing on a board cannot spectate. Otherwise the server (`server/src/state/spectate.rs`):

1. resolves the target, and refuses when `player_id` plays on it from another connection (another
   tab): the connection goes back to the menu board as on `Leave` and gets an "Already Playing"
   error, on which the client returns to the menu,
2. stops any previous spectating, detaches the connection's player binding, leaves its queue, and
   drops its preview watching,
3. answers `ServerMessage::SpectateEnded` when the target does not exist or already finished, or
4. registers the connection as a named spectator of the instance and sends it an `Init` with the
   nil player id.

Spectator names are cleaned like player names (control and invisible characters removed) and cut
to 32 characters; an empty name is replaced by a generated one.

The reverse holds too: when a player joins a board, connections spectating it under the same
player id get `SpectateEnded` and stop spectating, so nobody watches a game they play in.

## Spectators In An Instance

Each instance keeps a `SpectatorRoster` (`server/src/instance/spectators.rs`) in arrival order.
Spectators also receive every broadcast like other viewers. Their names are sent in `Init` and in
every `UpdateState` snapshot, and clients list them beneath the leaderboard, even in modes that
hide the scoreboard.

Arrivals and departures are recorded as hook events, which the `OnSpectatorJoin` and
`OnSpectatorLeave` hooks turn into system chat lines (see [modes.md](../config/modes.md)).

Named spectators chat in the room of the board they watch, under the name they spectate with.

## Leaving

`ClientMessage::Leave` stops spectating (announcing the departure) and returns the connection to
the menu board, like leaving a match. Closing the connection, joining a game, or spectating another
board also stops spectating.

## End Of A Private Match

Every instance has a `MatchStatus`: `Running`, or `Finished { at }`. A private match becomes
finished once no competing player remains (everyone left, lost, or won), for example when one side
of a duel wins, and stays finished. Public worlds never finish.

In the tick a match finishes, it sends `ServerMessage::MatchFinished { replay }` to everyone still
on its board (winners, eliminated players watching their end screen, and spectators), then sends
`SpectateEnded` to all its spectators and drops them, so spectators never keep a finished match
alive. The lobby forgets them on the next server tick. A finished match no longer appears in the
match browser and can no longer be spectated.

The client keeps the last board on screen and shows "Game ended" with a Return To Menu button,
which sends `Leave`. The victory and defeat screens hide their Spectate button once the match
finished. Both screens show an Analysis Board button when `MatchFinished` named a replay.

## Replays

A queue mode with `"allow_replays": true` records each of its private matches. After every
action a piece makes (player and NPC moves alike), and again at the end of every tick, the match
compares its board with the last recorded one and queues a `ReplayFrame` holding only what
changed (board size, players, shops, changed and removed pieces); the first frame holds the whole
board. Recording each action on its own keeps moves of one tick apart, such as a capture and the
guard taking the square back. Frame times never go back, so frames stay in order. Frames go to a background writer thread that stores them in a SQLite database at
`~/.local/share/ffchess/replays.sqlite3`, so a recording does not build up in server memory.

When the match finishes, the replay is saved with its `saved_at` time and becomes viewable through
`GET /api/replays/<replay id>`, which answers 404 while the match runs, after the replay expired,
or for unknown ids. Replays are deleted 24 hours after they were saved: overdue replays are cleaned
up when the server starts and every minute while it runs. Recordings of matches that never
finished (such as those cut off by a server restart) are deleted too.

A saved replay also records when the match started: the time its countdown unlocked moves, or
the start of the recording for modes without a countdown. Viewers start playback there, so the
countdown is skipped, and the board as it stood then is the first entry of the move history.
The viewer opens paused there.

The replay viewer downloads the replay, rebuilds the board after every frame, and plays it back
on a read-only board watched like a spectator watches a live match. The board fills the screen
as in a match, shifted left to make room for a panel floating on the right (or shifted up, with
the panel along the bottom, on narrow screens). The panel holds:

- the **move history**: one row per second of the match, labelled with the second it ends at,
  holding an entry for every moment in that second in which pieces moved. An entry is one box
  that wraps its moves inside it, in the order they were made, so a moment of many moves (such as
  a capture and the guard taking the square back) stays one highlight. Seconds without moves are
  left out, row 0 holds only the start, and an `end` entry sits in the row of the second the
  match finished in. Moves are written
  in chess notation (files from the left, ranks from the bottom, a figurine for non-pawns, `x`
  for captures) next to a swatch of the owner's color. The entry shown is highlighted and kept
  in view; clicking an entry shows it,
- buttons stepping through the history: ◀ and ▶ (or the left and right arrow keys) move one
  entry, ▲ and ▼ (or the up and down arrow keys) move to the nearest entry before or after that
  ends a row. Stepping pauses playback at the moment of the entry. Holding a button or arrow key
  steps once, then again every 80 ms after a 400 ms pause. In modes with
  `disable_camera_movement`, the mouse wheel over the board steps one entry too; in other modes it
  zooms the board,
- the playback bar: play and pause (Space), a scrub bar marking every capture, and a speed button
  cycling 1x, 2x, 4x and 8x,
- in the title bar, a button flipping the board, and a share button opening a menu that:
  - copies the share link, `/?replay=<replay id>#<mode id>`, which opens the viewer over the
    mode's menu as soon as the page loads (closing the viewer drops `replay` from the address),
  - downloads the replay as the server sent it, as `<mode id>-<replay id>.ffr.json`,
  - renders the board to a WebM video, `<mode id>-<replay id>.webm`, seen the way the viewer has
    it turned, with half a square of room around the board for the names over the kings. The
    video has a frame every server tick (`tick_interval_ms`), so players can seek anywhere, but
    the board only looks different when the replay moves on to another board or a cooldown ring
    charges up a step. Only those frames are drawn off screen by the board renderer and encoded
    with the browser's WebCodecs, with a keyframe at least every 30 seconds; with VP9, every
    other frame is a one-byte `show_existing_frame` showing the last picture again, worked out
    from the reference slots the encoded frames refresh. With VP8, the fallback, those frames
    are encoded too. The encoder and WebM muxer, `assets/static/js/replay_video.js`, are only
    loaded once a video is first rendered.

The board is turned the way the viewer played the match: the client looks for its stored
player id of the mode among the replay's players. Every player's pieces show their cooldown
rings, measured against the moment of the replay on screen.

Escape or the top-left button closes the viewer.

## Client Camera

Watching a match or a replay from the menu starts the camera in the mode's `starting_camera`
pose, centred on the board centre when it names a piece; watching the match just lost keeps the
view of its end.

While spectating, the camera is not centred or tied to a king: dragging, inertia, and zoom work
freely. Whenever the view centre leaves the board and no drag is in progress, the camera eases back
to the nearest point on the board, like the pull back towards the king while playing.
