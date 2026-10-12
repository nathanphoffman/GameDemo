use serde::{Deserialize, Serialize};

use crate::biome::Biome;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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
    /// The viewer is in the cold biome, where rain falls as snow. Set by whoever draws the
    /// weather (the host doesn't care).
    pub snowing: bool,
    /// The biome the viewer is in. Set by whoever draws the weather.
    pub biome: Biome,
    /// The camera is under water, so the scene is drawn blue. Set by whoever draws the weather.
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
}
