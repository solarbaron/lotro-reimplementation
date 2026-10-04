// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistence of the explorer's state in Tessera's settings file (`tessera-settings`):
//! the game directory, the last DID, recent DIDs, bookmarks and the window size.
//!
//! The file is read once at start-up (before the first frame). Changes are written by a
//! background thread, debounced, so the UI thread never waits for the disk.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tessera_settings::{Bookmark, Settings, SettingsStore};

/// Delay between the last change and the write.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(750);

/// Window key of the explorer's main window in the settings file.
pub const WINDOW_KEY: &str = "explorer";

/// The settings and their (optional) backing file.
#[derive(Debug)]
pub struct Persist {
    /// The current settings.
    pub settings: Settings,
    dirty_since: Option<Instant>,
    saver: Option<(Sender<Settings>, JoinHandle<()>)>,
    path: Option<std::path::PathBuf>,
}

impl Persist {
    /// Settings that are never written (`--no-persist`, tests).
    #[must_use]
    pub fn in_memory(settings: Settings) -> Self {
        Self {
            settings,
            dirty_since: None,
            saver: None,
            path: None,
        }
    }

    /// Loads the settings file (`TESSERA_SETTINGS_FILE` or the platform default) and
    /// starts the background writer. Load problems are logged and never fatal.
    #[must_use]
    pub fn load_default() -> Self {
        let store = match SettingsStore::open_default() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "no settings location; settings will not be saved");
                return Self::in_memory(Settings::default());
            }
        };
        Self::load_from(store)
    }

    /// Loads from `store` and starts the background writer.
    #[must_use]
    pub fn load_from(store: SettingsStore) -> Self {
        let loaded = store.load();
        for w in &loaded.warnings {
            tracing::warn!(warning = %w, "settings");
        }
        tracing::debug!(path = %store.path().display(), status = ?loaded.status, "settings loaded");
        let path = store.path().to_path_buf();
        let (tx, rx): (Sender<Settings>, Receiver<Settings>) = mpsc::channel();
        let saver = std::thread::Builder::new()
            .name("explorer-settings".into())
            .spawn(move || {
                while let Ok(mut s) = rx.recv() {
                    // Only the newest pending state matters.
                    while let Ok(newer) = rx.try_recv() {
                        s = newer;
                    }
                    if let Err(e) = store.save(&s) {
                        tracing::warn!(error = %e, "saving settings failed");
                    }
                }
            });
        let saver = match saver {
            Ok(h) => Some((tx, h)),
            Err(e) => {
                tracing::warn!(error = %e, "no settings writer thread");
                None
            }
        };
        Self {
            settings: loaded.settings,
            dirty_since: None,
            saver,
            path: Some(path),
        }
    }

    /// The settings file path, when persisting.
    #[must_use]
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Changes the settings and schedules a write.
    pub fn update(&mut self, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings);
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    /// Whether a change waits to be written.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    /// Hands the settings to the writer once the debounce delay has passed. Call once
    /// per frame; never blocks.
    pub fn tick(&mut self) {
        if self
            .dirty_since
            .is_some_and(|t| t.elapsed() >= SAVE_DEBOUNCE)
        {
            self.send();
        }
    }

    fn send(&mut self) {
        self.dirty_since = None;
        if let Some((tx, _)) = &self.saver {
            let _ = tx.send(self.settings.clone());
        }
    }

    /// Writes pending changes and waits for the writer (at exit).
    pub fn flush(&mut self) {
        if self.dirty_since.is_some() {
            self.send();
        }
        if let Some((tx, handle)) = self.saver.take() {
            drop(tx);
            let _ = handle.join();
        }
    }

    /// Whether `did` is bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, did: u32) -> bool {
        self.settings
            .explorer
            .bookmarks
            .iter()
            .any(|b| b.did == did)
    }

    /// Adds or removes a bookmark for `did`.
    pub fn toggle_bookmark(&mut self, did: u32, label: &str) {
        if self.is_bookmarked(did) {
            let removed: Vec<Bookmark> = self
                .settings
                .explorer
                .bookmarks
                .iter()
                .filter(|b| b.did == did)
                .cloned()
                .collect();
            self.update(|s| {
                for b in removed {
                    s.remove_bookmark(b.did, b.archive.as_deref());
                }
            });
        } else {
            self.update(|s| {
                if !s.add_bookmark(Bookmark::new(did, label)) {
                    tracing::warn!("bookmark list is full");
                }
            });
        }
    }
}

impl Drop for Persist {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_bookmarks_and_recents() {
        let mut p = Persist::in_memory(Settings::default());
        assert!(!p.is_dirty());
        p.toggle_bookmark(0x4100_0001, "surface");
        assert!(p.is_bookmarked(0x4100_0001));
        assert!(p.is_dirty());
        p.toggle_bookmark(0x4100_0001, "surface");
        assert!(!p.is_bookmarked(0x4100_0001));
        p.update(|s| s.push_recent_did(7));
        assert_eq!(p.settings.explorer.recent_dids, vec![7]);
        p.flush();
        assert!(!p.is_dirty());
        assert!(p.path().is_none());
    }

    #[test]
    fn writes_through_a_store() {
        let dir =
            std::env::temp_dir().join(format!("tessera-explorer-persist-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("settings.toml");
        let _ = std::fs::remove_file(&file);
        {
            let mut p = Persist::load_from(SettingsStore::new(&file));
            p.update(|s| s.push_recent_did(0x2500_0001));
            p.flush();
        }
        let reloaded = SettingsStore::new(&file).load();
        assert_eq!(reloaded.settings.explorer.recent_dids, vec![0x2500_0001]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
