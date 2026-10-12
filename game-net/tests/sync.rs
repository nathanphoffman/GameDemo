//! A real host on localhost with real connections: players see each other, and the world
//! they rebuild matches.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use game_core::game::{Command, PlayerInput};
use game_core::world::{Block, World};
use game_net::{Client, Mirror, Server};

const SEED: u32 = 3;

/// Takes in whatever has arrived for `client`.
fn pump(client: &mut Client, mirror: &mut Mirror) {
    for tick in client.poll().expect("still connected") {
        mirror.apply(&tick);
    }
}

/// Keeps pumping both until `done` is true, or fails after a few seconds.
fn wait_for(
    what: &str,
    a: (&mut Client, &mut Mirror),
    b: (&mut Client, &mut Mirror),
    done: impl Fn(&Mirror, &Mirror) -> bool,
) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        pump(a.0, a.1);
        pump(b.0, b.1);
        if done(a.1, b.1) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("gave up waiting for: {what}");
}

/// The cells in a little patch of the world, to compare worlds without comparing all of it.
fn patch(world: &World, around: (i32, i32)) -> Vec<Block> {
    let mut out = Vec::new();
    for z in around.1 - 4..=around.1 + 4 {
        for x in around.0 - 4..=around.0 + 4 {
            for y in 0..64 {
                out.push(world.get(x, y, z));
            }
        }
    }
    out
}

#[test]
fn players_see_each_other_and_the_same_world() {
    let server = Server::bind("127.0.0.1:0", SEED).unwrap();
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    let (mut a, welcome_a) = Client::connect(addr).unwrap();
    let (mut b, welcome_b) = Client::connect(addr).unwrap();
    assert_ne!(welcome_a.you, welcome_b.you);
    assert_eq!(welcome_a.seed, SEED);
    assert!(welcome_a.blocks.is_empty(), "nothing has been changed yet");
    let mut mirror_a = Mirror::new(&welcome_a);
    let mut mirror_b = Mirror::new(&welcome_b);

    // Both are in each other's snapshots.
    wait_for("both players in both views", (&mut a, &mut mirror_a), (&mut b, &mut mirror_b), |ma, mb| {
        let both = |m: &Mirror| m.latest.as_ref().is_some_and(|s| s.players.len() == 2);
        both(ma) && both(mb)
    });

    // A digs straight down; B's copy of the world has to change too.
    let spot = {
        let s = mirror_a.latest.as_ref().unwrap();
        let me = s.players.iter().find(|p| p.id == mirror_a.you).unwrap();
        (me.pos.x.floor() as i32, me.pos.z.floor() as i32)
    };
    let before = patch(&mirror_a.world, spot);
    let digging = PlayerInput {
        mining: true,
        pitch: -1.5,
        ..PlayerInput::default()
    };
    a.send(Command::SetInput(digging)).unwrap();
    wait_for("the dug block reaching B", (&mut a, &mut mirror_a), (&mut b, &mut mirror_b), |ma, mb| {
        patch(&ma.world, spot) != before && patch(&mb.world, spot) != before
    });
    a.send(Command::SetInput(PlayerInput::default())).unwrap();
    // Let the last changes land, then both copies must agree.
    thread::sleep(Duration::from_millis(300));
    pump(&mut a, &mut mirror_a);
    pump(&mut b, &mut mirror_b);
    assert_eq!(patch(&mirror_a.world, spot), patch(&mirror_b.world, spot));

    // A's resources went up, and only A's.
    let has = |m: &Mirror| m.latest.as_ref().unwrap().you.resources;
    assert_ne!(has(&mirror_a), has(&mirror_b));

    // Someone joining late is told about the dig in their welcome.
    let (mut c, welcome_c) = Client::connect(addr).unwrap();
    assert!(!welcome_c.blocks.is_empty());
    let mirror_c = Mirror::new(&welcome_c);
    assert_eq!(patch(&mirror_c.world, spot), patch(&mirror_b.world, spot));
    assert!(c.wait_tick(Duration::from_secs(2)).unwrap().is_some());

    // When A goes away the others stop seeing them.
    let a_id = welcome_a.you;
    drop(a);
    let start = Instant::now();
    loop {
        pump(&mut b, &mut mirror_b);
        let gone = mirror_b
            .latest
            .as_ref()
            .is_some_and(|s| s.players.iter().all(|p| p.id != a_id) && s.players.len() == 2);
        if gone {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "A never disappeared");
        thread::sleep(Duration::from_millis(10));
    }

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
}

#[test]
fn a_host_turns_away_a_player_with_the_wrong_version() {
    use std::io::{Read, Write};
    let server = Server::bind("127.0.0.1:0", SEED).unwrap();
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    // A hello with a version nobody speaks: [length][variant 0 = Hello][version as a varint].
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.write_all(&[2, 0, 0, 0, 0, 99]).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut reply = Vec::new();
    let _ = stream.read_to_end(&mut reply);
    assert!(!reply.is_empty(), "it says why");

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
}
