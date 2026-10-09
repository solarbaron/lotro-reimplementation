// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin-data writer: every physical `.plugindata` write goes through one FIFO queue,
//! run on a host worker (step-7 writes) or on the calling thread (writes of a closing
//! apartment, tools and tests), so writes of one file keep call order whoever asked for
//! them; and the latest saved bytes of every file with writes outstanding, so a `Load`
//! sees the last `Save`.
//!
//! Spec: `lua-plugin-api.md` §10.6 (per-key order, a later `Load` sees the last `Save`,
//! flush before close, atomic writes, "write on a worker thread if desired");
//! `docs/ARCHITECTURE.md` §21 (no blocking I/O on the main thread, exception (b) for the
//! writes and flush of a closing state).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use crate::host::HostApi;

/// One physical write waiting in the queue.
#[derive(Debug)]
struct Job {
    seq: u64,
    path: PathBuf,
    bytes: Arc<[u8]>,
    /// A synchronous writer waits for this job's result.
    sync: bool,
}

/// The latest saved bytes of a file and how many saves of it are not on disk yet.
#[derive(Debug)]
struct PendingFile {
    latest: Arc<[u8]>,
    saves: usize,
}

#[derive(Debug, Default)]
struct IoState {
    jobs: VecDeque<Job>,
    /// A worker job has been handed to the host and has not finished its loop.
    worker_running: bool,
    /// Sequence number of the job being written (by the worker or a synchronous writer);
    /// only one job is written at a time, so the queue order is the write order.
    in_flight: Option<u64>,
    next_seq: u64,
    /// Every job up to this sequence number has been written.
    completed_through: u64,
    /// Results of asynchronous jobs not yet collected by the main thread.
    done: Vec<(u64, bool)>,
    /// Results of synchronous jobs not yet read by their writer.
    sync_results: HashMap<u64, bool>,
    files: HashMap<PathBuf, PendingFile>,
    /// Bytes of saves not yet written (queued items and jobs).
    pending_bytes: usize,
}

/// The writer shared by the manager, the apartments and the worker.
#[derive(Debug, Default)]
pub(crate) struct DataIo {
    state: Mutex<IoState>,
    cond: Condvar,
}

impl DataIo {
    fn lock(&self) -> MutexGuard<'_, IoState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record a `Save` of `path` (queued or synchronous): its bytes become the file's
    /// latest value until written.
    pub(crate) fn register_save(&self, path: &Path, bytes: &Arc<[u8]>) {
        let mut st = self.lock();
        st.pending_bytes = st.pending_bytes.saturating_add(bytes.len());
        let entry = st
            .files
            .entry(path.to_path_buf())
            .or_insert_with(|| PendingFile {
                latest: Arc::clone(bytes),
                saves: 0,
            });
        entry.latest = Arc::clone(bytes);
        entry.saves += 1;
    }

    /// A queued save of `path` took new bytes (a merge into the latest queued write).
    pub(crate) fn replace_save(&self, path: &Path, old_len: usize, bytes: &Arc<[u8]>) {
        let mut st = self.lock();
        st.pending_bytes = st
            .pending_bytes
            .saturating_sub(old_len)
            .saturating_add(bytes.len());
        if let Some(f) = st.files.get_mut(path) {
            f.latest = Arc::clone(bytes);
        }
    }

    /// A save of `path` of `len` bytes ended without a write of its own (superseded by a
    /// later synchronous write of the same file).
    pub(crate) fn drop_save(&self, path: &Path, len: usize) {
        let mut st = self.lock();
        finish_save(&mut st, path, len);
    }

    /// Bytes of saves not yet written.
    pub(crate) fn pending_bytes(&self) -> usize {
        self.lock().pending_bytes
    }

    /// The bytes of the latest save of `path` not yet on disk, if any.
    pub(crate) fn latest(&self, path: &Path) -> Option<Arc<[u8]>> {
        self.lock().files.get(path).map(|f| Arc::clone(&f.latest))
    }

    /// Jobs waiting or being written.
    #[cfg(test)]
    fn outstanding(&self) -> usize {
        let st = self.lock();
        st.jobs.len() + usize::from(st.in_flight.is_some())
    }

    fn push(&self, st: &mut IoState, path: PathBuf, bytes: Arc<[u8]>, sync: bool) -> u64 {
        st.next_seq += 1;
        let seq = st.next_seq;
        st.jobs.push_back(Job {
            seq,
            path,
            bytes,
            sync,
        });
        seq
    }

    /// Queue an asynchronous write and make sure a worker runs the queue. Returns the
    /// job's sequence number; its result arrives through [`Self::take_done`].
    pub(crate) fn submit(
        self: &Arc<Self>,
        host: &dyn HostApi,
        path: PathBuf,
        bytes: Arc<[u8]>,
    ) -> u64 {
        let mut st = self.lock();
        let seq = self.push(&mut st, path, bytes, false);
        if !st.worker_running {
            st.worker_running = true;
            drop(st);
            let io = Arc::clone(self);
            host.spawn_worker(Box::new(move || io.worker_loop()));
        }
        seq
    }

    /// Write now, on the calling thread, after every write queued before it (waiting for
    /// the one a worker is writing). Returns whether the write succeeded.
    pub(crate) fn write_sync(&self, path: PathBuf, bytes: Arc<[u8]>) -> bool {
        let seq = {
            let mut st = self.lock();
            self.push(&mut st, path, bytes, true)
        };
        self.run_until(seq);
        self.lock().sync_results.remove(&seq).unwrap_or(false)
    }

    /// Write every job up to `seq` (the calling thread takes jobs whenever no other
    /// writer is busy, so a held or lost worker never blocks it).
    pub(crate) fn run_until(&self, seq: u64) {
        let mut st = self.lock();
        loop {
            if st.completed_through >= seq || (st.jobs.is_empty() && st.in_flight.is_none()) {
                return;
            }
            if st.in_flight.is_none()
                && let Some(job) = st.jobs.pop_front()
            {
                st.in_flight = Some(job.seq);
                drop(st);
                let ok = write_job(&job);
                st = self.lock();
                self.complete(&mut st, &job, ok);
                continue;
            }
            st = self.cond.wait(st).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Write everything queued so far (tools, tests and client exit).
    pub(crate) fn flush(&self) {
        let last = self.lock().next_seq;
        self.run_until(last);
    }

    /// Results of finished asynchronous writes, in write order.
    pub(crate) fn take_done(&self) -> Vec<(u64, bool)> {
        std::mem::take(&mut self.lock().done)
    }

    fn complete(&self, st: &mut IoState, job: &Job, ok: bool) {
        st.in_flight = None;
        st.completed_through = st.completed_through.max(job.seq);
        if job.sync {
            st.sync_results.insert(job.seq, ok);
        } else {
            st.done.push((job.seq, ok));
        }
        finish_save(st, &job.path, job.bytes.len());
        self.cond.notify_all();
    }

    fn worker_loop(&self) {
        let mut st = self.lock();
        loop {
            while st.in_flight.is_some() {
                st = self.cond.wait(st).unwrap_or_else(PoisonError::into_inner);
            }
            let Some(job) = st.jobs.pop_front() else {
                st.worker_running = false;
                return;
            };
            st.in_flight = Some(job.seq);
            drop(st);
            let ok = write_job(&job);
            st = self.lock();
            self.complete(&mut st, &job, ok);
        }
    }
}

fn finish_save(st: &mut IoState, path: &Path, len: usize) {
    st.pending_bytes = st.pending_bytes.saturating_sub(len);
    if let Some(f) = st.files.get_mut(path) {
        f.saves = f.saves.saturating_sub(1);
        if f.saves == 0 {
            st.files.remove(path);
        }
    }
}

fn write_job(job: &Job) -> bool {
    match crate::plugindata::write_atomic(&job.path, &job.bytes) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(path = %job.path.display(), "plugin data write failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoWorker;
    impl crate::host::CoreHost for NoWorker {}
    impl HostApi for NoWorker {
        fn write_line(&self, _text: &str) {}
        fn plugin_data_dir(&self, _scope: crate::host::DataScope) -> Option<PathBuf> {
            None
        }
        /// Jobs are lost: the queue must still drain through synchronous writers.
        fn spawn_worker(&self, _job: crate::host::WorkerJob) {}
    }

    #[test]
    fn queue_order_is_write_order_and_latest_is_tracked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k.plugindata");
        let io = Arc::new(DataIo::default());
        let a: Arc<[u8]> = Arc::from(&b"return 1"[..]);
        let b: Arc<[u8]> = Arc::from(&b"return 2"[..]);
        io.register_save(&path, &a);
        io.register_save(&path, &b);
        assert_eq!(io.latest(&path).as_deref(), Some(&b"return 2"[..]));
        assert_eq!(io.pending_bytes(), 16);
        // The async job is lost by the host; the synchronous write behind it runs it first.
        io.submit(&NoWorker, path.clone(), a);
        assert!(io.write_sync(path.clone(), b));
        assert_eq!(std::fs::read(&path).unwrap(), b"return 2");
        assert_eq!(io.take_done().len(), 1);
        assert_eq!(io.latest(&path), None);
        assert_eq!(io.pending_bytes(), 0);
        assert_eq!(io.outstanding(), 0);
    }
}
