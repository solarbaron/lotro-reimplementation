// SPDX-License-Identifier: GPL-3.0-or-later
//! Record-level commands: `cat`, `extract` and `strings`.

use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io::{IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tessera_formats::{AnyRecord, Did, DidKind, Record, StringTable, StringTableEntry, Surface};

use crate::Out;
use crate::input::{Ctx, Found, Input};
use crate::util::{did_hex, format_time, hex_bytes, hexdump, human_bytes};

/// Options of `cat`.
#[derive(Debug)]
pub struct CatOpts {
    /// `<dir|archive>`.
    pub target: String,
    /// Record DID.
    pub did: u32,
    /// Binary output to stdout.
    pub raw: bool,
    /// Stored instead of inflated bytes.
    pub stored: bool,
    /// Hexdump limit (0 = all).
    pub max_bytes: usize,
}

/// What `extract` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractMode {
    /// The inflated payload.
    Payload,
    /// The stored on-disk bytes.
    Stored,
    /// A render surface converted to PNG.
    Png,
    /// A string table as text lines.
    Text,
}

/// Options of `extract`.
#[derive(Debug)]
pub struct ExtractOpts {
    /// `<dir|archive>`.
    pub target: String,
    /// Record DID.
    pub did: u32,
    /// Output file.
    pub output: PathBuf,
    /// Output kind.
    pub mode: ExtractMode,
    /// Overwrite an existing file.
    pub force: bool,
}

/// One-line description of a decoded record, if this crate set can decode it.
fn describe(did: u32, payload: &[u8]) -> Option<String> {
    if !AnyRecord::is_decodable(did) {
        return None;
    }
    Some(match AnyRecord::decode(did, payload) {
        Ok(Some(AnyRecord::Surface(s))) => format!(
            "render surface {}x{} {} (category {}), {} data bytes",
            s.width,
            s.height,
            s.format,
            s.category,
            s.data.len()
        ),
        Ok(Some(AnyRecord::StringTable(t))) => format!("string table, {} entries", t.len()),
        Ok(Some(AnyRecord::Sound(s))) => {
            format!(
                "sound record, {} audio bytes ({:?})",
                s.audio.len(),
                s.container()
            )
        }
        Ok(Some(r)) => r.type_name().to_owned(),
        Ok(None) => return None,
        Err(e) => format!("does not decode: {e}"),
    })
}

fn entry_json(f: &Found<'_>) -> Value {
    let e = &f.entry;
    json!({
        "did": did_hex(e.did),
        "type": Did(e.did).kind().name(),
        "archive": f.origin,
        "routed": f.routed,
        "flags": e.flags,
        "compressed": e.is_compressed(),
        "stored": e.length,
        "iteration": e.iteration,
        "stamp": e.stamp,
        "time": e.time,
        "offset": e.offset,
        "alloc_size": e.alloc_size,
    })
}

fn write_entry_summary(w: &mut dyn Write, f: &Found<'_>, payload_len: Option<usize>) -> Result<()> {
    let e = &f.entry;
    let routed = match f.routed {
        Some(true) => " (routed)",
        Some(false) => " (found by fallback search)",
        None => "",
    };
    writeln!(
        w,
        "{} [{}] in {}{routed}",
        did_hex(e.did),
        Did(e.did).kind().name(),
        f.origin
    )?;
    writeln!(
        w,
        "stored {} bytes{}{}, iteration {}, stamp {}, written {} UTC",
        e.length,
        if e.is_compressed() { " (zlib)" } else { "" },
        payload_len.map_or(String::new(), |n| format!(", payload {n} bytes")),
        e.iteration,
        e.stamp,
        format_time(e.time)
    )?;
    Ok(())
}

/// `cat <dir|archive> <did>`.
pub fn cat(out: &mut Out, ctx: &Ctx, o: &CatOpts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let found = input.require(o.did)?;
    let data = if o.stored {
        found.read_stored()?
    } else {
        found.read()?
    };
    if o.raw {
        if std::io::stdout().is_terminal() {
            bail!("refusing to write binary data to a terminal; redirect stdout or drop --raw");
        }
        out.w.write_all(&data)?;
        return Ok(ExitCode::SUCCESS);
    }
    let shown = if o.max_bytes == 0 {
        data.len()
    } else {
        data.len().min(o.max_bytes)
    };
    let decoded = if o.stored {
        None
    } else {
        describe(o.did, &data)
    };
    if out.json {
        let mut v = entry_json(&found);
        if let Some(m) = v.as_object_mut() {
            m.insert("payload_len".into(), json!(data.len()));
            m.insert("decoded".into(), json!(decoded));
            m.insert("hex".into(), json!(hex_bytes(&data[..shown])));
            m.insert("truncated".into(), json!(shown < data.len()));
        }
        out.json(&v)?;
        return Ok(ExitCode::SUCCESS);
    }
    write_entry_summary(&mut out.w, &found, (!o.stored).then_some(data.len()))?;
    if let Some(d) = decoded {
        writeln!(out.w, "decoded: {d}")?;
    }
    writeln!(out.w)?;
    hexdump(&mut out.w, &data[..shown], 0)?;
    if shown < data.len() {
        writeln!(
            out.w,
            "... {} more bytes (use --max-bytes 0 for all)",
            data.len() - shown
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Resolves `p` to an absolute path with symlinks of its existing part resolved, so that
/// it can be compared against a protected directory.
fn resolve_for_compare(p: &Path) -> Result<PathBuf> {
    let abs = std::path::absolute(p).with_context(|| format!("bad path {}", p.display()))?;
    let mut existing = abs.as_path();
    let mut rest: Vec<Component<'_>> = Vec::new();
    loop {
        if existing.exists() {
            break;
        }
        match (existing.parent(), existing.components().next_back()) {
            (Some(parent), Some(last)) => {
                rest.push(last);
                existing = parent;
            }
            _ => break,
        }
    }
    let mut out = fs::canonicalize(existing).unwrap_or_else(|_| existing.to_owned());
    for c in rest.into_iter().rev() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

/// Fails if `output` would land inside any of `protected`.
pub fn check_output_path(output: &Path, protected: &[PathBuf]) -> Result<()> {
    let target = resolve_for_compare(output)?;
    for dir in protected {
        let Ok(dir) = fs::canonicalize(dir) else {
            continue;
        };
        if target.starts_with(&dir) {
            bail!(
                "refusing to write {}: it is inside the game directory {} (the install is \
                 only ever read)",
                output.display(),
                dir.display()
            );
        }
    }
    Ok(())
}

fn surface_png(did: u32, payload: &[u8]) -> Result<Vec<u8>> {
    let s = Surface::parse_expecting(payload, did).context("not a decodable render surface")?;
    let img = s
        .to_rgba8()
        .with_context(|| format!("cannot convert the {} surface to RGBA", s.format))?;
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().context("PNG header")?;
        w.write_image_data(&img.pixels).context("PNG data")?;
        w.finish().context("PNG finish")?;
    }
    Ok(buf)
}

/// Text of a string-table entry with argument slots shown as `{name}` (or `{0xID}`).
pub fn entry_text(e: &StringTableEntry) -> String {
    let names = e.arg_names();
    e.render(|id| {
        let name = e
            .arg_ids
            .iter()
            .position(|&x| x == id)
            .and_then(|i| names.and_then(|n| n.get(i)))
            .map(|n| n.to_string_lossy());
        Some(Cow::Owned(match name {
            Some(n) => format!("{{{n}}}"),
            None => format!("{{{id:#x}}}"),
        }))
    })
}

fn escape_line(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o
}

fn table_text(t: &StringTable) -> Vec<u8> {
    let mut s = String::new();
    for e in t.iter() {
        s.push_str(&format!(
            "0x{:08x}\t{}\n",
            e.token,
            escape_line(&entry_text(e))
        ));
    }
    s.into_bytes()
}

fn require_kind(did: u32, want: DidKind, flag: &str) -> Result<()> {
    let k = Did(did).kind();
    if k != want {
        bail!(
            "{flag} needs a {} DID; {} is a {}",
            want.name(),
            did_hex(did),
            k.name()
        );
    }
    Ok(())
}

/// `extract <dir|archive> <did> -o <path>`.
pub fn extract(out: &mut Out, ctx: &Ctx, o: &ExtractOpts) -> Result<ExitCode> {
    match o.mode {
        ExtractMode::Png => require_kind(o.did, DidKind::RenderSurface, "--png")?,
        ExtractMode::Text => require_kind(o.did, DidKind::StringTable, "--text")?,
        _ => {}
    }
    let input = Input::open(&o.target, ctx)?;
    let mut protected: Vec<PathBuf> = input.home_dir().into_iter().collect();
    protected.extend(ctx.game_dir.clone());
    // Every install discovery knows about is off limits too, not just the one being read.
    match tessera_install::discover() {
        Ok(d) => protected.extend(d.installs.into_iter().map(|i| i.root)),
        Err(e) => tracing::debug!(error = %e, "install discovery failed"),
    }
    check_output_path(&o.output, &protected)?;

    let found = input.require(o.did)?;
    let bytes = match o.mode {
        ExtractMode::Stored => found.read_stored()?,
        ExtractMode::Payload => found.read()?,
        ExtractMode::Png => surface_png(o.did, &found.read()?)?,
        ExtractMode::Text => {
            let payload = found.read()?;
            let t = StringTable::parse_expecting(&payload, o.did)
                .context("not a decodable string table")?;
            table_text(&t)
        }
    };

    if let Some(parent) = o.output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let mut file: File = if o.force {
        File::create(&o.output)
    } else {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&o.output)
    }
    .with_context(|| {
        format!(
            "cannot create {}{}",
            o.output.display(),
            if o.force {
                ""
            } else {
                " (use --force to overwrite)"
            }
        )
    })?;
    file.write_all(&bytes)
        .and_then(|()| file.flush())
        .with_context(|| format!("cannot write {}", o.output.display()))?;

    let what = match o.mode {
        ExtractMode::Payload => "payload",
        ExtractMode::Stored => "stored bytes",
        ExtractMode::Png => "PNG",
        ExtractMode::Text => "text",
    };
    if out.json {
        let mut v = entry_json(&found);
        if let Some(m) = v.as_object_mut() {
            m.insert("output".into(), json!(o.output));
            m.insert("written".into(), json!(bytes.len()));
            m.insert("format".into(), json!(what));
        }
        out.json(&v)?;
    } else {
        writeln!(
            out.w,
            "wrote {what} of {} from {} to {} ({})",
            did_hex(o.did),
            found.origin,
            o.output.display(),
            human_bytes(bytes.len() as u64)
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

/// `strings <dir|archive> <did>`.
pub fn strings(
    out: &mut Out,
    ctx: &Ctx,
    target: &str,
    did: u32,
    token: Option<u32>,
) -> Result<ExitCode> {
    require_kind(did, DidKind::StringTable, "strings")?;
    let input = Input::open(target, ctx)?;
    let found = input.require(did)?;
    let payload = found.read()?;
    let t = StringTable::parse_expecting(&payload, did).context("not a decodable string table")?;
    let entries: Vec<&StringTableEntry> = match token {
        Some(tok) => t.get(tok).into_iter().collect(),
        None => t.iter().collect(),
    };
    if let Some(tok) = token
        && entries.is_empty()
    {
        bail!("token {tok:#x} is not in {}", did_hex(did));
    }
    if out.json {
        let v: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "token": format!("0x{:08x}", e.token),
                    "text": entry_text(e),
                    "pieces": e.pieces.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                    "arg_ids": e.arg_ids.iter().map(|a| format!("{a:#x}")).collect::<Vec<_>>(),
                    "arg_names": e.arg_names().map(|n| n.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>()),
                })
            })
            .collect();
        out.json(&json!({
            "did": did_hex(did),
            "archive": found.origin,
            "flag": t.flag,
            "entries": v,
        }))?;
        return Ok(ExitCode::SUCCESS);
    }
    writeln!(
        out.w,
        "{} in {}: {} entries",
        did_hex(did),
        found.origin,
        t.len()
    )?;
    for e in entries {
        writeln!(out.w, "0x{:08x}  {}", e.token, escape_line(&entry_text(e)))?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_inside_protected_dir_is_refused() {
        let base = std::env::temp_dir().join(format!("dattool-guard-{}", std::process::id()));
        let game = base.join("game");
        fs::create_dir_all(&game).unwrap();
        let prot = vec![game.clone()];
        assert!(check_output_path(&game.join("x.bin"), &prot).is_err());
        assert!(check_output_path(&game.join("new/dir/x.bin"), &prot).is_err());
        assert!(check_output_path(&base.join("out/../game/x.bin"), &prot).is_err());
        assert!(check_output_path(&base.join("out/x.bin"), &prot).is_ok());
        assert!(check_output_path(&base.join("gamex.bin"), &prot).is_ok());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn lines_escape() {
        assert_eq!(escape_line("a\nb\tc\\"), "a\\nb\\tc\\");
    }
}
