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
| Left click | Attack the skeleton if the crosshair is on him |
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

## Attacking

A skeleton stands a few blocks ahead of where you spawn. Aim at him (within 5 blocks): he gets a wireframe highlight instead of the block behind him. Every left click rolls 2d6: two dice tumble across the top of the screen, land, and show the sum and the damage, which is applied as they land. Clicking again mid-roll starts a new roll. He starts at 12 health, shows cracks at 6 or less, and disappears at 0.

## Layout

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, input, rendering |
| `src/world.rs` | Block data, terrain generation, mesh building, collision queries |
| `src/enemy.rs` | Skeleton model, health, dice, ray-vs-box test |
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
