# Game Loop

The authoritative server loop lives in `server/src/main.rs`. It wakes every `100ms` and runs two
top-level steps through `ServerState::tick()`:

1. tick every `GameInstance` (`GameInstance::tick()`),
2. refresh queue-mode preview targets,
3. drop private matches nobody plays in or watches anymore.

Each instance keeps all of its mutable state in one `World` behind a single mutex, so a tick (or
any player request) runs atomically and messages leave in the same order as the state changes
they describe.

## Per-Instance Tick Order

`server/src/instance/ticks.rs` currently performs work in this order:

1. Check whether any player channels or spectator channels are attached.
2. Refresh `last_viewed_at` when someone is watching.
3. If the instance has been viewed within the last 5 seconds:
   - run queued piece actions whose piece is ready,
   - spawn NPCs up to the configured limits,
   - advance NPCs that are off cooldown.
4. Every 600 ticks of this instance (roughly 60 seconds with a 100ms server loop):
   - prune death timestamps older than 10 minutes,
   - prune session secrets older than 5 minutes,
   - prune stale color claims older than 24 hours.
5. Refresh color activity for currently connected players.
6. Recalculate the target board size and shrink only if no player-owned piece would fall outside the new boundary.
7. Resolve the hook events collected since the previous tick.
8. Encode a full `UpdateState` snapshot once and publish it to active players and passive viewers.

## Why NPCs Pause When Unwatched

The server intentionally gates queued actions and NPC simulation behind a recent-view check:

- if nobody has looked at the instance for more than 5 seconds, the board effectively idles,
- state still exists,
- players can reconnect and resume,
- but background NPC churn is avoided for dormant instances.

## Board Resizing In The Tick

Shrinking is conservative:

- the target size is computed from the mode's `board_size` expression,
- the board only shrinks if every player-owned piece is already inside the target boundary,
- once the shrink happens, NPCs and shops outside the new bounds are pruned immediately.

Growing is handled when new players join, not here.

## Broadcast Payload

At the end of the tick, the server sends one `ServerMessage::UpdateState` containing:

- all current players,
- all current pieces,
- all current shops,
- the current board size.

The snapshot is complete, so the client replaces its world with it instead of merging. It is
encoded to JSON once per tick and the same text is handed to every connection
(`server/src/outbound.rs`).

Each connection has two outbound lanes:

- **snapshots** keep only the newest unsent one, so a client that reads slowly gets a lower frame
  rate instead of a growing backlog, and memory stays one shared snapshot per connection;
- **events** (action results, chat, `Init`, errors, and the rest) wait in a queue bounded by
  `socket_event_capacity`. Events must not be lost, so a connection whose queue fills up is closed.

The socket writer takes the latest snapshot first, then drains the event queue, and writes the
events before the snapshot. Action results are queued before the snapshot that reflects them is
published, so they still arrive first. A snapshot taken together with an `Init` or `SpectateEnded`
is dropped, because it may show the board the client just left. A write blocked for
`socket_stall_timeout_ms` closes the connection.

The client treats this as authoritative state and layers prediction on top of it.

## Tick Diagram

```mermaid
flowchart TD
    A[Main loop wakes] --> B[tick per instance]
    B --> D{viewed recently?}
    D -- yes --> E[run ready queued actions]
    E --> F[spawn NPCs]
    F --> G[tick NPCs]
    D -- no --> H[skip hot simulation]
    G --> I[periodic cleanup]
    H --> I
    I --> J[refresh color activity]
    J --> K[attempt safe board shrink]
    K --> L[resolve hooks]
    L --> M[broadcast UpdateState]
```
