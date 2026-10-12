mod app;
mod gfx;
mod hud;
mod mesh;
mod music;
mod narrate;
mod textures;

// The game's rules live in the library; these keep the `crate::world::...` style paths working.
use game_core::{biome, combat, effects, enemy, game, player, weather, world};

/// What to call the player when they haven't said: their login name.
fn default_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "PLAYER".to_string())
}

fn main() {
    // `gamedemo` carries on your saved game (or starts one); `gamedemo new [seed]` (or just
    // `gamedemo 42`) starts a new world, keeping the old save as `single.save.bak`;
    // `gamedemo join host:7777 [name]` joins a game run by `gamedemo-host`. Coming back with
    // the same name gets your things back.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = default_name();
    let save = std::path::PathBuf::from("saves/single.save");
    match args.first().map(String::as_str) {
        Some("join") => {
            let Some(addr) = args.get(1) else {
                eprintln!("usage: gamedemo join <address:port> [name]");
                std::process::exit(1);
            };
            let name = args.get(2).cloned().unwrap_or(name);
            app::run(app::Start::Join { addr: addr.clone(), name });
        }
        Some("new") => {
            let seed = args.get(1).and_then(|s| s.parse::<u32>().ok());
            app::run(app::Start::Alone { seed, fresh: true, save, name });
        }
        Some(number) if number.parse::<u32>().is_ok() => {
            let seed = number.parse::<u32>().ok();
            app::run(app::Start::Alone { seed, fresh: true, save, name });
        }
        Some(other) => {
            eprintln!("don't know what {other:?} means; try `gamedemo`, `gamedemo new [seed]` or `gamedemo join <address:port> [name]`");
            std::process::exit(1);
        }
        None => app::run(app::Start::Alone { seed: None, fresh: false, save, name }),
    }
}
