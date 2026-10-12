use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const TRACK: &[u8] = include_bytes!("../assets/looping_tune.wav");
const RAIN: &[u8] = include_bytes!("../assets/rain_loop.wav");
const SNOW: &[u8] = include_bytes!("../assets/snow_loop.wav");
const VOLUME: f32 = 0.5;
/// How loud the rain and snow loops are at full strength.
const WEATHER_VOLUME: f32 = 0.7;
/// Weather sounds fade in and out at this much volume (0 to 1) per second.
const FADE_PER_SECOND: f32 = 0.5;

/// Keeps the soundtrack and the weather sounds playing; dropping this stops them.
pub struct Music {
    _stream: OutputStream,
    _handle: OutputStreamHandle,
    _sink: Sink,
    rain: Sink,
    snow: Sink,
    /// Current rain and snow levels, 0 to 1.
    levels: [f32; 2],
}

impl Music {
    /// Returns None (and the game stays silent) if there is no audio device.
    pub fn start() -> Option<Music> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let sink = Sink::try_new(&handle).ok()?;
        let source = Decoder::new(Cursor::new(TRACK)).ok()?.repeat_infinite();
        sink.set_volume(VOLUME);
        sink.append(source);

        // The weather loops play all the time, silently, so they are in step when they fade in.
        let looped = |bytes: &'static [u8]| -> Option<Sink> {
            let sink = Sink::try_new(&handle).ok()?;
            sink.set_volume(0.0);
            sink.append(Decoder::new(Cursor::new(bytes)).ok()?.repeat_infinite());
            Some(sink)
        };
        Some(Music {
            rain: looped(RAIN)?,
            snow: looped(SNOW)?,
            _stream: stream,
            _handle: handle,
            _sink: sink,
            levels: [0.0; 2],
        })
    }

    /// `rain` and `snow` are how loud each loop should be (0 to 1); they fade toward it.
    pub fn set_weather(&mut self, rain: f32, snow: f32, dt: f32) {
        let step = FADE_PER_SECOND * dt;
        for (level, target) in self.levels.iter_mut().zip([rain, snow]) {
            let target = target.clamp(0.0, 1.0);
            *level += (target - *level).clamp(-step, step);
        }
        self.rain.set_volume(self.levels[0] * WEATHER_VOLUME);
        self.snow.set_volume(self.levels[1] * WEATHER_VOLUME);
    }
}
