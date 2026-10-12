//! The host's side: accept players, run the game on a fixed tick, send each player their view.

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use game_core::game::{Command, Game, PlayerId};

use crate::protocol::{
    tidy_chat, tidy_name, ClientMessage, ServerMessage, Tick, Welcome, MAX_PLAYERS,
    PROTOCOL_VERSION, TICK_SECONDS,
};
use crate::wire;

/// How many updates can be waiting to go out to one player before they are dropped as too slow.
const OUTBOX: usize = 64;
/// How long a new connection has to say hello.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// Something the connection threads tell the game loop.
enum Incoming {
    Joined { conn: u64, name: String, outbox: SyncSender<Vec<u8>> },
    Command { conn: u64, command: Command },
    Chat { conn: u64, text: String },
    Left { conn: u64 },
}

struct Connected {
    player: PlayerId,
    name: String,
    outbox: SyncSender<Vec<u8>>,
}

pub struct Server {
    listener: TcpListener,
    seed: u32,
    stop: Arc<AtomicBool>,
    quiet: bool,
}

impl Server {
    /// Starts listening (use port 0 to have one picked, then ask `local_addr`).
    pub fn bind(addr: impl ToSocketAddrs, seed: u32) -> io::Result<Server> {
        Ok(Server {
            listener: TcpListener::bind(addr)?,
            seed,
            stop: Arc::new(AtomicBool::new(false)),
            quiet: false,
        })
    }

    /// Don't print who joins and leaves.
    pub fn quiet(mut self) -> Server {
        self.quiet = true;
        self
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Setting this to true makes `run` return.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    /// Runs the game until stopped.
    pub fn run(self) -> io::Result<()> {
        let (to_loop, incoming) = mpsc::channel::<Incoming>();
        let listener = self.listener.try_clone()?;
        let stop = self.stop.clone();
        thread::spawn(move || accept_loop(listener, to_loop, stop));

        let quiet = self.quiet;
        let mut game = Game::new(self.seed);
        let mut clients: HashMap<u64, Connected> = HashMap::new();
        let mut tick_number: u64 = 0;
        // (Nobody needs to know about chat lines that could not be delivered.)
        let mut game_chat_dropped = 0u64;
        let tick = Duration::from_secs_f32(TICK_SECONDS);
        let mut next = Instant::now() + tick;

        while !self.stop.load(Ordering::Relaxed) {
            // Who came, what they asked for, who left.
            while let Ok(message) = incoming.try_recv() {
                match message {
                    Incoming::Joined { conn, name, outbox } => {
                        let name = tidy_name(&name);
                        let refusal = if clients.len() >= MAX_PLAYERS {
                            Some("the host is full".to_string())
                        } else if game.name_in_use(&name) {
                            Some(format!("someone called {name} is already playing"))
                        } else {
                            None
                        };
                        if let Some(reason) = refusal {
                            if let Ok(bytes) = wire::encode(&ServerMessage::Rejected { reason }) {
                                let _ = outbox.try_send(bytes);
                            }
                            continue;
                        }
                        let (player, rejoined) = game.join(&name);
                        let welcome = ServerMessage::Welcome(Welcome {
                            you: player,
                            name: name.clone(),
                            rejoined,
                            seed: game.seed(),
                            blocks: game.world.all_edits(),
                        });
                        let sent = wire::encode(&welcome)
                            .is_ok_and(|bytes| outbox.try_send(bytes).is_ok());
                        if !sent {
                            game.remove_player(player);
                            continue;
                        }
                        clients.insert(conn, Connected { player, name: name.clone(), outbox });
                        if !quiet {
                            println!(
                                "{name} {} ({} here)",
                                if rejoined { "came back" } else { "joined" },
                                clients.len()
                            );
                        }
                        let what = if rejoined { "came back" } else { "joined" };
                        say(&clients, &mut game_chat_dropped, "", format!("{name} {what}"));
                        tell_roster(&clients);
                    }
                    Incoming::Command { conn, command } => {
                        if let Some(client) = clients.get(&conn) {
                            game.apply(client.player, command);
                        }
                    }
                    Incoming::Chat { conn, text } => {
                        if let (Some(client), Some(text)) = (clients.get(&conn), tidy_chat(&text)) {
                            let from = client.name.clone();
                            say(&clients, &mut game_chat_dropped, &from, text);
                        }
                    }
                    Incoming::Left { conn } => {
                        if let Some(client) = clients.remove(&conn) {
                            game.remove_player(client.player);
                            if !quiet {
                                println!("{} left ({} here)", client.name, clients.len());
                            }
                            say(&clients, &mut game_chat_dropped, "", format!("{} left", client.name));
                            tell_roster(&clients);
                        }
                    }
                }
            }

            game.update(TICK_SECONDS);
            tick_number += 1;

            let blocks = game.world.take_changes();
            let events = game.take_events();
            let mut too_slow = Vec::new();
            for (&conn, client) in &clients {
                let Some(snapshot) = game.snapshot_for(client.player) else {
                    continue;
                };
                let message = ServerMessage::Tick(Tick {
                    number: tick_number,
                    snapshot,
                    blocks: blocks.clone(),
                    events: events.clone(),
                });
                let Ok(bytes) = wire::encode(&message) else {
                    continue;
                };
                if client.outbox.try_send(bytes).is_err() {
                    too_slow.push(conn);
                }
            }
            for conn in too_slow {
                if let Some(client) = clients.remove(&conn) {
                    game.remove_player(client.player);
                    if !quiet {
                        println!("{} dropped (not keeping up)", client.name);
                    }
                    say(&clients, &mut game_chat_dropped, "", format!("{} dropped", client.name));
                    tell_roster(&clients);
                }
            }

            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
                next += tick;
            } else {
                // Running behind: don't try to catch up with a burst of ticks.
                next = now + tick;
            }
        }
        Ok(())
    }
}

fn accept_loop(listener: TcpListener, to_loop: Sender<Incoming>, stop: Arc<AtomicBool>) {
    let mut next_conn = 0u64;
    // Wake up now and then to notice `stop`.
    let _ = listener.set_nonblocking(true);
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let conn = next_conn;
                next_conn += 1;
                let to_loop = to_loop.clone();
                thread::spawn(move || serve(stream, conn, to_loop));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// One player's connection: read their messages until they go away.
fn serve(mut stream: TcpStream, conn: u64, to_loop: Sender<Incoming>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(HELLO_TIMEOUT));

    // The first message has to be a hello with the right version.
    let hello: io::Result<ClientMessage> = wire::read_message(&mut stream);
    let Ok(ClientMessage::Hello { version, name }) = hello else {
        return;
    };
    let (outbox, queue) = mpsc::sync_channel::<Vec<u8>>(OUTBOX);
    if version != PROTOCOL_VERSION {
        let reason = format!("host speaks protocol {PROTOCOL_VERSION}, you speak {version}");
        if let Ok(bytes) = wire::encode(&ServerMessage::Rejected { reason }) {
            let _ = stream.write_all(&bytes);
        }
        return;
    }
    let _ = stream.set_read_timeout(None);

    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    thread::spawn(move || write_loop(&mut writer, queue));

    if to_loop.send(Incoming::Joined { conn, name, outbox }).is_err() {
        return;
    }
    loop {
        match wire::read_message::<ClientMessage>(&mut stream) {
            Ok(ClientMessage::Command(command)) => {
                if to_loop.send(Incoming::Command { conn, command }).is_err() {
                    return;
                }
            }
            Ok(ClientMessage::Chat(text)) => {
                if to_loop.send(Incoming::Chat { conn, text }).is_err() {
                    return;
                }
            }
            // Hello again, or garbage or a closed connection: either way we are done.
            _ => break,
        }
    }
    let _ = to_loop.send(Incoming::Left { conn });
}

/// Sends what the game loop queues up. Ends when the queue is dropped (the player was removed)
/// or the connection breaks, which also ends the reader.
fn write_loop(stream: &mut TcpStream, queue: Receiver<Vec<u8>>) {
    while let Ok(bytes) = queue.recv() {
        if stream.write_all(&bytes).is_err() {
            break;
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// Sends a chat line (`from` empty for an announcement) to everyone.
fn say(clients: &HashMap<u64, Connected>, dropped: &mut u64, from: &str, text: String) {
    let message = ServerMessage::Chat { from: from.to_string(), text };
    let Ok(bytes) = wire::encode(&message) else {
        return;
    };
    for client in clients.values() {
        if client.outbox.try_send(bytes.clone()).is_err() {
            *dropped += 1;
        }
    }
}

/// Tells everyone who is playing.
fn tell_roster(clients: &HashMap<u64, Connected>) {
    let roster: Vec<(PlayerId, String)> =
        clients.values().map(|c| (c.player, c.name.clone())).collect();
    let Ok(bytes) = wire::encode(&ServerMessage::Roster(roster)) else {
        return;
    };
    for client in clients.values() {
        let _ = client.outbox.try_send(bytes.clone());
    }
}
