//! One frame: update everything, then draw.

use super::*;
use crate::biome::Biome;

impl App {
    /// Runs one frame: moves the player, mining, the fight, weather and background
    /// conversion, then draws everything.
    pub(crate) fn redraw(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        self.game.update(dt);
        self.show_changes();
        let hp = self.me().combat.player_hp as f32;
        self.hp_shown = (self.hp_shown - HP_DRAIN_PER_SECOND * dt).max(hp);

        // One crack stage, halfway through the mining time.
        let stage = if self.me().mine_timer >= MINE_INTERVAL / 2.0 { 1 } else { 0 };
        let weather = &self.game.weather;
        if let Some(music) = &mut self.music {
            // Rain you hear is snow in the cold; either way it is as loud as the rain is.
            let amount = weather.params.rain;
            let snowing = weather.snowing;
            let creepy = matches!(weather.biome, Biome::Volcanic | Biome::Swamp);
            let (rain, snow) = if snowing { (0.0, amount) } else { (amount, 0.0) };
            music.update(rain, snow, if creepy { 1.0 } else { 0.0 }, dt);
        }

        // Whatever the crosshair is on: an enemy, or else a block.
        let enemy_target = self.game.enemy_in_sights(self.me).map(|s| (s.min, s.max));
        let target = self.game.block_in_sights(self.me);
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

        let player = self.game.player(self.me).expect("the local player is in the game");
        let look_at = player.body.pos;
        let enemy_rolls: Vec<(u32, u32, f32)> = player
            .combat
            .enemy_rolls
            .iter()
            .map(|r| r.view())
            .collect();
        let enemy_mesh: Vec<Vertex> = self
            .game
            .enemies
            .iter()
            .flat_map(|e| mesh::creature::enemy_mesh(e, e.facing(look_at)))
            .collect();

        self.recenter_cursor();

        let target_label = self.target_label();

        let menu_labels = self.menu_labels();

        if let Some(gfx) = &mut self.gfx {
            gfx.set_outline(outline);
            gfx.set_crack(crack);
            gfx.set_enemies(&enemy_mesh);
            let hud = hud::build(
                gfx.config.width,
                gfx.config.height,
                &HudInfo {
                    resources: player.resources.as_array(),
                    dense: player.resources.dense,
                    menu: menu_labels,
                    converting: player
                        .converter
                        .selected()
                        .map(|kind| (kind, player.converter.fraction())),
                    ctrl_held: self.keys.ctrl,
                    weather_label: self.game.weather.label(),
                    selection: self
                        .selected_dense
                        .map(|kind| (kind, player.resources.dense[kind])),
                    target_label,
                    conversion_boost: player.conversion_boost,
                    player_roll: player.combat.player_roll.map(|r| r.view()),
                    enemy_rolls: &enemy_rolls,
                    hp: (self.hp_shown, player.combat.player_hp, MAX_PLAYER_HP),
                },
            );
            let time = self.started.elapsed().as_secs_f32();
            gfx.render(player.body.eye(), player.body.forward(), time, &self.game.weather, &hud);
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

    /// With a dense resource selected: what right click would build on the block the
    /// crosshair is on, and where on the screen to write it.
    fn target_label(&self) -> Option<(&'static str, f32, f32)> {
        // With a dense resource selected: what right click would build on the block
        // the crosshair is on, written over that block.
        let player = self.me();
        match (self.selected_dense, self.game.block_in_sights(self.me)) {
            (Some(kind), Some(pos)) if !self.ui_open() => {
                // Only if there is enough of it (a staircase needs one per step).
                let enough = |effect: &Effect| {
                    player.resources.dense[kind] >= effect.dense_cost()
                };
                self.game.effect_at(self.me, kind, pos).filter(enough).and_then(|effect| {
                    let (width, height) = self
                        .gfx
                        .as_ref()
                        .map_or((1.0, 1.0), |g| (g.config.width as f32, g.config.height as f32));
                    let view = Mat4::look_to_rh(
                        player.body.eye(),
                        player.body.forward(),
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
                    .game
                    .effect_at(self.me, kind, pos)
                    .map(|e| (e.label(), e.dense_cost()));
            }
            labels
        })
    }
}
