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
| Esc | Quit |

## Mining

Hold the left mouse button while the crosshair is on a block. Every two-thirds of a second of holding removes the block under the crosshair (within 5 blocks) and adds to the resource counters in the upper left (see below). Letting go or looking away resets the progress.

| Resource | Where it comes from |
| --- | --- |
| Grey | +1 for any earth block (dirt, grass, snow, stone) |
| White | +1 more for a snow block |
| Green | +1 more for a grass block, +2 for a leaf block |
| Brown | +1 for a tree trunk block (the rarest) |

## Fighting

You have 50 HP (the bar at the bottom of the screen). Enemies have a health bar over their head. Every fight is an exchange of two rolls: whoever starts rolls 2d6, and when those dice land the other side always rolls back. Dice tumble on screen (yours at the top, red ones above your health bar) and damage lands as they stop.

- **Hostile enemies (skeleton, 12 HP):** they chase you (slower than you walk) and attack on their own when they get close. You automatically hit back. No clicking needed, and clicking on them does nothing.
- **Passive enemies (deer, 8 HP):** they stand around and leave you alone. Get close (within 3 blocks), look at one, and left click to attack it. After that it turns hostile and fights like the skeleton.
- **Pause:** after a fight ends, hostile enemies wait a couple of seconds before starting another.
- **Clicking fast:** clicking a passive enemy again mid-fight skips ahead. Everything in flight lands immediately and the health bar jumps to its final value.
- **Dying:** enemies disappear at 0 HP. If you reach 0 HP the fight just stops (nothing else happens yet).

## Layout

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, input, rendering |
| `src/world.rs` | Block data, terrain generation, mesh building, collision queries |
| `src/enemy.rs` | Skeleton and deer models, chasing, health bars, dice, ray-vs-box test |
| `src/combat.rs` | Who attacks whom, when dice land, damage, replies |
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
