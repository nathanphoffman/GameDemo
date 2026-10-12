# Multiplayer to-do

## Done
- [x] Shared library: `game-core` crate with the world, creatures, combat, weather, and no graphics.
- [x] Mesh building moved to the game client (`src/mesh/`).
- [x] `Game` in `game-core/src/game/`: owns the world, creatures, weather, fires, and one
      `PlayerState` per player. Changed only through `Game::apply` (commands) and
      `Game::update` (time), and reports `Event`s.
- [x] Game client drives `Game` through commands (single player works through the same path).

## Next
1. **Network messages**: make `Command`, `Event`, and game state serializable (`serde` +
   `bincode`) and pick a transport (TCP is simplest to start).
2. **Host program**: headless binary that creates a `Game`, accepts connections, calls
   `add_player` for each, runs `update` on a fixed tick, applies incoming commands, and sends
   results back. Build a bare-bones test client first (just walks around).
3. **State sync, host to clients**:
   - On join: the world seed plus all block changes so far.
   - Each tick: player positions, creatures, and each player's own health and resources.
4. **Game client changes**: the client no longer owns the `Game`. It keeps a local copy of what
   the host sends, and sends commands over the connection instead of calling `apply` directly.
5. **Draw other players**: there is no player model or mesh yet.
6. **Single player**: run the host inside the game in a background thread and connect to it, so
   single player and multiplayer use the same path.
7. **Cleanup**:
   - Weather is currently based on the first player only (biome, snow, underwater).
   - Movement will feel laggy over a network without some prediction.
   - Handle disconnects (`remove_player`).
   - Per-player spawn limits for creatures are untested with real second players.
