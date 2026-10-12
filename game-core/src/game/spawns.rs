//! Where creatures live. Like the terrain, it comes from the seed: the world is cut into square
//! cells, and each cell has a few places where a creature might live. Everyone who plays the
//! same seed finds the same creatures in the same places.
//!
//! A creature only exists while a player is near its home (otherwise a big world would be full
//! of creatures nobody can see). When everyone goes away it is put back, and when someone
//! comes back it is waiting at home with full health. If it is defeated, its home stays empty
//! for `RESPAWN_SECONDS` of game time.
//!
//! Dangerous zones have more creatures (`creatures_per_cell`) and tougher ones (`Kind::max_health`).

use super::*;
use crate::biome::Biome;
use crate::world::hash;

/// Cells are this many blocks across.
const CELL: i32 = 40;
const CELLS_X: i32 = (world::SIZE_X as i32 + CELL - 1) / CELL;
/// Most creatures that can live in one cell.
const SLOTS: u32 = 3;
/// A creature appears at its home when a player gets this close (blocks, horizontally)...
const WAKE_DISTANCE: f32 = 75.0;
/// ...and is put away when no player is within this. Larger, so one standing on the edge doesn't
/// make it flicker.
const SLEEP_DISTANCE: f32 = 90.0;
/// How long a defeated creature's home stays empty, in seconds of game time.
pub const RESPAWN_SECONDS: f32 = 300.0;
/// Home numbers start here, which is also the number of the creature that lives there. (The
/// starter deer is 1.)
const FIRST_HOME_ID: u32 = 1000;
const STARTER_ID: u32 = 1;

/// How many creatures live in a cell of this biome, on average (at most 3). This is where the
/// zones are made safer or scarier.
pub fn creatures_per_cell(biome: Biome) -> f32 {
    match biome {
        Biome::Meadow => 0.15,
        Biome::Plains => 0.2,
        Biome::Forest => 0.4,
        Biome::Desert => 0.5,
        Biome::Snow => 0.8,
        Biome::Badlands => 0.9,
        Biome::Swamp => 1.0,
        Biome::Volcanic => 1.5,
    }
}

/// A place a creature lives.
#[derive(Clone, Copy, Debug)]
pub struct Home {
    pub id: u32,
    pub kind: Kind,
    pub pos: Vec3,
}

/// The closest spot to (x, z), within a few blocks, where a creature has room to stand.
fn find_spot(world: &World, x: i32, z: i32) -> Option<Vec3> {
    for radius in 0..6i32 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dz.abs()) == radius
                    && let Some(spot) = world.standing_spot(x + dx, z + dz)
                {
                    return Some(spot);
                }
            }
        }
    }
    None
}

fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

impl Game {
    /// The homes in one cell of the world.
    pub fn homes_in_cell(&mut self, cx: i32, cz: i32) -> Vec<Home> {
        if let Some(homes) = self.homes.get(&(cx, cz)) {
            return homes.clone();
        }
        let cell = (cz * CELLS_X + cx) as u32;
        let mut homes = Vec::new();
        for slot in 0..SLOTS {
            let h = |k: u32| hash(cell, slot * 8 + k, self.seed ^ 0xC4EA7);
            let x = cx * CELL + (h(0) * CELL as f32) as i32;
            let z = cz * CELL + (h(1) * CELL as f32) as i32;
            if x >= world::SIZE_X as i32 || z >= world::SIZE_Z as i32 {
                continue;
            }
            let biome = self.world.biome(x, z);
            if h(2) >= creatures_per_cell(biome) / SLOTS as f32 {
                continue;
            }
            if let Some(pos) = find_spot(&self.world, x, z) {
                homes.push(Home {
                    id: FIRST_HOME_ID + cell * SLOTS + slot,
                    kind: Kind::for_biome(biome),
                    pos,
                });
            }
        }
        // The deer that lives by the spawn point.
        let starter = self.starter_home;
        if (starter.x as i32 / CELL, starter.z as i32 / CELL) == (cx, cz) {
            homes.push(Home { id: STARTER_ID, kind: Kind::Deer, pos: starter });
        }
        self.homes.insert((cx, cz), homes.clone());
        homes
    }

    /// Puts creatures at the homes players are near, and away from the ones they have left, and
    /// counts down the wait for defeated ones.
    pub(super) fn update_creature_homes(&mut self, dt: f32) {
        self.defeated.retain(|_, left| {
            *left -= dt;
            *left > 0.0
        });

        let players: Vec<Vec3> = self.players.iter().map(|p| p.body.pos).collect();
        self.enemies
            .retain(|e| players.iter().any(|p| flat_distance(*p, e.pos()) <= SLEEP_DISTANCE));

        let reach = (WAKE_DISTANCE / CELL as f32).ceil() as i32;
        let cells_z = (world::SIZE_Z as i32 + CELL - 1) / CELL;
        for pos in players {
            let (pcx, pcz) = (pos.x as i32 / CELL, pos.z as i32 / CELL);
            for cz in (pcz - reach).max(0)..=(pcz + reach).min(cells_z - 1) {
                for cx in (pcx - reach).max(0)..=(pcx + reach).min(CELLS_X - 1) {
                    for home in self.homes_in_cell(cx, cz) {
                        let taken = self.enemies.iter().any(|e| e.id == home.id);
                        if taken
                            || self.defeated.contains_key(&home.id)
                            || flat_distance(pos, home.pos) > WAKE_DISTANCE
                        {
                            continue;
                        }
                        self.enemies.push(Enemy::new(home.id, home.kind, home.pos));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(game: &mut Game, seconds: f32, step: f32) {
        for _ in 0..(seconds / step) as u32 {
            game.update(step);
        }
    }

    #[test]
    fn scarier_places_have_more_creatures() {
        let order = [
            Biome::Meadow,
            Biome::Plains,
            Biome::Forest,
            Biome::Desert,
            Biome::Snow,
            Biome::Badlands,
            Biome::Swamp,
            Biome::Volcanic,
        ];
        for pair in order.windows(2) {
            assert!(creatures_per_cell(pair[0]) < creatures_per_cell(pair[1]), "{pair:?}");
        }
        // ...and tougher ones.
        let tough = [Kind::Deer, Kind::Boar, Kind::Wolf, Kind::Scorpion, Kind::Yeti, Kind::Golem, Kind::Skeleton];
        for pair in tough.windows(2) {
            assert!(pair[0].max_health() < pair[1].max_health(), "{pair:?}");
        }
        assert!(Kind::for_biome(Biome::Volcanic).max_health() > Kind::for_biome(Biome::Meadow).max_health());
    }

    #[test]
    fn the_same_seed_has_the_same_creatures_in_the_same_places() {
        let homes = |seed: u32| {
            let mut game = Game::new(seed);
            let mut all = Vec::new();
            for cz in 0..CELLS_X {
                for cx in 0..CELLS_X {
                    all.extend(game.homes_in_cell(cx, cz).into_iter().map(|h| (h.id, h.kind, h.pos.to_array())));
                }
            }
            all
        };
        let (a, b) = (homes(3), homes(3));
        assert!(a.len() > 100, "{} creatures in the world", a.len());
        assert_eq!(a, b);
        assert_ne!(a, homes(4));
        // The ones that live in dangerous country are tougher than in the meadows.
        let mut kinds: Vec<Kind> = Vec::new();
        for (_, kind, _) in &a {
            if !kinds.contains(kind) {
                kinds.push(*kind);
            }
        }
        assert!(kinds.len() >= 4, "{kinds:?}");
    }

    #[test]
    fn creatures_appear_near_players_and_only_near_them() {
        let mut game = Game::new(3);
        assert!(game.enemies.is_empty(), "nobody is near anything yet");
        let me = game.add_player();
        run(&mut game, 1.0, 1.0 / 30.0);
        assert!(game.enemies.iter().any(|e| e.id == STARTER_ID), "the deer by the spawn point");
        let spot = game.player(me).unwrap().body.pos;
        assert!(game.enemies.iter().all(|e| flat_distance(e.pos(), spot) <= SLEEP_DISTANCE));
        // Running longer doesn't pile up more: each home holds one creature.
        let count = game.enemies.len();
        run(&mut game, 2.0, 1.0 / 30.0);
        assert_eq!(game.enemies.len(), count);
    }

    #[test]
    fn a_defeated_creature_is_back_after_the_wait_with_full_health() {
        let mut game = Game::new(3);
        let _me = game.add_player();
        run(&mut game, 1.0, 1.0 / 30.0);
        let deer = game.enemies.iter().position(|e| e.id == STARTER_ID).unwrap();
        game.enemies[deer].health = 0;
        run(&mut game, 0.2, 1.0 / 30.0);
        assert!(game.enemies.iter().all(|e| e.id != STARTER_ID), "it was defeated");

        // Most of the wait: still gone.
        run(&mut game, RESPAWN_SECONDS - 20.0, 1.0);
        assert!(game.enemies.iter().all(|e| e.id != STARTER_ID));
        // After it: back, unhurt.
        run(&mut game, 25.0, 1.0);
        let back = game.enemies.iter().find(|e| e.id == STARTER_ID).expect("respawned");
        assert_eq!(back.health, Kind::Deer.max_health());
    }
}
