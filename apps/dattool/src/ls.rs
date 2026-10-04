// SPDX-License-Identifier: GPL-3.0-or-later
//! `ls`: stream the directory entries of one archive.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result};
use serde_json::json;
use tessera_formats::Did;

use crate::Out;
use crate::input::{Ctx, open_archive};
use crate::util::{did_hex, format_time};

/// `ls <archive> [--prefix 0xHH] [--from DID] [--limit N]`.
pub fn run(
    out: &mut Out,
    ctx: &Ctx,
    archive: &str,
    prefix: Option<u8>,
    from: Option<u32>,
    limit: Option<usize>,
) -> Result<ExitCode> {
    let (label, a) = open_archive(archive, ctx)?;
    let (lo, hi) = match prefix {
        Some(p) => (u32::from(p) << 24, (u32::from(p) << 24) | 0x00FF_FFFF),
        None => (0, u32::MAX),
    };
    let lo = from.map_or(lo, |f| f.max(lo));
    let limit = limit.unwrap_or(usize::MAX);
    let w = &mut out.w;
    if out.json {
        writeln!(w, "[")?;
    } else {
        writeln!(
            w,
            "{:<10}  {:<26}  {:>10}  {:>1}  {:>10}  {:>5}  TIME (UTC)",
            "DID", "TYPE", "STORED", "Z", "ITERATION", "STAMP"
        )?;
    }
    let mut n = 0usize;
    let mut bytes = 0u64;
    let mut walk_error = None;
    let mut truncated = false;
    if lo <= hi {
        for e in a.entries_range(lo..=hi) {
            if n >= limit {
                // Cut off only if this really was another entry.
                truncated = e.is_ok();
                break;
            }
            let e = match e {
                Ok(e) => e,
                Err(err) => {
                    walk_error = Some(err);
                    break;
                }
            };
            if out.json {
                if n > 0 {
                    writeln!(w, ",")?;
                }
                serde_json::to_writer(
                    &mut *w,
                    &json!({
                        "did": did_hex(e.did),
                        "type": Did(e.did).kind().name(),
                        "stored": e.length,
                        "compressed": e.is_compressed(),
                        "flags": e.flags,
                        "iteration": e.iteration,
                        "stamp": e.stamp,
                        "time": e.time,
                        "offset": e.offset,
                        "alloc_size": e.alloc_size,
                    }),
                )?;
            } else {
                writeln!(
                    w,
                    "{:<10}  {:<26}  {:>10}  {:>1}  {:>10}  {:>5}  {}",
                    did_hex(e.did),
                    Did(e.did).kind().name(),
                    e.length,
                    if e.is_compressed() { "z" } else { "" },
                    e.iteration,
                    e.stamp,
                    format_time(e.time)
                )?;
            }
            n += 1;
            bytes += u64::from(e.length);
        }
    }
    if out.json {
        writeln!(w, "\n]")?;
    } else {
        writeln!(
            w,
            "-- {n} entries, {bytes} stored bytes ({label}){}",
            if truncated { ", limit reached" } else { "" }
        )?;
    }
    if let Some(e) = walk_error {
        return Err(e).context(format!("directory walk of {label} failed"));
    }
    Ok(ExitCode::SUCCESS)
}
