//! A real host on localhost with real connections: players see each other, and the world
//! they rebuild matches.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use game_core::game::{Command, PlayerInput};
use game_core::world::{Block, World};
use game_net::{Client, Mirror, Server, Update};

const SEED: u32 = 3;

/// Takes in whatever has arrived for `client`.
fn pump(client: &mut Client, mirror: &mut Mirror) {
    for update in client.poll().expect("still connected") {
        mirror.apply_update(&update);
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

    let (mut a, welcome_a) = Client::connect(addr, "Ann").unwrap();
    let (mut b, welcome_b) = Client::connect(addr, "Bob").unwrap();
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
    let (mut c, welcome_c) = Client::connect(addr, "Cy").unwrap();
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

    // A hello with a version nobody speaks: [length][variant 0 = Hello][version][empty name].
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream.write_all(&[3, 0, 0, 0, 0, 99, 0]).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut reply = Vec::new();
    let _ = stream.read_to_end(&mut reply);
    assert!(!reply.is_empty(), "it says why");

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
}

/// Pumps `client` for a bit and returns the chat lines that came (as "from: text").
fn chat_for(client: &mut Client, mirror: &mut Mirror, ms: u64) -> Vec<String> {
    let mut lines = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {
        for update in client.poll().expect("still connected") {
            if let Update::Chat { from, text } = &update {
                lines.push(format!("{from}: {text}"));
            }
            mirror.apply_update(&update);
        }
        thread::sleep(Duration::from_millis(10));
    }
    lines
}

#[test]
fn names_chat_and_coming_back() {
    let server = Server::bind("127.0.0.1:0", SEED).unwrap().quiet();
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    let (mut ann, welcome) = Client::connect(addr, "  Ann\u{7}  ").unwrap();
    assert_eq!(welcome.name, "Ann", "tidied up");
    assert!(!welcome.rejoined);
    let mut ann_view = Mirror::new(&welcome);
    let ann_id = welcome.you;

    // Two players can't be called the same thing.
    let refused = Client::connect(addr, "ann").err().expect("refused");
    assert!(refused.to_string().contains("already playing"), "{refused}");

    let (mut bob, welcome_b) = Client::connect(addr, "Bob").unwrap();
    let mut bob_view = Mirror::new(&welcome_b);
    let said = chat_for(&mut ann, &mut ann_view, 300);
    assert!(said.contains(&": Bob joined".to_string()), "{said:?}");
    assert_eq!(ann_view.name_of(welcome_b.you), Some("Bob"));
    assert_eq!(ann_view.name_of(ann_id), Some("Ann"));

    // Chat goes to everyone, with the sender's name, and nothing can inject control characters.
    bob.say("hello there\u{1b}[31m").unwrap();
    bob.say("   ").unwrap();
    let heard = chat_for(&mut ann, &mut ann_view, 300);
    assert_eq!(heard, vec!["Bob: hello there[31m".to_string()], "{heard:?}");
    let own = chat_for(&mut bob, &mut bob_view, 100);
    assert!(own.contains(&"Bob: hello there[31m".to_string()), "{own:?}");

    // Ann digs for a moment, leaves, and comes back as herself.
    let digging = PlayerInput { mining: true, pitch: -1.5, ..PlayerInput::default() };
    ann.send(Command::SetInput(digging)).unwrap();
    let start = Instant::now();
    let dug = |m: &Mirror| m.latest.as_ref().is_some_and(|s| s.you.resources.grey > 100);
    while !dug(&ann_view) {
        assert!(start.elapsed() < Duration::from_secs(10), "never dug");
        pump(&mut ann, &mut ann_view);
        thread::sleep(Duration::from_millis(10));
    }
    ann.send(Command::SetInput(PlayerInput::default())).unwrap();
    thread::sleep(Duration::from_millis(100));
    pump(&mut ann, &mut ann_view);
    let had = ann_view.latest.as_ref().unwrap().you.resources;
    drop(ann);
    let said = chat_for(&mut bob, &mut bob_view, 400);
    assert!(said.contains(&": Ann left".to_string()), "{said:?}");
    assert_eq!(bob_view.name_of(ann_id), None);

    let (mut ann, welcome) = Client::connect(addr, "ANN").unwrap();
    assert!(welcome.rejoined);
    assert_ne!(welcome.you, ann_id, "a new number");
    let mut ann_view = Mirror::new(&welcome);
    let start = Instant::now();
    while ann_view.latest.is_none() {
        assert!(start.elapsed() < Duration::from_secs(5));
        pump(&mut ann, &mut ann_view);
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(ann_view.latest.as_ref().unwrap().you.resources, had);
    let said = chat_for(&mut bob, &mut bob_view, 300);
    assert!(said.contains(&": ANN came back".to_string()), "{said:?}");

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
}
