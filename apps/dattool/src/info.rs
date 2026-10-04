// SPDX-License-Identifier: GPL-3.0-or-later
//! `info` (header fields and size statistics) and `stat` (DID prefix histogram).

use std::io::Write;
use std::process::ExitCode;

use anyhow::Result;
use serde_json::{Value, json};
use tessera_dat::{Archive, Header};
use tessera_formats::{Did, DidKind};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::{Align, Table, did_hex, format_time, hex_bytes, human_bytes, par_map};

/// Per-prefix bucket of the histogram.
#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
    count: u64,
    bytes: u64,
}

/// What one walk over an archive's directory gathers.
#[derive(Debug, Clone)]
struct Walk {
    entries: u64,
    metadata: u64,
    compressed: u64,
    stored: u64,
    min: u32,
    max: u32,
    max_did: u32,
    oldest: u32,
    newest: u32,
    max_iteration: u32,
    hist: Vec<Bucket>,
    /// The directory walk stopped early with this error.
    error: Option<String>,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            entries: 0,
            metadata: 0,
            compressed: 0,
            stored: 0,
            min: u32::MAX,
            max: 0,
            max_did: 0,
            oldest: u32::MAX,
            newest: 0,
            max_iteration: 0,
            hist: vec![Bucket::default(); 256],
            error: None,
        }
    }
}

fn walk(a: &Archive) -> Walk {
    let mut w = Walk::default();
    for e in a.entries() {
        let e = match e {
            Ok(e) => e,
            Err(err) => {
                w.error = Some(err.to_string());
                break;
            }
        };
        w.entries += 1;
        if e.is_metadata() {
            w.metadata += 1;
        }
        if e.is_compressed() {
            w.compressed += 1;
        }
        w.stored += u64::from(e.length);
        w.min = w.min.min(e.length);
        if e.length >= w.max {
            w.max = e.length;
            w.max_did = e.did;
        }
        if e.time != 0 {
            w.oldest = w.oldest.min(e.time);
            w.newest = w.newest.max(e.time);
        }
        w.max_iteration = w.max_iteration.max(e.iteration);
        let b = &mut w.hist[usize::from(e.did_high_byte())];
        b.count += 1;
        b.bytes += u64::from(e.length);
    }
    if w.entries == 0 {
        w.min = 0;
    }
    if w.oldest == u32::MAX {
        w.oldest = 0;
    }
    w
}

fn kind_name(prefix: u8) -> &'static str {
    match prefix {
        0xFF => "archive metadata",
        0x80 => "landblock family",
        p => DidKind::of(Did(u32::from(p) << 24)).name(),
    }
}

fn header_json(h: &Header) -> Value {
    json!({
        "version": h.version,
        "min_alloc": h.min_alloc,
        "file_size": h.file_size,
        "dataset": h.dataset,
        "dataset_type": h.dataset_type(),
        "archive_kind_code": h.archive_kind_code(),
        "subset": h.subset,
        "free_table_offset": h.free_table_offset,
        "free_table_size": h.free_table_size,
        "free_count": h.free_count,
        "free_bytes": h.free_bytes,
        "root_offset": h.root_offset,
        "root_size": h.root_size,
        "master_map_did": h.master_map_did,
        "engine_version": h.engine_version,
        "game_data_version": h.game_data_version,
        "identifier": hex_bytes(&h.identifier),
        "minor_version": h.minor_version,
        "superblock_len": h.superblock_len,
        "entry_size": h.entry_size,
        "pointer_size": h.pointer_size,
        "journal_empty": h.journal_is_empty(),
    })
}

fn walk_json(w: &Walk) -> Value {
    json!({
        "entries": w.entries,
        "metadata_entries": w.metadata,
        "compressed": w.compressed,
        "stored_bytes": w.stored,
        "min_stored": w.min,
        "max_stored": w.max,
        "max_stored_did": did_hex(w.max_did),
        "oldest_time": w.oldest,
        "newest_time": w.newest,
        "max_iteration": w.max_iteration,
        "walk_error": w.error,
    })
}

fn pct(part: u64, whole: u64) -> String {
    if whole == 0 {
        "-".into()
    } else {
        format!("{:.1}%", part as f64 * 100.0 / whole as f64)
    }
}

fn info_one(out: &mut Out, label: &str, a: &Archive) -> Result<()> {
    let h = a.header();
    let w = walk(a);
    let membership = a.membership().ok().flatten();
    if out.json {
        return out.json(&json!({
            "archive": label,
            "path": a.path(),
            "file_len": a.file_len(),
            "header": header_json(h),
            "membership": membership.map(|m| json!({
                "set_id": hex_bytes(&m.set_id),
                "part_index": m.part_index,
                "part_count": m.part_count,
            })),
            "stats": walk_json(&w),
        }));
    }
    let o = &mut out.w;
    writeln!(
        o,
        "Archive {label}: {}",
        a.path()
            .map_or("<memory>".into(), |p| p.display().to_string())
    )?;
    let mut kv = |k: &str, v: String| writeln!(o, "  {k:<22} {v}");
    kv(
        "file size",
        format!("{} ({} bytes)", human_bytes(a.file_len()), a.file_len()),
    )?;
    let size_note = if u64::from(h.file_size) == a.file_len() {
        "matches"
    } else {
        "DIFFERS"
    };
    kv("header file size", format!("{} ({size_note})", h.file_size))?;
    kv(
        "version string",
        if h.version.is_empty() {
            "(empty)".into()
        } else {
            h.version.clone()
        },
    )?;
    kv(
        "dataset",
        format!(
            "{:#010x} (type {}, kind {}), subset {}",
            h.dataset,
            h.dataset_type(),
            h.archive_kind_code(),
            h.subset
        ),
    )?;
    kv("engine version", h.engine_version.to_string())?;
    kv("game data version", h.game_data_version.to_string())?;
    kv("minor version", h.minor_version.to_string())?;
    kv("identifier", hex_bytes(&h.identifier))?;
    if h.master_map_did != 0 {
        kv("master map DID", did_hex(h.master_map_did))?;
    }
    kv("min allocation", h.min_alloc.to_string())?;
    kv(
        "layout sizes",
        format!(
            "superblock {} / entry {} / pointer {}",
            h.superblock_len, h.entry_size, h.pointer_size
        ),
    )?;
    kv(
        "root node",
        format!("{:#x} (+{:#x})", h.root_offset, h.root_size),
    )?;
    kv(
        "free space",
        format!(
            "{} extents, {} in a {:#x}-byte table at {:#x}",
            h.free_count,
            human_bytes(u64::from(h.free_bytes)),
            h.free_table_size,
            h.free_table_offset
        ),
    )?;
    kv(
        "journal",
        if h.journal_is_empty() {
            "empty"
        } else {
            "NOT EMPTY"
        }
        .into(),
    )?;
    if let Some(m) = membership {
        kv(
            "set membership",
            format!(
                "part {} of {} (set {})",
                m.part_index,
                m.part_count,
                hex_bytes(&m.set_id)
            ),
        )?;
    }
    kv(
        "entries",
        format!("{} ({} metadata)", w.entries, w.metadata),
    )?;
    kv(
        "compressed",
        format!("{} ({})", w.compressed, pct(w.compressed, w.entries)),
    )?;
    kv(
        "stored bytes",
        format!("{} ({} bytes)", human_bytes(w.stored), w.stored),
    )?;
    if let Some(mean) = w.stored.checked_div(w.entries) {
        kv(
            "record size",
            format!(
                "min {} / mean {mean} / max {} ({})",
                w.min,
                w.max,
                did_hex(w.max_did)
            ),
        )?;
        kv(
            "timestamps",
            format!("{} .. {} UTC", format_time(w.oldest), format_time(w.newest)),
        )?;
        kv("max iteration", w.max_iteration.to_string())?;
    }
    if let Some(e) = &w.error {
        kv("WALK ERROR", e.clone())?;
    }
    Ok(())
}

/// `info <archive|dir>`.
pub fn info(out: &mut Out, ctx: &Ctx, target: &str) -> Result<ExitCode> {
    let input = Input::open(target, ctx)?;
    let archives = input.archives();
    if let Input::Archive { label, archive } = &input {
        info_one(out, label, archive)?;
        return Ok(ExitCode::SUCCESS);
    }
    let walks = par_map(&archives, |(_, a)| walk(a));
    if out.json {
        let v: Vec<Value> = archives
            .iter()
            .zip(&walks)
            .map(|((label, a), w)| {
                json!({
                    "archive": label,
                    "path": a.path(),
                    "file_len": a.file_len(),
                    "header": header_json(a.header()),
                    "stats": walk_json(w),
                })
            })
            .collect();
        out.json(&json!({ "dir": input.home_dir(), "archives": v }))?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut t = Table::new(&[
        ("ARCHIVE", Align::Left),
        ("FILE", Align::Right),
        ("ENTRIES", Align::Right),
        ("COMPR", Align::Right),
        ("STORED", Align::Right),
        ("MAX REC", Align::Right),
        ("DATASET", Align::Left),
        ("SUB", Align::Right),
        ("ENG", Align::Right),
        ("DATA VER", Align::Right),
        ("NEWEST (UTC)", Align::Left),
    ]);
    let (mut tf, mut te, mut ts) = (0u64, 0u64, 0u64);
    for ((label, a), w) in archives.iter().zip(&walks) {
        let h = a.header();
        tf += a.file_len();
        te += w.entries;
        ts += w.stored;
        t.row(vec![
            if w.error.is_some() {
                format!("{label} (!)")
            } else {
                label.clone()
            },
            human_bytes(a.file_len()),
            w.entries.to_string(),
            pct(w.compressed, w.entries),
            human_bytes(w.stored),
            human_bytes(u64::from(w.max)),
            format!("{:#010x}", h.dataset),
            h.subset.to_string(),
            h.engine_version.to_string(),
            h.game_data_version.to_string(),
            format_time(w.newest),
        ]);
    }
    t.row(vec![
        format!("total ({})", archives.len()),
        human_bytes(tf),
        te.to_string(),
        String::new(),
        human_bytes(ts),
    ]);
    if let Some(d) = input.home_dir() {
        writeln!(out.w, "Directory {}", d.display())?;
    }
    t.write(&mut out.w)?;
    for ((label, _), w) in archives.iter().zip(&walks) {
        if let Some(e) = &w.error {
            writeln!(out.w, "(!) {label}: directory walk stopped: {e}")?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn hist_table(hist: &[Bucket], total: u64) -> Table {
    let mut t = Table::new(&[
        ("PREFIX", Align::Left),
        ("TYPE", Align::Left),
        ("COUNT", Align::Right),
        ("SHARE", Align::Right),
        ("STORED", Align::Right),
    ]);
    for (p, b) in hist.iter().enumerate() {
        if b.count == 0 {
            continue;
        }
        t.row(vec![
            format!("0x{p:02X}"),
            kind_name(p as u8).to_owned(),
            b.count.to_string(),
            pct(b.count, total),
            human_bytes(b.bytes),
        ]);
    }
    t
}

fn hist_json(hist: &[Bucket]) -> Value {
    Value::Array(
        hist.iter()
            .enumerate()
            .filter(|(_, b)| b.count > 0)
            .map(|(p, b)| {
                json!({
                    "prefix": format!("0x{p:02X}"),
                    "type": kind_name(p as u8),
                    "count": b.count,
                    "stored_bytes": b.bytes,
                })
            })
            .collect(),
    )
}

/// `stat <dir|archive>`.
pub fn stat(out: &mut Out, ctx: &Ctx, target: &str, totals_only: bool) -> Result<ExitCode> {
    let input = Input::open(target, ctx)?;
    let archives = input.archives();
    let walks = par_map(&archives, |(_, a)| walk(a));
    let mut total = vec![Bucket::default(); 256];
    let mut holders: Vec<Vec<&str>> = vec![Vec::new(); 256];
    for ((label, _), w) in archives.iter().zip(&walks) {
        for (p, b) in w.hist.iter().enumerate() {
            if b.count > 0 {
                total[p].count += b.count;
                total[p].bytes += b.bytes;
                holders[p].push(label);
            }
        }
    }
    let grand: u64 = walks.iter().map(|w| w.entries).sum();
    if out.json {
        let per: Vec<Value> = archives
            .iter()
            .zip(&walks)
            .map(|((label, _), w)| {
                json!({
                    "archive": label,
                    "entries": w.entries,
                    "histogram": hist_json(&w.hist),
                    "walk_error": w.error,
                })
            })
            .collect();
        out.json(&json!({
            "archives": if totals_only { Value::Null } else { Value::Array(per) },
            "totals": hist_json(&total),
            "entries": grand,
        }))?;
        return Ok(ExitCode::SUCCESS);
    }
    if !totals_only {
        for ((label, _), w) in archives.iter().zip(&walks) {
            writeln!(
                out.w,
                "== {label}: {} entries, {} stored",
                w.entries,
                human_bytes(w.stored)
            )?;
            hist_table(&w.hist, w.entries).write(&mut out.w)?;
            if let Some(e) = &w.error {
                writeln!(out.w, "(!) directory walk stopped: {e}")?;
            }
            writeln!(out.w)?;
        }
    }
    if archives.len() > 1 || totals_only {
        writeln!(out.w, "== all {} archives: {grand} entries", archives.len())?;
        let mut t = Table::new(&[
            ("PREFIX", Align::Left),
            ("TYPE", Align::Left),
            ("COUNT", Align::Right),
            ("SHARE", Align::Right),
            ("STORED", Align::Right),
            ("ARCHIVES", Align::Left),
        ]);
        for (p, b) in total.iter().enumerate() {
            if b.count == 0 {
                continue;
            }
            let mut names = holders[p].clone();
            let more = names.len().saturating_sub(4);
            names.truncate(4);
            let mut list = names.join(", ");
            if more > 0 {
                list.push_str(&format!(" +{more}"));
            }
            t.row(vec![
                format!("0x{p:02X}"),
                kind_name(p as u8).to_owned(),
                b.count.to_string(),
                pct(b.count, grand),
                human_bytes(b.bytes),
                list,
            ]);
        }
        t.write(&mut out.w)?;
    }
    Ok(ExitCode::SUCCESS)
}
