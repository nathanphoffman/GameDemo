# GameDemo

A small first-person voxel demo written in Rust. You spawn on randomly generated hills made of blocks, with grass and snow zones that blend into each other, trees, and can walk and jump around them.

Built from scratch with `winit` (window and input), `wgpu` (graphics), and `glam` (math).

## Run

```
cargo run --release
```

To get the same world again, pass a seed (the seed is printed on launch):

```
cargo run --release -- 42
```

## Controls

| Key | Action |
| --- | --- |
| W A S D | Move |
| Mouse | Look |
| Space | Jump |
| Left Shift | Sprint |
| Left click | Attack a passive animal (deer) if you are close and looking at it |
| Left click (hold) | Mine the block the crosshair is on |
| Right click | Place a block against the face the crosshair is on |
| Esc | Quit |

## Mining

Hold the left mouse button while the crosshair is on a block. Every 0.55 seconds of holding removes the block under the crosshair (within 5 blocks) and adds to the resource counters in the upper left (see below). Letting go or looking away resets the progress.

| Resource | Where it comes from |
| --- | --- |
| Grey | +1 for any earth block (dirt, grass, snow, stone) |
| White | +1 more for a snow block |
| Green | +1 more for a grass block, +2 for a leaf block |
| Brown | +1 for a tree trunk block (the rarest) |

## Building

Right click places a block instantly, against the face of the block the crosshair is on (within 5 blocks). It is the same type as that block, and it costs what mining that type gives you: dirt or stone cost 1 grey, grass 1 grey + 1 green, snow 1 grey + 1 white, a trunk 1 brown, a leaf 2 green. Nothing is placed if you can't afford it, if the spot is inside you or a creature, or if you are looking at a creature.

## Fighting

You have 50 HP (the bar at the bottom of the screen). Enemies have a health bar over their head. Fighting happens in rounds, and the dice tumble on screen so you can see each roll land.

- **A round:** every hostile creature within 2.3 blocks rolls 2d6 against you at the same time (red dice above your health bar, one pair each). Then you get exactly one attack, 2d6 against the weakest hostile creature within 3 blocks (your dice, at the top of the screen). Damage lands as each set of dice stops.
- **Who goes first:** when a creature starts the round, it hits you first. When you start it by clicking a passive animal, you hit first.
- **Between rounds:** a 2 second pause, then the next round starts if anything hostile is still close.
- **Hostile creatures (skeleton, 12 HP):** when you are within 12 blocks they chase you (slowly) and rounds start on their own. You never need to click, and clicking on them does nothing.
- **Giving up:** get more than 20 blocks away and they give up and walk back to where they started. A skeleton stays hostile and will notice you again; a deer that you provoked calms down and goes back to being passive.
- **Passive creatures (deer, 8 HP):** they stand around and leave you alone. Get close (within 3 blocks), look at one, and click to provoke it. It turns hostile and a round begins with your attack, which goes to the weakest hostile creature in range (which might be a different one).
- **Clicking fast:** clicking a passive animal mid-round skips ahead. Everything in flight lands immediately and the health bar jumps to its final value.
- **Dying:** creatures disappear at 0 HP. If you reach 0 HP the fighting just stops (nothing else happens yet).

## Layout

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, input, rendering |
| `src/world.rs` | Block data, terrain generation, mesh building, collision queries |
| `src/enemy.rs` | Skeleton and deer models, chasing, health bars, dice, ray-vs-box test |
| `src/combat.rs` | Rounds: who rolls, who gets hit, when dice land |
| `src/hud.rs` | Crosshair, rock counter, and mode label (tiny built-in pixel font) |
| `src/resources.rs` | What each mined block gives you |
| `src/player.rs` | Movement, gravity, jumping, collision |
| `src/textures.rs` | Block textures, generated in code |
| `src/shader.wgsl` | Sky, block, and crosshair shaders |

## Tests

```
cargo test --release
```

## Tweaking

- Hill height and steepness: `generate` in `src/world.rs`
- Walk speed, sprint speed, mouse sensitivity, field of view: constants at the top of `src/main.rs`
- Jump height and gravity: constants at the top of `src/player.rs`
