mod app;
mod gfx;
mod hud;
mod mesh;
mod music;
mod textures;

// The game's rules live in the library; these keep the `crate::world::...` style paths working.
use gamedemo::{biome, combat, effects, enemy, player, resources, weather, world};

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Optional: pass a number to get the same world again, e.g. `cargo run --release -- 42`.
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
                .unwrap_or(1)
        });
    println!("world seed: {seed}");
    app::run(seed);
}
