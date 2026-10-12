//! The messages.

use game_core::game::{Command, Event, PlayerId, Snapshot};
use game_core::world::BlockChange;
use serde::{Deserialize, Serialize};

/// Bumped whenever the messages change, so old and new programs don't misread each other.
pub const PROTOCOL_VERSION: u32 = 1;

/// How often the host runs the game and sends an update, in seconds.
pub const TICK_SECONDS: f32 = 1.0 / 30.0;

/// The most players one host takes.
pub const MAX_PLAYERS: usize = 16;

/// Player to host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Always the first message.
    Hello { version: u32 },
    Command(Command),
}

/// Host to player.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// The answer to a good `Hello`: who you are and what the world looks like right now.
    Welcome(Welcome),
    /// Sent instead of a welcome, just before the host hangs up.
    Rejected { reason: String },
    Tick(Tick),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Welcome {
    pub you: PlayerId,
    /// Generate the world from this...
    pub seed: u32,
    /// ...then apply these to get the world as it is now.
    pub blocks: Vec<BlockChange>,
}

/// What happened in one step of the game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tick {
    pub number: u64,
    pub snapshot: Snapshot,
    /// Blocks that changed during this step.
    pub blocks: Vec<BlockChange>,
    pub events: Vec<Event>,
}
