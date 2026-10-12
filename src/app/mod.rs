//! The game itself: state, and what happens each frame and on each input.

mod blocks;
mod fighting;
mod frame;
mod input;
mod menu;
mod update;

use std::time::Instant;

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

use crate::combat::{Candidate, Combat, Side, MAX_PLAYER_HP};
use crate::effects::{self, Effect, Fires, STAIR_STEPS};
use crate::enemy::{self, Dice, Enemy, Kind, Sight};
use crate::gfx::{Gfx, FOV_Y_DEGREES};
use crate::hud::{self, HudInfo};
use crate::music;
use crate::player::{self, Player};
use crate::resources::{Converter, Resources};
use crate::weather::Weather;
use crate::world::{self, Vertex, World};
use input::Keys;

const WALK_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;
const MINE_INTERVAL: f32 = 0.55; // seconds of holding to mine one block
const REACH: f32 = 5.0; // blocks
// Hostile creatures can attack you from this close (horizontally)...
const MELEE_RANGE: f32 = 2.3;
// ...you can attack them from this close (and click a passive one to start a fight).
const NEAR_RANGE: f32 = 3.0;
// A forge this close (in blocks) doubles the conversion speed; a furnace quadruples it from
// further away.
const FORGE_RANGE: f32 = 6.0;
const FURNACE_RANGE: f32 = 10.0;
// After a round ends, the next one can't start for this long.
const ROUND_PAUSE: f32 = 2.0;
// How fast the health bar's draining chunk catches up, in hp per second.
const HP_DRAIN_PER_SECOND: f32 = 30.0;

pub(crate) struct App {
    gfx: Option<Gfx>,
    world: World,
    player: Player,
    keys: Keys,
    last_frame: Instant,
    /// When the game started, for things that move with time (the clouds).
    started: Instant,
    weather: Weather,
    resources: Resources,
    converter: Converter,
    fires: Fires,
    /// The apply menu, open on this block, or None.
    menu: Option<(i32, i32, i32)>,
    /// What right click does: `None` clones the block it is aimed at (normal block placing),
    /// `Some(kind)` applies that dense resource to it. Changed with the scroll wheel.
    selected_dense: Option<usize>,
    /// Last known mouse position in window pixels (only meaningful while Ctrl frees it).
    cursor_pos: (f32, f32),
    mine_timer: f32,
    enemies: Vec<Enemy>,
    dice: Dice,
    /// Last roll and how long ago it happened.
    combat: Combat,
    /// Health shown on the bar; trails the real value after a hit so the loss is visible.
    hp_shown: f32,
    round_pause: f32,
    /// Seconds until the next try at spawning a creature near the player.
    spawn_timer: f32,
    spawn_dice: Dice,
    next_enemy_id: u32,
    /// The window has keyboard focus (and so should own the mouse).
    focused: bool,
    /// The OS really locked the pointer. If not, it is only confined, and the game keeps
    /// pulling it back to the middle of the window so it can't wander onto another monitor.
    cursor_locked: bool,
    was_busy: bool,
    music: Option<music::Music>,
}

impl App {
    fn new(seed: u32) -> Self {
        let world = World::generate(seed);
        let player = Player::new(world.spawn_point());
        // A deer close to the spawn point; the rest of the creatures turn up as you explore.
        let (cx, cz) = (world::SIZE_X as i32 / 2, world::SIZE_Z as i32 / 2);
        let enemies = vec![Enemy::new(1, Kind::Deer, world.open_spot(cx - 9, cz - 3))];
        App {
            gfx: None,
            world,
            player,
            keys: Keys::default(),
            last_frame: Instant::now(),
            started: Instant::now(),
            weather: Weather::new(seed ^ 0x5EA7),
            // A big pile of everything to try things out with. Use `Resources::default()` for a
            // normal start.
            resources: Resources::test_kit(),
            converter: Converter::default(),
            fires: Fires::default(),
            menu: None,
            selected_dense: None,
            cursor_pos: (0.0, 0.0),
            mine_timer: 0.0,
            enemies,
            dice: Dice::new(),
            combat: Combat::new(),
            hp_shown: MAX_PLAYER_HP as f32,
            round_pause: 0.0,
            spawn_timer: 3.0,
            spawn_dice: Dice::new(),
            next_enemy_id: 2,
            focused: true,
            cursor_locked: false,
            was_busy: false,
            music: music::Music::start(),
        }
    }

    /// True while the mouse is pointing at the HUD (Ctrl held, or the apply menu open).
    pub(crate) fn ui_open(&self) -> bool {
        self.keys.ctrl || self.menu.is_some()
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

/// Opens the window and runs the game until it is closed.
pub(crate) fn run(seed: u32) {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(seed);
    event_loop.run_app(&mut app).unwrap();
}
