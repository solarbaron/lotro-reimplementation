// SPDX-License-Identifier: GPL-3.0-or-later
//! The hash workers and the writer thread (`test-server.md` §8.1).
//!
//! Hash workers compute Argon2id only (login verification, registration, re-hashing; §6.2);
//! they also perform the accounts-file read-modify-write that registration and re-hashing
//! need, under the accounts lock. The writer performs every other file write atomically,
//! with one pending job per file (a newer snapshot replaces an older one, §10.1). Neither
//! the network nor the simulation thread ever computes a hash or touches a file (§14).

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::accounts::{self, AccountStore, HashParams, PhcHash};
use crate::datadir;
use crate::error::ServerError;
use crate::shared::{HashJob, HashOutcome, HashResult, SimInput, Stats, WriterMsg};

/// Login verifications that may wait for a worker (§6.2 step 3).
pub const HASH_QUEUE: usize = 64;
/// Most files the writer holds pending (§10.1).
pub const WRITER_QUEUE: usize = 4_096;
/// How often the writer re-checks the accounts file (§3.7).
pub const ACCOUNTS_RELOAD_INTERVAL: Duration = Duration::from_secs(10);

/// What a hash worker needs.
#[derive(Debug)]
pub struct HashContext {
    /// The accounts.
    pub accounts: Arc<AccountStore>,
    /// Parameters for new hashes.
    pub params: HashParams,
    /// The dummy hash verified for unknown accounts (§6.2 step 4).
    pub dummy: PhcHash,
    /// Where results go.
    pub results: SyncSender<SimInput>,
    /// Counters.
    pub stats: Arc<Stats>,
}

impl HashContext {
    /// A dummy hash with the configured parameters and a random password (§6.2 step 4).
    pub fn make_dummy(params: HashParams) -> Result<PhcHash, ServerError> {
        let mut pw = [0u8; 32];
        accounts::random_bytes(&mut pw)?;
        let phc = accounts::hash_password(&pw, params)?;
        PhcHash::parse(&phc).ok_or_else(|| ServerError::Runtime("dummy hash".into()))
    }

    fn run(&self, job: HashJob) -> HashResult {
        let HashJob {
            conn,
            account,
            password,
            register,
        } = job;
        self.accounts.refresh_if_changed();
        let outcome = if register {
            match accounts::hash_password(password.as_bytes(), self.params) {
                Ok(hash) => {
                    let name = account.clone();
                    match self.accounts.modify(move |f| {
                        if f.accounts.contains_key(&name) {
                            return Ok(HashOutcome::AccountExists);
                        }
                        f.accounts
                            .insert(name.clone(), accounts::new_account(name.clone(), hash));
                        // A file every reader still accepts, or the server could not start
                        // again (registrations are otherwise unbounded).
                        if !accounts::fits(f) {
                            f.accounts.remove(&name);
                            tracing::warn!("accounts file full: registration refused");
                            return Ok(HashOutcome::RegistrationFull);
                        }
                        Ok(HashOutcome::Verified)
                    }) {
                        Ok(o) => o,
                        Err(e) => {
                            tracing::error!(error = %e, "registration could not be saved");
                            HashOutcome::Error
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "hashing failed");
                    HashOutcome::Error
                }
            }
        } else {
            let stored = self
                .accounts
                .get(&account)
                .and_then(|a| PhcHash::parse(&a.hash));
            let ok = match &stored {
                Some(h) => h.verify(password.as_bytes()),
                None => {
                    let _ = self.dummy.verify(password.as_bytes());
                    false
                }
            };
            if ok
                && let Some(h) = &stored
                && h.params.below(&self.params)
            {
                self.rehash(&account, &password);
            }
            if ok {
                HashOutcome::Verified
            } else {
                HashOutcome::BadCredentials
            }
        };
        accounts::wipe(password);
        HashResult {
            conn,
            account,
            outcome,
            register,
        }
    }

    /// Replaces a hash whose parameters are below the configured ones (§3.4).
    fn rehash(&self, account: &str, password: &str) {
        let Ok(hash) = accounts::hash_password(password.as_bytes(), self.params) else {
            return;
        };
        let name = account.to_owned();
        if let Err(e) = self.accounts.modify(move |f| {
            if let Some(a) = f.accounts.get_mut(&name) {
                a.hash = hash;
            }
            Ok(())
        }) {
            tracing::warn!(error = %e, "re-hash could not be saved");
        }
    }
}

/// The hash worker pool.
#[derive(Debug)]
pub struct HashPool {
    sender: Option<SyncSender<HashJob>>,
    threads: Vec<JoinHandle<()>>,
}

impl HashPool {
    /// Starts `workers` threads; at most `workers` + [`HASH_QUEUE`] jobs are held.
    pub fn start(workers: usize, ctx: HashContext) -> Result<Self, ServerError> {
        let workers = workers.max(1);
        let (tx, rx) = std::sync::mpsc::sync_channel::<HashJob>(workers + HASH_QUEUE);
        let rx = Arc::new(Mutex::new(rx));
        let ctx = Arc::new(ctx);
        let mut threads = Vec::new();
        for i in 0..workers {
            let rx = Arc::clone(&rx);
            let ctx = Arc::clone(&ctx);
            let t = std::thread::Builder::new()
                .name(format!("tessera-hash-{i}"))
                .spawn(move || {
                    loop {
                        let job = match rx.lock() {
                            Ok(guard) => guard.recv(),
                            Err(_) => return,
                        };
                        let Ok(job) = job else { return };
                        let result = ctx.run(job);
                        ctx.stats.hash_jobs.fetch_sub(1, Ordering::Relaxed);
                        if ctx.results.send(SimInput::Hash(result)).is_err() {
                            return;
                        }
                    }
                })
                .map_err(|e| ServerError::Runtime(format!("cannot start a hash worker: {e}")))?;
            threads.push(t);
        }
        Ok(Self {
            sender: Some(tx),
            threads,
        })
    }

    /// The job queue (the simulation's end).
    #[must_use]
    pub fn sender(&self) -> SyncSender<HashJob> {
        self.sender
            .clone()
            .unwrap_or_else(|| std::sync::mpsc::sync_channel(0).0)
    }

    /// Stops the workers after the queued jobs.
    pub fn stop(mut self) {
        self.sender = None;
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// The writer thread.
#[derive(Debug)]
pub struct Writer {
    sender: SyncSender<WriterMsg>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Starts the writer. `results` receives `WriteDone` for jobs with a token; `accounts`
    /// is re-checked every 10 s (§3.7).
    pub fn start(
        results: SyncSender<SimInput>,
        accounts: Option<Arc<AccountStore>>,
        stats: Arc<Stats>,
    ) -> Result<Self, ServerError> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<WriterMsg>(WRITER_QUEUE);
        let thread = std::thread::Builder::new()
            .name("tessera-writer".into())
            .spawn(move || writer_loop(&rx, &results, accounts.as_deref(), &stats))
            .map_err(|e| ServerError::Runtime(format!("cannot start the writer: {e}")))?;
        Ok(Self {
            sender: tx,
            thread: Some(thread),
        })
    }

    /// The job queue.
    #[must_use]
    pub fn sender(&self) -> SyncSender<WriterMsg> {
        self.sender.clone()
    }

    /// Finishes pending jobs and stops.
    pub fn stop(mut self) {
        let _ = self.sender.send(WriterMsg::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn writer_loop(
    rx: &Receiver<WriterMsg>,
    results: &SyncSender<SimInput>,
    accounts: Option<&AccountStore>,
    stats: &Stats,
) {
    // One pending job per file, in arrival order of the file's first job.
    let mut pending: BTreeMap<PathBuf, (u64, crate::shared::WriteJob, Vec<u64>)> = BTreeMap::new();
    let mut order: VecDeque<(u64, PathBuf)> = VecDeque::new();
    let mut seq = 0u64;
    let mut failures = 0usize;
    let mut last_reload = Instant::now();
    let mut stop = false;
    // Results the simulation's input channel had no room for. The writer never blocks on
    // that channel: the simulation may itself be waiting for room in the writer queue (a
    // save that is never skipped) or for a barrier (shutdown).
    let mut undelivered: VecDeque<SimInput> = VecDeque::new();
    while !stop {
        let wait = if undelivered.is_empty() {
            Duration::from_secs(1)
        } else {
            Duration::from_millis(20)
        };
        let first = match rx.recv_timeout(wait) {
            Ok(m) => Some(m),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(WriterMsg::Stop),
        };
        let mut barriers = Vec::new();
        let mut handle =
            |m: WriterMsg,
             pending: &mut BTreeMap<PathBuf, (u64, crate::shared::WriteJob, Vec<u64>)>,
             order: &mut VecDeque<(u64, PathBuf)>,
             barriers: &mut Vec<std::sync::mpsc::Sender<usize>>,
             stop: &mut bool| match m {
                WriterMsg::Write(job) => {
                    let path = job.path.clone();
                    match pending.get_mut(&path) {
                        Some(entry) => {
                            // A newer snapshot replaces the older one; both tokens are answered.
                            if let Some(t) = entry.1.token {
                                entry.2.push(t);
                            }
                            entry.1 = job;
                            stats.writer_jobs.fetch_sub(1, Ordering::Relaxed);
                        }
                        None => {
                            seq += 1;
                            order.push_back((seq, path.clone()));
                            pending.insert(path, (seq, job, Vec::new()));
                        }
                    }
                }
                WriterMsg::Barrier(reply) => barriers.push(reply),
                WriterMsg::Stop => *stop = true,
            };
        if let Some(m) = first {
            handle(m, &mut pending, &mut order, &mut barriers, &mut stop);
        }
        while let Ok(m) = rx.try_recv() {
            handle(m, &mut pending, &mut order, &mut barriers, &mut stop);
        }
        while let Some((s, path)) = order.pop_front() {
            let Some((js, job, extra_tokens)) = pending.remove(&path) else {
                continue;
            };
            if js != s {
                continue;
            }
            stats.writer_jobs.fetch_sub(1, Ordering::Relaxed);
            let ok = match write_one(&job) {
                Ok(()) => true,
                Err(e) => {
                    tracing::error!(file = %job.path.display(), error = %e, "save failed");
                    failures += 1;
                    false
                }
            };
            for token in job.token.into_iter().chain(extra_tokens) {
                undelivered.push_back(SimInput::WriteDone { token, ok });
            }
        }
        while let Some(m) = undelivered.pop_front() {
            match results.try_send(m) {
                Ok(()) => {}
                Err(std::sync::mpsc::TrySendError::Full(m)) => {
                    undelivered.push_front(m);
                    break;
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                    undelivered.clear();
                    break;
                }
            }
        }
        for b in barriers {
            let _ = b.send(failures);
            failures = 0;
        }
        if last_reload.elapsed() >= ACCOUNTS_RELOAD_INTERVAL {
            last_reload = Instant::now();
            if let Some(a) = accounts {
                a.refresh_if_changed();
            }
        }
    }
}

fn write_one(job: &crate::shared::WriteJob) -> std::io::Result<()> {
    if let Some(parent) = job.path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.is_dir()
    {
        datadir::create_private_dir(parent).map_err(|e| std::io::Error::other(e.to_string()))?;
    }
    datadir::atomic_write(&job.path, &job.contents, job.mode)
}
