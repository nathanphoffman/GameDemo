//! The game client: the window, the input, and drawing what the host says is going on.
//!
//! The game itself (the rules, the creatures, everyone's resources) runs on a host. In single
//! player that is a host running on its own thread inside this program; in a network game it
//! is someone else's computer. Either way this program only keeps a copy of what it needs to
//! draw, and sends what the player does as commands.

mod frame;
mod input;
mod menu;

use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::CursorGrabMode;

use glam::{Mat4, Vec3};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};
use std::sync::Arc;

use game_net::{Client, Mirror, Server, Update};

use crate::combat::MAX_PLAYER_HP;
use crate::effects::{self, Effect};
use crate::enemy::Sight;
use crate::game::sights::{self, Creature};
use crate::game::{self, Command, Event, PlayerId, PlayerInput, MINE_INTERVAL};
use crate::gfx::{Gfx, FOV_Y_DEGREES};
use crate::hud::{self, HudInfo};
use crate::mesh::{self, Vertex};
use crate::music;
use crate::player::Player;
use crate::weather::Weather;
use crate::world::{self, World};
use input::Keys;

const MOUSE_SENSITIVITY: f32 = 0.0022;
// How fast the health bar's draining chunk catches up, in hp per second.
const HP_DRAIN_PER_SECOND: f32 = 30.0;
// How far away (in blocks) other players' digging can be heard.
const HEARING_RANGE: f32 = 40.0;
// Our position is sent to the host at most this often (seconds), unless we do something.
const POSE_INTERVAL: f32 = 1.0 / 60.0;

/// How to start.
pub(crate) enum Start {
    /// Play alone, on a host that runs inside this program, keeping the game in `save`. With
    /// `fresh`, a new world (of `seed`, if given) replaces the saved one (which is kept as
    /// `save.bak`); otherwise the saved game is carried on, if there is one.
    Alone { seed: Option<u32>, fresh: bool, save: std::path::PathBuf, name: String },
    /// Join a host at this address.
    Join { addr: String, name: String },
}

/// Where everyone is right now, smoothed between the host's updates.
#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) players: Vec<game::PlayerView>,
    pub(crate) enemies: Vec<game::EnemyView>,
}

pub(crate) struct App {
    gfx: Option<Gfx>,
    /// The connection to the host.
    net: Client,
    /// The host's game as far as we know: the world, and the latest update.
    mirror: Mirror,
    me: PlayerId,
    /// Our own body. It is walked here, in our copy of the world, so moving feels instant, and
    /// the host is told where it ended up.
    body: Player,
    /// Keys held, and which way we are looking (kept in step with `body`).
    input: PlayerInput,
    keys: Keys,
    /// The number the host last gave us for where we are: it changes when the host moves us
    /// (we died), and our positions are sent with it so the host knows they are up to date.
    epoch: u32,
    /// What the host was last told about our position, and when.
    sent_pose: (Vec3, f32, f32),
    sent_pose_at: Instant,
    scene: Scene,
    /// When the latest update from the host arrived, for smoothing movement between them.
    update_at: Instant,
    /// Our picture of the weather: the kind comes from the host, how it looks is worked out here.
    weather: Weather,
    last_frame: Instant,
    /// When the game started, for things that move with time (the clouds).
    started: Instant,
    /// The apply menu, open on this block, or None.
    menu: Option<(i32, i32, i32)>,
    /// What right click does: `None` clones the block it is aimed at (normal block placing),
    /// `Some(kind)` applies that dense resource to it. Changed with the scroll wheel.
    selected_dense: Option<usize>,
    /// Last known mouse position in window pixels (only meaningful while Ctrl frees it).
    cursor_pos: (f32, f32),
    /// Health shown on the bar; trails the real value after a hit so the loss is visible.
    hp_shown: f32,
    /// The window has keyboard focus (and so should own the mouse).
    focused: bool,
    /// The OS really locked the pointer. If not, it is only confined, and the game keeps
    /// pulling it back to the middle of the window so it can't wander onto another monitor.
    cursor_locked: bool,
    /// The host went away; the window closes.
    connection_lost: bool,
    /// Chat lines (already in showable form) and when they arrived.
    chat_log: Vec<(Instant, String)>,
    /// The line being typed, while the chat is open.
    chat_input: Option<String>,
    /// Tab is held: show who is playing.
    show_players: bool,
    music: Option<music::Music>,
}

impl App {
    fn new(net: Client, mirror: Mirror) -> Self {
        let me = mirror.you;
        let snapshot = mirror.latest.as_ref().expect("the host has sent an update");
        let start = snapshot
            .players
            .iter()
            .find(|p| p.id == me)
            .map_or(Vec3::ZERO, |p| p.pos);
        let epoch = snapshot.you.epoch;
        let seed = 0x5EA7;
        let now = Instant::now();
        let mut app = App {
            gfx: None,
            net,
            mirror,
            me,
            body: Player::new(start),
            input: PlayerInput::default(),
            keys: Keys::default(),
            epoch,
            sent_pose: (start, 0.0, 0.0),
            sent_pose_at: now,
            scene: Scene::default(),
            update_at: now,
            // The weather never changes by itself here: the host says what it is.
            weather: Weather::with_settings(seed, f32::MAX, 0.0),
            last_frame: now,
            started: now,
            menu: None,
            selected_dense: None,
            cursor_pos: (0.0, 0.0),
            hp_shown: MAX_PLAYER_HP as f32,
            focused: true,
            cursor_locked: false,
            connection_lost: false,
            chat_log: Vec::new(),
            chat_input: None,
            show_players: false,
            music: music::Music::start(),
        };
        app.refresh_scene();
        app
    }

    /// What the host last told us about our own numbers.
    fn you(&self) -> &game::YouView {
        &self.mirror.latest.as_ref().expect("the host has sent an update").you
    }

    /// Takes in whatever the host has sent: the world changes, the new picture of everyone, and
    /// what happened (for sounds).
    fn receive(&mut self) {
        let updates = match self.net.poll() {
            Ok(updates) => updates,
            Err(e) => {
                eprintln!("lost the connection to the host: {e}");
                self.connection_lost = true;
                return;
            }
        };
        for update in &updates {
            self.mirror.apply_update(update);
            match update {
                Update::Tick(tick) => self.take_in(tick),
                Update::Chat { from, text } => {
                    let line = if from.is_empty() {
                        format!("* {text}")
                    } else {
                        format!("{from}: {text}")
                    };
                    self.chat_log.push((Instant::now(), hud::display_text(&line)));
                    // Keep the log short: only the last few lines are ever shown.
                    let extra = self.chat_log.len().saturating_sub(40);
                    self.chat_log.drain(..extra);
                }
                Update::Roster(_) => {}
            }
        }
    }

    /// One update of the game from the host: the sounds and effects of what happened.
    fn take_in(&mut self, tick: &game_net::Tick) {
        self.update_at = Instant::now();
        // The host moved us (we died): go where it says, and send positions with its number.
        if tick.snapshot.you.epoch != self.epoch {
            self.epoch = tick.snapshot.you.epoch;
            if let Some(me) = tick.snapshot.players.iter().find(|p| p.id == self.me) {
                self.body.pos = me.pos;
                self.body.vel_y = 0.0;
                self.sent_pose = (me.pos, self.body.yaw, self.body.pitch);
            }
            self.hp_shown = tick.snapshot.you.hp as f32;
        }
        for event in &tick.events {
            match *event {
                Event::Dug { player, block, ice, pos } => {
                    // Your own digging is at full volume; other players' gets quieter with
                    // distance and can't be heard from far off.
                    let volume = if player == self.me {
                        1.0
                    } else {
                        let center =
                            Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
                        let fade = 1.0 - center.distance(self.body.pos) / HEARING_RANGE;
                        fade * fade
                    };
                    if volume > 0.02
                        && let Some(music) = &self.music
                    {
                        music.dig_at(block, ice, volume);
                    }
                }
                Event::Died { player } => {
                    let who = if player == self.me {
                        "YOU".to_string()
                    } else {
                        self.mirror.name_of(player).unwrap_or("SOMEONE").to_string()
                    };
                    let line = format!("* {} DIED", hud::display_text(&who));
                    self.chat_log.push((Instant::now(), line));
                }
                Event::RoundSkipped { player } if player == self.me => {
                    // The health bar jumps to its final value instead of draining.
                    self.hp_shown = tick.snapshot.you.hp as f32;
                }
                _ => {}
            }
        }
    }

    /// Says what has been typed (if anything) and closes the chat.
    pub(crate) fn finish_chat(&mut self, send: bool) {
        if let Some(text) = self.chat_input.take()
            && send
            && !text.trim().is_empty()
            && self.net.say(&text).is_err()
        {
            self.connection_lost = true;
        }
    }

    /// The chat lines to show: the recent ones, or the last few while typing.
    fn visible_chat(&self) -> Vec<String> {
        const SHOWN: usize = 8;
        const SECONDS: f32 = 12.0;
        let typing = self.chat_input.is_some();
        let recent: Vec<&(Instant, String)> = self
            .chat_log
            .iter()
            .filter(|(at, _)| typing || at.elapsed().as_secs_f32() < SECONDS)
            .collect();
        let skip = recent.len().saturating_sub(SHOWN);
        recent[skip..].iter().map(|(_, text)| text.clone()).collect()
    }

    /// Works out where everyone is drawn: between the last two updates, so it is smooth.
    fn refresh_scene(&mut self) {
        let Some(latest) = &self.mirror.latest else {
            return;
        };
        let t = self.update_at.elapsed().as_secs_f32() / game_net::TICK_SECONDS;
        let (players, enemies) = latest.blended(self.mirror.previous.as_ref(), t);
        self.scene = Scene { players, enemies };
    }

    /// Sends our position and look direction, if they changed (and, unless `now`, not too
    /// often).
    fn send_pose(&mut self, now: bool) {
        let pose = (self.body.pos, self.body.yaw, self.body.pitch);
        if pose == self.sent_pose {
            return;
        }
        if !now && self.sent_pose_at.elapsed().as_secs_f32() < POSE_INTERVAL {
            return;
        }
        self.send(Command::SetPose { pos: pose.0, yaw: pose.1, pitch: pose.2, epoch: self.epoch });
        self.sent_pose = pose;
        self.sent_pose_at = Instant::now();
    }

    fn send(&mut self, command: Command) {
        if self.net.send(command).is_err() {
            self.connection_lost = true;
        }
    }

    /// Something the player did that the host decides: first the host is told exactly where
    /// we are and looking, so it aims the same way we did.
    fn command(&mut self, command: Command) {
        self.send_pose(true);
        self.send(command);
    }

    /// Starts or stops mining (the left button held).
    fn set_mining(&mut self, mining: bool) {
        if self.input.mining != mining {
            self.input.mining = mining;
            self.send_pose(true);
            // The host doesn't walk us: it is told our position instead.
            let held = PlayerInput {
                mining,
                yaw: self.input.yaw,
                pitch: self.input.pitch,
                ..PlayerInput::default()
            };
            self.send(Command::SetInput(held));
        }
    }

    /// Sends the chunks of the world that changed to the GPU.
    fn upload_changed_chunks(&mut self) {
        for (cx, cz) in self.mirror.world.take_dirty() {
            if let Some(gfx) = &mut self.gfx {
                let (vertices, indices) = mesh::terrain::build_chunk(&self.mirror.world, cx, cz);
                gfx.set_chunk(cx, cz, &vertices, &indices);
            }
        }
    }

    // ---- what the crosshair is on, worked out from our copy (the host decides for real) ----

    fn creatures(&self) -> Vec<Creature> {
        self.scene
            .enemies
            .iter()
            .map(|e| Creature::new(e.id, e.kind, e.hostile, e.pos))
            .collect()
    }

    pub(crate) fn block_in_sights(&self) -> Option<(i32, i32, i32)> {
        sights::block_hit_in_sights(
            &self.mirror.world,
            self.body.eye(),
            self.body.forward(),
            self.body.pos,
            &self.creatures(),
        )
        .map(|hit| hit.block)
    }

    pub(crate) fn enemy_in_sights(&self) -> Option<Sight> {
        sights::enemy_in_sights(
            &self.mirror.world,
            self.body.eye(),
            self.body.forward(),
            self.body.pos,
            &self.creatures(),
        )
    }

    /// What applying dense resource `kind` to the block at `pos` would do right now.
    pub(crate) fn effect_at(&self, kind: usize, pos: (i32, i32, i32)) -> Option<Effect> {
        let mut occupants = vec![sights::player_box(self.body.pos)];
        occupants.extend(
            self.scene
                .players
                .iter()
                .filter(|p| p.id != self.me)
                .map(|p| sights::player_box(p.pos)),
        );
        occupants.extend(self.creatures().iter().map(|c| (c.min, c.max)));
        sights::effect_at(&self.mirror.world, kind, pos, self.body.yaw, &occupants)
    }

    /// Whether applying `kind` there would do anything, and we have enough of it.
    pub(crate) fn can_apply(&self, kind: usize, pos: (i32, i32, i32)) -> bool {
        kind < 5
            && self
                .effect_at(kind, pos)
                .is_some_and(|e| self.you().resources.dense[kind] >= e.dense_cost())
    }

    /// True while the mouse is pointing at the HUD (Ctrl held, or the apply menu open), or a
    /// chat line is being typed (the game waits).
    pub(crate) fn ui_open(&self) -> bool {
        self.keys.ctrl || self.menu.is_some() || self.chat_input.is_some()
    }

    /// Takes the mouse: hidden, and locked (or, where locking isn't available, confined to
    /// the window). Safe to call again; it is repeated on focus and click in case the first
    /// attempt didn't take or the grab was lost.
    pub(crate) fn grab_cursor(&mut self) {
        let Some(gfx) = &self.gfx else {
            return;
        };
        let window = &gfx.window;
        let locked = window.set_cursor_grab(CursorGrabMode::Locked).is_ok();
        if !locked {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
        }
        window.set_cursor_visible(false);
        self.cursor_locked = locked;
    }

    /// Gives the mouse back, for when the window isn't the one being played in.
    pub(crate) fn release_cursor(&self) {
        if let Some(gfx) = &self.gfx {
            let _ = gfx.window.set_cursor_grab(CursorGrabMode::None);
            gfx.window.set_cursor_visible(true);
        }
    }
}

/// The host that runs inside the program in single player, which has to be told to stop (it
/// saves the game then) and waited for before the program ends.
struct InnerHost {
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

impl InnerHost {
    fn finish(self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.thread.join();
    }
}

/// A random world number.
fn random_seed() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(1)
}

/// Connects to a host (starting one first when playing alone) and waits for the first update.
fn connect(start: Start) -> Result<(Client, Mirror, Option<InnerHost>), String> {
    let mut inner = None;
    let (client, mirror) = match start {
        Start::Alone { seed, fresh, save, name } => {
            if fresh && save.exists() {
                let mut old = save.clone().into_os_string();
                old.push(".bak");
                std::fs::rename(&save, old).map_err(|e| format!("can't keep the old save: {e}"))?;
            }
            let server = Server::bind("127.0.0.1:0", seed.unwrap_or_else(random_seed))
                .and_then(|server| server.quiet().with_save(&save))
                .map_err(|e| format!("can't start the game: {e}"))?;
            let addr = server.local_addr().map_err(|e| e.to_string())?;
            let world_seed = server.seed();
            if server.continues_a_save() {
                println!("carrying on the game in {}", save.display());
            }
            println!("world seed: {world_seed}");
            let stop = server.stop_flag();
            let thread = std::thread::spawn(move || {
                if let Err(e) = server.run() {
                    eprintln!("the game stopped: {e}");
                }
            });
            inner = Some(InnerHost { stop, thread });
            // The host is making its world; make ours at the same time.
            let world = World::generate(world_seed);
            let (client, welcome) =
                Client::connect(addr, &name).map_err(|e| format!("can't start the game: {e}"))?;
            (client, Mirror::with_world(&welcome, world))
        }
        Start::Join { addr, name } => {
            let target = addr
                .to_socket_addrs()
                .map_err(|e| format!("bad address {addr}: {e}"))?
                .next()
                .ok_or_else(|| format!("no such address: {addr}"))?;
            println!("joining {target} ...");
            let (client, welcome) =
                Client::connect(target, &name).map_err(|e| format!("can't join {addr}: {e}"))?;
            println!(
                "joined as {}{}, building the world (seed {}) ...",
                welcome.name,
                if welcome.rejoined { " (welcome back)" } else { "" },
                welcome.seed
            );
            (client, Mirror::new(&welcome))
        }
    };
    let (mut client, mut mirror) = (client, mirror);
    let started = Instant::now();
    while mirror.latest.is_none() {
        if started.elapsed() > Duration::from_secs(15) {
            return Err("the host didn't send anything".into());
        }
        match client.wait_tick(Duration::from_millis(200)) {
            Ok(Some(tick)) => mirror.apply(&tick),
            Ok(None) => {}
            Err(e) => return Err(format!("lost the host: {e}")),
        }
    }
    Ok((client, mirror, inner))
}

/// Opens the window and runs the game until it is closed.
pub(crate) fn run(start: Start) {
    let (net, mirror, inner) = match connect(start) {
        Ok(connection) => connection,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(net, mirror);
    event_loop.run_app(&mut app).unwrap();
    let lost = app.connection_lost;
    // Let go of the connection, then give the inner host the moment it needs to save the game.
    drop(app);
    if let Some(host) = inner {
        host.finish();
    }
    if lost {
        // Not a normal quit: the launcher script offers to join again.
        std::process::exit(2);
    }
}
