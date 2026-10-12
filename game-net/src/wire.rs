//! Putting messages on a TCP stream: a 4 byte length, then the message.

use std::io::{self, Read, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Bigger than this is not a real message. A new player's welcome (every block ever changed) is
/// the largest thing sent.
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// The message as bytes, with its length in front, ready to write.
pub fn encode<T: Serialize>(message: &T) -> io::Result<Vec<u8>> {
    let body = postcard::to_stdvec(message).map_err(io::Error::other)?;
    if body.len() > MAX_FRAME {
        return Err(io::Error::other("message too big"));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn read_message<T: DeserializeOwned>(stream: &mut impl Read) -> io::Result<T> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too big"));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;
    postcard::from_bytes(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_message<T: Serialize>(stream: &mut impl Write, message: &T) -> io::Result<()> {
    stream.write_all(&encode(message)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ClientMessage, PROTOCOL_VERSION};
    use game_core::game::{Command, PlayerInput};

    #[test]
    fn messages_survive_the_trip() {
        let mut bytes = Vec::new();
        write_message(&mut bytes, &ClientMessage::Hello { version: PROTOCOL_VERSION, name: "Ann".into() }).unwrap();
        let input = PlayerInput { forward: true, yaw: 1.5, ..PlayerInput::default() };
        write_message(&mut bytes, &ClientMessage::Command(Command::SetInput(input))).unwrap();

        let mut stream = &bytes[..];
        let first: ClientMessage = read_message(&mut stream).unwrap();
        assert!(matches!(first, ClientMessage::Hello { version, ref name } if version == PROTOCOL_VERSION && name == "Ann"));
        let second: ClientMessage = read_message(&mut stream).unwrap();
        match second {
            ClientMessage::Command(Command::SetInput(got)) => {
                assert!(got.forward && !got.back);
                assert_eq!(got.yaw, 1.5);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nonsense_and_oversized_frames_are_errors_not_crashes() {
        let junk = [3u8, 0, 0, 0, 0xFF, 0xFF, 0xFF];
        assert!(read_message::<ClientMessage>(&mut &junk[..]).is_err());
        let huge = [0xFFu8, 0xFF, 0xFF, 0xFF];
        assert!(read_message::<ClientMessage>(&mut &huge[..]).is_err());
        let cut_short = [10u8, 0, 0, 0, 1, 2];
        assert!(read_message::<ClientMessage>(&mut &cut_short[..]).is_err());
    }
}
