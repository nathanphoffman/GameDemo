//! One frame: update everything, then draw.

use super::*;
use crate::biome::Biome;

impl App {
    /// Runs one frame: takes in what the host sent, walks, then draws everything.
    pub(crate) fn redraw(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        self.receive();
        self.upload_changed_chunks();
        self.refresh_scene();

        // Walk here and now, on our copy of the world, and tell the host where we are.
        self.body.yaw = self.input.yaw;
        self.body.pitch = self.input.pitch;
        game::walk(&mut self.body, &self.input, dt, &self.mirror.world);
        self.send_pose(false);

        let you = self.you().clone();
        self.hp_shown = (self.hp_shown - HP_DRAIN_PER_SECOND * dt).max(you.hp as f32);

        // The host says what the weather is; how it looks (fading, clouds) is worked out here.
        if let Some(latest) = &self.mirror.latest {
            self.weather.kind = latest.weather;
        }
        let (px, pz) = (self.body.pos.x.floor() as i32, self.body.pos.z.floor() as i32);
        self.weather.biome = self.mirror.world.biome(px, pz);
        self.weather.snowing = self.weather.biome == Biome::Snow;
        self.weather.underwater = self.mirror.world.in_water(self.body.eye());
        // Write up a change in the chat, if it is one that reaches us (a desert doesn't notice
        // fog turning to mist, for one).
        let reaches_us = self.weather.effective_kind();
        if self.weather.kind != self.announced_weather.0 && reaches_us != self.announced_weather.1 {
            let line = narrate::weather_change(reaches_us, self.weather.snowing, self.weather.biome);
            self.log_line(&format!("* {line}"));
        }
        self.announced_weather = (self.weather.kind, reaches_us);
        self.weather.update(dt);
        if let Some(music) = &mut self.music {
            // Rain you hear is snow in the cold; either way it is as loud as the rain is.
            let amount = self.weather.params.rain;
            let snowing = self.weather.snowing;
            let creepy = matches!(self.weather.biome, Biome::Volcanic | Biome::Swamp);
            let (rain, snow) = if snowing { (0.0, amount) } else { (amount, 0.0) };
            music.update(rain, snow, if creepy { 1.0 } else { 0.0 }, dt);
        }

        // One crack stage, halfway through the mining time.
        let stage = if you.mine_timer >= MINE_INTERVAL / 2.0 { 1 } else { 0 };

        // Whatever the crosshair is on: an enemy, or else a block.
        let enemy_target = self.enemy_in_sights().map(|s| (s.min, s.max));
        let target = self.block_in_sights();
        let crack = target
            .filter(|_| stage >= 1)
            .map(|block| (block, stage));

        // One wireframe, around whichever it is.
        let outline = enemy_target.or_else(|| {
            target.map(|(x, y, z)| {
                let min = Vec3::new(x as f32, y as f32, z as f32);
                (min, min + Vec3::ONE)
            })
        });

        // Creatures face us (unless they are walking home), and other players are drawn too.
        let look_at = self.body.pos;
        let mut enemy_mesh: Vec<Vertex> = Vec::new();
        for e in &self.scene.enemies {
            let facing = e.returning_home.unwrap_or(look_at);
            enemy_mesh.extend(mesh::creature::creature_mesh(e.kind, e.pos, e.health, facing));
        }
        for p in self.scene.players.iter().filter(|p| p.id != self.me) {
            enemy_mesh.extend(mesh::creature::player_mesh(p.pos, p.yaw, p.hp, look_at));
        }

        let other_cracks: Vec<(i32, i32, i32)> = self
            .scene
            .players
            .iter()
            .filter(|p| p.id != self.me)
            .filter_map(|p| p.cracking)
            .collect();

        self.recenter_cursor();

        let target_label = self.target_label();

        let menu_labels = self.menu_labels();

        // Chat, the list of players, and names floating over the others.
        let chat = self.visible_chat();
        // The grey box behind it fades in while there is something to read.
        let want = if !chat.is_empty() || self.chat_input.is_some() { 1.0 } else { 0.0 };
        let speed = if want > self.chat_box { 5.0 } else { 2.0 };
        let step = speed * dt;
        self.chat_box = if want > self.chat_box {
            (self.chat_box + step).min(want)
        } else {
            (self.chat_box - step).max(want)
        };
        let player_list: Vec<String> = self
            .mirror
            .roster
            .iter()
            .map(|(_, name)| hud::display_text(name))
            .collect();
        let name_tags = self.name_tags();

        if let Some(gfx) = &mut self.gfx {
            gfx.set_outline(outline);
            gfx.set_crack(crack);
            gfx.set_enemies(&enemy_mesh);
            gfx.set_other_cracks(&other_cracks);
            let hud = hud::build(
                gfx.config.width,
                gfx.config.height,
                &HudInfo {
                    resources: you.resources.as_array(),
                    dense: you.resources.dense,
                    menu: menu_labels,
                    converting: you.converting,
                    ctrl_held: self.keys.ctrl,
                    weather_label: self.weather.label(),
                    selection: self
                        .selected_dense
                        .map(|kind| (kind, you.resources.dense[kind])),
                    target_label,
                    conversion_boost: you.conversion_boost,
                    player_roll: you.player_roll,
                    enemy_rolls: &you.enemy_rolls,
                    hp: (self.hp_shown, you.hp, MAX_PLAYER_HP),
                    sickness: (you.sickness > 0.0)
                        .then_some((you.sickness, game::SICKNESS_SECONDS)),
                    chat: &chat,
                    chat_input: self.chat_input.as_deref().map(hud::display_text).as_deref(),
                    chat_box: self.chat_box,
                    player_list: self.show_players.then_some(&player_list[..]),
                    name_tags: &name_tags,
                },
            );
            let time = self.started.elapsed().as_secs_f32();
            gfx.render(self.body.eye(), self.body.forward(), time, &self.weather, &hud);
        }
    }

    /// Without a real pointer lock, keeps the pointer parked in the middle of the window.
    /// Looking around uses raw mouse motion, which this doesn't disturb.
    fn recenter_cursor(&self) {
        // Without a real pointer lock, keep the pointer parked in the middle of the
        // window. Looking around uses raw mouse motion, which this doesn't disturb.
        if self.focused && !self.cursor_locked && !self.ui_open()
            && let Some(gfx) = &self.gfx {
                let size = gfx.window.inner_size();
                let _ = gfx.window.set_cursor_position(PhysicalPosition::new(
                    size.width / 2,
                    size.height / 2,
                ));
            }
    }

    /// Other players' names, with where on the screen to write them.
    fn name_tags(&self) -> Vec<(String, f32, f32)> {
        const RANGE: f32 = 48.0;
        let Some(gfx) = &self.gfx else {
            return Vec::new();
        };
        let (width, height) = (gfx.config.width as f32, gfx.config.height as f32);
        let view = Mat4::look_to_rh(self.body.eye(), self.body.forward(), Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y_DEGREES.to_radians(), width / height, 0.05, 1000.0);
        self.scene
            .players
            .iter()
            .filter(|p| p.id != self.me && p.pos.distance(self.body.pos) <= RANGE)
            .filter_map(|p| {
                let name = self.mirror.name_of(p.id)?;
                let above = p.pos + Vec3::new(0.0, 2.7, 0.0);
                let (x, y) = hud::project_to_screen(proj * view, above, width, height)?;
                Some((hud::display_text(name), x, y))
            })
            .collect()
    }

    /// With a dense resource selected: what right click would build on the block the
    /// crosshair is on, and where on the screen to write it.
    fn target_label(&self) -> Option<(&'static str, f32, f32)> {
        // With a dense resource selected: what right click would build on the block
        // the crosshair is on, written over that block.
        match (self.selected_dense, self.block_in_sights()) {
            (Some(kind), Some(pos)) if !self.ui_open() => {
                // Only if there is enough of it (a staircase needs one per step).
                let enough = |effect: &Effect| {
                    self.you().resources.dense[kind] >= effect.dense_cost()
                };
                self.effect_at(kind, pos).filter(enough).and_then(|effect| {
                    let (width, height) = self
                        .gfx
                        .as_ref()
                        .map_or((1.0, 1.0), |g| (g.config.width as f32, g.config.height as f32));
                    let view = Mat4::look_to_rh(
                        self.body.eye(),
                        self.body.forward(),
                        Vec3::Y,
                    );
                    let proj = Mat4::perspective_rh(
                        FOV_Y_DEGREES.to_radians(),
                        width / height,
                        0.05,
                        1000.0,
                    );
                    let center =
                        Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
                    hud::project_to_screen(proj * view, center, width, height)
                        .map(|(x, y)| (effect.label(), x, y))
                })
            }
            _ => None,
        }
    }

    /// What each dense resource would do to the block the menu is open on.
    fn menu_labels(&self) -> Option<[Option<(&'static str, u32)>; 5]> {
        // What each dense resource would do to the block the menu is open on.
        self.menu.map(|pos| {
            let mut labels = [None; 5];
            for (kind, label) in labels.iter_mut().enumerate() {
                *label = self
                    .effect_at(kind, pos)
                    .map(|e| (e.label(), e.dense_cost()));
            }
            labels
        })
    }
}
