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
| Ctrl (hold) + left click | On a resource in the list: pick what the converter works on |
| Middle click or E | Open the apply menu on the block the crosshair is on |
| Esc | Close the apply menu, or quit |

## Mining

Hold the left mouse button while the crosshair is on a block. Every 0.55 seconds of holding removes the block under the crosshair (within 5 blocks) and adds to the resource counters in the upper left (see below). Letting go or looking away resets the progress.

| Resource | Where it comes from |
| --- | --- |
| Grey | +1 for any earth block (dirt, grass, snow, stone) |
| White | +1 more for a snow block |
| Green | +1 more for a grass block, +2 for an oak leaf block, +1 for a pine needle block |
| Brown | +1 for an oak trunk block, +2 for a pine trunk block |
| Black | +1 for a coal block (see below) |

## Converting

Each resource row shows its count, then a triangle of the same color with the count of its dense version. A single conversion pipeline works on one resource at a time. Hold Ctrl (the mouse is freed and the rows light up) and click a row to select it; click it again to turn the pipeline off. Every 10 seconds, if you have 4 of the selected resource, they are turned into 1 dense one. The row's backing fills as the wait goes. Dense resources aren't used for anything yet.

## Applying resources

Look at a block and press E (or middle click) to open the apply menu. The mouse is freed and a list of your dense (triangle) resources appears. Click one to apply it to that block; click outside the menu or press Esc to close it. If a resource does nothing to that block, nothing is used up.

| Apply this | to this | What happens |
| --- | --- | --- |
| Flint (dense grey) | grass, leaves, or wood | Sets it on fire. After 10 seconds it burns out into a black coal block (+1 black when mined). |
| Dense white | a burning block | Puts the fire out and leaves a water block instead of coal. |
| Dense white | grass | Turns it into snow. |
| Dense green (seed) | dirt | Turns it into grass. |
| Dense green | grass | Grows an oak, the native tree of the grasslands, if there is room. |
| Dense green | snow | Grows a pine, the native tree of the snow, if there is room. |
| Dense brown (wood) | grass, dirt, snow or stone | Turns it into a wooden floor. |
| Flint (dense grey) | a wooden floor | Turns it into a stone floor (for later). |
| Dense green | a wooden floor | Puts a house plant on top of it. |
| Dense black (carbon) | a wooden floor | Builds a forge on top of it. While you are within 6 blocks of a forge, the resource converter runs twice as fast ("X2" shows next to the row). |

Mining a block that is still burning, a water block, or any of the crafted blocks gives nothing, and none of them can be placed or bought. Trees need a clear column above the block, and things built on a floor need the cell above it free. The menu only labels the rows that would work.

## Building

Right click places a block instantly, against the face of the block the crosshair is on (within 5 blocks). It is the same type as that block, and it costs what mining that type gives you: dirt or stone cost 1 grey, grass 1 grey + 1 green, snow 1 grey + 1 white, an oak trunk 1 brown, a pine trunk 2 brown, an oak leaf 2 green, a pine needle 1 green. Nothing is placed if you can't afford it, if the spot is inside you or a creature, or if you are looking at a creature.

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
| `src/effects.rs` | What applying a resource to a block does, and burning timers |
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
