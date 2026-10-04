// SPDX-License-Identifier: GPL-3.0-or-later
//! `validate`: run the container validator over every archive.

use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use serde_json::{Value, json};
use tessera_dat::{Issue, Severity, ValidateOptions, ValidationReport};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::{Align, Table, human_bytes, par_map};

/// Options of `validate`.
#[derive(Debug, Clone)]
pub struct Opts {
    /// Inflate every compressed record.
    pub inflate: bool,
    /// Run the tiling check.
    pub tiling: bool,
    /// Issue lines printed per archive.
    pub max_issues: usize,
}

fn issue_json(i: &Issue) -> Value {
    json!({
        "invariant": format!("I-{}", i.invariant),
        "severity": match i.severity { Severity::Warning => "warning", Severity::Error => "error" },
        "offset": i.offset,
        "message": i.message,
    })
}

fn report_json(label: &str, r: &ValidationReport) -> Value {
    let s = &r.stats;
    json!({
        "archive": label,
        "path": r.path,
        "ok": r.is_ok(),
        "errors": r.errors(),
        "warnings": r.warnings(),
        "suppressed": r.suppressed,
        "counts": r.counts.iter().map(|(k, (w, e))| (format!("I-{k}"), json!({"warnings": w, "errors": e}))).collect::<serde_json::Map<_, _>>(),
        "issues": r.issues.iter().map(issue_json).collect::<Vec<_>>(),
        "stats": {
            "entries": s.entries,
            "metadata_entries": s.metadata_entries,
            "nodes": s.nodes,
            "max_depth": s.max_depth,
            "compressed": s.compressed,
            "fragmented": s.fragmented,
            "max_fragments": s.max_fragments,
            "legacy": s.legacy,
            "slack_records": s.slack_records,
            "largest_stored": s.largest_stored,
            "inflated": s.inflated,
            "free_extents": s.free_extents,
            "free_payload_bytes": s.free_payload_bytes,
            "tiling_gaps": s.tiling_gaps,
            "tiling_gap_bytes": s.tiling_gap_bytes,
            "tiling_overlaps": s.tiling_overlaps,
        },
    })
}

/// `validate <dir|archive>`.
pub fn run(out: &mut Out, ctx: &Ctx, target: &str, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(target, ctx)?;
    let opts = ValidateOptions {
        check_tiling: o.tiling,
        inflate_compressed: o.inflate,
        ..ValidateOptions::default()
    };
    let started = Instant::now();
    let (archives, set_issues, duplicates): (Vec<(String, ValidationReport)>, Vec<Issue>, u64) =
        match &input {
            Input::Set(set) => {
                let r = set.validate(&opts);
                let archives = r
                    .archives
                    .into_iter()
                    .map(|(id, rep)| (id.to_string(), rep))
                    .collect();
                (archives, r.issues, r.duplicate_dids)
            }
            Input::Archive { .. } => {
                let list = input.archives();
                let reps = par_map(&list, |(_, a)| a.validate_with(&opts));
                (
                    list.into_iter().map(|(l, _)| l).zip(reps).collect(),
                    Vec::new(),
                    0,
                )
            }
        };
    let elapsed = started.elapsed();
    let errors: u64 = archives.iter().map(|(_, r)| r.errors()).sum::<u64>()
        + set_issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count() as u64;
    let warnings: u64 = archives.iter().map(|(_, r)| r.warnings()).sum::<u64>()
        + set_issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count() as u64;
    let code = if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };

    if out.json {
        out.json(&json!({
            "ok": errors == 0,
            "errors": errors,
            "warnings": warnings,
            "duplicate_dids": duplicates,
            "seconds": elapsed.as_secs_f64(),
            "archives": archives.iter().map(|(l, r)| report_json(l, r)).collect::<Vec<_>>(),
            "set_issues": set_issues.iter().map(issue_json).collect::<Vec<_>>(),
        }))?;
        return Ok(code);
    }

    let mut t = Table::new(&[
        ("ARCHIVE", Align::Left),
        ("STATUS", Align::Left),
        ("ENTRIES", Align::Right),
        ("NODES", Align::Right),
        ("DEPTH", Align::Right),
        ("COMPR", Align::Right),
        ("FRAGM", Align::Right),
        ("LARGEST", Align::Right),
        ("FREE", Align::Right),
        ("GAPS", Align::Right),
        ("WARN", Align::Right),
        ("ERR", Align::Right),
    ]);
    for (label, r) in &archives {
        let s = &r.stats;
        t.row(vec![
            label.clone(),
            if r.is_ok() { "ok" } else { "ERRORS" }.into(),
            s.entries.to_string(),
            s.nodes.to_string(),
            (s.max_depth + 1).to_string(),
            s.compressed.to_string(),
            s.fragmented.to_string(),
            human_bytes(u64::from(s.largest_stored)),
            human_bytes(s.free_payload_bytes),
            if o.tiling {
                s.tiling_gaps.to_string()
            } else {
                "-".into()
            },
            r.warnings().to_string(),
            r.errors().to_string(),
        ]);
    }
    t.write(&mut out.w)?;

    for (label, r) in &archives {
        if r.issues.is_empty() {
            continue;
        }
        writeln!(out.w, "\n{label}:")?;
        let mut sorted: Vec<&Issue> = r.issues.iter().collect();
        sorted.sort_by_key(|i| std::cmp::Reverse(i.severity));
        for i in sorted.iter().take(o.max_issues) {
            writeln!(out.w, "  {i}")?;
        }
        let hidden = (r.issues.len().saturating_sub(o.max_issues)) as u64 + r.suppressed;
        if hidden > 0 {
            writeln!(out.w, "  ... {hidden} more")?;
        }
    }
    if !set_issues.is_empty() {
        writeln!(out.w, "\nset:")?;
        for i in set_issues.iter().take(o.max_issues) {
            writeln!(out.w, "  {i}")?;
        }
        if set_issues.len() > o.max_issues {
            writeln!(out.w, "  ... {} more", set_issues.len() - o.max_issues)?;
        }
    }
    let total_entries: u64 = archives.iter().map(|(_, r)| r.stats.entries).sum();
    writeln!(
        out.w,
        "\n{} archives, {total_entries} entries, {errors} errors, {warnings} warnings{} in {:.1}s: {}",
        archives.len(),
        if matches!(input, Input::Set(_)) {
            format!(", {duplicates} DIDs in more than one archive")
        } else {
            String::new()
        },
        elapsed.as_secs_f64(),
        if errors == 0 { "OK" } else { "FAILED" }
    )?;
    Ok(code)
}
