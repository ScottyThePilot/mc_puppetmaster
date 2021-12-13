use lazy_static::lazy_static;
use regex::Regex;
use vte::{Parser, Perform};

use std::str::FromStr;

// Line parsing may be really slow because of my crude death message parsing.

/// A utility enum for easily matching against common or important console lines.
/// An instance of `ConsoleLine` can be obtained with `str::parse` or `ConsoleLine::parse_from`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ConsoleLine {
  /// Server finished starting up.
  DoneLoading { time: f64 },
  /// Server is starting up.
  /// Warning: this line is not guaranteed to be emitted.
  StartingServer { version: String },
  /// Server is shutting down.
  /// Warning: this line is not guaranteed to be emitted.
  StoppingServer,
  /// Server is complaining about being overloaded.
  Overloaded { ticks_behind: u32, ms_behind: u32 },
  /// A player has 'moved wrongly' or 'moved too quickly'.
  PlayerMovedWrongly { username: String },
  /// A player has died.
  PlayerDied { death_message: String },
  /// A player has sent a message in chat.
  ChatMessage { username: String, message: String },
  /// A player has joined the server.
  PlayerJoined { username: String },
  /// A player has left the server.
  PlayerLeft { username: String }
}

impl ConsoleLine {
  /// Parse an instance of `ConsoleLine` from a str.
  pub fn parse_from(line: impl AsRef<str>) -> Option<Self> {
    let line = strip_ansi_escapes(line.as_ref());
    if let Some(time) = match_done_loading(&line) {
      Some(ConsoleLine::DoneLoading { time })
    } else if let Some(version) = match_starting_server(&line) {
      Some(ConsoleLine::StartingServer { version })
    } else if match_stopping_server(&line) {
      Some(ConsoleLine::StoppingServer)
    } else if let Some((ticks_behind, ms_behind)) = match_overloaded(&line) {
      Some(ConsoleLine::Overloaded { ticks_behind, ms_behind })
    } else if let Some(username) = match_player_moved_wrongly(&line) {
      Some(ConsoleLine::PlayerMovedWrongly { username })
    } else if let Some(death_message) = match_player_died(&line) {
      Some(ConsoleLine::PlayerDied { death_message })
    } else if let Some((username, message)) = match_chat_message(&line) {
      Some(ConsoleLine::ChatMessage { username, message })
    } else if let Some(username) = match_player_joined(&line) {
      Some(ConsoleLine::PlayerJoined { username })
    } else if let Some(username) = match_player_left(&line) {
      Some(ConsoleLine::PlayerLeft { username })
    } else {
      None
    }
  }
}

impl FromStr for ConsoleLine {
  type Err = ();

  fn from_str(line: &str) -> Result<Self, ()> {
    ConsoleLine::parse_from(line).ok_or(())
  }
}

macro_rules! regex {
  ($($arg:tt)*) => ({
    let rx = format!($($arg)*);
    if cfg!(debug_assertions) { println!("{}", rx) };
    Regex::new(rx.as_str()).unwrap()
  });
}

// This list is up-to-date as of 1.17.1.
// It is probably missing death messages from 1.18+ and might be missing messages from older versions.
const DEATH_MESSAGES: &[&str] = &[
  "{1} fell off a ladder",
  "{1} fell off some vines",
  "{1} fell off some weeping vines",
  "{1} fell off some twisting vines",
  "{1} fell off scaffolding",
  "{1} fell while climbing",
  "{1} fell from a high place",
  "{1} was doomed to fall",
  "{1} was doomed to fall by {2}",
  "{1} was doomed to fall by {2} using {item}",
  "{1} fell too far and was finished by {2}",
  "{1} fell too far and was finished by {2} using {item}",
  "{1} was struck by lightning",
  "{1} was struck by lightning whilst fighting {2}",
  "{1} went up in flames",
  "{1} walked into fire whilst fighting {2}",
  "{1} burned to death",
  "{1} was burnt to a crisp whilst fighting {2}",
  "{1} tried to swim in lava",
  "{1} tried to swim in lava to escape {2}",
  "{1} discovered the floor was lava",
  "{1} walked into danger zone due to {2}",
  "{1} suffocated in a wall",
  "{1} suffocated in a wall whilst fighting {2}",
  "{1} was squished too much",
  "{1} was squashed by {2}",
  "{1} drowned",
  "{1} drowned whilst trying to escape {2}",
  "{1} died from dehydration",
  "{1} died from dehydration whilst trying to escape {2}",
  "{1} starved to death",
  "{1} starved to death whilst fighting {2}",
  "{1} was pricked to death",
  "{1} walked into a cactus whilst trying to escape {2}",
  "{1} died",
  "{1} died because of {2}",
  "{1} blew up",
  "{1} was blown up by {2}",
  "{1} was blown up by {2} using {item}",
  "{1} was killed by magic",
  "{1} was killed by magic whilst trying to escape {2}",
  "{1} was killed by even more magic",
  "{1} withered away",
  "{1} withered away whilst fighting {2}",
  "{1} was shot by a skull from {2}",
  "{1} was squashed by a falling anvil",
  "{1} was squashed by a falling anvil whilst fighting {2}",
  "{1} was squashed by a falling block",
  "{1} was squashed by a falling block whilst fighting {2}",
  "{1} was impaled on a stalagmite",
  "{1} was impaled on a stalagmite whilst fighting {2}",
  "{1} was skewered by a falling stalactite",
  "{1} was skewered by a falling stalactite whilst fighting {2}",
  "{1} was slain by {2}",
  "{1} was slain by {2} using {item}",
  "{1} was slain by {2}",
  "{1} was slain by {2} using {item}",
  "{1} was shot by {2}",
  "{1} was shot by {2} using {item}",
  "{1} was fireballed by {2}",
  "{1} was fireballed by {2} using {item}",
  "{1} was pummeled by {2}",
  "{1} was pummeled by {2} using {item}",
  "{1} was killed by {2} using magic",
  "{1} was killed by {2} using {item}",
  "{1} was killed trying to hurt {2}",
  "{1} was killed by {item} trying to hurt {2}",
  "{1} was impaled by {2}",
  "{1} was impaled by {2} with {item}",
  "{1} hit the ground too hard",
  "{1} hit the ground too hard whilst trying to escape {2}",
  "{1} fell out of the world",
  "{1} didn't want to live in the same world as {2}",
  "{1} was roasted in dragon breath",
  "{1} was roasted in dragon breath by {2}",
  "{1} experienced kinetic energy",
  "{1} experienced kinetic energy whilst trying to escape {2}",
  "{1} went off with a bang",
  "{1} went off with a bang whilst fighting {2}",
  "{1} went off with a bang due to a firework fired from {item} by {2}",
  "{1} was killed by Intentional Game Design",
  "{1} was poked to death by a sweet berry bush",
  "{1} was poked to death by a sweet berry bush whilst trying to escape {2}",
  "{1} was stung to death",
  "{1} was stung to death by {2}",
  "{1} froze to death",
  "{1} was frozen to death by {2}"
];

const MATCH_USERNAME: &str = r"[\w\d]{3,16}";
const MATCH_INFO_LOG_CHAT: &str = r"^\[(?:[\d\w]{9} [\d:.]{12}|[\d:]{8})\] \[(?:Server thread/INFO|Async Chat Thread - #\d+/INFO)\](?: \[[\w.]+/?\])?:";
const MATCH_INFO_LOG: &str = r"^\[(?:[\d\w]{9} [\d:.]{12}|[\d:]{8})\] \[Server thread/INFO\](?: \[[\w.]+/?\])?:";
const MATCH_WARN_LOG: &str = r"^\[(?:[\d\w]{9} [\d:.]{12}|[\d:]{8})\] \[Server thread/WARN\](?: \[[\w.]+/?\])?:";

lazy_static!{
  static ref RX_DONE_LOADING: Regex = regex!(r#"{} Done \((\d+\.\d+)s\)! For help, type "help""#, MATCH_INFO_LOG);
  static ref RX_STARTING_SERVER: Regex = regex!(r"{} Starting minecraft server version (.+)", MATCH_INFO_LOG);
  static ref RX_STOPPING_SERVER: Regex = regex!(r"{} Stopping server", MATCH_INFO_LOG);
  static ref RX_OVERLOADED: Regex = regex!(r"{} Can't keep up! Is the server overloaded\? Running (\d+)ms or (\d+) ticks behind", MATCH_WARN_LOG);
  static ref RX_PLAYER_MOVED_WRONGLY: Regex = regex!(r"{} (?:({u})|.+ \(vehicle of ({u})\)) moved (?:too quickly|wrongly)!.*", u = MATCH_USERNAME);
  static ref RX_PLAYER_DIED: Regex = regex!(r"{} ({})", MATCH_INFO_LOG, match_death_messages());
  static ref RX_CHAT_MESSAGE: Regex = regex!(r"{} <({})> (.+)", MATCH_INFO_LOG_CHAT, MATCH_USERNAME);
  static ref RX_PLAYER_JOINED: Regex = regex!(r"{} ({}) joined the game", MATCH_INFO_LOG, MATCH_USERNAME);
  static ref RX_PLAYER_LEFT: Regex = regex!(r"{} ({}) left the game", MATCH_INFO_LOG, MATCH_USERNAME);
}

fn match_death_messages() -> String {
  let joined = DEATH_MESSAGES.join("|")
    .replace("{1}", MATCH_USERNAME)
    .replace("{2}", ".+")
    .replace("{item}", ".+");
  format!("(?:{})", joined)
}

pub fn load_all() {
  let _ = &[
    &*RX_DONE_LOADING,
    &*RX_STARTING_SERVER,
    &*RX_STOPPING_SERVER,
    &*RX_OVERLOADED,
    &*RX_PLAYER_MOVED_WRONGLY,
    &*RX_PLAYER_DIED,
    &*RX_CHAT_MESSAGE,
    &*RX_PLAYER_JOINED,
    &*RX_PLAYER_LEFT
  ];
}

/*lazy_static!{
  static ref RX_DONE_LOADING: Regex = regex!(ST_INFO, r#"Done \((\d+\.\d+)s\)! For help, type "help""#);
  static ref RX_OVERLOADED: Regex = regex!(ST_WARN, r"Can't keep up! Is the server overloaded\? Running (\d+)ms or (\d+) ticks behind");
  static ref RX_CHAT_MESSAGE: Regex = regex!(ST_INFO, r"<([\w\d]{3,16})> (.+)");
  static ref RX_PLAYER_JOINED: Regex = regex!(ST_INFO, r"([\w\d]{3,16}) joined the game");
  static ref RX_PLAYER_LEFT: Regex = regex!(ST_INFO, r"([\w\d]{3,16}) left the game");
}*/

fn match_done_loading(line: &str) -> Option<f64> {
  let captures = RX_DONE_LOADING.captures(line)?;
  let time = captures.get(1).unwrap().as_str();
  let time = time.parse::<f64>().ok()?;
  Some(time)
}

fn match_starting_server(line: &str) -> Option<String> {
  let captures = RX_STARTING_SERVER.captures(line)?;
  let version = captures.get(1).unwrap().as_str();
  Some(version.to_owned())
}

fn match_stopping_server(line: &str) -> bool {
  RX_STOPPING_SERVER.is_match(line)
}

fn match_overloaded(line: &str) -> Option<(u32, u32)> {
  let captures = RX_OVERLOADED.captures(line)?;
  let ticks_behind = captures.get(1).unwrap().as_str();
  let ticks_behind = ticks_behind.parse::<u32>().ok()?;
  let ms_behind = captures.get(2).unwrap().as_str();
  let ms_behind = ms_behind.parse::<u32>().ok()?;
  Some((ticks_behind, ms_behind))
}

fn match_player_moved_wrongly(line: &str) -> Option<String> {
  let captures = RX_PLAYER_MOVED_WRONGLY.captures(line)?;
  let username = Option::or(captures.get(1), captures.get(2)).unwrap().as_str();
  Some(username.to_owned())
}

fn match_player_died(line: &str) -> Option<String> {
  let captures = RX_PLAYER_DIED.captures(line)?;
  let death_message = captures.get(1).unwrap().as_str();
  Some(death_message.to_owned())
}

fn match_chat_message(line: &str) -> Option<(String, String)> {
  let captures = RX_CHAT_MESSAGE.captures(line)?;
  let username = captures.get(1).unwrap().as_str();
  let message = captures.get(2).unwrap().as_str();
  Some((username.to_owned(), message.to_owned()))
}

fn match_player_joined(line: &str) -> Option<String> {
  let captures = RX_PLAYER_JOINED.captures(line)?;
  let username = captures.get(1).unwrap().as_str();
  Some(username.to_owned())
}

fn match_player_left(line: &str) -> Option<String> {
  let captures = RX_PLAYER_LEFT.captures(line)?;
  let username = captures.get(1).unwrap().as_str();
  Some(username.to_owned())
}



fn strip_ansi_escapes(buf: &str) -> String {
  let mut performer = Performer { buf: String::new() };
  let mut parser = Parser::new();
  for &b in buf.as_bytes().iter() {
    parser.advance(&mut performer, b);
  };

  performer.buf
}

#[repr(transparent)]
struct Performer {
  buf: String
}

impl Perform for Performer {
  fn print(&mut self, c: char) {
    self.buf.push(c);
  }

  fn execute(&mut self, byte: u8) {
    if byte == b'\n' {
      self.buf.push('\n');
    };
  }
}
