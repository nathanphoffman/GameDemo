# Multiplayer

Status: playable. Single player and network games use the same code path. What is left is
polish and hardening (bottom of this file).

## How it fits together

| Part | What it is |
| --- | --- |
| `game-core/` | The game's rules and state. No graphics, no networking. `Game` owns the world, creatures, weather, fires and one `PlayerState` per player. It changes only through `Game::apply` (commands) and `Game::update` (time), and reports `Event`s. `game::sights` answers "what is the crosshair on / what could be built here" and is shared by the host and the client. |
| `game-net/` | The messages (`ClientMessage`, `ServerMessage`, `Tick`), TCP framing (`serde` + `postcard`), the host's `Server`, and the player's `Client` and `Mirror` (a copy of the world kept in step with the host). |
| `host/` | `gamedemo-host`: the headless host program, and `walker`, a window-less test player. |
| `src/` (the game) | The window, input, drawing and sound. It keeps a `Mirror`, never a `Game`, and sends commands. |

How a game runs:
- **Single player**: the game starts a host on a background thread (127.0.0.1) and connects to it.
- **Network game**: someone runs `gamedemo-host`; everyone else runs `gamedemo join <address:port>`.
- The host runs `Game` at 30 ticks a second and sends each player a `Tick`: their view of all
  players and creatures, the weather kind, their own resources/health/dice, the blocks that
  changed, and events (for sounds).
- A new player is sent the world seed plus every block changed so far, and rebuilds the world.
- Creatures live at fixed homes worked out from the seed (`game::spawns`), so everyone finds the
  same ones in the same places. A creature only exists while a player is within 75 blocks of its
  home; a defeated one is back after 5 minutes. Scarier biomes have more creatures
  (`creatures_per_cell`) and tougher ones (`Kind::max_health`).
- Players walk on their own copy of the world and report their position (`Command::SetPose`), so
  movement feels instant. The host refuses impossible jumps. Everything else (mining, placing,
  fighting, applying resources, converting) is decided by the host.

## Names, chat and rejoining

- A player's name comes from `gamedemo join <address:port> [name]` (default: your login name).
  Two players can't share a name at once (capital letters don't count); the host tidies names to
  16 characters.
- **Rejoining**: when a player leaves, the host keeps their resources, converter, health and
  position under their name (up to 64 people, until the host stops). Coming back with the same
  name gets those back (and "NAME came back" in the chat). Anyone using a new name starts fresh.
- **Chat**: press Enter, type, Enter to send (Esc cancels; the game waits while you type). The
  last few lines show in the lower left for 12 seconds. Lines are capped at 120 characters. The
  host announces who joined and left. The font only has capitals, digits and a little
  punctuation; anything else shows as `?`.
- **Player list**: hold Tab. Other players' names float over their heads (within 48 blocks).

## Running it

```
cargo run --release                              # single player (optionally: -- 42 for a seed)
cargo run --release -p gamedemo-host -- 7777 42  # host: port, seed (both optional)
cargo run --release -- join 192.168.1.20:7777 Ann  # join a host (name is optional)
cargo run -p gamedemo-host --bin walker -- 127.0.0.1:7777 20 BOT  # window-less test player
cargo test --workspace
```

The host listens on all addresses on its port, takes up to 16 players, and nothing is saved
when it stops.

## Done

- [x] Shared library (`game-core`), and mesh building moved to the game client (`src/mesh/`).
- [x] `Game` with commands in, events out, one `PlayerState` per player.
- [x] Network messages, TCP transport, host program, state sync, test player.
- [x] Game client plays through a host (single player included), with crosshair targeting and
      the apply menu worked out from its own copy of the world.
- [x] Other players are drawn (blocky figure, health bar over the head), and creatures and
      players are blended between updates so they move smoothly.
- [x] Other players' actions: you hear their digging (quieter with distance, silent past 40
      blocks) and see cracks on the block they are mining.
- [x] Creatures placed from the seed like terrain, more and tougher in dangerous biomes, and
      respawning after being defeated (about 350 in a world; only the ones near players exist).
- [x] Names, a player list (Tab), name tags over heads, chat (Enter), and rejoining with the same
      name to get your things back. Covered by `game-net/tests/sync.rs` and checked in the real
      window.
- [x] Weather: the host only tracks which weather it is; each player's game works out how it
      looks there (snow in the cold, dust storms in the desert, blue under water).
- [x] Play-tested: a scripted session over the network (`game-net/tests/play.rs`: build a floor
      and a forge, place and mine blocks, fight the deer, with a second player watching), and the
      real window driven with injected mouse and keyboard input (look, walk, jump, mine, place,
      scroll-select, apply, open and close the menu).

## Left

None of these stop it from working; they are roughly in the order worth doing.

1. **Fighting through the real window, and longer play sessions.** The fight is covered by the
   scripted network test, but not by driving the window.
2. **Saving.** Nothing is saved: the world and every player's resources are gone when the host
   stops, and so is what the host remembers about players who left (so rejoining only works
   while the host runs). (Single player never saved either.) The block changes already tracked
   by the world would make saving the world easy; player state would need a file too, and could
   be keyed by name like rejoining is.
3. **Single-player memory.** The host and the game each build their own copy of the world, so it
   uses about twice the memory (they are built at the same time, so start-up isn't doubled).
   Could be fixed by sharing one copy in single player.
4. **Tuning creatures.** The numbers are guesses until played: creatures per cell by biome
   (`spawns::creatures_per_cell`), health per kind (`Kind::max_health`), wake distance and the
   5 minute respawn (`spawns.rs`). All creatures hit for the same 2d6, so difficulty is health
   and number only; a per-kind damage bonus would be the next lever.

Decided not to do: cheating protection, internet play help (typing in a public IP works), and
sending big worlds to a joining player in pieces.
