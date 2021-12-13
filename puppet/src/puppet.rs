use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Command, Child, ChildStdout, ChildStdin};
use tokio::sync::Mutex;

use std::process::{Stdio, ExitStatus};
use std::path::{Path, PathBuf};
use std::io;

/// A struct for configuring and instantiating a Puppet.
#[derive(Debug, Clone)]
pub struct PuppetBuilder {
  jar_path: Option<PathBuf>,
  max_memory: Option<String>,
  min_memory: Option<String>
}

impl PuppetBuilder {
  /// Create a new, default `PuppetBuilder`.
  pub fn new() -> PuppetBuilder {
    PuppetBuilder::default()
  }

  /// Set the path to the server `.jar` file.
  /// NOTE: The server will use the current working directory as its own working directory,
  /// which may cause unintended behavior if set to a file outside of the current directory.
  pub fn jar_path(mut self, jar_path: impl AsRef<Path>) -> Self {
    self.jar_path = Some(jar_path.as_ref().to_owned());
    self
  }

  /// Set the maximum memory (`-Xmx`) for the server.
  /// Examples of valid values are `2G`, `1024K`.
  pub fn max_memory(mut self, max_memory: impl AsRef<str>) -> Self {
    self.max_memory = Some(max_memory.as_ref().to_owned());
    self
  }

  /// Set the minimum memory (`-Xms`) for the server.
  /// Examples of valid values are `2G`, `1024K`.
  pub fn min_memory(mut self, min_memory: impl AsRef<str>) -> Self {
    self.min_memory = Some(min_memory.as_ref().to_owned());
    self
  }

  /// Launch the server and return a handle (`Puppet`) for it.
  pub fn finish(self) -> io::Result<Puppet> {
    let xmx = self.max_memory.unwrap_or_else(|| "2g".to_owned());
    let xms = self.min_memory.unwrap_or_else(|| "2g".to_owned());
    let jar = self.jar_path.unwrap_or_else(|| PathBuf::from("minecraft_server.jar"));

    let child = Command::new("java")
      .arg(format!("-Xmx{}", xmx))
      .arg(format!("-Xms{}", xms))
      .arg("-jar")
      .arg(jar)
      .arg("nogui")
      .stdout(Stdio::piped())
      .stdin(Stdio::piped())
      .spawn()?;
    Ok(Puppet::from_child(child))
  }
}

impl Default for PuppetBuilder {
  fn default() -> PuppetBuilder {
    PuppetBuilder {
      jar_path: None,
      max_memory: None,
      min_memory: None
    }
  }
}

/// A handle for a Minecraft server's process, allowing reading of the console and execution of commands.
#[derive(Debug)]
pub struct Puppet {
  child: Mutex<Child>,
  child_stdout: Mutex<ChildStdout>,
  child_stdin: Mutex<ChildStdin>
}

impl Puppet {
  /// Create a new puppet builder
  pub fn builder() -> PuppetBuilder {
    PuppetBuilder::new()
  }

  /// Manually construct a puppet from a `Child`.
  pub fn from_child(mut child: Child) -> Self {
    let child_stdout = child.stdout.take()
      .expect("no stdout captured");
    let child_stdin = child.stdin.take()
      .expect("no stdin captured");

    Puppet {
      child: Mutex::new(child),
      child_stdout: Mutex::new(child_stdout),
      child_stdin: Mutex::new(child_stdin)
    }
  }

  /// Begin mirroring the process' stdin to the puppet's stdin, as well as mirroring
  /// the puppet's stdout to an event handler and the process' stdout.
  /// The future returned by this function will resolve once the server has closed.
  pub async fn start(&self, event_handler: impl EventHandler) -> io::Result<()> {
    tokio::try_join!(
      self.start_dispatching_stdin(),
      self.start_dispatching_stdout(&event_handler)
    )?;
    Ok(())
  }

  /// Reads lines one at a time from stdin, sending each to the child stdin
  async fn start_dispatching_stdin(&self) -> io::Result<()> {
    use std::io::ErrorKind;
    let mut process_stdin = BufReader::new(tokio::io::stdin());
    let mut buf = String::new();
    loop {
      match process_stdin.read_line(&mut buf).await {
        Ok(0) => break, Ok(_) => (),
        Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
        Err(e) => return Err(e)
      };

      let mut child_stdin = self.child_stdin.lock().await;
      match child_stdin.write_all(buf.as_bytes()).await {
        Ok(()) => (),
        // If the pipe is broken, just ignore it and return `Ok`
        Err(ref e) if e.kind() == ErrorKind::BrokenPipe => break,
        Err(e) => return Err(e)
      };

      buf.clear();
    };

    Ok(())
  }

  /// Reads lines one at a time from the child stdout, sending each to stdin.
  /// NOTE: This function will lock the `child_stdout` mutex until it returns.
  async fn start_dispatching_stdout(&self, event_handler: &impl EventHandler) -> io::Result<()> {
    use std::io::ErrorKind;
    let mut process_stdout = tokio::io::stdout();
    let mut lock = self.child_stdout.lock().await;
    let mut child_stdout = BufReader::new(&mut *lock);
    let mut buf = String::new();
    loop {
      match child_stdout.read_line(&mut buf).await {
        Ok(0) => break, Ok(_) => (),
        Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
        // If the pipe is broken, just ignore it and return `Ok`
        Err(ref e) if e.kind() == ErrorKind::BrokenPipe => break,
        Err(e) => return Err(e)
      };

      process_stdout.write_all(buf.as_bytes()).await?;
      event_handler.console_line(self, buf.trim_end()).await;

      buf.clear();
    };

    Ok(())
  }

  /// Send a command to the server's console.
  pub async fn command(&self, command: impl AsRef<str>) -> io::Result<()> {
    let command = command.as_ref().trim().as_bytes();
    let mut lock = self.child_stdin.lock().await;
    lock.write_all(command).await?;
    lock.write_u8(b'\n').await?;
    lock.flush().await?;
    Ok(())
  }

  /// Wait for the server to close.
  pub async fn wait(&self) -> io::Result<ExitStatus> {
    let mut lock = self.child.lock().await;
    lock.wait().await
  }

  /// Force-kill the server.
  pub async fn kill(&self) -> io::Result<()> {
    let mut lock = self.child.lock().await;
    lock.kill().await
  }
}

/// The core trait for handling events dispatched by a puppet.
#[async_trait]
pub trait EventHandler: Send + Sync {
  /// Dispatched when the minecraft server spits out a line in the console.
  async fn console_line(&self, _puppet: &Puppet, _line: &str) {}
}

pub struct NoHandler;

#[async_trait]
impl EventHandler for NoHandler {}
