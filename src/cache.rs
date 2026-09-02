//! A plain JSON file per cache key, each stamped with its fetch time.
//!
//! The read path is what makes `price` feel instant: a fresh entry is rendered
//! without touching the network, and a merely-warm entry is rendered *and* then
//! refreshed by a detached background process, so the next run is fresh again.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Below this age we refresh in the background instead of blocking on it.
pub const WARM_WINDOW: Duration = Duration::from_secs(10 * 60);
/// How long a warm-lock is honoured before we assume the warmer died.
const LOCK_TTL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct Hit<T> {
    pub value: T,
    pub age: Duration,
}

#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
}

#[derive(serde::Deserialize, Serialize)]
struct Entry<T> {
    fetched_at: u64,
    payload: T,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Cache {
        Cache { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, key: &str) -> PathBuf {
        let safe: String = key
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe}.json"))
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<Hit<T>> {
        let text = std::fs::read_to_string(self.path(key)).ok()?;
        let entry: Entry<T> = serde_json::from_str(&text).ok()?;
        let now = unix_now();
        // A clock that moved backwards shouldn't make an entry look fresh forever.
        let age = Duration::from_secs(now.saturating_sub(entry.fetched_at));
        Some(Hit { value: entry.payload, age })
    }

    /// Same as [`get`], but only if the entry is younger than `ttl`.
    pub fn get_fresh<T: DeserializeOwned>(&self, key: &str, ttl: Duration) -> Option<T> {
        self.get(key).filter(|h| h.age < ttl).map(|h| h.value)
    }

    pub fn put<T: Serialize>(&self, key: &str, payload: &T) {
        // Cache writes are a convenience: a failure must never fail the command.
        let _ = std::fs::create_dir_all(&self.dir);
        let entry = Entry { fetched_at: unix_now(), payload };
        if let Ok(text) = serde_json::to_string(&entry) {
            let path = self.path(key);
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// Spawns a detached `price __warm` unless one is already running. The
    /// stamp doubles as the lock: a warmer that dies leaves it to expire.
    pub fn spawn_warm(&self) {
        if let Some(hit) = self.get::<u64>("warm_lock_stamp") {
            if hit.age < LOCK_TTL {
                return;
            }
        }
        let Ok(exe) = std::env::current_exe() else { return };
        self.put("warm_lock_stamp", &unix_now());
        let _ = std::process::Command::new(exe)
            .arg("__warm")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
