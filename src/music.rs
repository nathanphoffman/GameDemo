use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

use crate::biome::Biome;
use crate::world::Block;

const RAIN: &[u8] = include_bytes!("../assets/rain_loop.wav");
const SNOW: &[u8] = include_bytes!("../assets/snow_loop.wav");
const DIG_GRASS: &[u8] = include_bytes!("../assets/dig_grass.wav");
const DIG_DIRT: &[u8] = include_bytes!("../assets/dig_dirt.wav");
const DIG_STONE: &[u8] = include_bytes!("../assets/dig_stone.wav");
const DIG_SNOW: &[u8] = include_bytes!("../assets/dig_snow.wav");
const DIG_ICE: &[u8] = include_bytes!("../assets/dig_ice.wav");
const DIG_CARBON: &[u8] = include_bytes!("../assets/dig_carbon.wav");
// The tracks for each biome (made by tools/make_music.py, apart from the first plains one and
// the first volcanic one, which are the original tune and scary loop).
const PLAINS: [&[u8]; 3] = [
    include_bytes!("../assets/looping_tune.wav"),
    include_bytes!("../assets/music/plains_1.wav"),
    include_bytes!("../assets/music/plains_2.wav"),
];
const MEADOW: [&[u8]; 3] = [
    include_bytes!("../assets/music/meadow_1.wav"),
    include_bytes!("../assets/music/meadow_2.wav"),
    include_bytes!("../assets/music/meadow_3.wav"),
];
const FOREST: [&[u8]; 3] = [
    include_bytes!("../assets/music/forest_1.wav"),
    include_bytes!("../assets/music/forest_2.wav"),
    include_bytes!("../assets/music/forest_3.wav"),
];
const SWAMP: [&[u8]; 3] = [
    include_bytes!("../assets/music/swamp_1.wav"),
    include_bytes!("../assets/music/swamp_2.wav"),
    include_bytes!("../assets/music/swamp_3.wav"),
];
const DESERT: [&[u8]; 3] = [
    include_bytes!("../assets/music/desert_1.wav"),
    include_bytes!("../assets/music/desert_2.wav"),
    include_bytes!("../assets/music/desert_3.wav"),
];
const BADLANDS: [&[u8]; 3] = [
    include_bytes!("../assets/music/badlands_1.wav"),
    include_bytes!("../assets/music/badlands_2.wav"),
    include_bytes!("../assets/music/badlands_3.wav"),
];
const VOLCANIC: [&[u8]; 3] = [
    include_bytes!("../assets/scary_loop.wav"),
    include_bytes!("../assets/music/volcanic_1.wav"),
    include_bytes!("../assets/music/volcanic_2.wav"),
];
const SNOWY: [&[u8]; 3] = [
    include_bytes!("../assets/music/snow_1.wav"),
    include_bytes!("../assets/music/snow_2.wav"),
    include_bytes!("../assets/music/snow_3.wav"),
];

/// The tracks that can play in a biome.
fn tracks_for(biome: Biome) -> &'static [&'static [u8]] {
    match biome {
        Biome::Plains => &PLAINS,
        Biome::Meadow => &MEADOW,
        Biome::Forest => &FOREST,
        Biome::Swamp => &SWAMP,
        Biome::Desert => &DESERT,
        Biome::Badlands => &BADLANDS,
        Biome::Volcanic => &VOLCANIC,
        Biome::Snow => &SNOWY,
    }
}

const VOLUME: f32 = 0.5;
/// A track plays for a random time between these (seconds), looping, and then fades out...
const PLAY_SECONDS: (f32, f32) = (30.0, 120.0);
/// ...over this long, ...
const FADE_OUT_SECONDS: f32 = 4.0;
/// ...and is followed by a random silence between these (seconds).
const SILENCE_SECONDS: (f32, f32) = (0.0, 15.0);
/// A new track fades in over this long.
const FADE_IN_SECONDS: f32 = 2.0;
/// Having walked into another biome, this long passes before the music follows, so that
/// walking along a border doesn't keep restarting it.
const BIOME_SETTLE_SECONDS: f32 = 3.0;
/// How loud the rain and snow loops are at full strength.
const WEATHER_VOLUME: f32 = 0.7;
/// Weather sounds fade in and out at this much volume (0 to 1) per second.
const FADE_PER_SECOND: f32 = 0.5;

/// Which sound mining a block makes.
fn dig_sound(block: Block, ice: bool) -> &'static [u8] {
    match block {
        _ if ice => DIG_ICE,
        Block::Grass | Block::Leaves | Block::PineLeaves | Block::Cactus | Block::HousePlant => {
            DIG_GRASS
        }
        Block::Snow => DIG_SNOW,
        Block::Coal | Block::Burning | Block::Magma => DIG_CARBON,
        Block::Stone
        | Block::RedRock
        | Block::Basalt
        | Block::StoneFloor
        | Block::Forge
        | Block::Furnace => DIG_STONE,
        // Dirt, sand, clay and wood: a dull thud.
        _ => DIG_DIRT,
    }
}

/// Decides what music plays when: a random track from the biome for a random while, a fade,
/// a random silence, and the next one. Knows nothing about speakers, so it can be tested.
struct Schedule {
    rng: u32,
    /// The biome the music is for (the player's, once they have stayed a few seconds).
    biome: Biome,
    /// A different biome the player is in, and for how long.
    pending: Option<(Biome, f32)>,
    state: State,
    /// The last track that played, so that the next one is a different one.
    last: Option<usize>,
}

enum State {
    /// Nothing plays for this many more seconds.
    Silent(f32),
    /// A track is playing, and has this many seconds left before it fades.
    Playing { biome: Biome, left: f32 },
}

#[derive(PartialEq, Debug)]
enum Cue {
    /// Fade out whatever is playing.
    FadeOut,
    /// Start track `index` of `biome`'s tracks.
    Start { biome: Biome, index: usize },
}

impl Schedule {
    fn new(seed: u32, biome: Biome) -> Schedule {
        let mut schedule = Schedule {
            rng: seed.max(1),
            biome,
            pending: None,
            state: State::Silent(0.0),
            last: None,
        };
        // The first one starts after a short wait.
        let wait = schedule.between(0.0, SILENCE_SECONDS.1.min(5.0));
        schedule.state = State::Silent(wait);
        schedule
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

    fn between(&mut self, low: f32, high: f32) -> f32 {
        let unit = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        low + (high - low) * unit
    }

    fn pick(&mut self, biome: Biome) -> usize {
        let count = tracks_for(biome).len();
        let mut index = (self.next_u32() >> 8) as usize % count;
        if count > 1 && Some(index) == self.last {
            index = (index + 1) % count;
        }
        self.last = Some(index);
        index
    }

    fn update(&mut self, dt: f32, in_biome: Biome) -> Option<Cue> {
        // Follow the player into another biome once they have stayed.
        if in_biome == self.biome {
            self.pending = None;
        } else {
            let waited = match self.pending {
                Some((biome, waited)) if biome == in_biome => waited + dt,
                _ => dt,
            };
            self.pending = Some((in_biome, waited));
            if waited >= BIOME_SETTLE_SECONDS {
                self.biome = in_biome;
                self.pending = None;
                if let State::Playing { biome, .. } = self.state
                    && biome != in_biome
                {
                    // The track from the other biome goes, and this one's music starts
                    // straight away.
                    self.state = State::Silent(0.0);
                    return Some(Cue::FadeOut);
                }
            }
        }

        match &mut self.state {
            State::Playing { left, .. } => {
                *left -= dt;
                if *left <= 0.0 {
                    let silence = self.between(SILENCE_SECONDS.0, SILENCE_SECONDS.1);
                    self.state = State::Silent(silence);
                    return Some(Cue::FadeOut);
                }
                None
            }
            State::Silent(left) => {
                *left -= dt;
                if *left > 0.0 {
                    return None;
                }
                let biome = self.biome;
                let index = self.pick(biome);
                let length = self.between(PLAY_SECONDS.0, PLAY_SECONDS.1);
                self.state = State::Playing { biome, left: length };
                Some(Cue::Start { biome, index })
            }
        }
    }
}

/// A track that is playing, and how long it has been.
struct Playing {
    sink: Sink,
    age: f32,
}

/// Keeps the soundtrack, the weather sounds and the mining sounds going; dropping this stops
/// them.
pub struct Music {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    schedule: Schedule,
    /// The track that is playing (fading in, then at full volume).
    playing: Option<Playing>,
    /// Tracks on their way out: the sink, and the seconds of fading it has left.
    fading: Vec<(Sink, f32)>,
    rain: Sink,
    snow: Sink,
    /// Current levels, 0 to 1: rain and snow.
    levels: [f32; 2],
}

impl Music {
    /// Returns None (and the game stays silent) if there is no audio device.
    pub fn start() -> Option<Music> {
        let (stream, handle) = OutputStream::try_default().ok()?;

        // The weather loops play all the time, silently, so they are in step when they fade
        // in.
        let looped = |bytes: &'static [u8]| -> Option<Sink> {
            let sink = Sink::try_new(&handle).ok()?;
            sink.set_volume(0.0);
            sink.append(Decoder::new(Cursor::new(bytes)).ok()?.repeat_infinite());
            Some(sink)
        };
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(1);
        Some(Music {
            rain: looped(RAIN)?,
            snow: looped(SNOW)?,
            schedule: Schedule::new(seed, Biome::Plains),
            playing: None,
            fading: Vec::new(),
            _stream: stream,
            handle,
            levels: [0.0; 2],
        })
    }

    /// `rain` and `snow` are how loud each weather loop should be (0 to 1), and `biome` is
    /// where the player is, which says what music plays. Everything fades.
    pub fn update(&mut self, rain: f32, snow: f32, biome: Biome, dt: f32) {
        let step = FADE_PER_SECOND * dt;
        for (level, target) in self.levels.iter_mut().zip([rain, snow]) {
            let target = target.clamp(0.0, 1.0);
            *level += (target - *level).clamp(-step, step);
        }
        self.rain.set_volume(self.levels[0] * WEATHER_VOLUME);
        self.snow.set_volume(self.levels[1] * WEATHER_VOLUME);

        match self.schedule.update(dt, biome) {
            Some(Cue::FadeOut) => {
                if let Some(track) = self.playing.take() {
                    self.fading.push((track.sink, FADE_OUT_SECONDS));
                }
            }
            Some(Cue::Start { biome, index }) => self.start_track(biome, index),
            None => {}
        }

        if let Some(track) = &mut self.playing {
            track.age += dt;
            let fade_in = (track.age / FADE_IN_SECONDS).min(1.0);
            track.sink.set_volume(fade_in * VOLUME);
        }
        for (sink, left) in &mut self.fading {
            *left -= dt;
            sink.set_volume((*left / FADE_OUT_SECONDS).max(0.0) * VOLUME);
        }
        self.fading.retain(|(_, left)| *left > 0.0);
    }

    fn start_track(&mut self, biome: Biome, index: usize) {
        let bytes = tracks_for(biome)[index];
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        let Ok(source) = Decoder::new(Cursor::new(bytes)) else {
            return;
        };
        sink.set_volume(0.0);
        sink.append(source.repeat_infinite());
        self.playing = Some(Playing { sink, age: 0.0 });
    }

    /// Plays the sound of mining `block` once, at `volume` (1 is full). `ice` is for water that
    /// is frozen over. Someone else digging is quieter the further away they are.
    pub fn dig_at(&self, block: Block, ice: bool, volume: f32) {
        if let Ok(source) = Decoder::new(Cursor::new(dig_sound(block, ice))) {
            let _ = self
                .handle
                .play_raw(source.convert_samples::<f32>().amplify(volume.clamp(0.0, 1.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the schedule for a long time and says when each track started and each fade began.
    fn run(schedule: &mut Schedule, biome: impl Fn(f32) -> Biome, seconds: f32) -> Vec<(f32, Cue)> {
        let mut cues = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            if let Some(cue) = schedule.update(0.1, biome(t)) {
                cues.push((t, cue));
            }
            t += 0.1;
        }
        cues
    }

    #[test]
    fn tracks_last_30_to_120_seconds_with_up_to_15_of_silence() {
        let mut schedule = Schedule::new(42, Biome::Plains);
        let cues = run(&mut schedule, |_| Biome::Plains, 3.0 * 3600.0);
        let mut started = None;
        let mut faded = None;
        let mut played = 0;
        for (t, cue) in cues {
            match cue {
                Cue::Start { .. } => {
                    if let Some(at) = faded {
                        let gap = t - at;
                        assert!((0.0..=15.2).contains(&gap), "silence of {gap}");
                    }
                    started = Some(t);
                }
                Cue::FadeOut => {
                    let length = t - started.expect("it started first");
                    assert!((29.8..=120.2).contains(&length), "played for {length}");
                    faded = Some(t);
                    played += 1;
                }
            }
        }
        assert!(played > 50, "{played} tracks");
    }

    #[test]
    fn every_track_of_a_biome_comes_up_and_never_the_same_one_twice_in_a_row() {
        let mut schedule = Schedule::new(7, Biome::Swamp);
        let cues = run(&mut schedule, |_| Biome::Swamp, 6.0 * 3600.0);
        let mut seen = Vec::new();
        let mut previous = None;
        for (_, cue) in cues {
            if let Cue::Start { biome, index } = cue {
                assert_eq!(biome, Biome::Swamp);
                assert_ne!(Some(index), previous);
                previous = Some(index);
                if !seen.contains(&index) {
                    seen.push(index);
                }
            }
        }
        assert_eq!(seen.len(), tracks_for(Biome::Swamp).len());
    }

    #[test]
    fn walking_into_another_biome_swaps_the_music_after_a_few_seconds() {
        let mut schedule = Schedule::new(3, Biome::Plains);
        let in_desert_from = 200.0;
        let biome = |t: f32| if t < in_desert_from { Biome::Plains } else { Biome::Desert };
        let cues = run(&mut schedule, biome, 400.0);
        let after: Vec<&(f32, Cue)> = cues.iter().filter(|(t, _)| *t >= in_desert_from).collect();
        // Something from the desert starts, no sooner than the settle time.
        let first_desert = after
            .iter()
            .find(|(_, cue)| matches!(cue, Cue::Start { biome: Biome::Desert, .. }))
            .expect("desert music plays");
        assert!(first_desert.0 >= in_desert_from + BIOME_SETTLE_SECONDS - 0.2);
        // And nothing from the plains starts after the walk.
        assert!(!after.iter().any(|(_, cue)| matches!(cue, Cue::Start { biome: Biome::Plains, .. })));
    }

    #[test]
    fn a_quick_step_over_a_border_does_not_restart_the_music() {
        let mut schedule = Schedule::new(5, Biome::Plains);
        // Start the music, then flicker between two biomes every second.
        run(&mut schedule, |_| Biome::Plains, 10.0);
        let cues = run(
            &mut schedule,
            |t| if (t as u32) % 2 == 0 { Biome::Plains } else { Biome::Forest },
            2.0,
        );
        assert!(cues.is_empty(), "{cues:?}");
    }

    #[test]
    fn every_biome_has_tracks() {
        let all = [
            Biome::Plains,
            Biome::Meadow,
            Biome::Forest,
            Biome::Swamp,
            Biome::Desert,
            Biome::Badlands,
            Biome::Volcanic,
            Biome::Snow,
        ];
        for biome in all {
            assert!(!tracks_for(biome).is_empty(), "{biome:?}");
        }
    }
}
