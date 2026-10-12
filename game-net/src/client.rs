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

/// Something the host sent.
#[derive(Clone, Debug)]
pub enum Update {
    Tick(Tick),
    /// Who is playing now.
    Roster(Vec<(PlayerId, String)>),
    /// A chat line; `from` is empty for announcements.
    Chat { from: String, text: String },
}

pub struct Client {
    stream: TcpStream,
    incoming: Receiver<io::Result<Update>>,
    /// Things that arrived while waiting for a tick.
    pending: Vec<Update>,
}

impl Client {
    /// Connects and says hello as `name`. Returns once the host has welcomed us.
    pub fn connect(addr: impl ToSocketAddrs, name: &str) -> io::Result<(Client, Welcome)> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let hello = ClientMessage::Hello { version: PROTOCOL_VERSION, name: name.to_string() };
        wire::write_message(&mut stream, &hello)?;

        let welcome = match wire::read_message::<ServerMessage>(&mut stream)? {
            ServerMessage::Welcome(welcome) => welcome,
            ServerMessage::Rejected { reason } => {
                return Err(io::Error::new(ErrorKind::ConnectionRefused, reason));
            }
            _ => {
                return Err(io::Error::new(ErrorKind::InvalidData, "expected a welcome"));
            }
        };

        let mut reader = stream.try_clone()?;
        let (to_client, incoming) = mpsc::channel();
        thread::spawn(move || loop {
            match wire::read_message::<ServerMessage>(&mut reader) {
                Ok(message) => {
                    let update = match message {
                        ServerMessage::Tick(tick) => Update::Tick(tick),
                        ServerMessage::Roster(roster) => Update::Roster(roster),
                        ServerMessage::Chat { from, text } => Update::Chat { from, text },
                        _ => continue,
                    };
                    if to_client.send(Ok(update)).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = to_client.send(Err(e));
                    return;
                }
            }
        });
        Ok((Client { stream, incoming, pending: Vec::new() }, welcome))
    }

    pub fn send(&mut self, command: Command) -> io::Result<()> {
        wire::write_message(&mut self.stream, &ClientMessage::Command(command))
    }

    /// Says something to everyone.
    pub fn say(&mut self, text: &str) -> io::Result<()> {
        wire::write_message(&mut self.stream, &ClientMessage::Chat(text.to_string()))
    }

    /// Everything that has arrived so far, without waiting. An error means the connection is
    /// gone.
    pub fn poll(&mut self) -> io::Result<Vec<Update>> {
        let mut out = std::mem::take(&mut self.pending);
        loop {
            match self.incoming.try_recv() {
                Ok(update) => out.push(update?),
                Err(TryRecvError::Empty) => return Ok(out),
                Err(TryRecvError::Disconnected) => {
                    return Err(io::Error::new(ErrorKind::ConnectionAborted, "disconnected"));
                }
            }
        }
    }

    /// Waits up to `timeout` for the next tick. Anything else that comes first is kept for the
    /// next `poll`.
    pub fn wait_tick(&mut self, timeout: Duration) -> io::Result<Option<Tick>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.incoming.recv_timeout(left) {
                Ok(Ok(Update::Tick(tick))) => return Ok(Some(tick)),
                Ok(Ok(other)) => self.pending.push(other),
                Ok(Err(e)) => return Err(e),
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(ErrorKind::ConnectionAborted, "disconnected"));
                }
            }
        }
    }
}

/// The host's game as far as this player knows it: the world, rebuilt from the seed and kept
/// current from the changes the host sends, and the latest snapshot.
pub struct Mirror {
    pub you: PlayerId,
    /// What the host calls you.
    pub name: String,
    /// Who is playing, by name.
    pub roster: Vec<(PlayerId, String)>,
    pub world: World,
    pub latest: Option<Snapshot>,
    /// The update before `latest`, for smoothing movement between updates.
    pub previous: Option<Snapshot>,
    pub last_tick: u64,
}

impl Mirror {
    /// Builds the world the host described.
    pub fn new(welcome: &Welcome) -> Mirror {
        Mirror::with_world(welcome, World::generate(welcome.seed))
    }

    /// Same, for a world already generated from the host's seed (generating takes a while, so
    /// it can be done ahead of time).
    pub fn with_world(welcome: &Welcome, mut world: World) -> Mirror {
        for change in &welcome.blocks {
            world.apply_change(*change);
        }
        Mirror {
            you: welcome.you,
            name: welcome.name.clone(),
            roster: Vec::new(),
            world,
            latest: None,
            previous: None,
            last_tick: 0,
        }
    }

    /// Takes in one update from the host.
    pub fn apply(&mut self, tick: &Tick) {
        for change in &tick.blocks {
            self.world.apply_change(*change);
        }
        self.previous = self.latest.take();
        self.latest = Some(tick.snapshot.clone());
        self.last_tick = tick.number;
    }

    /// Takes in anything from the host. (Chat lines are for whoever shows them.)
    pub fn apply_update(&mut self, update: &Update) {
        match update {
            Update::Tick(tick) => self.apply(tick),
            Update::Roster(roster) => self.roster = roster.clone(),
            Update::Chat { .. } => {}
        }
    }

    /// The name of a player, if they are playing.
    pub fn name_of(&self, id: PlayerId) -> Option<&str> {
        self.roster.iter().find(|(p, _)| *p == id).map(|(_, n)| n.as_str())
    }
}
