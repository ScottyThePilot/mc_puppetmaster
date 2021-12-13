extern crate chrono;
extern crate dunce;
extern crate puppet;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
extern crate time;
extern crate tokio;
extern crate toml;

mod config;
mod util;

use chrono::prelude::*;
use chrono::Duration;
use console::{Term, style};
use puppet::{Puppet, NoHandler};
use tokio::runtime::Builder;
use tokio::time::Instant;

use crate::config::Config;
use crate::util::AtomicFlag;

use std::path::PathBuf;
use std::io::{self, Write};
use std::process;

fn main() {
  let result = Builder::new_multi_thread()
    .enable_all().build().unwrap()
    .block_on(run());
  if let Err(err) = result {
    let err = style(err).red().bright();
    let mut term = Term::stdout();
    writeln!(term, "{}", err).unwrap();
    io::stdin()
      .read_line(&mut String::new())
      .unwrap();
    process::exit(1);
  } else {
    io::stdin()
      .read_line(&mut String::new())
      .unwrap();
    process::exit(0);
  };
}

#[inline]
async fn run() -> Result<(), Error> {
  let config = Config::load("puppetmaster.toml").await?;
  let parent = dunce::canonicalize(&config.jar_path)
    .map_err(Error::InvalidJarPathCanonicalize)?
    .parent().ok_or(Error::InvalidJarPath)?
    .to_owned();
  std::env::set_current_dir(parent)?;

  loop {
    let inst_now = Instant::now();
    let now = Utc::now();
    let remaining = config.next_restart(now) - now;
    let remaining_f = format!("{} hours, {} minutes", remaining.num_hours(), remaining.num_minutes());
    println!("[Puppetmaster] Starting server");
    println!("[Puppetmaster] Server scheduled to restart in {}", remaining_f);

    let restart = AtomicFlag::new();
    let puppet = Puppet::builder()
      .jar_path(&config.jar_path)
      .max_memory(&config.max_memory)
      .min_memory(&config.min_memory)
      .finish()?;
    tokio::select!{
      result = wait_and_restart(&puppet, &restart, inst_now, remaining) => match result {
        Err(err) => return Err(err),
        Ok(()) => continue
      },
      result = puppet.start(NoHandler) => match result {
        Err(err) => return Err(err.into()),
        Ok(()) => match restart.get() {
          true => continue,
          false => break
        }
      },
    };
  }

  println!("[Puppetmaster] Server has terminated");

  Ok(())
}

#[derive(Debug)]
enum Warning {
  Remaining(u32),
  RestartingNow
}

async fn wait_and_restart(puppet: &Puppet, restart: &AtomicFlag, now: Instant, remaining: Duration) -> Result<(), Error> {
  let warnings = [
    (Warning::Remaining(30), time_remaining_minus(now, remaining, 30)),
    (Warning::Remaining(10), time_remaining_minus(now, remaining, 10)),
    (Warning::Remaining(5), time_remaining_minus(now, remaining, 5)),
    (Warning::Remaining(1), time_remaining_minus(now, remaining, 1)),
    (Warning::RestartingNow, Some(time_remaining(now, remaining)))
  ];

  for (warning, instant) in warnings {
    if let Some(instant) = instant {
      tokio::time::sleep_until(instant).await;

      match warning {
        Warning::Remaining(mins) => {
          puppet.command(format!("say {} minutes until server restart", mins)).await?;
        },
        Warning::RestartingNow => {
          restart.set();
          puppet.command("stop").await?;
          puppet.wait().await?;
        }
      };
    };
  };

  Ok(())
}

// Returns `None` when the resulting instant would be prior in time than `t`
fn time_remaining_minus(t: Instant, remaining: Duration, a: u32) -> Option<Instant> {
  (remaining - Duration::minutes(a as i64)).to_std().ok().map(|d| t + d)
}

fn time_remaining(t: Instant, remaining: Duration) -> Instant {
  t + remaining.to_std().unwrap()
}

#[derive(Debug, Error)]
pub enum Error {
  #[error("Error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Config Error: {0}")]
  SerializeToml(#[from] toml::ser::Error),
  #[error("Config Error: {0}")]
  DeserializeToml(#[from] toml::de::Error),
  #[error("Config Error: {} was not present, a default one has been created", .0.display())]
  NotConfigured(PathBuf),
  #[error("Error: Background task failed")]
  BackgroundTaskFailed,
  #[error("Error: Invalid jarfile path: {0}")]
  InvalidJarPathCanonicalize(std::io::Error),
  #[error("Error: Invalid jarfile path")]
  InvalidJarPath,
}
