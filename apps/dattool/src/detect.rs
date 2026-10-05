// SPDX-License-Identifier: GPL-3.0-or-later
//! `detect`: install discovery plus fingerprints.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use serde_json::{Value, json};
use tessera_dat::Archive;
use tessera_install::{
    ContentEntry, ContentFingerprint, DID_INSTALL_STAMP, DID_ITERATION, DID_SET_MEMBERSHIP,
    GameInstall, GameType, InstallStamp, InstallWideStamp, IterationRecord, ProjectVersion,
    SearchEnv, SetMembership, Source, SplitSet, SplitSetStatus, discover_with, stamp_hex,
    verify_split_set,
};

use crate::Out;
use crate::input::Ctx;
use crate::util::{human_bytes, opt_path_json, par_map, path_json};

fn source_text(s: &Source) -> String {
    match s {
        Source::Explicit => "explicit --game-dir".into(),
        Source::Environment => format!("${}", tessera_install::GAME_DIR_ENV),
        Source::Steam { library } => format!("Steam library {}", library.display()),
        Source::LauncherConfig { config } => format!("launcher config {}", config.display()),
        Source::Standalone => "standalone default path".into(),
        Source::WinePrefix { prefix } => format!("Wine prefix {}", prefix.display()),
        Source::HomeScan => "home directory scan".into(),
    }
}

fn game_type_text(g: &GameType) -> String {
    match g {
        GameType::Compatible => "compatible".into(),
        GameType::Unsupported(n) => format!("unsupported ({n})"),
        GameType::Unknown => "unknown".into(),
    }
}

fn project_version_text(p: &ProjectVersion) -> String {
    match p {
        ProjectVersion::Missing => "missing".into(),
        ProjectVersion::Malformed => "malformed".into(),
        ProjectVersion::Value(v) => v.to_string(),
    }
}

/// Reads one housekeeping record and decodes it; any failure is logged and gives `None`.
fn record<T>(
    a: &Archive,
    name: &str,
    did: u32,
    parse: impl Fn(&[u8]) -> Result<T, tessera_install::RecordError>,
) -> Option<T> {
    let entry = match a.lookup(did) {
        Ok(Some(e)) => e,
        Ok(None) => return None,
        Err(e) => {
            tracing::debug!(archive = name, did, error = %e, "lookup failed");
            return None;
        }
    };
    let bytes = a
        .read_entry(&entry)
        .map_err(|e| tracing::debug!(archive = name, did, error = %e, "read failed"))
        .ok()?;
    parse(&bytes)
        .map_err(|e| tracing::debug!(archive = name, did, error = %e, "record malformed"))
        .ok()
}

fn content_entry(path: &Path) -> ContentEntry {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match Archive::open(path) {
        Ok(a) => ContentEntry {
            iteration: record(&a, &file_name, DID_ITERATION, IterationRecord::parse),
            membership: record(&a, &file_name, DID_SET_MEMBERSHIP, SetMembership::parse),
            install_stamp: record(&a, &file_name, DID_INSTALL_STAMP, InstallStamp::parse),
            file_name,
        },
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "cannot open archive");
            ContentEntry {
                file_name,
                iteration: None,
                membership: None,
                install_stamp: None,
            }
        }
    }
}

struct Report {
    content: Option<(ContentFingerprint, Vec<ContentEntry>)>,
    split_sets: Vec<SplitSet>,
}

fn analyse(inst: &GameInstall, with_content: bool) -> Report {
    if !with_content {
        return Report {
            content: None,
            split_sets: inst.split_sets.clone(),
        };
    }
    let ok: Vec<_> = inst.archives.iter().filter(|a| a.status.is_ok()).collect();
    let entries = par_map(&ok, |a| content_entry(&a.path));
    let fp = ContentFingerprint::compute(&entries);
    let split_sets = inst
        .split_sets
        .iter()
        .map(|s| {
            let records: Vec<(u32, Option<SetMembership>)> = s
                .members
                .iter()
                .map(|&n| {
                    let m = ok
                        .iter()
                        .zip(&entries)
                        .find(|(a, _)| a.kind == s.kind && a.aux_index == n)
                        .and_then(|(_, e)| e.membership);
                    (n, m)
                })
                .collect();
            verify_split_set(s.kind.clone(), &s.members, &records)
        })
        .collect();
    Report {
        content: Some((fp, entries)),
        split_sets,
    }
}

fn split_text(s: &SplitSet) -> String {
    let members: Vec<String> = s.members.iter().map(u32::to_string).collect();
    let status = match &s.status {
        SplitSetStatus::Complete => "complete".to_owned(),
        SplitSetStatus::Incomplete { missing } => format!("INCOMPLETE, missing {missing:?}"),
        SplitSetStatus::Unverified => "unverified".to_owned(),
    };
    let stale = if s.stale.is_empty() {
        String::new()
    } else {
        format!(", stale aux {:?}", s.stale)
    };
    format!("{} parts [{}] {status}{stale}", s.kind, members.join(", "))
}

fn stamp_text(s: &InstallWideStamp) -> String {
    match s {
        InstallWideStamp::Uniform(a, b) => format!("{} / {}", stamp_hex(a), stamp_hex(b)),
        InstallWideStamp::Mixed => "mixed".into(),
        InstallWideStamp::Absent => "absent".into(),
    }
}

/// Runs discovery and prints only the default install's root directory (JSON: `{"root":
/// ...}`), so a shell can export it as `TESSERA_GAME_DIR`.
pub fn run_path(out: &mut Out, ctx: &Ctx) -> Result<ExitCode> {
    let env = SearchEnv::from_process();
    let disc = discover_with(&env, ctx.game_dir.as_deref())?;
    let Some(inst) = disc.default_install() else {
        eprintln!(
            "No usable install found. Pass --game-dir or set {}.",
            tessera_install::GAME_DIR_ENV
        );
        return Ok(ExitCode::FAILURE);
    };
    if out.json {
        out.json(&json!({ "root": path_json(&inst.root) }))?;
    } else {
        writeln!(out.w, "{}", inst.root.display())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Runs discovery and prints every install.
pub fn run(out: &mut Out, ctx: &Ctx, with_content: bool) -> Result<ExitCode> {
    let env = SearchEnv::from_process();
    let disc = discover_with(&env, ctx.game_dir.as_deref())?;
    let reports: Vec<Report> = disc
        .installs
        .iter()
        .map(|i| analyse(i, with_content))
        .collect();

    if out.json {
        let installs: Vec<Value> = disc
            .installs
            .iter()
            .zip(&reports)
            .map(|(i, r)| {
                let schema = i.schema_fingerprint();
                json!({
                    "root": path_json(&i.root),
                    "sources": i.sources.iter().map(source_text).collect::<Vec<_>>(),
                    "game_type": game_type_text(&i.game_type),
                    "project_version": project_version_text(&i.project_version),
                    "languages": i.languages,
                    "client_dir": opt_path_json(i.client_dir.as_deref()),
                    "prefix": opt_path_json(i.prefix.as_deref()),
                    "steam": i.steam.as_ref().map(|s| json!({
                        "library": path_json(&s.library),
                        "buildid": s.buildid,
                        "proton_version": s.proton_version,
                    })),
                    "archives": i.archives.iter().map(|a| json!({
                        "file": a.file_name(),
                        "kind": a.kind.to_string(),
                        "aux_index": a.aux_index,
                        "size": a.size,
                        "ok": a.status.is_ok(),
                    })).collect::<Vec<_>>(),
                    "incomplete": i.incomplete,
                    "warnings": i.warnings.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "split_sets": r.split_sets.iter().map(split_text).collect::<Vec<_>>(),
                    "schema_fingerprint": schema.sha256,
                    "content_fingerprint": r.content.as_ref().map(|(fp, _)| fp.sha256.clone()),
                    "install_stamp": r.content.as_ref().map(|(fp, _)| stamp_text(&fp.install_stamp)),
                })
            })
            .collect();
        let rejected: Vec<Value> = disc
            .rejected
            .iter()
            .map(|r| {
                json!({
                    "path": path_json(&r.path),
                    "sources": r.sources.iter().map(source_text).collect::<Vec<_>>(),
                    "problems": r.problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
                })
            })
            .collect();
        out.json(&json!({ "installs": installs, "rejected": rejected }))?;
    } else {
        let w = &mut out.w;
        if disc.installs.is_empty() {
            writeln!(
                w,
                "No usable install found. Pass --game-dir or set {}.",
                tessera_install::GAME_DIR_ENV
            )?;
        }
        for (n, (i, r)) in disc.installs.iter().zip(&reports).enumerate() {
            let total: u64 = i.archives.iter().map(|a| a.size).sum();
            let ok = i.archives.iter().filter(|a| a.status.is_ok()).count();
            let schema = i.schema_fingerprint();
            writeln!(
                w,
                "Install #{}{}: {}",
                n + 1,
                if n == 0 { " (default)" } else { "" },
                i.root.display()
            )?;
            let mut kv = |k: &str, v: String| writeln!(w, "  {k:<20} {v}");
            kv(
                "found via",
                i.sources
                    .iter()
                    .map(source_text)
                    .collect::<Vec<_>>()
                    .join("; "),
            )?;
            kv("game type", game_type_text(&i.game_type))?;
            kv("project version", project_version_text(&i.project_version))?;
            kv("languages", i.languages.join(", "))?;
            kv(
                "64-bit client dir",
                i.client_dir
                    .as_ref()
                    .map_or("-".into(), |p| p.display().to_string()),
            )?;
            if let Some(p) = &i.prefix {
                kv("wine prefix", p.display().to_string())?;
            }
            if let Some(s) = &i.steam {
                kv(
                    "steam",
                    format!(
                        "build {} proton {}",
                        s.buildid.map_or("-".into(), |b| b.to_string()),
                        s.proton_version.as_deref().unwrap_or("-")
                    ),
                )?;
            }
            kv(
                "archives",
                format!(
                    "{} files ({ok} ok), {}",
                    i.archives.len(),
                    human_bytes(total)
                ),
            )?;
            kv("complete", if i.incomplete { "NO" } else { "yes" }.into())?;
            for s in r.split_sets.iter().filter(|s| s.members.len() > 1) {
                kv("split set", split_text(s))?;
            }
            kv(
                "schema fingerprint",
                format!("{} (header fields)", schema.short()),
            )?;
            if let Some((fp, entries)) = &r.content {
                let with_iter = entries.iter().filter(|e| e.iteration.is_some()).count();
                kv(
                    "content fingerprint",
                    format!(
                        "{} (housekeeping records of {with_iter}/{} archives)",
                        fp.short(),
                        entries.len()
                    ),
                )?;
                kv("install stamp", stamp_text(&fp.install_stamp))?;
            }
            for warn in &i.warnings {
                writeln!(w, "  warning: {warn}")?;
            }
            writeln!(w)?;
        }
        for r in &disc.rejected {
            writeln!(w, "Rejected: {}", r.path.display())?;
            for p in &r.problems {
                writeln!(w, "  - {p}")?;
            }
        }
    }
    Ok(if disc.installs.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
