//! A host that is stopped and started again carries on where it was.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use game_core::game::{Command, PlayerInput};
use game_net::{Client, Mirror, Server};

fn pump(client: &mut Client, mirror: &mut Mirror) {
    for update in client.poll().expect("still connected") {
        mirror.apply_update(&update);
    }
}

fn wait(client: &mut Client, mirror: &mut Mirror, what: &str, done: impl Fn(&Mirror) -> bool) {
    let start = Instant::now();
    while !done(mirror) {
        assert!(start.elapsed() < Duration::from_secs(10), "gave up waiting for {what}");
        pump(client, mirror);
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_stopped_host_carries_on_from_its_save() {
    let dir = std::env::temp_dir().join(format!("gamedemo-host-test-{}", std::process::id()));
    let path = dir.join("world.save");
    let _ = std::fs::remove_dir_all(&dir);

    // First run: dig for a bit, then the host is stopped.
    let server = Server::bind("127.0.0.1:0", 3).unwrap().quiet().with_save(&path).unwrap();
    assert!(!server.continues_a_save());
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    let (mut ann, welcome) = Client::connect(addr, "Ann").unwrap();
    let mut view = Mirror::new(&welcome);
    wait(&mut ann, &mut view, "a first update", |m| m.latest.is_some());
    let digging = PlayerInput { mining: true, pitch: -1.5, ..PlayerInput::default() };
    ann.send(Command::SetInput(digging)).unwrap();
    wait(&mut ann, &mut view, "some digging", |m| {
        m.latest.as_ref().is_some_and(|s| s.you.resources.grey > 100)
    });
    ann.send(Command::SetInput(PlayerInput::default())).unwrap();
    thread::sleep(Duration::from_millis(200));
    pump(&mut ann, &mut view);
    let had = view.latest.as_ref().unwrap().you.resources;

    // The host is stopped with Ann still connected.
    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
    assert!(path.exists(), "the save was written when the host stopped");
    drop(ann);

    // Second run: a different seed was asked for, but the save's is used.
    let server = Server::bind("127.0.0.1:0", 99).unwrap().quiet().with_save(&path).unwrap();
    assert!(server.continues_a_save());
    assert_eq!(server.seed(), 3);
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    let (mut ann, welcome) = Client::connect(addr, "ann").unwrap();
    assert_eq!(welcome.seed, 3);
    assert!(welcome.rejoined, "she is remembered");
    assert!(!welcome.blocks.is_empty(), "the holes she dug are still there");
    let mut view = Mirror::new(&welcome);
    wait(&mut ann, &mut view, "a first update", |m| m.latest.is_some());
    assert_eq!(view.latest.as_ref().unwrap().you.resources, had);

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();

    // A damaged save is not written over.
    std::fs::write(&path, b"GDSVjunk").unwrap();
    let refused = Server::bind("127.0.0.1:0", 3).unwrap().with_save(&path);
    assert!(refused.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"GDSVjunk");
    let _ = std::fs::remove_dir_all(&dir);
}
