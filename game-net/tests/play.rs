//! A scripted play session over a real connection: place, build, apply resources, fight, and
//! another player watching. Stands in for a person at the keyboard.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use game_core::combat::MAX_PLAYER_HP;
use game_core::game::{Command, Event, PlayerInput};
use game_core::glam::Vec3;
use game_core::world::{Block, World};
use game_net::{Client, Mirror, Server, Update};

const SEED: u32 = 3;
const WOOD: usize = 3;
const CARBON: usize = 4;

struct Player {
    client: Client,
    mirror: Mirror,
    events: Vec<Event>,
}

impl Player {
    fn join(addr: std::net::SocketAddr, name: &str) -> Player {
        let (client, welcome) = Client::connect(addr, name).unwrap();
        Player { client, mirror: Mirror::new(&welcome), events: Vec::new() }
    }

    fn pump(&mut self) {
        for update in self.client.poll().expect("still connected") {
            if let Update::Tick(tick) = &update {
                self.events.extend(tick.events.iter().copied());
            }
            self.mirror.apply_update(&update);
        }
    }

    fn me(&self) -> game_core::game::PlayerView {
        let s = self.mirror.latest.as_ref().unwrap();
        *s.players.iter().find(|p| p.id == self.mirror.you).unwrap()
    }

    fn send(&mut self, command: Command) {
        self.client.send(command).unwrap();
    }

    /// Stands at `pos` looking at `target` (the eye is 1.62 above the feet).
    fn stand_and_look(&mut self, pos: Vec3, target: Vec3) {
        let d = target - (pos + Vec3::new(0.0, 1.62, 0.0));
        let yaw = d.x.atan2(-d.z);
        let pitch = d.y.atan2(d.x.hypot(d.z));
        self.send(Command::SetPose { pos, yaw, pitch, epoch: 0 });
    }
}

/// Pumps every player until `done` holds, or fails after `seconds`.
fn until(what: &str, seconds: u64, players: &mut [&mut Player], done: impl Fn(&[&mut Player]) -> bool) {
    let start = Instant::now();
    loop {
        for p in players.iter_mut() {
            p.pump();
        }
        if done(players) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(seconds), "gave up waiting for: {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn dense(p: &Player, kind: usize) -> u32 {
    p.mirror.latest.as_ref().unwrap().you.resources.dense[kind]
}

#[test]
fn a_session_of_building_and_fighting() {
    let server = Server::bind("127.0.0.1:0", SEED).unwrap();
    let addr = server.local_addr().unwrap();
    let stop = server.stop_flag();
    let host = thread::spawn(move || server.run().unwrap());

    let mut a = Player::join(addr, "Ann");
    let mut b = Player::join(addr, "Bob");
    until("both settled", 10, &mut [&mut a, &mut b], |ps| {
        ps.iter().all(|p| p.mirror.latest.as_ref().is_some_and(|s| s.players.len() == 2))
    });
    // Let them land on the ground.
    thread::sleep(Duration::from_millis(1000));
    until("a tick", 5, &mut [&mut a, &mut b], |_| true);
    let spawn = a.me().pos;

    // --- build a wooden floor on the ground a few blocks away ---
    let mut floor = None;
    for dx in 3..7 {
        let (x, z) = (spawn.x.floor() as i32 + dx, spawn.z.floor() as i32);
        let top = (0..64).rev().find(|&y| a.mirror.world.get(x, y, z) != Block::Air).unwrap();
        if matches!(a.mirror.world.get(x, top, z), Block::Grass | Block::Dirt | Block::Sand) {
            floor = Some((x, top, z));
            break;
        }
    }
    let (fx, fy, fz) = floor.expect("some open ground nearby");
    let look = Vec3::new(fx as f32 + 0.5, fy as f32 + 0.9, fz as f32 + 0.5);
    a.stand_and_look(spawn, look);
    thread::sleep(Duration::from_millis(100));
    let wood_before = dense(&a, WOOD);
    a.send(Command::ApplyToTarget { kind: WOOD });
    until("a wooden floor", 5, &mut [&mut a, &mut b], |ps| {
        (fx - 1..=fx + 1).any(|x| (fy - 1..=fy + 1).any(|y| ps[0].mirror.world.get(x, y, fz) == Block::WoodFloor))
    });
    until("the wood being used", 5, &mut [&mut a, &mut b], |ps| dense(ps[0], WOOD) == wood_before - 1);
    until("b seeing the floor too", 5, &mut [&mut a, &mut b], |ps| {
        (fx - 1..=fx + 1).any(|x| (fy - 1..=fy + 1).any(|y| ps[1].mirror.world.get(x, y, fz) == Block::WoodFloor))
    });

    // --- build a forge on top of it (carbon on a wooden floor) ---
    let floor_cell = (fx - 1..=fx + 1)
        .flat_map(|x| (fy - 1..=fy + 1).map(move |y| (x, y, fz)))
        .find(|&(x, y, z)| a.mirror.world.get(x, y, z) == Block::WoodFloor)
        .unwrap();
    a.send(Command::ApplyAt { kind: CARBON, pos: floor_cell });
    let above = (floor_cell.0, floor_cell.1 + 1, floor_cell.2);
    until("a forge", 5, &mut [&mut a, &mut b], |ps| ps[0].mirror.world.get(above.0, above.1, above.2) == Block::Forge);
    until("b seeing the forge", 5, &mut [&mut a, &mut b], |ps| ps[1].mirror.world.get(above.0, above.1, above.2) == Block::Forge);

    // --- place and mine a block, with b watching the cracks ---
    // Look at the ground a little ahead and put a block down against it.
    let ahead = Vec3::new(spawn.x - 3.0, spawn.y - 0.2, spawn.z);
    a.stand_and_look(spawn, ahead);
    thread::sleep(Duration::from_millis(100));
    a.send(Command::PlaceBlock);
    thread::sleep(Duration::from_millis(300));
    a.pump();

    let down = PlayerInput { mining: true, pitch: -1.4, ..PlayerInput::default() };
    a.send(Command::SetPose { pos: spawn, yaw: 0.0, pitch: -1.4, epoch: 0 });
    a.send(Command::SetInput(down));
    until("b seeing a crack and hearing a dig", 10, &mut [&mut a, &mut b], |ps| {
        let cracking = ps[1].mirror.latest.as_ref().unwrap().players.iter().any(|p| p.id == ps[0].mirror.you && p.cracking.is_some());
        let dug = ps[1].events.iter().any(|e| matches!(e, Event::Dug { player, .. } if *player == ps[0].mirror.you));
        cracking && dug
    });
    a.send(Command::SetInput(PlayerInput { pitch: -1.4, ..PlayerInput::default() }));

    // --- pick a fight with the deer ---
    let deer = *a
        .mirror
        .latest
        .as_ref()
        .unwrap()
        .enemies
        .iter()
        .find(|e| e.kind == game_core::enemy::Kind::Deer)
        .expect("a deer near the spawn point");
    assert!(!deer.hostile);
    let flat = Vec3::new(spawn.x - deer.pos.x, 0.0, spawn.z - deer.pos.z).normalize();
    let stand = deer.pos + flat * 2.0;
    a.stand_and_look(stand, deer.pos + Vec3::new(0.0, 1.0, 0.0));
    thread::sleep(Duration::from_millis(150));
    a.send(Command::Attack);
    until("the deer turning hostile", 5, &mut [&mut a, &mut b], |ps| {
        ps[0].mirror.latest.as_ref().unwrap().enemies.iter().any(|e| e.id == deer.id && e.hostile)
    });
    // Someone gets hurt: the deer loses health (or dies) or the player does. b sees it too.
    until("a hurt deer or player", 30, &mut [&mut a, &mut b], |ps| {
        let s = ps[1].mirror.latest.as_ref().unwrap();
        let deer_hurt = s.enemies.iter().find(|e| e.id == deer.id).is_none_or(|e| e.health < deer.health);
        let player_hurt = s.players.iter().any(|p| p.id == ps[0].mirror.you && p.hp < MAX_PLAYER_HP);
        deer_hurt || player_hurt
    });

    // Nothing above should have needed the world to be rebuilt differently on the two sides.
    let baseline = World::generate(SEED);
    let differs = |w: &World| {
        let mut n = 0;
        for x in fx - 8..fx + 8 {
            for z in fz - 8..fz + 8 {
                for y in 0..40 {
                    if w.get(x, y, z) != baseline.get(x, y, z) {
                        n += 1;
                    }
                }
            }
        }
        n
    };
    a.pump();
    b.pump();
    assert!(differs(&a.mirror.world) >= 2);
    assert_eq!(differs(&a.mirror.world), differs(&b.mirror.world));

    stop.store(true, Ordering::Relaxed);
    host.join().unwrap();
}
