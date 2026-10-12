//! The host: runs the game for everyone who connects. No window, no graphics.
//!
//! `gamedemo-host [port] [seed]`

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
    let server = match Server::bind(("0.0.0.0", port), seed) {
        Ok(server) => server,
        Err(e) => {
            eprintln!("can't listen on port {port}: {e}");
            std::process::exit(1);
        }
    };
    println!("world seed: {seed}");
    println!("listening on {}", server.local_addr().unwrap());
    if let Err(e) = server.run() {
        eprintln!("host stopped: {e}");
        std::process::exit(1);
    }
}
