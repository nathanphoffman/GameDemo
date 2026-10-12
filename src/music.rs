use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

use crate::world::Block;

const TRACK: &[u8] = include_bytes!("../assets/looping_tune.wav");
const RAIN: &[u8] = include_bytes!("../assets/rain_loop.wav");
const SNOW: &[u8] = include_bytes!("../assets/snow_loop.wav");
const SCARY: &[u8] = include_bytes!("../assets/scary_loop.wav");
const DIG_GRASS: &[u8] = include_bytes!("../assets/dig_grass.wav");
const DIG_DIRT: &[u8] = include_bytes!("../assets/dig_dirt.wav");
const DIG_STONE: &[u8] = include_bytes!("../assets/dig_stone.wav");
const DIG_SNOW: &[u8] = include_bytes!("../assets/dig_snow.wav");
const DIG_ICE: &[u8] = include_bytes!("../assets/dig_ice.wav");
const DIG_CARBON: &[u8] = include_bytes!("../assets/dig_carbon.wav");
const VOLUME: f32 = 0.5;
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

/// Keeps the soundtrack, the weather sounds and the mining sounds going; dropping this stops
/// them.
pub struct Music {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    tune: Sink,
    scary: Sink,
    rain: Sink,
    snow: Sink,
    /// Current levels, 0 to 1: rain, snow, and the scary music (the tune is the rest).
    levels: [f32; 3],
}

impl Music {
    /// Returns None (and the game stays silent) if there is no audio device.
    pub fn start() -> Option<Music> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let tune = Sink::try_new(&handle).ok()?;
        tune.set_volume(VOLUME);
        tune.append(Decoder::new(Cursor::new(TRACK)).ok()?.repeat_infinite());

        // The weather loops and the scary music play all the time, silently, so they are in
        // step when they fade in.
        let looped = |bytes: &'static [u8]| -> Option<Sink> {
            let sink = Sink::try_new(&handle).ok()?;
            sink.set_volume(0.0);
            sink.append(Decoder::new(Cursor::new(bytes)).ok()?.repeat_infinite());
            Some(sink)
        };
        Some(Music {
            rain: looped(RAIN)?,
            snow: looped(SNOW)?,
            scary: looped(SCARY)?,
            tune,
            _stream: stream,
            handle,
            levels: [0.0; 3],
        })
    }

    /// `rain` and `snow` are how loud each weather loop should be (0 to 1), and `scary` is 1 in
    /// the creepy biomes, where the scary music takes over from the tune. Everything fades.
    pub fn update(&mut self, rain: f32, snow: f32, scary: f32, dt: f32) {
        let step = FADE_PER_SECOND * dt;
        for (level, target) in self.levels.iter_mut().zip([rain, snow, scary]) {
            let target = target.clamp(0.0, 1.0);
            *level += (target - *level).clamp(-step, step);
        }
        self.rain.set_volume(self.levels[0] * WEATHER_VOLUME);
        self.snow.set_volume(self.levels[1] * WEATHER_VOLUME);
        self.scary.set_volume(self.levels[2] * VOLUME);
        self.tune.set_volume((1.0 - self.levels[2]) * VOLUME);
    }

    /// Plays the sound of mining `block` once. `ice` is for water that is frozen over.
    pub fn dig(&self, block: Block, ice: bool) {
        if let Ok(source) = Decoder::new(Cursor::new(dig_sound(block, ice))) {
            let _ = self.handle.play_raw(source.convert_samples());
        }
    }
}
