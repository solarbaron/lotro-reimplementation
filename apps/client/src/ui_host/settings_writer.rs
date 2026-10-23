// SPDX-License-Identifier: GPL-3.0-or-later
//! The game UI's one settings writer (ARCHITECTURE §21: no blocking I/O on the main thread):
//! every write the frame loop asks for — the settled UI scale and safe aspect
//! (`ui-scaling.md` §4.4, §4.5 step 5), the Interface window's optional rows (§7.1), the saved
//! panel placements of `[UI.panels]` (§5.8), the per-window factors of `[UI.group_scale]`
//! (§7.4) and the bag windows' column settings
//! (`character-panels.md` §4.1) — is a [`SettingsJob`] handed to one long-lived worker thread.
//!
//! The worker applies the jobs it finds waiting, in order, in **one** locked
//! read-modify-write of Tessera's settings file (`SettingsStore::update`), so two threads of
//! this process never read-modify-write the file at the same time and a burst of jobs costs
//! one write. The frame loop only sends on a channel and never waits; [`SettingsWriter::flush`]
//! waits for the jobs sent so far (tests and the end of the run). When no thread can be
//! spawned the jobs are written on the calling thread instead (logged).

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{JoinHandle, ThreadId};

use tessera_settings::{PanelPlacement, SafeAspect, Settings, SettingsStore, UiPrefs, UiScale};

use super::interface::ExtraPersist;

/// One write the frame loop asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsJob {
    /// `ui_scale` and `ui_safe_aspect` (`ui-scaling.md` §4.4).
    UiScale {
        /// The user's scale.
        scale: UiScale,
        /// The safe aspect.
        aspect: SafeAspect,
    },
    /// The Interface window's optional rows (§7.1): art filter, text size and the resets.
    UiExtra(ExtraPersist),
    /// Every saved panel placement (§5.8): `[UI.panels]` becomes exactly this map.
    Panels(BTreeMap<String, PanelPlacement>),
    /// Bag-window column settings (`character-panels.md` §4.1), by setting name.
    Columns(Vec<(String, i32)>),
    /// Every per-window factor (`ui-scaling.md` §7.4): `[UI.group_scale]` becomes exactly this
    /// map.
    GroupScales(BTreeMap<u32, f32>),
}

/// Applies one job to `s` (the settings as the file holds them now).
fn apply(s: &mut Settings, job: &SettingsJob) {
    match job {
        SettingsJob::UiScale { scale, aspect } => {
            let (mut prefs, _) = UiPrefs::from_settings(s);
            prefs.ui_scale = *scale;
            prefs.safe_aspect = *aspect;
            prefs.store(s);
        }
        SettingsJob::UiExtra(extra) => {
            let (mut prefs, _) = UiPrefs::from_settings(s);
            prefs.art_filter = extra.art;
            prefs.text_scale = extra.text_scale;
            if extra.clear_panels {
                prefs.panels.clear();
            }
            if extra.clear_groups {
                prefs.group_scale.clear();
            }
            prefs.store(s);
        }
        SettingsJob::Panels(panels) => {
            let (mut prefs, _) = UiPrefs::from_settings(s);
            prefs.panels.clone_from(panels);
            prefs.store(s);
        }
        SettingsJob::GroupScales(groups) => {
            let (mut prefs, _) = UiPrefs::from_settings(s);
            prefs.group_scale.clone_from(groups);
            prefs.store(s);
        }
        SettingsJob::Columns(changes) => {
            for (name, c) in changes {
                crate::character::settings::store_columns(s, name, *c);
            }
        }
    }
}

/// One locked read-modify-write with `jobs` applied in order. A failure is logged, not fatal.
fn write(store: &SettingsStore, jobs: &[SettingsJob]) {
    if jobs.is_empty() {
        return;
    }
    if let Err(e) = store.update(|s| {
        for j in jobs {
            apply(s, j);
        }
    }) {
        tracing::warn!("UI settings not saved: {e}");
    }
}

enum Msg {
    Job(SettingsJob),
    Flush(Sender<()>),
}

/// Most jobs applied in one write (a bound on one batch; the rest go in the next write).
pub const MAX_BATCH: usize = 256;

fn run(store: &SettingsStore, rx: &Receiver<Msg>) {
    while let Ok(first) = rx.recv() {
        let mut jobs = Vec::new();
        let mut flushes = Vec::new();
        let mut next = Some(first);
        while let Some(m) = next.take() {
            match m {
                Msg::Job(j) => jobs.push(j),
                Msg::Flush(done) => flushes.push(done),
            }
            if jobs.len() >= MAX_BATCH {
                break;
            }
            next = rx.try_recv().ok();
        }
        write(store, &jobs);
        for done in flushes {
            let _ = done.send(());
        }
    }
}

/// The settings writer (module docs).
#[derive(Debug)]
pub struct SettingsWriter {
    /// Where the jobs go; `None` when the settings file is unavailable (jobs are dropped).
    store: Option<SettingsStore>,
    tx: Option<Sender<Msg>>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsWriter {
    /// A writer for `store` (`None`: no settings file; every job is dropped, logged once
    /// per job at debug level). The worker thread starts with the first job.
    #[must_use]
    pub fn new(store: Option<SettingsStore>) -> Self {
        Self {
            store,
            tx: None,
            worker: None,
        }
    }

    /// A writer for the user's settings file.
    #[must_use]
    pub fn open_default() -> Self {
        match SettingsStore::open_default() {
            Ok(s) => Self::new(Some(s)),
            Err(e) => {
                tracing::debug!("settings unavailable ({e}); UI settings are not saved");
                Self::new(None)
            }
        }
    }

    /// The worker's thread, once it runs (tests).
    #[must_use]
    pub fn thread_id(&self) -> Option<ThreadId> {
        self.worker.as_ref().map(|h| h.thread().id())
    }

    fn sender(&mut self) -> Option<&Sender<Msg>> {
        if self.tx.is_none() {
            let store = self.store.clone()?;
            let (tx, rx) = mpsc::channel();
            match std::thread::Builder::new()
                .name("ui-settings".to_owned())
                .spawn(move || run(&store, &rx))
            {
                Ok(h) => {
                    self.tx = Some(tx);
                    self.worker = Some(h);
                }
                Err(e) => {
                    tracing::debug!("no settings thread ({e}); writing on this thread");
                    return None;
                }
            }
        }
        self.tx.as_ref()
    }

    /// Hands `job` to the worker; never waits. Without a worker (no thread could be spawned)
    /// the job is written on this thread.
    pub fn submit(&mut self, job: SettingsJob) {
        if self.store.is_none() {
            tracing::debug!("settings unavailable; {job:?} not saved");
            return;
        }
        let job = match self.sender() {
            Some(tx) => match tx.send(Msg::Job(job)) {
                Ok(()) => return,
                Err(mpsc::SendError(Msg::Job(j))) => j,
                Err(_) => return,
            },
            None => job,
        };
        // The worker is gone (or never started): write here.
        self.tx = None;
        if let Some(store) = &self.store {
            write(store, &[job]);
        }
    }

    /// Waits until every job submitted so far is written (tests; the end of the run).
    pub fn flush(&mut self) {
        let Some(tx) = &self.tx else { return };
        let (done, wait) = mpsc::channel();
        if tx.send(Msg::Flush(done)).is_ok() {
            let _ = wait.recv();
        }
    }
}

impl Drop for SettingsWriter {
    fn drop(&mut self) {
        // Closing the channel ends the worker after the jobs already sent; joining waits for
        // that last write (the end of the run, after the last frame).
        self.tx = None;
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_settings::{AnchorX, AnchorY, ArtFilter, PanelRef};

    fn placement(dx: f32) -> PanelPlacement {
        PanelPlacement {
            reference: PanelRef::Safe,
            ax: AnchorX::Right,
            dx,
            ay: AnchorY::Top,
            dy: 12.0,
            w: None,
            h: None,
            shown: None,
        }
    }

    #[test]
    fn jobs_are_written_off_the_calling_thread_in_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tessera.toml");
        let mut w = SettingsWriter::new(Some(SettingsStore::new(path.clone())));
        w.submit(SettingsJob::UiScale {
            scale: UiScale::Fixed(1.5),
            aspect: SafeAspect::R16_9,
        });
        w.submit(SettingsJob::Columns(vec![("UI.Bag2Columns".into(), 9)]));
        let mut panels = BTreeMap::new();
        panels.insert("0x22000064/0x10000024".to_owned(), placement(40.0));
        w.submit(SettingsJob::Panels(panels));
        // An Interface reset after the placement clears it again (order kept).
        w.submit(SettingsJob::UiExtra(ExtraPersist {
            art: ArtFilter::Smooth,
            text_scale: 1.25,
            clear_panels: true,
            clear_groups: false,
        }));
        w.flush();
        assert_ne!(
            w.thread_id(),
            Some(std::thread::current().id()),
            "written on the worker"
        );
        let s = SettingsStore::new(path).update(|_| {}).expect("read back");
        let (prefs, _) = UiPrefs::from_settings(&s);
        assert_eq!(prefs.ui_scale, UiScale::Fixed(1.5));
        assert_eq!(prefs.safe_aspect, SafeAspect::R16_9);
        assert_eq!(prefs.art_filter, ArtFilter::Smooth);
        assert!(prefs.panels.is_empty(), "the later reset wins");
        let cols = crate::character::settings::P1Settings::from_settings(&s);
        assert_eq!(cols.columns("UI.Bag2Columns"), Some(9));
    }

    /// `[UI.group_scale]` (`ui-scaling.md` §7.4) is replaced by the job's map, and the
    /// Interface window's "Reset window scales" clears it again.
    #[test]
    fn group_scales_are_written_and_reset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tessera.toml");
        let mut w = SettingsWriter::new(Some(SettingsStore::new(path.clone())));
        w.submit(SettingsJob::GroupScales([(7, 1.25), (12, 0.75)].into()));
        w.flush();
        let s = SettingsStore::new(path.clone())
            .update(|_| {})
            .expect("read");
        let (prefs, _) = UiPrefs::from_settings(&s);
        assert_eq!(prefs.group_scale, [(7, 1.25), (12, 0.75)].into());
        w.submit(SettingsJob::UiExtra(ExtraPersist {
            art: ArtFilter::Sharp,
            text_scale: 1.0,
            clear_panels: false,
            clear_groups: true,
        }));
        w.flush();
        let s = SettingsStore::new(path).update(|_| {}).expect("read");
        let (prefs, _) = UiPrefs::from_settings(&s);
        assert!(prefs.group_scale.is_empty());
    }

    #[test]
    fn without_a_store_jobs_are_dropped() {
        let mut w = SettingsWriter::new(None);
        w.submit(SettingsJob::Columns(vec![("UI.Bag1Columns".into(), 4)]));
        w.flush();
        assert!(w.thread_id().is_none());
    }
}
