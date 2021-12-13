use chrono::prelude::*;

use std::fs;
use std::path::{PathBuf, Path};

use crate::Error;



#[derive(Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
  pub jar_path: PathBuf,
  pub max_memory: String,
  pub min_memory: String,
  pub restart_time: NaiveTime
}

impl Config {
  pub async fn load<P: AsRef<Path> + Send>(path: P) -> Result<Config, Error> {
    use std::io::ErrorKind;
    let path = path.as_ref().to_owned();
    asyncify(move || {
      Ok(match fs::read(&path) {
        Ok(data) => toml::from_slice::<Config>(&data)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
          let config = Config::default();
          let data = toml::to_vec(&config)?;
          fs::write(&path, data)?;
          return Err(Error::NotConfigured(path))
        },
        Err(err) => return Err(err.into())
      })
    }).await
  }

  pub fn next_restart<Tz: TimeZone>(&self, now: DateTime<Tz>) -> DateTime<Tz> {
    let time = now.date()
      .and_time(self.restart_time)
      .expect("error getting restart time");
    if time <= now {
      now.date().succ()
        .and_time(self.restart_time)
        .expect("error getting restart time")
    } else {
      time
    }
  }
}

impl Default for Config {
  fn default() -> Self {
    Config {
      jar_path: "server.jar".into(),
      max_memory: "2g".into(),
      min_memory: "2g".into(),
      restart_time: NaiveTime::from_hms(22, 0, 0)
    }
  }
}

pub(crate) async fn asyncify<F, T>(f: F) -> Result<T, Error>
where F: FnOnce() -> Result<T, Error> + Send + 'static, T: Send + 'static {
  match tokio::task::spawn_blocking(f).await {
    Ok(res) => res.map_err(From::from),
    Err(_) => Err(Error::BackgroundTaskFailed)
  }
}
