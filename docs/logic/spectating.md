# Spectating

Besides the anonymous board shown behind the menu, a connection can watch a board as a **named
spectator**. Named spectators are announced in chat, listed under the leaderboard of everyone on
that board, and can pan the whole board freely without fog of war.

## Starting To Spectate

The client sends `ClientMessage::Spectate { name, target }`, where `target` is a `SpectateTarget`:

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

1. resolves the target,
2. stops any previous spectating, detaches the connection's player binding, leaves its queue, and
   drops its preview watching,
3. answers `ServerMessage::SpectateEnded` when the target does not exist or already finished, or
4. registers the connection as a named spectator of the instance and sends it an `Init` with the
   nil player id.

Spectator names are trimmed and cut to 32 characters; an empty name is replaced by a generated one.

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

A private match is finished once no competing player remains (everyone left, lost, or won). At the
end of every tick, a finished match sends `SpectateEnded` to all its spectators and drops them, so
spectators never keep a finished match alive. The lobby forgets them on the next server tick.

The client keeps the last board on screen and shows "Game ended" with a Return To Menu button,
which sends `Leave`. Public worlds never finish.

## Client Camera

While spectating, the camera is not centred or tied to a king: dragging, inertia, and zoom work
freely. Whenever the view centre leaves the board and no drag is in progress, the camera eases back
to the nearest point on the board, like the pull back towards the king while playing.
