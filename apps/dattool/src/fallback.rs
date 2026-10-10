// SPDX-License-Identifier: GPL-3.0-or-later
//! `fallback <dir> [DID...]`: why surfaces end up without their base-colour texture (the
//! white or untextured look), through `tessera_material::diagnose` (`material.md` §9–§10,
//! `texture-list.md` §5.2, `water.md` §8.2).
//!
//! Each DID may be a surface record (0x31), a mesh (0x06: every surface of its header,
//! `material.md` §9.1) or a cell geometry record (0x01: the surfaces of its embedded render
//! mesh, `collision.md` §5.7). `--all` diagnoses every surface record of the install and
//! prints counts by reason and material kind. Base textures are decoded unless
//! `--no-decode` is given. The command reads only; it prints DIDs, reasons and counts, never
//! record content.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tessera_formats::collision::CellGeometry;
use tessera_formats::{Did, DidKind};
use tessera_material::diagnose::SurfaceDiagnosis;
use tessera_material::{FallbackReason, MaterialKind, MaterialLibrary};
use tessera_mesh::{DecodeOptions, Mesh};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::{Align, Table, did_hex, par_map};
use crate::world::Schema;

/// The texture cap the walker uses by default (`walker.md` §4, `--texture-cap`).
const TEXTURE_CAP: u32 = 512;

/// Options of `fallback`.
#[derive(Debug)]
pub struct Opts {
    /// `<dir>`.
    pub target: String,
    /// DIDs to diagnose (0x31, 0x06, 0x01).
    pub dids: Vec<u32>,
    /// Diagnose every surface record.
    pub all: bool,
    /// Decode the base textures.
    pub decode: bool,
    /// DIDs listed per reason with `--all` (0 = none).
    pub top: usize,
    /// Only water, mirror and liquid-distortion surfaces (`material.md` §11.6) in lists.
    pub water: bool,
}

/// Every surface DID a mesh or cell names, in submesh order (with repeats removed).
fn surfaces_of(input: &Input, schema: &Schema, did: u32) -> Result<Vec<Did>> {
    let found = input.require(did)?;
    let payload = found.read()?;
    let stamp = Some(found.entry.stamp);
    let options = DecodeOptions::render_only();
    let mesh = match Did(did).kind() {
        DidKind::Mesh => Mesh::decode_expecting(&payload, Did(did), stamp, &options)
            .with_context(|| format!("{} does not decode as a mesh", did_hex(did)))?,
        _ if Did(did).prefix() == 0x01 => {
            let cell = CellGeometry::parse_expecting(&payload, Did(did), stamp, schema.get())
                .with_context(|| format!("{} does not decode as a cell", did_hex(did)))?;
            Mesh::decode(&cell.render_mesh_record(), stamp, &options)
                .with_context(|| format!("{}: embedded render mesh", did_hex(did)))?
        }
        _ => bail!(
            "{} is not a surface record (0x31), mesh (0x06) or cell (0x01)",
            did_hex(did)
        ),
    };
    let mut out: Vec<Did> = Vec::new();
    for s in mesh.surfaces {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    Ok(out)
}

fn reason_name(d: &SurfaceDiagnosis) -> &'static str {
    d.fallback.map_or("textured", FallbackReason::name)
}

fn diag_json(d: &SurfaceDiagnosis) -> Value {
    json!({
        "surface": did_hex(d.surface.0),
        "shader": d.shader.map(|s| did_hex(s.0)),
        "materials": d.materials.iter().map(|m| did_hex(m.0)).collect::<Vec<_>>(),
        "skipped_materials": d.skipped_materials,
        "kind": d.kind.name(),
        "fallback": d.fallback.map(FallbackReason::name),
        "base_texture": d.base_texture.map(|t| did_hex(t.0)),
        "tint": d.tint,
        "translucent": d.translucent,
        "animated": d.animated,
        "uv_transform": d.uv_transform,
        "lit": d.lit,
        "slots": d.slots.iter().map(|s| json!({
            "id": s.id,
            "role": s.role,
            "texture": s.texture.map(|t| did_hex(t.0)),
        })).collect::<Vec<_>>(),
        "error": d.error,
    })
}

fn flags(d: &SurfaceDiagnosis) -> String {
    let mut f = Vec::new();
    if d.translucent {
        f.push("translucent");
    }
    if d.animated {
        f.push("animated");
    }
    if d.uv_transform {
        f.push("uv-transform");
    }
    if d.lit {
        f.push("lit");
    }
    f.join(",")
}

fn slots_text(d: &SurfaceDiagnosis) -> String {
    d.slots
        .iter()
        .map(|s| {
            format!(
                "{}{}{}",
                s.id,
                s.role.as_deref().map_or(String::new(), |r| format!(":{r}")),
                if s.texture.is_some() { "" } else { "(0)" }
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs `fallback`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let Input::Set(set) = &input else {
        bail!("`fallback` needs an install directory (the material chain spans archives)");
    };
    if o.dids.is_empty() && !o.all {
        bail!("give surface, mesh or cell DIDs, or --all");
    }
    let schema = Schema::load(&input, ctx)?;
    let lib = MaterialLibrary::new(set);
    let cap = Some(TEXTURE_CAP);

    // Explicit DIDs: (requested DID, its surfaces).
    let mut asked: Vec<(u32, Vec<Did>)> = Vec::new();
    for &did in &o.dids {
        if Did(did).prefix() == 0x31 {
            asked.push((did, vec![Did(did)]));
        } else {
            asked.push((did, surfaces_of(&input, &schema, did)?));
        }
    }
    if !asked.is_empty() {
        let mut docs = Vec::new();
        let mut table = Table::new(&[
            ("from", Align::Left),
            ("surface", Align::Left),
            ("shader", Align::Left),
            ("kind", Align::Left),
            ("result", Align::Left),
            ("base", Align::Left),
            ("flags", Align::Left),
            ("slots", Align::Left),
        ]);
        for (from, surfaces) in &asked {
            for &s in surfaces {
                let d = lib.diagnose_surface(s, cap, o.decode);
                if out.json {
                    let mut j = diag_json(&d);
                    j["from"] = Value::String(did_hex(*from));
                    docs.push(j);
                } else {
                    table.row(vec![
                        did_hex(*from),
                        did_hex(s.0),
                        d.shader.map_or_else(|| "-".into(), |s| did_hex(s.0)),
                        d.kind.name().into(),
                        reason_name(&d).into(),
                        d.base_texture.map_or_else(|| "-".into(), |t| did_hex(t.0)),
                        flags(&d),
                        slots_text(&d),
                    ]);
                }
            }
        }
        if out.json && !o.all {
            out.json(&Value::Array(docs))?;
            return Ok(ExitCode::SUCCESS);
        }
        if !out.json {
            table.write(&mut out.w)?;
        }
    }
    if !o.all {
        return Ok(ExitCode::SUCCESS);
    }

    // Every surface record.
    let mut dids: Vec<u32> = Vec::new();
    for m in set.members() {
        for e in m.archive.entries_range(0x3100_0000..=0x31FF_FFFF) {
            let e = e.context("directory walk")?;
            dids.push(e.did);
        }
    }
    dids.sort_unstable();
    dids.dedup();
    let diags = par_map(&dids, |&d| lib.diagnose_surface(Did(d), cap, o.decode));
    let mut by: BTreeMap<(&'static str, MaterialKind), Vec<&SurfaceDiagnosis>> = BTreeMap::new();
    for d in &diags {
        by.entry((reason_name(d), d.kind)).or_default().push(d);
    }
    let fallbacks = diags.iter().filter(|d| d.fallback.is_some()).count();
    if out.json {
        let rows: Vec<Value> = by
            .iter()
            .map(|((reason, kind), v)| {
                let listed: Vec<Value> = v
                    .iter()
                    .filter(|d| !o.water || d.kind.needs_own_program())
                    .take(o.top)
                    .map(|d| diag_json(d))
                    .collect();
                json!({ "result": reason, "kind": kind.name(), "surfaces": v.len(), "listed": listed })
            })
            .collect();
        out.json(&json!({
            "surface_records": diags.len(),
            "fallbacks": fallbacks,
            "decoded_textures": o.decode,
            "groups": rows,
        }))?;
        return Ok(ExitCode::SUCCESS);
    }
    writeln!(
        out.w,
        "{} surface records, {fallbacks} without a usable base texture{}",
        diags.len(),
        if o.decode {
            " (base textures decoded)"
        } else {
            " (textures not decoded)"
        }
    )?;
    let mut t = Table::new(&[
        ("result", Align::Left),
        ("kind", Align::Left),
        ("surfaces", Align::Right),
        ("translucent", Align::Right),
        ("animated", Align::Right),
        ("uv-transform", Align::Right),
    ]);
    for ((reason, kind), v) in &by {
        t.row(vec![
            (*reason).into(),
            kind.name().into(),
            v.len().to_string(),
            v.iter().filter(|d| d.translucent).count().to_string(),
            v.iter().filter(|d| d.animated).count().to_string(),
            v.iter().filter(|d| d.uv_transform).count().to_string(),
        ]);
    }
    t.write(&mut out.w)?;
    // The shader records of each own-program kind (`material.md` §11.6), as recognised.
    let mut shaders: BTreeMap<MaterialKind, std::collections::BTreeSet<u32>> = BTreeMap::new();
    for d in diags.iter().filter(|d| d.kind.needs_own_program()) {
        if let Some(s) = d.shader {
            shaders.entry(d.kind).or_default().insert(s.0);
        }
    }
    for (kind, set) in &shaders {
        let list: Vec<String> = set.iter().map(|&s| did_hex(s)).collect();
        writeln!(out.w, "{} records: {}", kind.name(), list.join(" "))?;
    }
    if o.top > 0 {
        let mut l = Table::new(&[
            ("result", Align::Left),
            ("surface", Align::Left),
            ("shader", Align::Left),
            ("kind", Align::Left),
            ("flags", Align::Left),
            ("slots", Align::Left),
        ]);
        for ((reason, _), v) in by.iter().filter(|((r, _), _)| *r != "textured") {
            for d in v
                .iter()
                .filter(|d| !o.water || d.kind.needs_own_program())
                .take(o.top)
            {
                l.row(vec![
                    (*reason).into(),
                    did_hex(d.surface.0),
                    d.shader.map_or_else(|| "-".into(), |s| did_hex(s.0)),
                    d.kind.name().into(),
                    flags(d),
                    slots_text(d),
                ]);
            }
        }
        writeln!(out.w)?;
        l.write(&mut out.w)?;
    }
    Ok(ExitCode::SUCCESS)
}
