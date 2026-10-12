//! A player's side: the connection, and a copy of the host's world kept up to date.

use std::io::{self, ErrorKind};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::Duration;

use game_core::game::{Command, PlayerId, Snapshot};
use game_core::world::World;

use crate::protocol::{ClientMessage, ServerMessage, Tick, Welcome, PROTOCOL_VERSION};
use crate::wire;

pub struct Client {
    stream: TcpStream,
    incoming: Receiver<io::Result<Tick>>,
}

impl Client {
    /// Connects and says hello. Returns once the host has welcomed us.
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<(Client, Welcome)> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        wire::write_message(&mut stream, &ClientMessage::Hello { version: PROTOCOL_VERSION })?;

        let welcome = match wire::read_message::<ServerMessage>(&mut stream)? {
            ServerMessage::Welcome(welcome) => welcome,
            ServerMessage::Rejected { reason } => {
                return Err(io::Error::new(ErrorKind::ConnectionRefused, reason));
            }
            ServerMessage::Tick(_) => {
                return Err(io::Error::new(ErrorKind::InvalidData, "expected a welcome"));
            }
        };

        let mut reader = stream.try_clone()?;
        let (to_client, incoming) = mpsc::channel();
        thread::spawn(move || loop {
            match wire::read_message::<ServerMessage>(&mut reader) {
                Ok(ServerMessage::Tick(tick)) => {
                    if to_client.send(Ok(tick)).is_err() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    let _ = to_client.send(Err(e));
                    return;
                }
            }
        });
        Ok((Client { stream, incoming }, welcome))
    }

    pub fn send(&mut self, command: Command) -> io::Result<()> {
        wire::write_message(&mut self.stream, &ClientMessage::Command(command))
    }

    /// Everything that has arrived so far, without waiting. An error means the connection is
    /// gone.
    pub fn poll(&mut self) -> io::Result<Vec<Tick>> {
        let mut out = Vec::new();
        loop {
            match self.incoming.try_recv() {
                Ok(tick) => out.push(tick?),
                Err(TryRecvError::Empty) => return Ok(out),
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::new(ErrorKind::ConnectionAborted, "disconnected"));
                }
            }
        }
    }

    /// Waits up to `timeout` for the next tick.
    pub fn wait_tick(&mut self, timeout: Duration) -> io::Result<Option<Tick>> {
        match self.incoming.recv_timeout(timeout) {
            Ok(tick) => tick.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(io::Error::new(ErrorKind::ConnectionAborted, "disconnected"))
            }
        }
    }
}

/// The host's game as far as this player knows it: the world, rebuilt from the seed and kept
/// current from the changes the host sends, and the latest snapshot.
pub struct Mirror {
    pub you: PlayerId,
    pub world: World,
    pub latest: Option<Snapshot>,
    pub last_tick: u64,
}

impl Mirror {
    /// Builds the world the host described.
    pub fn new(welcome: &Welcome) -> Mirror {
        let mut world = World::generate(welcome.seed);
        for change in &welcome.blocks {
            world.apply_change(*change);
        }
        Mirror { you: welcome.you, world, latest: None, last_tick: 0 }
    }

    /// Takes in one update from the host.
    pub fn apply(&mut self, tick: &Tick) {
        for change in &tick.blocks {
            self.world.apply_change(*change);
        }
        self.latest = Some(tick.snapshot.clone());
        self.last_tick = tick.number;
    }
}
