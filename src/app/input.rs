//! Keyboard, mouse and window events.

use super::*;

#[derive(Default)]
pub(super) struct Keys {
    pub(super) w: bool,
    pub(super) a: bool,
    pub(super) s: bool,
    pub(super) d: bool,
    pub(super) sprint: bool,
    pub(super) jump: bool,
    /// Left mouse button held.
    pub(super) mining: bool,
    /// Ctrl held: the mouse is free to click the resource list.
    pub(super) ctrl: bool,
}

impl App {
    /// Left button pressed or released: picks from the menu, picks the converter's
    /// resource (with Ctrl), or starts and stops mining and attacking.
    pub(crate) fn left_click(&mut self, pressed: bool) {
        if self.menu.is_some() {
            self.keys.mining = false;
            if pressed {
                self.menu_click();
            }
        } else if self.keys.ctrl {
            // Ctrl + click on a resource row picks what the converter works on.
            self.keys.mining = false;
            if pressed {
                let height = self.gfx.as_ref().map_or(0, |g| g.config.height);
                let (x, y) = self.cursor_pos;
                if let Some(row) = hud::resource_row_at(height, x, y) {
                    self.converter.toggle(row);
                }
            }
        } else {
            self.keys.mining = pressed;
            if pressed {
                if self.focused {
                    // A click is a good moment to retake the mouse if we somehow lost it.
                    self.grab_cursor();
                }
                // Every click is an attack, if the enemy is what the crosshair is on.
                self.attack();
            }
        }
    }

    /// A key went down or up. Esc backs out of the menu first, and only then quits.
    pub(crate) fn key_changed(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, down: bool) {
        match code {
            KeyCode::KeyW => self.keys.w = down,
            KeyCode::KeyA => self.keys.a = down,
            KeyCode::KeyS => self.keys.s = down,
            KeyCode::KeyD => self.keys.d = down,
            KeyCode::ShiftLeft => self.keys.sprint = down,
            KeyCode::Space => self.keys.jump = down,
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                if down != self.keys.ctrl {
                    self.keys.ctrl = down;
                    self.keys.mining = false;
                    // Free the pointer so it can be moved to the list; take it
                    // back (and recenter) when Ctrl is let go.
                    if down {
                        self.release_cursor();
                    } else if self.focused && self.menu.is_none() {
                        self.grab_cursor();
                    }
                }
            }
            KeyCode::KeyE if down => self.open_menu(),
            KeyCode::Escape if down => {
                // Esc backs out of the menu first, and only then quits.
                if self.menu.is_some() {
                    self.close_menu();
                } else {
                    event_loop.exit();
                }
            }
            _ => {}
        }

    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("GameDemo")
                        .with_fullscreen(Some(Fullscreen::Borderless(None))),
                )
                .unwrap(),
        );
        let mut gfx = Gfx::new(window);
        for cz in 0..world::CHUNKS_Z {
            for cx in 0..world::CHUNKS_X {
                let (vertices, indices) = mesh::terrain::build_chunk(&self.world, cx, cz);
                gfx.set_chunk(cx, cz, &vertices, &indices);
            }
        }
        self.world.take_dirty();
        self.gfx = Some(gfx);
        self.grab_cursor();
        self.last_frame = Instant::now();
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.ui_open() {
            return; // the mouse is pointing at the HUD, not turning the camera
        }
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.player.yaw += dx as f32 * MOUSE_SENSITIVITY;
            self.player.pitch -= dy as f32 * MOUSE_SENSITIVITY;
            let limit = 89.0_f32.to_radians();
            self.player.pitch = self.player.pitch.clamp(-limit, limit);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size);
                }
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if focused {
                    if self.menu.is_none() {
                        self.grab_cursor();
                    }
                } else {
                    // Let go of everything so nothing stays "held" while we're away.
                    self.keys = Keys::default();
                    self.release_cursor();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                button: MouseButton::Middle,
                state: ElementState::Pressed,
                ..
            } => {
                self.open_menu();
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => self.left_click(state == ElementState::Pressed),
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            } => {
                if !self.ui_open() {
                    match self.selected_dense {
                        None => self.place_block(),
                        Some(kind) => self.apply_selected(kind),
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if amount != 0.0 && !self.ui_open() {
                    self.selected_dense =
                        effects::cycle_selection(self.selected_dense, amount > 0.0);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key_changed(event_loop, code, down);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}
