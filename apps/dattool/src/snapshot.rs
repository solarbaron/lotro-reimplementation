// SPDX-License-Identifier: GPL-3.0-or-later
//! Indexing live archives, loading snapshot files, and the `snapshot` subcommand.
//!
//! The snapshot file format itself (and [`Index`]) lives in the library module
//! [`tessera_dattool::snapfile`], where `fuzz/` can reach its decoder.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;
use tessera_dat::{Archive, Entry};
pub use tessera_dattool::snapfile::{
    ArchiveIndex, Index, MAGIC, Rec, SourceKind, content_hash, decode_from, encode,
};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::output::{check_output_path, protected_dirs, write_output};
use crate::util::{format_time, human_bytes, par_map};

/// Records hashed per parallel work unit.
const HASH_CHUNK: usize = 256;

fn rec_from_entry(e: &Entry) -> Rec {
    Rec {
        did: e.did,
        iteration: e.iteration,
        size: e.length,
        time: e.time,
        stamp: e.stamp,
        flags: e.flags,
        hash: None,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn walk(a: &Archive) -> (Vec<Entry>, Option<String>) {
    let mut v = Vec::new();
    for e in a.entries() {
        match e {
            Ok(e) => v.push(e),
            Err(err) => return (v, Some(err.to_string())),
        }
    }
    (v, None)
}

/// Which payloads [`index_input`] reads and hashes.
#[derive(Debug, Clone, Default)]
pub struct HashPlan {
    /// Hash at all. The resulting index is marked hashed exactly when this is set.
    pub enabled: bool,
    /// Only archives with one of these labels (compared ignoring ASCII case); `None` = all.
    pub archives: Option<Vec<String>>,
    /// Only DIDs with this high byte.
    pub prefix: Option<u8>,
}

impl HashPlan {
    /// Hash every payload of the input.
    pub fn all() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    fn wants_archive(&self, label: &str) -> bool {
        self.enabled
            && self
                .archives
                .as_ref()
                .is_none_or(|l| l.iter().any(|x| x.eq_ignore_ascii_case(label)))
    }

    fn wants_did(&self, did: u32) -> bool {
        self.prefix.is_none_or(|p| (did >> 24) as u8 == p)
    }
}

/// Builds the index of an opened input. Payloads selected by `plan` are read, inflated and
/// hashed (slow: hashing a whole install reads every record). Records outside the plan
/// carry no hash and do not count as unreadable.
pub fn index_input(input: &Input, source: &str, plan: &HashPlan) -> Index {
    let archives = input.archives();
    let walks = par_map(&archives, |(_, a)| walk(a));
    // Work units of up to HASH_CHUNK selected records across all archives, so one huge
    // archive does not serialise the run: (archive, indexes into its walk).
    let mut units: Vec<(usize, Vec<usize>)> = Vec::new();
    for (ai, ((label, _), (entries, _))) in archives.iter().zip(&walks).enumerate() {
        if !plan.wants_archive(label) {
            continue;
        }
        let mut cur = Vec::new();
        for (k, e) in entries.iter().enumerate() {
            if plan.wants_did(e.did) {
                cur.push(k);
                if cur.len() == HASH_CHUNK {
                    units.push((ai, std::mem::take(&mut cur)));
                }
            }
        }
        if !cur.is_empty() {
            units.push((ai, cur));
        }
    }
    let done = par_map(&units, |(ai, ks)| {
        let a = archives[*ai].1;
        ks.iter()
            .map(|&k| {
                let entry = &walks[*ai].0[k];
                match a.read_entry(entry) {
                    Ok(p) => Some(content_hash(&p)),
                    Err(err) => {
                        tracing::warn!(
                            archive = %archives[*ai].0,
                            did = format_args!("{:#010x}", entry.did),
                            error = %err,
                            "cannot read payload for hashing"
                        );
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
    });
    let mut recs: Vec<Vec<Rec>> = walks
        .iter()
        .map(|(entries, _)| entries.iter().map(rec_from_entry).collect())
        .collect();
    let mut unreadable = vec![0u64; archives.len()];
    for ((ai, ks), hs) in units.iter().zip(done) {
        for (&k, h) in ks.iter().zip(hs) {
            match h {
                Some(h) => recs[*ai][k].hash = Some(h),
                None => unreadable[*ai] += 1,
            }
        }
    }
    let mut out = Vec::with_capacity(archives.len());
    for (((label, _), (_, err)), (recs, unreadable)) in archives
        .iter()
        .zip(walks)
        .zip(recs.into_iter().zip(unreadable))
    {
        if let Some(e) = &err {
            tracing::warn!(archive = %label, error = %e, "directory walk stopped early");
        }
        out.push(ArchiveIndex {
            label: label.clone(),
            recs,
            complete: err.is_none(),
            walk_error: err,
            unreadable,
        });
    }
    Index {
        source: source.to_owned(),
        source_kind: match input {
            Input::Archive { .. } => SourceKind::Archive,
            Input::Set(_) => SourceKind::Dir,
        },
        created: now_secs(),
        tool: format!("tessera-dattool {}", env!("CARGO_PKG_VERSION")),
        hashed: plan.enabled,
        archives: out,
    }
}

/// Reads `path` as a snapshot if it is a file starting with [`MAGIC`]; `Ok(None)` otherwise.
/// The file is streamed through the decoder, never read into memory whole.
pub fn load_if_snapshot(path: &Path) -> Result<Option<Index>> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut f = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut magic = [0u8; 8];
    let mut got = 0;
    while got < magic.len() {
        match f.read(&mut magic[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }
    if got < magic.len() || &magic != MAGIC {
        return Ok(None);
    }
    f.seek(SeekFrom::Start(0))
        .with_context(|| format!("cannot read {}", path.display()))?;
    let index = decode_from(BufReader::with_capacity(1 << 16, f))
        .with_context(|| format!("cannot load {}", path.display()))?;
    Ok(Some(index))
}

// ---------------------------------------------------------------------------------------
// `snapshot` subcommand

/// Options of `snapshot`.
#[derive(Debug, Clone)]
pub struct Opts {
    /// Install directory or archive.
    pub target: String,
    /// Output file.
    pub output: PathBuf,
    /// Hash every payload.
    pub content: bool,
    /// Overwrite an existing output.
    pub force: bool,
}

/// `snapshot <dir|archive> -o FILE [--content] [--force]`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let protected = protected_dirs(ctx, &input);
    // Refuse before the (possibly long) indexing; write_output checks again.
    check_output_path(&o.output, &protected)?;

    let source = match &input {
        Input::Archive { archive, .. } => archive
            .path()
            .and_then(|p| std::path::absolute(p).ok())
            .map_or_else(|| o.target.clone(), |p| p.display().to_string()),
        Input::Set(set) => std::path::absolute(set.dir())
            .map_or_else(|_| o.target.clone(), |p| p.display().to_string()),
    };
    let plan = if o.content {
        HashPlan::all()
    } else {
        HashPlan::default()
    };
    let index = index_input(&input, &source, &plan);
    let bytes = encode(&index).context("cannot encode the snapshot")?;
    write_output(&o.output, &bytes, o.force, &protected)?;

    let incomplete: Vec<&str> = index
        .archives
        .iter()
        .filter(|a| !a.complete)
        .map(|a| a.label.as_str())
        .collect();
    let unreadable: u64 = index.archives.iter().map(|a| a.unreadable).sum();
    if out.json {
        out.json(&json!({
            "output": o.output.display().to_string(),
            "source": index.source,
            "source_kind": index.source_kind.name(),
            "created": index.created,
            "archives": index.archives.len(),
            "records": index.records(),
            "hashes": index.hashed,
            "unreadable": unreadable,
            "incomplete": incomplete,
            "bytes": bytes.len(),
        }))?;
    } else {
        writeln!(
            out.w,
            "wrote {}: {} archives, {} records{}, {} ({})",
            o.output.display(),
            index.archives.len(),
            index.records(),
            if index.hashed { ", content hashes" } else { "" },
            human_bytes(bytes.len() as u64),
            format_time(u32::try_from(index.created).unwrap_or(u32::MAX)),
        )?;
        if unreadable > 0 {
            writeln!(
                out.w,
                "(!) {unreadable} payloads could not be read for hashing"
            )?;
        }
        for a in index.archives.iter().filter(|a| !a.complete) {
            writeln!(
                out.w,
                "(!) {}: directory walk stopped early: {}",
                a.label,
                a.walk_error.as_deref().unwrap_or("record limit reached")
            )?;
        }
    }
    Ok(if incomplete.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
