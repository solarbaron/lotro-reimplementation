// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's saved game-window choices (`[GameWindows]` of the settings file,
//! [`WindowPrefs`]): the dialogue font size (`npc-windows.md` §5.2 Tessera rule), the
//! quest tracker's visibility, the quest log's font index and the overhead quest icons
//! (`quest-journal.md` §4.8, §8).
//!
//! [`load_window_prefs`] reads them (on the NPC windows' names loader, off the frame);
//! [`save_window_prefs`] hands one save to a single writer thread (`ARCHITECTURE.md` §21:
//! no file I/O on the frame thread), which applies the saves in the order they were made
//! with a locked read-modify-write of the settings file, so the last choice is the one
//! kept. The NPC hub owns the saver the windows use ([`crate::npc::NpcHub::save_prefs`]).

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use tessera_settings::WindowPrefs;

/// Reads Tessera's saved window choices (`None` without a settings location); warnings are
/// logged. Call off the frame thread.
#[must_use]
pub fn load_window_prefs() -> Option<WindowPrefs> {
    match tessera_settings::SettingsStore::open_default() {
        Ok(store) => {
            let (prefs, warnings) = WindowPrefs::from_settings(&store.load().settings);
            for w in warnings {
                tracing::warn!("settings: {w}");
            }
            Some(prefs)
        }
        Err(e) => {
            tracing::debug!("settings unavailable ({e}); window choices not saved");
            None
        }
    }
}

/// The one writer thread of [`save_window_prefs`] (started on first use; `None` when it
/// could not be started).
static WRITER: OnceLock<Option<Sender<WindowPrefs>>> = OnceLock::new();

/// Writes the keys of one save into Tessera's settings file (a locked read-modify-write).
fn write_window_prefs(prefs: WindowPrefs) {
    let store = match tessera_settings::SettingsStore::open_default() {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("window choices not saved: {e}");
            return;
        }
    };
    if let Err(e) = store.update(|s| prefs.store(s)) {
        tracing::info!("window choices not saved: {e}");
    }
}

/// Writes the keys `prefs` sets into Tessera's settings file off the frame thread (module
/// docs); failures are logged. Never blocks on file I/O.
pub fn save_window_prefs(prefs: WindowPrefs) {
    let writer = WRITER.get_or_init(|| {
        let (tx, rx) = channel::<WindowPrefs>();
        match std::thread::Builder::new()
            .name("tessera-window-settings".into())
            .spawn(move || {
                while let Ok(p) = rx.recv() {
                    write_window_prefs(p);
                }
            }) {
            Ok(_) => Some(tx),
            Err(e) => {
                tracing::info!("no settings writer thread ({e}); window choices not saved");
                None
            }
        }
    });
    if !writer.as_ref().is_some_and(|tx| tx.send(prefs).is_ok()) {
        tracing::info!("window choices not saved: no settings writer");
    }
}
