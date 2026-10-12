//! The host: runs the game for everyone who connects. No window, no graphics.
//!
//! `gamedemo-host [port] [seed] [save file]`
//!
//! The game is kept in the save file (`host.save` unless another is named): it is written every
//! half minute and when the host is stopped with Ctrl-C, and the next start carries on from it
//! (the seed is then the saved one). To start a new world, name a different save file or move
//! the old one away.

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use game_net::Server;

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(7777);
    let seed = args.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
            .unwrap_or(1)
    });
    let save_file = args.next().unwrap_or_else(|| "host.save".to_string());

    let server = Server::bind(("0.0.0.0", port), seed)
        .and_then(|server| server.with_save(&save_file))
        .unwrap_or_else(|e| {
            eprintln!("can't start: {e}");
            std::process::exit(1);
        });
    if server.continues_a_save() {
        println!("carrying on from {save_file}");
    } else {
        println!("new world, saved in {save_file}");
    }
    println!("world seed: {}", server.seed());
    println!("listening on {}", server.local_addr().unwrap());

    // Ctrl-C asks the game to stop, which writes the save first.
    let stop = server.stop_flag();
    if ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed)).is_err() {
        eprintln!("(Ctrl-C will not save: the game is saved every 30 seconds)");
    }
    if let Err(e) = server.run() {
        eprintln!("host stopped: {e}");
        std::process::exit(1);
    }
    println!("saved to {save_file}");
}
