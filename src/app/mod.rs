//! The game itself: state, and what happens each frame and on each input.

mod frame;
mod input;
mod menu;

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

use crate::combat::MAX_PLAYER_HP;
use crate::effects::{self, Effect};
use crate::game::{Command, Event, Game, PlayerId, PlayerInput, PlayerState, MINE_INTERVAL};
use crate::gfx::{Gfx, FOV_Y_DEGREES};
use crate::hud::{self, HudInfo};
use crate::mesh::{self, Vertex};
use crate::music;
use crate::world;
use input::Keys;

const MOUSE_SENSITIVITY: f32 = 0.0022;
// How fast the health bar's draining chunk catches up, in hp per second.
const HP_DRAIN_PER_SECOND: f32 = 30.0;

pub(crate) struct App {
    gfx: Option<Gfx>,
    /// The game's state and rules. Everything the player does goes in as a `Command`.
    game: Game,
    /// Which of the game's players this window plays.
    me: PlayerId,
    /// What this player is holding down and looking at; sent to the game whenever it changes.
    input: PlayerInput,
    keys: Keys,
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
    music: Option<music::Music>,
}

impl App {
    fn new(seed: u32) -> Self {
        let mut game = Game::new(seed);
        let me = game.add_player();
        App {
            gfx: None,
            game,
            me,
            input: PlayerInput::default(),
            keys: Keys::default(),
            last_frame: Instant::now(),
            started: Instant::now(),
            menu: None,
            selected_dense: None,
            cursor_pos: (0.0, 0.0),
            hp_shown: MAX_PLAYER_HP as f32,
            focused: true,
            cursor_locked: false,
            music: music::Music::start(),
        }
    }

    /// The player this window plays.
    fn me(&self) -> &PlayerState {
        self.game.player(self.me).expect("the local player is in the game")
    }

    /// Sends the current keys and look direction to the game.
    fn send_input(&mut self) {
        self.game.apply(self.me, Command::SetInput(self.input));
    }

    /// Starts or stops mining (the left button held).
    fn set_mining(&mut self, mining: bool) {
        if self.input.mining != mining {
            self.input.mining = mining;
            self.send_input();
        }
    }

    /// Does something in the game, then shows what came of it.
    fn command(&mut self, command: Command) {
        self.game.apply(self.me, command);
        self.show_changes();
    }

    /// Plays the sounds of what happened and sends the changed chunks of the world to the GPU.
    fn show_changes(&mut self) {
        for event in self.game.take_events() {
            match event {
                Event::Dug { player, block, ice } if player == self.me => {
                    if let Some(music) = &self.music {
                        music.dig(block, ice);
                    }
                }
                Event::RoundSkipped { player } if player == self.me => {
                    // The health bar jumps to its final value instead of draining.
                    self.hp_shown = self.me().combat.player_hp as f32;
                }
                _ => {}
            }
        }
        for (cx, cz) in self.game.world.take_dirty() {
            if let Some(gfx) = &mut self.gfx {
                let (vertices, indices) = mesh::terrain::build_chunk(&self.game.world, cx, cz);
                gfx.set_chunk(cx, cz, &vertices, &indices);
            }
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
