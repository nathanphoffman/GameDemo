mod app;
mod gfx;
mod hud;
mod mesh;
mod music;
mod textures;

// The game's rules live in the library; these keep the `crate::world::...` style paths working.
use game_core::{biome, combat, effects, enemy, game, player, weather, world};

use std::time::{SystemTime, UNIX_EPOCH};

/// What to call the player when they haven't said: their login name.
fn default_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "PLAYER".to_string())
}

fn main() {
    // `gamedemo` or `gamedemo 42` plays alone (42 picks the world); `gamedemo join host:7777
    // [name]` joins a game run by `gamedemo-host`. Coming back with the same name gets your
    // things back.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("join") {
        let Some(addr) = args.get(1) else {
            eprintln!("usage: gamedemo join <address:port> [name]");
            std::process::exit(1);
        };
        let name = args.get(2).cloned().unwrap_or_else(default_name);
        app::run(app::Start::Join { addr: addr.clone(), name });
        return;
    }
    // Optional: pass a number to get the same world again, e.g. `cargo run --release -- 42`.
    let seed = args
        .first()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
                .unwrap_or(1)
        });
    println!("world seed: {seed}");
    app::run(app::Start::Alone { seed, name: default_name() });
}
