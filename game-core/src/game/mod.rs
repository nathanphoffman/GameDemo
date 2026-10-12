//! The game itself, minus anything you can see or hear: the world, the creatures, the weather,
//! and one `PlayerState` per player. It changes only through `Game::apply` (what a player
//! does) and `Game::update` (time passing), and reports what happened through `Event`s. That
//! is the whole interface, so a window, a bot or a network host can all drive it.

mod blocks;
mod fight;
pub mod sights;
mod snapshot;
pub mod spawns;
mod update;

pub use snapshot::{EnemyView, PlayerView, Snapshot, YouView};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::combat::{Candidate, Combat, Side};
use crate::effects::{Effect, Fires, STAIR_STEPS};
use crate::enemy::{Dice, Enemy, Kind, Sight};
use crate::player::Player;
use crate::resources::{Converter, Resources};
use crate::weather::Weather;
use crate::world::{self, Block, World};

pub const WALK_SPEED: f32 = 5.0;
pub const SPRINT_SPEED: f32 = 8.0;
/// Seconds of holding to mine one block.
pub const MINE_INTERVAL: f32 = 0.55;
/// How far (in blocks) the player can reach.
pub const REACH: f32 = 5.0;
/// Hostile creatures can attack you from this close (horizontally)...
pub const MELEE_RANGE: f32 = 2.3;
/// ...you can attack them from this close (and click a passive one to start a fight).
pub const NEAR_RANGE: f32 = 3.0;
/// A forge this close (in blocks) doubles the conversion speed; a furnace quadruples it from
/// further away.
pub const FORGE_RANGE: f32 = 6.0;
pub const FURNACE_RANGE: f32 = 10.0;
/// After a round ends, the next one can't start for this long.
pub const ROUND_PAUSE: f32 = 2.0;
/// Fastest a player can really move, in blocks per second (falling is the fastest).
const MAX_POSE_SPEED: f32 = 80.0;
/// How many kinds of dense resource there are.
const DENSE_KINDS: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

/// Moves `body` for `dt` seconds along the keys in `input` (and jumps), in `world`.
pub fn walk(body: &mut Player, input: &PlayerInput, dt: f32, world: &World) {
    let (forward, right) = body.flat_axes();
    let mut dir = Vec3::ZERO;
    if input.forward {
        dir += forward;
    }
    if input.back {
        dir -= forward;
    }
    if input.right {
        dir += right;
    }
    if input.left {
        dir -= right;
    }
    let speed = if input.sprint { SPRINT_SPEED } else { WALK_SPEED };
    let velocity = dir.normalize_or_zero() * speed;
    body.update(dt, velocity, input.jump, world);
}

/// What a player is holding down and where they are looking. Sent whole whenever it changes.
#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct PlayerInput {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub sprint: bool,
    pub jump: bool,
    /// Left mouse button held: mining, while a block is under the crosshair.
    pub mining: bool,
    pub yaw: f32,
    pub pitch: f32,
}

/// Everything that belongs to one player.
pub struct PlayerState {
    pub id: PlayerId,
    pub body: Player,
    pub input: PlayerInput,
    pub resources: Resources,
    pub converter: Converter,
    pub combat: Combat,
    /// Seconds the button has been held on the current block.
    pub mine_timer: f32,
    /// Seconds until the next round of a fight may start.
    pub round_pause: f32,
    /// The player's own computer works out where they walk (see `Command::SetPose`), so the
    /// host stops moving them.
    pose_from_client: bool,
    /// Seconds since a pose was last accepted.
    since_pose: f32,
    /// How much faster conversion is right now (a forge or furnace is close).
    pub conversion_boost: u32,
    was_busy: bool,
}

impl PlayerState {
    fn new(id: PlayerId, pos: Vec3) -> Self {
        PlayerState {
            id,
            body: Player::new(pos),
            input: PlayerInput::default(),
            // A big pile of everything to try things out with. Use `Resources::default()` for
            // a normal start.
            resources: Resources::test_kit(),
            converter: Converter::default(),
            combat: Combat::new(),
            mine_timer: 0.0,
            round_pause: 0.0,
            pose_from_client: false,
            since_pose: 0.0,
            conversion_boost: 1,
            was_busy: false,
        }
    }
}

/// Things a player can do.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Command {
    /// Keys held and look direction.
    SetInput(PlayerInput),
    /// Where the player's own computer says they are, and where they look. This is how a
    /// player moves in a network game: they walk on their own copy of the world, so it feels
    /// instant, and the host takes their word for it, as long as it is not an impossible jump.
    SetPose { pos: Vec3, yaw: f32, pitch: f32 },
    /// A click: starts a fight with the passive creature under the crosshair, or skips ahead
    /// in the current round.
    Attack,
    /// Right click with nothing selected: puts down a copy of the block under the crosshair.
    PlaceBlock,
    /// Right click with a dense resource selected: applies it to the block under the crosshair.
    ApplyToTarget { kind: usize },
    /// Applies a dense resource to a particular block (from the apply menu).
    ApplyAt { kind: usize, pos: (i32, i32, i32) },
    /// Picks what the converter works on (or stops it, if it is already on that).
    ToggleConversion(usize),
}

/// Things that happened that a screen or speaker may want to show.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Event {
    /// A player mined the block at `pos` (`ice` if it was ice).
    Dug { player: PlayerId, block: Block, ice: bool, pos: (i32, i32, i32) },
    /// A click ended the round in flight early, so health bars should jump to their end value.
    RoundSkipped { player: PlayerId },
}

pub struct Game {
    seed: u32,
    pub world: World,
    pub enemies: Vec<Enemy>,
    pub weather: Weather,
    pub fires: Fires,
    players: Vec<PlayerState>,
    dice: Dice,
    /// Where creatures live, worked out per area of the world the first time it is needed.
    homes: std::collections::HashMap<(i32, i32), Vec<spawns::Home>>,
    /// Homes whose creature was defeated, and the seconds left until it is back.
    defeated: std::collections::HashMap<u32, f32>,
    /// Where the creature that lives near the spawn point is.
    starter_home: Vec3,
    next_player_id: u32,
    events: Vec<Event>,
}

impl Game {
    pub fn new(seed: u32) -> Self {
        let world = World::generate(seed);
        // A deer lives close to the spawn point; the rest of the creatures are placed from the
        // seed (see `spawns`).
        let (cx, cz) = (world::SIZE_X as i32 / 2, world::SIZE_Z as i32 / 2);
        let starter_home = world.open_spot(cx - 9, cz - 3);
        Game {
            seed,
            world,
            enemies: Vec::new(),
            weather: Weather::new(seed ^ 0x5EA7),
            fires: Fires::default(),
            players: Vec::new(),
            dice: Dice::with_seed(seed ^ 0xD1CE),
            homes: Default::default(),
            defeated: Default::default(),
            starter_home,
            next_player_id: 1,
            events: Vec::new(),
        }
    }

    /// The seed the world was generated from.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// A new player, standing at the world's spawn point.
    pub fn add_player(&mut self) -> PlayerId {
        let id = PlayerId(self.next_player_id);
        self.next_player_id += 1;
        self.players.push(PlayerState::new(id, self.world.spawn_point()));
        id
    }

    pub fn remove_player(&mut self, id: PlayerId) {
        self.players.retain(|p| p.id != id);
    }

    pub fn player(&self, id: PlayerId) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }

    /// Everything that happened since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn index(&self, id: PlayerId) -> Option<usize> {
        self.players.iter().position(|p| p.id == id)
    }

    /// Does what the player asked for, if it can be done.
    pub fn apply(&mut self, id: PlayerId, command: Command) {
        let Some(p) = self.index(id) else {
            return;
        };
        match command {
            Command::SetInput(mut input) => {
                // Commands can come off a network, so don't let a bad number into the game.
                if !input.yaw.is_finite() || !input.pitch.is_finite() {
                    return;
                }
                let limit = 89.0_f32.to_radians();
                input.pitch = input.pitch.clamp(-limit, limit);
                let player = &mut self.players[p];
                player.input = input;
                player.body.yaw = input.yaw;
                player.body.pitch = input.pitch;
            }
            Command::SetPose { pos, yaw, pitch } => {
                let in_world = pos.x >= 0.0
                    && pos.z >= 0.0
                    && pos.x < world::SIZE_X as f32
                    && pos.z < world::SIZE_Z as f32
                    && pos.y > -1.0
                    && pos.y < world::SIZE_Y as f32 * 2.0;
                if !(pos.is_finite() && yaw.is_finite() && pitch.is_finite() && in_world) {
                    return;
                }
                let player = &mut self.players[p];
                // Faster than anyone can fall or run is not walking.
                let allowed = MAX_POSE_SPEED * (player.since_pose + 0.1) + 1.0;
                if (pos - player.body.pos).length() > allowed {
                    return;
                }
                let limit = 89.0_f32.to_radians();
                let pitch = pitch.clamp(-limit, limit);
                player.body.pos = pos;
                player.body.yaw = yaw;
                player.body.pitch = pitch;
                player.input.yaw = yaw;
                player.input.pitch = pitch;
                player.pose_from_client = true;
                player.since_pose = 0.0;
            }
            Command::Attack => self.attack(p),
            Command::PlaceBlock => self.place_block(p),
            Command::ApplyToTarget { kind } => {
                if kind >= DENSE_KINDS {
                    return;
                }
                if let Some(pos) = self.block_in_sights_of(p) {
                    self.apply_dense(p, kind, pos);
                }
            }
            Command::ApplyAt { kind, pos } => {
                if kind < DENSE_KINDS && World::in_bounds(pos.0, pos.1, pos.2) {
                    self.apply_dense(p, kind, pos);
                }
            }
            Command::ToggleConversion(row) => self.players[p].converter.toggle(row),
        }
    }

    // ---- questions a screen asks (nothing here changes the game) ----

    /// The block under the player's crosshair within reach, unless a creature is what they
    /// are looking at.
    pub fn block_in_sights(&self, id: PlayerId) -> Option<(i32, i32, i32)> {
        self.block_in_sights_of(self.index(id)?)
    }

    /// The closest creature in reach, under the crosshair, and not behind a block.
    pub fn enemy_in_sights(&self, id: PlayerId) -> Option<Sight> {
        self.enemy_in_sights_of(self.index(id)?)
    }

    /// What applying dense resource `kind` to the block at `pos` would do right now.
    pub fn effect_at(&self, id: PlayerId, kind: usize, pos: (i32, i32, i32)) -> Option<Effect> {
        self.effect_at_of(self.index(id)?, kind, pos)
    }

    /// Whether applying `kind` to `pos` would do anything, and the player has enough of it.
    pub fn can_apply(&self, id: PlayerId, kind: usize, pos: (i32, i32, i32)) -> bool {
        let Some(p) = self.index(id) else {
            return false;
        };
        if kind >= DENSE_KINDS {
            return false;
        }
        self.effect_at_of(p, kind, pos)
            .is_some_and(|e| self.players[p].resources.dense[kind] >= e.dense_cost())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn looking_down() -> PlayerInput {
        PlayerInput {
            mining: true,
            pitch: -std::f32::consts::FRAC_PI_2 + 0.01,
            ..PlayerInput::default()
        }
    }

    fn run(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 60.0) as u32 {
            game.update(1.0 / 60.0);
        }
    }

    #[test]
    fn a_player_walks_where_the_input_says() {
        let mut game = Game::new(3);
        let me = game.add_player();
        run(&mut game, 1.0); // settle onto the ground
        let start = game.player(me).unwrap().body.pos;
        let input = PlayerInput { forward: true, yaw: 1.0, ..PlayerInput::default() };
        game.apply(me, Command::SetInput(input));
        run(&mut game, 1.0);
        let moved = game.player(me).unwrap().body.pos - start;
        assert!(Vec3::new(moved.x, 0.0, moved.z).length() > 2.0, "{moved:?}");
        // Yaw 1.0 faces +x and -z.
        assert!(moved.x > 0.0 && moved.z < 0.0, "{moved:?}");
    }

    #[test]
    fn holding_the_button_on_a_block_mines_it_and_says_so() {
        let mut game = Game::new(3);
        let me = game.add_player();
        run(&mut game, 1.0);
        let before = game.player(me).unwrap().resources.as_array();
        game.apply(me, Command::SetInput(looking_down()));
        run(&mut game, MINE_INTERVAL + 0.5);
        let after = game.player(me).unwrap().resources.as_array();
        assert_ne!(after, before, "mining gave the player something");
        let events = game.take_events();
        assert!(events.iter().any(|e| matches!(e, Event::Dug { player, .. } if *player == me)));
        assert!(game.take_events().is_empty(), "events are handed out once");
        assert!(!game.world.take_dirty().is_empty(), "the changed chunk is marked");
    }

    #[test]
    fn players_have_their_own_resources_and_the_world_is_shared() {
        let mut game = Game::new(3);
        let (a, b) = (game.add_player(), game.add_player());
        run(&mut game, 1.0);
        let b_before = game.player(b).unwrap().resources.as_array();
        game.apply(a, Command::SetInput(looking_down()));
        run(&mut game, MINE_INTERVAL + 0.5);
        assert_eq!(game.player(b).unwrap().resources.as_array(), b_before);
        // The hole a dug is in b's world too.
        let pos = game.player(a).unwrap().body.pos;
        let (x, z) = (pos.x.floor() as i32, pos.z.floor() as i32);
        let ground = (0..64).rev().find(|&y| game.world.get(x, y, z) != Block::Air);
        assert!(ground.is_some_and(|y| (y as f32) < pos.y), "{ground:?} {pos:?}");
    }

    #[test]
    fn commands_for_a_player_who_left_do_nothing() {
        let mut game = Game::new(3);
        let me = game.add_player();
        game.remove_player(me);
        game.apply(me, Command::Attack);
        game.apply(me, Command::PlaceBlock);
        game.update(1.0 / 60.0);
        assert!(game.player(me).is_none());
        assert!(game.block_in_sights(me).is_none());
    }

    #[test]
    fn a_player_who_reports_their_own_position_is_not_moved_by_the_host() {
        let mut game = Game::new(3);
        let me = game.add_player();
        run(&mut game, 1.0);
        let start = game.player(me).unwrap().body.pos;
        let target = start + Vec3::new(3.0, 0.0, 0.0);
        game.apply(me, Command::SetPose { pos: target, yaw: 0.5, pitch: 0.2 });
        // The keys say forward, but the player's own computer is in charge.
        let input = PlayerInput { forward: true, yaw: 0.5, ..PlayerInput::default() };
        game.apply(me, Command::SetInput(input));
        run(&mut game, 0.5);
        let body = &game.player(me).unwrap().body;
        assert!((body.pos - target).length() < 1e-4, "{:?} vs {target:?}", body.pos);
        assert_eq!(body.yaw, 0.5);
    }

    #[test]
    fn impossible_positions_are_refused() {
        let mut game = Game::new(3);
        let me = game.add_player();
        run(&mut game, 1.0);
        let start = game.player(me).unwrap().body.pos;
        for pos in [
            start + Vec3::new(500.0, 0.0, 0.0),
            Vec3::new(f32::NAN, 5.0, 5.0),
            Vec3::new(-10.0, 5.0, 5.0),
            Vec3::new(5.0, 5000.0, 5.0),
        ] {
            game.apply(me, Command::SetPose { pos, yaw: 0.0, pitch: 0.0 });
            assert_eq!(game.player(me).unwrap().body.pos, start, "{pos:?}");
        }
        game.apply(me, Command::SetPose { pos: start, yaw: f32::NAN, pitch: 0.0 });
        assert_eq!(game.player(me).unwrap().body.yaw, 0.0);
        // A wild kind of resource doesn't crash anything.
        game.apply(me, Command::ApplyToTarget { kind: 99 });
        game.apply(me, Command::ApplyAt { kind: 99, pos: (1, 1, 1) });
        game.apply(me, Command::ApplyAt { kind: 0, pos: (i32::MAX, 0, i32::MIN) });
        assert!(!game.can_apply(me, 99, (1, 1, 1)));
    }
}
