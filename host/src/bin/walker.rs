//! A bare-bones player with no window: connects to a host and walks around, digging now and
//! then, and prints what the host tells it. For checking the host works.
//!
//! `walker [address] [seconds]`

use std::time::{Duration, Instant};

use game_core::game::{Command, PlayerInput};
use game_net::{Client, Mirror};

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = args.next().unwrap_or_else(|| "127.0.0.1:7777".into());
    let seconds: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(20.0);

    let (mut client, welcome) = Client::connect(&addr).unwrap_or_else(|e| {
        eprintln!("can't join {addr}: {e}");
        std::process::exit(1);
    });
    println!(
        "joined as player {}, seed {}, {} changed blocks",
        welcome.you.0,
        welcome.seed,
        welcome.blocks.len()
    );
    let mut mirror = Mirror::new(&welcome);

    let start = Instant::now();
    let mut next_report = 1.0;
    let mut block_changes = 0usize;
    while start.elapsed().as_secs_f32() < seconds {
        let t = start.elapsed().as_secs_f32();
        // Walk forward, slowly turning, and dig for a second out of every five.
        let input = PlayerInput {
            forward: true,
            sprint: true,
            jump: (t * 2.0) as u32 % 3 == 0,
            mining: (t as u32) % 5 == 4,
            yaw: t * 0.4,
            pitch: if (t as u32) % 5 == 4 { -1.2 } else { 0.0 },
            ..PlayerInput::default()
        };
        if client.send(Command::SetInput(input)).is_err() {
            break;
        }
        match client.wait_tick(Duration::from_millis(100)) {
            Ok(Some(tick)) => {
                block_changes += tick.blocks.len();
                mirror.apply(&tick);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("lost the host: {e}");
                break;
            }
        }
        if t >= next_report {
            next_report += 1.0;
            if let Some(s) = &mirror.latest {
                let me = s.players.iter().find(|p| p.id == mirror.you);
                println!(
                    "tick {:5}  me {:?}  players {}  creatures {}  hp {}  blocks changed {}",
                    mirror.last_tick,
                    me.map(|p| (p.pos.x.round(), p.pos.y.round(), p.pos.z.round())),
                    s.players.len(),
                    s.enemies.len(),
                    s.you.hp,
                    block_changes
                );
            }
        }
    }
}
