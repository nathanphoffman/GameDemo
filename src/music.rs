use std::io::Cursor;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const TRACK: &[u8] = include_bytes!("../assets/looping_tune.wav");
const VOLUME: f32 = 0.5;

/// Keeps the soundtrack playing; dropping this stops the music.
pub struct Music {
    _stream: OutputStream,
    _handle: OutputStreamHandle,
    _sink: Sink,
}

impl Music {
    /// Returns None (and the game stays silent) if there is no audio device.
    pub fn start() -> Option<Music> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let sink = Sink::try_new(&handle).ok()?;
        let source = Decoder::new(Cursor::new(TRACK)).ok()?.repeat_infinite();
        sink.set_volume(VOLUME);
        sink.append(source);
        Some(Music {
            _stream: stream,
            _handle: handle,
            _sink: sink,
        })
    }
}
