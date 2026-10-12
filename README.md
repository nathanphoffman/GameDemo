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
| Left click (hold) | Mine the targeted block |
| Scroll wheel | Switch mode: mine / attack / off |
| Esc | Quit |

## Mining

Hold the left mouse button in mine mode. Every 3 seconds of holding removes the block under the crosshair (within 5 blocks) and adds 1 to the rock counter in the upper left. Letting go resets the progress. 
## Attacking

A skeleton stands a few blocks ahead of where you spawn. Scroll to attack mode and aim at him (within 5 blocks): he gets a wireframe highlight. While you hold the left button, every second rolls 2d6, shows the roll in the upper left, and deals that much damage. He starts at 12 health, shows cracks at 6 or less, and disappears at 0. Off mode does nothing.

## Layout

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, input, rendering |
| `src/world.rs` | Block data, terrain generation, mesh building, collision queries |
| `src/enemy.rs` | Skeleton model, health, dice, ray-vs-box test |
| `src/hud.rs` | Crosshair, rock counter, and mode label (tiny built-in pixel font) |
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
