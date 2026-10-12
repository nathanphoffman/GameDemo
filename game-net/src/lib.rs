//! Talking between a host and its players: the messages, how they are put on the wire, the
//! host's server loop and the player's side of the connection.
//!
//! The host runs the only real `Game`. Players send `Command`s; the host sends back a `Tick`
//! 30 times a second with what each player's screen needs.

mod client;
mod protocol;
mod save;
mod server;
mod wire;

pub use client::{Client, Mirror, Update};
pub use protocol::{
    tidy_chat, tidy_name, ClientMessage, ServerMessage, Tick, Welcome, MAX_CHAT, MAX_NAME,
    MAX_PLAYERS, PROTOCOL_VERSION, TICK_SECONDS,
};
pub use save::{read_save, write_save};
pub use server::Server;
