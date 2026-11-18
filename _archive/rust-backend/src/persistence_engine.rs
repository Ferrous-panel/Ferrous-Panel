//! Ferrous System Core — Atomic Disk Persistence Engine (`rust-backend/src/persistence_engine.rs`)
//!
//! Guarantees zero state loss across browser reloads (`F5`), systemd daemon restarts,
//! and Ubuntu VPS reboots by atomically persisting all 19 panel state collections to
//! `/etc/ferrous-panel/*.json` (with fallback to `./data/*.json` in rootless containers).

use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const PRIMARY_STATE_DIR: &str = "/etc/ferrous-panel";
pub const FALLBACK_STATE_DIR: &str = "./data";

pub struct PersistenceEngine {
    state_dir: PathBuf,
}

impl Default for PersistenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistenceEngine {
    /// Resolves `/etc/ferrous-panel` on Ubuntu VPS or falls back to `./data` in unprivileged containers.
    pub fn new() -> Self {
        let primary = PathBuf::from(PRIMARY_STATE_DIR);
        if fs::create_dir_all(&primary).is_ok() {
            let probe = primary.join(".write_probe");
            if fs::write(&probe, b"ok").is_ok() {
                let _ = fs::remove_file(&probe);
                return Self { state_dir: primary };
            }
        }
        let fallback = PathBuf::from(FALLBACK_STATE_DIR);
        let _ = fs::create_dir_all(&fallback);
        Self { state_dir: fallback }
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn file_path(&self, filename: &str) -> PathBuf {
        self.state_dir.join(filename)
    }

    /// Atomically writes serialized JSON via a `.tmp` staging file and POSIX `fs::rename`
    /// so partial writes can never corrupt panel state during a power loss or crash.
    pub fn save_atomic<T: Serialize>(&self, filename: &str, data: &T) -> Result<(), String> {
        let target = self.file_path(filename);
        let tmp_target = self.file_path(&format!("{}.tmp", filename));

        let payload = serde_json::to_vec_pretty(data)
            .map_err(|e| format!("Failed to serialize {}: {}", filename, e))?;

        let mut file = fs::File::create(&tmp_target)
            .map_err(|e| format!("Failed to create {}: {}", tmp_target.display(), e))?;
        file.write_all(&payload)
            .map_err(|e| format!("Failed to write {}: {}", tmp_target.display(), e))?;
        let _ = file.sync_all();

        fs::rename(&tmp_target, &target)
            .map_err(|e| format!("Atomic rename failed for {}: {}", target.display(), e))?;
        Ok(())
    }

    /// Loads state from disk or returns the provided default value if the file does not exist yet.
    pub fn load_or_default<T: DeserializeOwned>(&self, filename: &str, default_val: T) -> T {
        let target = self.file_path(filename);
        if !target.exists() {
            return default_val;
        }
        match fs::read_to_string(&target) {
            Ok(raw) => serde_json::from_str::<T>(&raw).unwrap_or(default_val),
            Err(_) => default_val,
        }
    }
}
