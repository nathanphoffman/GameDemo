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

- [x] Game client talks to a host: it keeps a `Mirror` of the world and the latest update, and
      sends commands. Crosshair targeting and the apply menu work from the client's copy
      (`game_core::game::sights`, shared with the host's `Game`).
- [x] Other players are drawn (`mesh::creature::player_mesh`).
- [x] Single player runs a host on a background thread inside the game and connects to it, so it
      uses the same path as a network game. Join one with `gamedemo join <address:port>`.
- [x] Movement: each player walks on their own copy of the world and tells the host where they
      are (`Command::SetPose`), so it feels instant. The host refuses impossible jumps.
- [x] Creatures and other players are drawn blended between the host's updates, so they move
      smoothly.

## Left
- **Weather**: the host rolls the weather for everyone (the biome bend and snow are worked out
  per player on the client now), but the host's own copy still uses the first player's biome.
- **Cheating**: a modified client could report any position (up to the speed limit) and apply
  resources to far-away blocks. Fine for friends, not for strangers.
- **Rejoining**: a dropped player comes back as a new player with new resources.
- **Names, chat, a player list**: none.
- **Creature spawning** with several real players spread out is untested.
- **Memory and start-up in single player**: the host and the game each build their own copy of
  the world (about twice the memory, built at the same time).
- **Other players' actions**: their digging and fighting make no sound or effect on your screen
  yet (only your own).
- **Play-testing**: the window code was run and checked by screenshot, but not played for long.
