# Multiplayer to-do

## Done
- [x] Shared library: `game-core` crate with the world, creatures, combat, weather, and no graphics.
- [x] Mesh building moved to the game client (`src/mesh/`).
- [x] `Game` in `game-core/src/game/`: owns the world, creatures, weather, fires, and one
      `PlayerState` per player. Changed only through `Game::apply` (commands) and
      `Game::update` (time), and reports `Event`s.
- [x] Game client drives `Game` through commands (single player works through the same path).

- [x] Network messages: `Command`, `Event`, and snapshots are serializable (`serde` + `postcard`),
      sent over TCP with a length prefix (`game-net`).
- [x] Host program (`host/`, run with `cargo run -p gamedemo-host -- [port] [seed]`): runs `Game`
      at 30 ticks a second, adds a player per connection, applies their commands, sends each
      player a `Tick` (snapshot, changed blocks, events).
- [x] State sync host to clients: new players get the seed plus all block changes; every tick
      carries the new block changes, all players, creatures, weather kind, and your own
      resources, health and dice. `Mirror` in `game-net` rebuilds this on the player's side.
- [x] Bare-bones test client: `cargo run -p gamedemo-host --bin walker -- [address] [seconds]`.

## Next
4. **Game client changes**: the client no longer owns the `Game`. It keeps a `Mirror` of what
   the host sends, and sends commands over the connection instead of calling `apply` directly.
   (Drawing, crosshair targeting and the apply menu need the mirror's world and snapshot, and
   `Game`'s sight/effect questions need a version that works from the mirror.)
5. **Draw other players**: there is no player model or mesh yet.
6. **Single player**: run the host inside the game in a background thread and connect to it, so
   single player and multiplayer use the same path.
7. **Cleanup**:
   - Weather is currently based on the first player only (biome, snow, underwater).
   - Movement will feel laggy over a network without some prediction.
   - Players can't be told apart by name; no chat.
   - Per-player spawn limits for creatures are untested with real second players.
   - The host trusts commands for reach: a modified client could apply resources to far away
     blocks (bad numbers and out of range positions are already refused).
   - A player who is too slow to keep up is dropped, and there is no way to rejoin the same
     character (a rejoin is a new player).
