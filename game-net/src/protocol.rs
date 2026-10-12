//! The messages.

use game_core::game::{Command, Event, PlayerId, Snapshot};
use game_core::world::BlockChange;
use serde::{Deserialize, Serialize};

/// Bumped whenever the messages change, so old and new programs don't misread each other.
pub const PROTOCOL_VERSION: u32 = 2;

/// How often the host runs the game and sends an update, in seconds.
pub const TICK_SECONDS: f32 = 1.0 / 30.0;

/// The most players one host takes.
pub const MAX_PLAYERS: usize = 16;

/// Longest name and longest chat line, in characters.
pub const MAX_NAME: usize = 16;
pub const MAX_CHAT: usize = 120;

/// A name as the host keeps it: no control characters or surrounding spaces, shortened. Empty
/// becomes "PLAYER".
pub fn tidy_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    let name: String = name.trim().chars().take(MAX_NAME).collect();
    let name = name.trim().to_string();
    if name.is_empty() { "PLAYER".to_string() } else { name }
}

/// A chat line as the host sends it on, or None if there is nothing to say.
pub fn tidy_chat(text: &str) -> Option<String> {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let text: String = text.trim().chars().take(MAX_CHAT).collect();
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Player to host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Always the first message. `name` is what the player is called; coming back with a name
    /// that was used before gets that player's things back.
    Hello { version: u32, name: String },
    Command(Command),
    /// Something to say to everyone.
    Chat(String),
}

/// Host to player.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// The answer to a good `Hello`: who you are and what the world looks like right now.
    Welcome(Welcome),
    /// Sent instead of a welcome, just before the host hangs up.
    Rejected { reason: String },
    Tick(Tick),
    /// Who is playing. Sent to everyone whenever someone comes or goes.
    Roster(Vec<(PlayerId, String)>),
    /// A line for the chat. `from` is empty for announcements ("Ann joined").
    Chat { from: String, text: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Welcome {
    pub you: PlayerId,
    /// The name the host knows you by (it can be tidied up from the one you asked for).
    pub name: String,
    /// You had played before under this name, so you have your things back.
    pub rejoined: bool,
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
