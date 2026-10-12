use glam::Vec3;

use crate::biome::Biome;
use crate::world::{hash, Vertex};

/// Seconds of gameplay between weather checks.
pub const CHECK_INTERVAL_SECONDS: f32 = 300.0;
/// Chance that a check picks one of the non-sunny kinds, from 0 to 1. Otherwise the weather
/// is sunny, so about two checks in three give sun.
pub const NON_SUNNY_CHANCE: f32 = 1.0 / 3.0;

/// How long a change takes to fade in (roughly: it is 95% of the way there by then).
const BLEND_SECONDS: f32 = 4.0;
/// Clouds drift this many cloud cells per second on a calm day, and `1 + 6 * wind` times that
/// when it is windy.
const BASE_DRIFT: [f32; 2] = [0.018, 0.007];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Sunny,
    Fog,
    LightRain,
    HeavyRain,
    Windy,
    /// A light haze, mostly in swamps and on volcanoes.
    Mist,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Sunny,
        Kind::Fog,
        Kind::LightRain,
        Kind::HeavyRain,
        Kind::Windy,
        Kind::Mist,
    ];

    /// Shown in the corner of the screen.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Sunny => "SUNNY",
            Kind::Fog => "FOG",
            Kind::LightRain => "RAIN",
            Kind::HeavyRain => "HEAVY RAIN",
            Kind::Windy => "WINDY",
            Kind::Mist => "MIST",
        }
    }

    /// What this weather looks like once fully faded in.
    pub fn params(self) -> Params {
        let (fog_start, fog_end, overcast, dim, rain, wind) = match self {
            Kind::Sunny => (70.0, 230.0, 0.0, 0.0, 0.0, 0.3),
            Kind::Fog => (4.0, 55.0, 0.35, 0.15, 0.0, 0.2),
            Kind::LightRain => (40.0, 170.0, 0.7, 0.2, 0.45, 0.4),
            Kind::HeavyRain => (20.0, 110.0, 1.0, 0.4, 1.0, 0.8),
            Kind::Windy => (70.0, 230.0, 0.2, 0.0, 0.0, 1.0),
            Kind::Mist => (20.0, 130.0, 0.3, 0.1, 0.0, 0.15),
        };
        Params {
            fog_start,
            fog_end,
            overcast,
            dim,
            rain,
            wind,
        }
    }
}

/// The numbers the renderer uses.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Params {
    /// Distance fog fades in between these two distances, in blocks.
    pub fog_start: f32,
    pub fog_end: f32,
    /// 0 = clear blue sky, 1 = full grey overcast (more, darker clouds, greyer sky).
    pub overcast: f32,
    /// How much the whole scene is darkened, 0 to 1.
    pub dim: f32,
    /// Rain streaks on screen, 0 to 1.
    pub rain: f32,
    /// 0 = calm, 1 = very windy (fast clouds, streaks of wind, slanted rain).
    pub wind: f32,
}

impl Params {
    fn toward(self, target: Params, amount: f32) -> Params {
        let mix = |a: f32, b: f32| a + (b - a) * amount;
        Params {
            fog_start: mix(self.fog_start, target.fog_start),
            fog_end: mix(self.fog_end, target.fog_end),
            overcast: mix(self.overcast, target.overcast),
            dim: mix(self.dim, target.dim),
            rain: mix(self.rain, target.rain),
            wind: mix(self.wind, target.wind),
        }
    }
}

pub struct Weather {
    pub kind: Kind,
    /// What is on screen right now: fades toward the current kind's numbers.
    pub params: Params,
    /// How far the clouds have drifted, in cloud cells.
    pub cloud_offset: [f32; 2],
    /// The player is in the cold biome, where rain falls as snow. Set by the game each frame.
    pub snowing: bool,
    /// The biome the player is in. Set by the game each frame.
    pub biome: Biome,
    /// The camera is under water, so the scene is drawn blue. Set by the game each frame.
    pub underwater: bool,
    clock: f32,
    check_interval: f32,
    chance: f32,
    rng: u32,
}

impl Weather {
    pub fn new(seed: u32) -> Weather {
        Weather::with_settings(seed, CHECK_INTERVAL_SECONDS, NON_SUNNY_CHANCE)
    }

    pub fn with_settings(seed: u32, check_interval: f32, chance: f32) -> Weather {
        Weather {
            kind: Kind::Sunny,
            params: Kind::Sunny.params(),
            cloud_offset: [0.0, 0.0],
            snowing: false,
            biome: Biome::Plains,
            underwater: false,
            clock: 0.0,
            check_interval,
            chance,
            rng: seed.max(1),
        }
    }

    /// Shown in the corner of the screen: the kind's name, with rain called snow in the cold.
    pub fn label(&self) -> &'static str {
        match (self.effective_kind(), self.snowing) {
            (Kind::LightRain, true) => "SNOW",
            (Kind::HeavyRain, true) => "HEAVY SNOW",
            (kind, _) => kind.label(),
        }
    }

    /// The weather that actually reaches the player: the world's weather, bent by the biome.
    /// Deserts turn rain into dust storms, swamps and volcanoes are hazy, forests are wetter.
    pub fn effective_kind(&self) -> Kind {
        match (self.biome, self.kind) {
            (Biome::Desert, Kind::LightRain | Kind::HeavyRain) => Kind::Windy,
            (Biome::Desert, Kind::Fog | Kind::Mist) => Kind::Sunny,
            (Biome::Swamp | Biome::Volcanic, Kind::Sunny) => Kind::Mist,
            (Biome::Volcanic, Kind::LightRain | Kind::HeavyRain) => Kind::Fog,
            (Biome::Forest, Kind::Fog | Kind::Mist) => Kind::LightRain,
            (_, kind) => kind,
        }
    }

    fn next_u32(&mut self) -> u32 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    /// A number from 0 up to (not including) 1.
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Advances the clock by `dt` seconds of gameplay.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        while self.clock >= self.check_interval {
            self.clock -= self.check_interval;
            self.kind = if self.unit() < self.chance {
                self.pick_non_sunny()
            } else {
                Kind::Sunny
            };
        }

        let rate = 3.0 / BLEND_SECONDS; // 1 - e^-3 is about 95%
        let amount = (dt * rate).min(1.0);
        self.params = self.params.toward(self.effective_kind().params(), amount);

        let speed = 1.0 + 6.0 * self.params.wind;
        self.cloud_offset[0] += BASE_DRIFT[0] * speed * dt;
        self.cloud_offset[1] += BASE_DRIFT[1] * speed * dt;
    }

    /// A random kind of weather other than sunny. It can be the same as the current one.
    fn pick_non_sunny(&mut self) -> Kind {
        let others: Vec<Kind> = Kind::ALL
            .into_iter()
            .filter(|k| *k != Kind::Sunny)
            .collect();
        others[(self.next_u32() >> 8) as usize % others.len()]
    }
}

// ---- rain and wind streaks ----
//
// These are real little lines floating in the world around the player, not a picture glued to
// the screen. Each one has a fixed place in a box that follows the player (it wraps around
// when the player walks away from it), so turning your head or walking moves them across the
// view the way real rain or wind would.

/// The most rain streaks drawn at once (at full rain), and the most wind streaks.
pub const MAX_RAIN: usize = 1400;
pub const MAX_WIND: usize = 90;
/// Most vertices `streak_mesh` can return.
pub const MAX_STREAK_VERTICES: usize = (MAX_RAIN + MAX_WIND) * 4;

/// Size of the box around the player the rain and the wind streaks live in, in blocks.
const RAIN_BOX: Vec3 = Vec3::new(30.0, 20.0, 30.0);
const WIND_BOX: Vec3 = Vec3::new(44.0, 12.0, 44.0);
/// Blocks per second.
const RAIN_FALL_SPEED: f32 = 15.0;
const WIND_SPEED: f32 = 9.0;

// Texture layers (see textures.rs).
const RAIN_LAYER: f32 = 32.0;
const WIND_LAYER: f32 = 33.0;
const RAIN_WIDTH: f32 = 0.012;
const SNOW_WIDTH: f32 = 0.035;
/// Snowflakes drift down slowly, as short fat streaks.
const SNOW_FALL_SPEED: f32 = 2.5;
const WIND_WIDTH: f32 = 0.02;

/// Where the wind blows, as a direction along the ground. It wanders a little over time.
fn wind_direction(time: f32) -> Vec3 {
    let angle = 0.9 + 0.25 * (time * 0.05).sin();
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

/// Wraps `p` into a box of size `size` centered on `center`.
fn wrap_into_box(p: Vec3, center: Vec3, size: Vec3) -> Vec3 {
    let rel = p - center;
    let shift = (rel / size + Vec3::splat(0.5)).floor() * size;
    center + rel - shift
}

/// A streak's (head, tail) position.
type Streak = (Vec3, Vec3);

/// All the streaks' (head, tail) positions: rain, then wind.
pub fn streaks(
    cam: Vec3,
    time: f32,
    rain: f32,
    wind: f32,
    snowing: bool,
) -> (Vec<Streak>, Vec<Streak>) {
    let wind_dir = wind_direction(time);

    let rain_count = (MAX_RAIN as f32 * rain.clamp(0.0, 1.0)) as usize;
    let mut rain_streaks = Vec::with_capacity(rain_count);
    for i in 0..rain_count {
        let h = |k: u32| hash(i as u32, k, 0x4A1E);
        let base = Vec3::new(h(0), h(1), h(2)) * RAIN_BOX;
        let fall = if snowing { SNOW_FALL_SPEED } else { RAIN_FALL_SPEED } * (0.8 + 0.4 * h(3));
        // The wind leans the rain over, and each drop leans a little differently.
        let lean = wind * 3.0 * (0.8 + 0.4 * h(4));
        let velocity = Vec3::new(wind_dir.x * lean, -fall, wind_dir.z * lean);
        let position = wrap_into_box(base + velocity * time, cam, RAIN_BOX);
        let length = if snowing { 0.1 + 0.1 * h(5) } else { 0.5 + 0.6 * h(5) };
        let tail = position - velocity.normalize() * length;
        rain_streaks.push((position, tail));
    }

    // Wind streaks only show up when it is properly windy.
    let gale = ((wind - 0.55) / 0.45).clamp(0.0, 1.0);
    let wind_count = (MAX_WIND as f32 * gale) as usize;
    let mut wind_streaks = Vec::with_capacity(wind_count);
    for i in 0..wind_count {
        let h = |k: u32| hash(i as u32, k, 0x71D0);
        // Each streak has its own heading, a few degrees off the wind's, and its own drift
        // up or down, so they are never perfectly parallel.
        let turn = (h(0) - 0.5) * 0.7;
        let (sin, cos) = turn.sin_cos();
        let heading = Vec3::new(
            wind_dir.x * cos - wind_dir.z * sin,
            (h(1) - 0.5) * 0.35,
            wind_dir.x * sin + wind_dir.z * cos,
        )
        .normalize();
        let speed = WIND_SPEED * (0.7 + 0.6 * h(2));
        let base = Vec3::new(h(3), h(4), h(5)) * WIND_BOX;
        // Streaks float around head height, spread through the box.
        let position = wrap_into_box(base + heading * speed * time, cam + Vec3::Y * 2.0, WIND_BOX);
        let length = 2.0 + 2.5 * h(6);
        wind_streaks.push((position, position - heading * length));
    }
    (rain_streaks, wind_streaks)
}

/// Flat quads for all the streaks, each turned to face the camera. Four vertices per streak.
pub fn streak_mesh(cam: Vec3, time: f32, rain: f32, wind: f32, snowing: bool) -> Vec<Vertex> {
    let (rain_streaks, wind_streaks) = streaks(cam, time, rain, wind, snowing);
    let mut out = Vec::with_capacity((rain_streaks.len() + wind_streaks.len()) * 4);
    let mut push = |(a, b): (Vec3, Vec3), half_width: f32, layer: f32| {
        let along = b - a;
        let to_camera = cam - (a + b) * 0.5;
        let side = along.cross(to_camera).try_normalize().unwrap_or(Vec3::X) * half_width;
        for corner in [a - side, a + side, b + side, b - side] {
            out.push(Vertex {
                pos: corner.to_array(),
                uv: [0.0, 0.0],
                layer,
                shade: 1.0,
            });
        }
    };
    for streak in rain_streaks {
        if snowing {
            push(streak, SNOW_WIDTH, WIND_LAYER); // plain white
        } else {
            push(streak, RAIN_WIDTH, RAIN_LAYER);
        }
    }
    for streak in wind_streaks {
        push(streak, WIND_WIDTH, WIND_LAYER);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(weather: &mut Weather, seconds: f32) {
        let mut t = 0.0;
        while t < seconds {
            weather.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn starts_sunny_and_only_changes_at_each_check() {
        let mut w = Weather::with_settings(7, 300.0, 1.0);
        assert_eq!(w.kind, Kind::Sunny);
        run(&mut w, 299.0);
        assert_eq!(w.kind, Kind::Sunny, "not before the first check");
        run(&mut w, 1.5);
        assert_ne!(w.kind, Kind::Sunny, "a certain change happens at 5 minutes");
    }

    #[test]
    fn a_zero_chance_stays_sunny_and_one_in_three_checks_is_not_sunny() {
        let mut never = Weather::with_settings(3, 300.0, 0.0);
        run(&mut never, 3000.0);
        assert_eq!(never.kind, Kind::Sunny);

        let mut w = Weather::with_settings(11, 300.0, NON_SUNNY_CHANCE);
        let mut not_sunny = 0;
        for _ in 0..300 {
            run(&mut w, 300.0);
            if w.kind != Kind::Sunny {
                not_sunny += 1;
            }
        }
        assert!((70..130).contains(&not_sunny), "{not_sunny} of 300 checks not sunny");
    }

    #[test]
    fn every_kind_shows_up_eventually() {
        let mut w = Weather::with_settings(5, 15.0, 0.5);
        let mut seen = vec![Kind::Sunny];
        for _ in 0..100 {
            run(&mut w, 15.0);
            if !seen.contains(&w.kind) {
                seen.push(w.kind);
            }
        }
        assert_eq!(seen.len(), Kind::ALL.len(), "{seen:?}");
    }

    #[test]
    fn changes_fade_in_instead_of_popping() {
        let mut w = Weather::with_settings(9, 15.0, 1.0);
        w.kind = Kind::HeavyRain; // pretend a change just happened
        w.update(0.016);
        assert!(w.params.rain > 0.0 && w.params.rain < 0.1, "starts to fade in");
        run(&mut w, 8.0);
        let target = Kind::HeavyRain.params();
        assert!((w.params.rain - target.rain).abs() < 0.05);
        assert!((w.params.fog_end - target.fog_end).abs() < 5.0);
    }

    #[test]
    fn rain_is_wetter_and_foggier_than_sun_and_windy_has_the_most_wind() {
        let sunny = Kind::Sunny.params();
        for kind in [Kind::Fog, Kind::LightRain, Kind::HeavyRain] {
            assert!(kind.params().fog_end < sunny.fog_end, "{kind:?}");
        }
        assert!(Kind::HeavyRain.params().rain > Kind::LightRain.params().rain);
        assert_eq!(Kind::Sunny.params().rain, 0.0);
        for kind in Kind::ALL {
            assert!(kind.params().wind <= Kind::Windy.params().wind);
        }
    }

    #[test]
    fn clouds_drift_faster_in_the_wind() {
        let mut calm = Weather::with_settings(1, 1000.0, 0.0);
        run(&mut calm, 10.0);
        let mut windy = Weather::with_settings(1, 1000.0, 0.0);
        windy.kind = Kind::Windy;
        windy.params = Kind::Windy.params();
        run(&mut windy, 10.0);
        assert!(windy.cloud_offset[0] > calm.cloud_offset[0] * 2.0);
    }

    #[test]
    fn streak_counts_follow_rain_and_wind() {
        let cam = Vec3::new(100.0, 20.0, 100.0);
        let count = |rain: f32, wind: f32| {
            let (r, w) = streaks(cam, 3.0, rain, wind, false);
            (r.len(), w.len())
        };
        assert_eq!(count(0.0, 0.3), (0, 0), "sunny: nothing");
        let (light, _) = count(0.45, 0.4);
        let (heavy, _) = count(1.0, 0.8);
        assert!(0 < light && light < heavy);
        assert_eq!(heavy, MAX_RAIN);
        let (_, calm_wind) = count(0.0, 0.5);
        let (_, gale) = count(0.0, 1.0);
        assert_eq!(calm_wind, 0, "a breeze has no streaks");
        assert_eq!(gale, MAX_WIND);
        assert_eq!(streak_mesh(cam, 3.0, 1.0, 1.0, false).len(), (MAX_RAIN + MAX_WIND) * 4);
        assert!(streak_mesh(cam, 3.0, 1.0, 1.0, false).len() <= MAX_STREAK_VERTICES);
    }

    #[test]
    fn streaks_stay_where_they_are_in_the_world_when_the_player_moves() {
        let cam = Vec3::new(100.0, 20.0, 100.0);
        let (before, before_wind) = streaks(cam, 5.0, 1.0, 1.0, false);
        let (after, after_wind) = streaks(cam + Vec3::new(1.5, 0.0, -1.0), 5.0, 1.0, 1.0, false);
        // Almost every streak is at exactly the same place in the world; only the few that
        // had to wrap around to the other side of the box have jumped.
        let same = |a: &[(Vec3, Vec3)], b: &[(Vec3, Vec3)]| {
            a.iter()
                .zip(b)
                .filter(|(p, q)| (p.0 - q.0).length() < 1e-3)
                .count() as f32
                / a.len() as f32
        };
        assert!(same(&before, &after) > 0.85, "rain {}", same(&before, &after));
        assert!(
            same(&before_wind, &after_wind) > 0.85,
            "wind {}",
            same(&before_wind, &after_wind)
        );
    }

    #[test]
    fn rain_falls_and_stays_near_the_player() {
        let cam = Vec3::new(50.0, 30.0, 50.0);
        let (now, _) = streaks(cam, 2.0, 1.0, 0.0, false);
        let (later, _) = streaks(cam, 2.05, 1.0, 0.0, false);
        let dropped = now
            .iter()
            .zip(&later)
            .filter(|(a, b)| b.0.y < a.0.y)
            .count();
        assert!(dropped as f32 / now.len() as f32 > 0.9, "falling, bar the wrapped ones");
        for (head, _) in &now {
            let rel = *head - cam;
            assert!(rel.x.abs() <= RAIN_BOX.x / 2.0 + 1e-3);
            assert!(rel.y.abs() <= RAIN_BOX.y / 2.0 + 1e-3);
            assert!(rel.z.abs() <= RAIN_BOX.z / 2.0 + 1e-3);
        }
    }

    #[test]
    fn wind_streaks_are_not_parallel() {
        let (_, wind) = streaks(Vec3::new(0.0, 20.0, 0.0), 1.0, 0.0, 1.0, false);
        let directions: Vec<Vec3> = wind
            .iter()
            .map(|(head, tail)| (*head - *tail).normalize())
            .collect();
        let mut widest: f32 = 0.0;
        for d in &directions {
            widest = widest.max(directions[0].dot(*d).clamp(-1.0, 1.0).acos());
        }
        assert!(widest > 0.2, "widest spread {widest} radians");
        // ...but they all roughly blow the same way.
        let mean = directions.iter().fold(Vec3::ZERO, |a, b| a + *b).normalize();
        assert!(directions.iter().all(|d| d.dot(mean) > 0.5));
    }
}
