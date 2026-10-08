// SPDX-License-Identifier: GPL-3.0-or-later
//! `show <dir|archive> <did>`: decode one record and print its fields.
//!
//! Covers the world records of M4.5 (terrain type tables, terrain-scene and scene-choice
//! tables, scenes, water and fog-zone tables, the sky description, day descriptions, game
//! time, the LOD-class table, camera states, collision meshes and cell geometry), the region
//! descriptor's eight table slots, and the landblock family (trees, ceilings, light maps,
//! distant-landscape sectors).
//! Other decodable records get a one-line summary. Text output lists at most `--max-list`
//! rows per list. JSON output lists every row of the records with a field view
//! ([`has_field_view`]), except that cell geometry gives its property set as a count and its
//! embedded render mesh as a byte length, and landblock properties give their property set
//! as a count; landblock info, block maps and block-map descriptors are summarised as counts
//! in both outputs. Distant-landscape sectors (`distant-landscape.md`) show their header,
//! per-landblock heights, water and ceilings, the water list, the DID tail and the invariant
//! report, with sprites and entities listed (text: up to `--max-list`). Field names follow
//! the decoders in `tessera-formats`, which cite the spec section of every field.

use std::fmt::Write as _;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tessera_formats::collision::{CellGeometry, CollisionMesh, TreeCode};
use tessera_formats::environment::{
    CameraBody, CameraState, DayDescription, GameTime, ListRef, LodClassTable, SkyDescription,
    TargetBlock, WeightedList,
};
use tessera_formats::landblock::{
    CollisionTree, LandblockData, LandblockPids, LandblockProperties, LayoutRule, RegionDescriptor,
    SectorMesh, SectorRecord, SurfaceChunk,
};
use tessera_formats::scenery::{
    ChildRef, PlacementNode, Scene, SceneChild, SceneChoiceTable, SceneSite, TerrainSceneEntry,
    TerrainSceneTable,
};
use tessera_formats::terrain_type::{Argb, FloatCurve, TerrainType, TerrainTypeTable};
use tessera_formats::water::{FogZone, FogZoneTable, WaterTable, WaterType};
use tessera_formats::{AnyRecord, Did};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::read::{entry_json, write_entry_summary};
use crate::util::{Align, Table, did_hex};
use crate::world::{self, Decoded, Schema};

/// Options of `show`.
#[derive(Debug)]
pub struct Opts {
    /// `<dir|archive>`.
    pub target: String,
    /// Record DID.
    pub did: u32,
    /// Rows per list in text output (0 = all).
    pub max_list: usize,
}

/// `show <dir|archive> <did>`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let kind = Did(o.did).kind();
    if !world::is_decodable(o.did) {
        bail!(
            "{} ({kind}): `show` has no decoder for it (try `cat`)",
            did_hex(o.did)
        );
    }
    let input = Input::open(&o.target, ctx)?;
    let found = input.require(o.did)?;
    let payload = found.read()?;
    let schema = if world::needs_schema(o.did) {
        Schema::load(&input, ctx)?
    } else {
        Schema::none("not needed")
    };
    let rec = world::decode(o.did, &payload, Some(found.entry.stamp), schema.get())
        .with_context(|| {
            format!(
                "{} does not decode as a {} (property dictionary: {})",
                did_hex(o.did),
                kind.name(),
                schema.source
            )
        })?
        .with_context(|| format!("no decoder for {}", did_hex(o.did)))?;
    let view = View {
        input: &input,
        schema: &schema,
        max: if o.max_list == 0 {
            usize::MAX
        } else {
            o.max_list
        },
    };
    if out.json {
        let mut v = entry_json(&found);
        if let Some(m) = v.as_object_mut() {
            m.insert("payload_len".into(), json!(payload.len()));
            m.insert("record".into(), view.json(&rec)?);
        }
        out.json(&v)?;
        return Ok(ExitCode::SUCCESS);
    }
    write_entry_summary(&mut out.w, &found, Some(payload.len()))?;
    writeln!(out.w)?;
    let mut s = String::new();
    view.text(&rec, &mut s)?;
    out.w.write_all(s.as_bytes())?;
    Ok(ExitCode::SUCCESS)
}

/// Whether `show` prints the fields of `did`'s record, rather than a one-line summary
/// (counts, or only that it decodes).
pub fn has_field_view(did: u32) -> bool {
    use tessera_formats::{DidKind as K, LandblockKind as L};
    matches!(
        Did(did).kind(),
        K::TerrainTypeTable
            | K::TerrainSceneTable
            | K::SceneChoiceTable
            | K::Scene
            | K::WaterTable
            | K::FogZoneTable
            | K::SkyDescription
            | K::DayDescription
            | K::GameTime
            | K::LodClassTable
            | K::CameraState
            | K::CollisionMesh
            | K::CollisionGeometry
            | K::RegionDescriptor
            | K::Landblock(L::Heightmap | L::Properties | L::Other(5))
    )
}

/// Rendering context.
struct View<'a> {
    input: &'a Input,
    schema: &'a Schema,
    max: usize,
}

// ------------------------------------------------------------------- small formatters

fn hx(d: Did) -> String {
    did_hex(d.0)
}

fn jd(d: Did) -> Value {
    json!(did_hex(d.0))
}

fn jod(d: Option<Did>) -> Value {
    d.map_or(Value::Null, jd)
}

/// A DID that may be 0, shown as `-` when it is.
fn od(d: Did) -> String {
    if d.is_null() { "-".into() } else { hx(d) }
}

fn argb(c: Argb) -> String {
    format!("0x{:08X}", c.0)
}

fn v3(v: [f32; 3]) -> String {
    format!("({}, {}, {})", v[0], v[1], v[2])
}

fn flag(on: bool, c: char) -> char {
    if on { c } else { '-' }
}

fn rule(r: LayoutRule) -> &'static str {
    match r {
        LayoutRule::Stamp => "stamp",
        LayoutRule::Fallback => "fallback (no stamp)",
        LayoutRule::UnobservedStamp => "fallback (unobserved stamp)",
    }
}

fn curve_json(c: &FloatCurve) -> Value {
    match c {
        FloatCurve::Constant(v) => json!({"mode": 1, "constant": v}),
        FloatCurve::Waveform { mode, w } => json!({"mode": mode.get(), "w": w}),
        FloatCurve::Keyframed {
            w,
            period,
            looping,
            keys,
        } => json!({
            "mode": 10,
            "w": w,
            "period": period,
            "looping": looping,
            "keys": keys.iter().map(|k| json!({"value": k.value, "position": k.position})).collect::<Vec<_>>(),
        }),
    }
}

fn curve_text(c: &FloatCurve) -> String {
    match c {
        FloatCurve::Constant(v) => format!("{v}"),
        FloatCurve::Waveform { mode, w } => format!("wave{} [{}, {}, ...]", mode.get(), w[0], w[1]),
        FloatCurve::Keyframed {
            period,
            looping,
            keys,
            ..
        } => format!(
            "{} keys / {period} s{}",
            keys.len(),
            if *looping { " loop" } else { "" }
        ),
    }
}

/// Appends `... N more` when a list was cut.
fn more(s: &mut String, total: usize, shown: usize) -> std::fmt::Result {
    if shown < total {
        writeln!(s, "... {} more (use --max-list 0 for all)", total - shown)?;
    }
    Ok(())
}

fn table_into(s: &mut String, t: &Table) -> Result<()> {
    let mut buf = Vec::new();
    t.write(&mut buf)?;
    s.push_str(&String::from_utf8_lossy(&buf));
    Ok(())
}

fn tree_json(t: TreeCode<'_>, build_byte: Option<u8>, key_limit: Option<u64>) -> Value {
    let mut v = json!({
        "origin": t.origin,
        "scale": t.scale,
        "code_len": t.code.len(),
        "build_byte": build_byte,
    });
    if let Some(m) = v.as_object_mut() {
        match t.walk() {
            Ok(w) => {
                m.insert(
                    "walk".into(),
                    json!({
                        "keys": w.keys.len(),
                        "max_key": w.keys.last(),
                        "keys_in_range": key_limit.map(|l| w.keys_below(l)),
                        "keys_from_several_leaves": w.keys_from_several_leaves,
                        "states": w.states,
                        "instructions": w.counts.total(),
                    }),
                );
            }
            Err(e) => {
                m.insert("walk_error".into(), json!(e.to_string()));
            }
        }
    }
    v
}

fn tree_text(
    s: &mut String,
    label: &str,
    t: TreeCode<'_>,
    build_byte: Option<u8>,
    key_limit: Option<u64>,
) -> std::fmt::Result {
    write!(
        s,
        "{label}: origin {}, scale {}, {} code bytes, build byte {}",
        v3(t.origin),
        t.scale,
        t.code.len(),
        build_byte.map_or("absent".into(), |b| b.to_string())
    )?;
    match t.walk() {
        Ok(w) => {
            write!(
                s,
                "; walk: {} keys, {} instructions, {} states",
                w.keys.len(),
                w.counts.total(),
                w.states
            )?;
            if let Some(l) = key_limit
                && !w.keys_below(l)
            {
                write!(s, ", KEYS OUT OF RANGE (limit {l})")?;
            }
            writeln!(s)
        }
        Err(e) => writeln!(s, "; WALK FAILED: {e}"),
    }
}

// ------------------------------------------------------------------- dispatch

impl View<'_> {
    fn json(&self, rec: &Decoded<'_>) -> Result<Value> {
        Ok(match rec {
            Decoded::Any(a) => match a.as_ref() {
                AnyRecord::TerrainTypeTable(t) => terrain_types_json(t),
                AnyRecord::TerrainSceneTable(t) => terrain_scenes_json(t),
                AnyRecord::SceneChoiceTable(t) => scene_choices_json(t),
                AnyRecord::Scene(t) => scene_json(t),
                AnyRecord::WaterTable(t) => water_json(t),
                AnyRecord::FogZoneTable(t) => fog_json(t),
                AnyRecord::SkyDescription(t) => sky_json(t),
                AnyRecord::DayDescription(t) => day_json(t),
                AnyRecord::GameTime(t) => game_time_json(t),
                AnyRecord::LodClassTable(t) => lod_json(t),
                AnyRecord::CameraState { state, .. } => camera_json(state),
                AnyRecord::CollisionMesh { mesh, .. } => mesh_json(mesh),
                AnyRecord::CellGeometry(c) => cell_json(c),
                other => json!({"type": other.type_name(), "did": jd(other.did())}),
            },
            Decoded::Region(r) => self.region_json(r)?,
            Decoded::LandblockData(d) => landblock_data_json(d),
            Decoded::LandblockProperties(p) => self.properties_json(p),
            Decoded::BlockMapDescriptor(d) => json!({
                "type": "block-map descriptor",
                "did": jd(d.did),
                "entries": d.entries.len(),
            }),
            Decoded::BlockMap(b) => json!({
                "type": "block map",
                "did": jd(b.did),
                "entries": b.entries.len(),
            }),
            Decoded::LandblockInfo(i) => json!({
                "type": "landblock info",
                "did": jd(i.did),
                "cells": i.cells.as_ref().map(|c| c.items.len()),
                "static_entities": i.entities.len(),
                "links": i.links.len(),
                "weenies": i.weenies.items.len(),
                "layout_rule": rule(i.layout_rule),
            }),
            Decoded::LandblockGroup(g) => sector_json(g),
            Decoded::Material { did, name } => json!({"type": name, "did": jd(*did)}),
        })
    }

    fn text(&self, rec: &Decoded<'_>, s: &mut String) -> Result<()> {
        match rec {
            Decoded::Any(a) => match a.as_ref() {
                AnyRecord::TerrainTypeTable(t) => self.terrain_types_text(t, s)?,
                AnyRecord::TerrainSceneTable(t) => self.terrain_scenes_text(t, s)?,
                AnyRecord::SceneChoiceTable(t) => self.scene_choices_text(t, s)?,
                AnyRecord::Scene(t) => self.scene_text(t, s)?,
                AnyRecord::WaterTable(t) => self.water_text(t, s)?,
                AnyRecord::FogZoneTable(t) => self.fog_text(t, s)?,
                AnyRecord::SkyDescription(t) => self.sky_text(t, s)?,
                AnyRecord::DayDescription(t) => self.day_text(t, s)?,
                AnyRecord::GameTime(t) => self.game_time_text(t, s)?,
                AnyRecord::LodClassTable(t) => self.lod_text(t, s)?,
                AnyRecord::CameraState { state, .. } => camera_text(state, s)?,
                AnyRecord::CollisionMesh { mesh, .. } => mesh_text(mesh, "", s)?,
                AnyRecord::CellGeometry(c) => self.cell_text(c, s)?,
                AnyRecord::StringTable(t) => writeln!(
                    s,
                    "string table {}: {} entries (use `strings` for them)",
                    hx(tessera_formats::Record::did(t)),
                    t.len()
                )?,
                other => writeln!(
                    s,
                    "{} {} decodes; `show` has no field view for this kind (`cat` dumps its bytes)",
                    other.type_name(),
                    hx(other.did())
                )?,
            },
            Decoded::Region(r) => self.region_text(r, s)?,
            Decoded::LandblockData(d) => self.landblock_data_text(d, s)?,
            Decoded::LandblockProperties(p) => self.properties_text(p, s)?,
            Decoded::BlockMapDescriptor(d) => writeln!(
                s,
                "block-map descriptor {}: {} property entries",
                hx(d.did),
                d.entries.len()
            )?,
            Decoded::BlockMap(b) => {
                writeln!(s, "block map {}: {} entries", hx(b.did), b.entries.len())?;
            }
            Decoded::LandblockInfo(i) => writeln!(
                s,
                "landblock info {}: {} cells, {} static entities, {} links, {} weenies \
                 (layout: {})",
                hx(i.did),
                i.cells.as_ref().map_or(0, |c| c.items.len()),
                i.entities.len(),
                i.links.len(),
                i.weenies.items.len(),
                rule(i.layout_rule)
            )?,
            Decoded::LandblockGroup(g) => self.sector_text(g, s)?,
            Decoded::Material { did, name } => writeln!(
                s,
                "{name} {} decodes (the explorer shows the material family's fields)",
                hx(*did)
            )?,
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x19

fn terrain_type_json(e: &TerrainType) -> Value {
    json!({
        "key": e.key,
        "label": e.label,
        "draw_order": e.draw_order,
        "diffuse": jd(e.diffuse),
        "normal": jd(e.normal),
        "emissive": jd(e.emissive),
        "glow": jd(e.glow),
        "physics_material": jd(e.physics_material),
        "u_scale": e.u_scale,
        "v_scale": e.v_scale,
        "rotation": e.rotation,
        "u_offset": curve_json(&e.u_offset),
        "v_offset": curve_json(&e.v_offset),
        "projection": e.projection.raw(),
        "use_opacity_mask": e.use_opacity_mask,
        "use_specular": e.use_specular,
        "use_gloss_specular": e.use_gloss_specular,
        "specular_from_normal": e.specular_from_normal,
        "slope_orientation": e.slope_orientation,
        "alpha_test_threshold": e.alpha_test_threshold,
        "alpha_blend_max": e.alpha_blend_max,
        "mask_resolution": e.mask_resolution,
        "procedural": e.procedural,
        "diffuse_scale": argb(e.diffuse_scale),
        "glow_colour": argb(e.glow_colour),
        "specular_colour": argb(e.specular_colour),
        "specular_power": e.specular_power,
    })
}

fn terrain_types_json(t: &TerrainTypeTable) -> Value {
    json!({
        "type": "terrain type table",
        "did": jd(t.did),
        "entries": t.entries().iter().map(terrain_type_json).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn terrain_types_text(&self, t: &TerrainTypeTable, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "terrain type table {}: {} entries (by key; flags: O opacity mask, S specular, \
             G gloss, N specular from normal, L slope orientation, P procedural)",
            hx(t.did),
            t.len()
        )?;
        let mut tab = Table::new(&[
            ("KEY", Align::Right),
            ("ORDER", Align::Right),
            ("LABEL", Align::Left),
            ("DIFFUSE", Align::Left),
            ("NORMAL", Align::Left),
            ("PROJ", Align::Right),
            ("U×V SCALE", Align::Left),
            ("ROT", Align::Right),
            ("U/V OFFSET", Align::Left),
            ("FLAGS", Align::Left),
            ("MASK", Align::Right),
        ]);
        let mut shown = 0;
        for e in t.iter_by_key().take(self.max) {
            shown += 1;
            tab.row(vec![
                e.key.to_string(),
                e.draw_order.to_string(),
                e.label.clone(),
                od(e.diffuse),
                od(e.normal),
                e.projection.raw().to_string(),
                format!("{}×{}", e.u_scale, e.v_scale),
                e.rotation.to_string(),
                format!("{} / {}", curve_text(&e.u_offset), curve_text(&e.v_offset)),
                [
                    flag(e.use_opacity_mask, 'O'),
                    flag(e.use_specular, 'S'),
                    flag(e.use_gloss_specular, 'G'),
                    flag(e.specular_from_normal, 'N'),
                    flag(e.slope_orientation, 'L'),
                    flag(e.procedural, 'P'),
                ]
                .iter()
                .collect(),
                e.mask_resolution.to_string(),
            ]);
        }
        table_into(s, &tab)?;
        more(s, t.len(), shown)?;
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x12, 0x11

fn terrain_scene_entry_json(e: &TerrainSceneEntry) -> Value {
    json!({
        "name": e.name,
        "kind": e.kind,
        "id": e.id,
        "unknown_f": e.unknown_f,
        "default_pair": e.default_pair.map(|p| json!({"scene_key": p.scene_key, "slot": p.slot})),
        "pairs": e.pairs.iter().map(|p| json!({"scene_key": p.scene_key, "slot": p.slot})).collect::<Vec<_>>(),
    })
}

fn terrain_scenes_json(t: &TerrainSceneTable) -> Value {
    json!({
        "type": "terrain-scene table",
        "did": jd(t.did),
        "reserved": t.reserved,
        "default_entry": t.default_entry().map(terrain_scene_entry_json),
        "entries": t.entries().iter().map(terrain_scene_entry_json).collect::<Vec<_>>(),
    })
}

fn pairs_text(e: &TerrainSceneEntry, max: usize) -> String {
    let mut v: Vec<String> = e
        .pairs
        .iter()
        .take(max)
        .map(|p| format!("{}:{:#x}", p.slot, p.scene_key))
        .collect();
    if e.pairs.len() > max {
        v.push("...".into());
    }
    v.join(" ")
}

fn scene_choices_json(t: &SceneChoiceTable) -> Value {
    json!({
        "type": "scene-choice table",
        "did": jd(t.did),
        "reserved": t.reserved,
        "entries": t.entries().iter().map(|e| json!({
            "key": e.key,
            "total_weight": e.total_weight(),
            "choices": e.choices.iter().map(|c| json!({
                "scene": jd(c.scene),
                "weight": c.weight,
                "flag": c.flag,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn terrain_scenes_text(&self, t: &TerrainSceneTable, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "terrain-scene table {}: {} entries (pairs as slot:scene key)",
            hx(t.did),
            t.entries().len()
        )?;
        let mut tab = Table::new(&[
            ("ID", Align::Right),
            ("NAME", Align::Left),
            ("KIND", Align::Right),
            ("DEFAULT", Align::Left),
            ("PAIRS", Align::Right),
            ("SLOT:KEY", Align::Left),
        ]);
        let row = |e: &TerrainSceneEntry, id: String| {
            vec![
                id,
                e.name.clone(),
                e.kind.to_string(),
                e.default_pair
                    .map_or("-".into(), |p| format!("{}:{:#x}", p.slot, p.scene_key)),
                e.pairs.len().to_string(),
                pairs_text(e, 8),
            ]
        };
        if let Some(d) = t.default_entry() {
            tab.row(row(d, "default".into()));
        }
        let mut shown = 0;
        for e in t.entries().iter().take(self.max) {
            shown += 1;
            tab.row(row(e, e.id.to_string()));
        }
        table_into(s, &tab)?;
        more(s, t.entries().len(), shown)?;
        Ok(())
    }

    fn scene_choices_text(&self, t: &SceneChoiceTable, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "scene-choice table {}: {} entries",
            hx(t.did),
            t.entries().len()
        )?;
        let mut tab = Table::new(&[
            ("KEY", Align::Right),
            ("CHOICES", Align::Right),
            ("WEIGHT", Align::Right),
            ("SCENE(WEIGHT)", Align::Left),
        ]);
        let mut shown = 0;
        for e in t.entries().iter().take(self.max) {
            shown += 1;
            let mut list: Vec<String> = e
                .choices
                .iter()
                .take(6)
                .map(|c| format!("{}({})", od(c.scene), c.weight))
                .collect();
            if e.choices.len() > 6 {
                list.push("...".into());
            }
            tab.row(vec![
                format!("{:#x}", e.key),
                e.choices.len().to_string(),
                e.total_weight().to_string(),
                list.join(" "),
            ]);
        }
        table_into(s, &tab)?;
        more(s, t.entries().len(), shown)?;
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x02

fn range_json(r: tessera_formats::scenery::FloatRange) -> Value {
    json!([r.min, r.max])
}

fn child_json(c: &SceneChild) -> Value {
    match c {
        SceneChild::Object(d) => json!({"object": jd(*d)}),
        SceneChild::Scene(d) => json!({"scene": jd(*d)}),
        SceneChild::Node(n) => json!({"node": node_json(n)}),
    }
}

fn node_json(n: &PlacementNode) -> Value {
    // Two halves: one `json!` of every field exceeds the macro recursion limit.
    let mut a = json!({
        "disp_min": n.disp_min,
        "disp_max": n.disp_max,
        "heading": range_json(n.heading),
        "angle2": range_json(n.angle2),
        "angle3": range_json(n.angle3),
        "frequency": n.frequency,
        "scale_min": n.scale_min,
        "scale_max": n.scale_max,
        "stretch_ref": n.stretch_ref,
        "slope_max_cos": n.slope_max_cos,
        "slope_min_cos": n.slope_min_cos,
        "align_to_slope": n.align_to_slope,
        "face_direction": n.face_direction,
        "above_ground": range_json(n.above_ground),
        "above_water": range_json(n.above_water),
        "below_water": range_json(n.below_water),
        "water_depth": range_json(n.water_depth),
        "below_ground": range_json(n.below_ground()),
        "z_range": range_json(n.z_range),
        "spacing": n.spacing,
        "clearance": n.clearance,
        "clearance_scaled": n.clearance_scaled,
        "clear_max_cos": n.clear_max_cos,
        "clear_min_cos": n.clear_min_cos,
        "clear_min_dot": n.clear_min_dot,
        "clear_aspect_tol": n.clear_aspect_tol,
    });
    let b = json!({
        "max_objects": n.max_objects,
        "unknown_163": n.unknown_163,
        "disp_scaled": n.disp_scaled,
        "inherit_scale": n.inherit_scale,
        "disp_relative": n.disp_relative,
        "orient_relative": n.orient_relative,
        "spacing_scaled": n.spacing_scaled,
        "no_checks": n.no_checks,
        "spacing_3d": n.spacing_3d,
        "unknown_174": n.unknown_174,
        "no_spacing": n.no_spacing,
        "height_mode": n.height_mode,
        "scale_mode": n.scale_mode,
        "range_modes": [
            n.above_ground_mode,
            n.above_water_mode,
            n.below_water_mode,
            n.water_depth_mode,
            n.below_ground_mode,
        ],
        "children": n.children.iter().map(child_json).collect::<Vec<_>>(),
    });
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

fn scene_json(t: &Scene) -> Value {
    json!({
        "type": "scene",
        "did": jd(t.did),
        "reserved": t.reserved,
        "flag_a": t.flag_a,
        "objects": t.objects().count(),
        "scene_references": t.scene_references().count(),
        "nodes": t.nodes().count(),
        "frill_layer": t.frill_layer.as_ref().map(node_json),
        "groups": t.groups.iter().map(|g| json!({
            "group_id": g.group_id,
            "children": g.children.iter().map(child_json).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn node_line(n: &PlacementNode) -> String {
    format!(
        "node: frequency {}, disp {}..{}, scale {}..{}, heading {}..{}, spacing {}, \
         clearance {}, max objects {}, height mode {}, {} children",
        n.frequency,
        v3(n.disp_min),
        v3(n.disp_max),
        v3(n.scale_min),
        v3(n.scale_max),
        n.heading.min,
        n.heading.max,
        n.spacing,
        n.clearance,
        n.max_objects,
        n.height_mode,
        n.children.len()
    )
}

impl View<'_> {
    fn scene_text(&self, t: &Scene, s: &mut String) -> Result<()> {
        scene_text(t, self.max, s)?;
        Ok(())
    }
}

/// Prints up to `max - *shown` children of `site` from the front of `items` (which yields
/// [`Scene::descendants`] in order), consuming nothing once `*shown` reaches `max`.
fn site_children<'a, I: Iterator<Item = ChildRef<'a>>>(
    s: &mut String,
    items: &mut std::iter::Peekable<I>,
    site: SceneSite,
    max: usize,
    shown: &mut usize,
) -> std::fmt::Result {
    while *shown < max {
        let Some(c) = items.next_if(|c| c.site == site) else {
            break;
        };
        *shown += 1;
        let pad = "  ".repeat(c.depth);
        match c.child {
            SceneChild::Object(d) => writeln!(s, "{pad}object {}", hx(*d))?,
            SceneChild::Scene(d) => writeln!(s, "{pad}scene {}", hx(*d))?,
            SceneChild::Node(n) => writeln!(s, "{pad}{}", node_line(n))?,
        }
    }
    Ok(())
}

/// A scene as text: at most `max` group headers and `max` child lines.
///
/// One pass over [`Scene::descendants`], which yields the frill layer's children first and
/// then each group's in stored order, so the cost is linear in the record however many
/// groups it has; once `max` child lines are printed the walk stops.
fn scene_text(t: &Scene, max: usize, s: &mut String) -> std::fmt::Result {
    let total = t.descendants().count();
    writeln!(
        s,
        "scene {}: {} groups, frill layer {}, {} objects, {} scene references, {} \
         placement nodes (flag_a {}, reserved {})",
        hx(t.did),
        t.groups.len(),
        if t.frill_layer.is_some() { "yes" } else { "no" },
        t.objects().count(),
        t.scene_references().count(),
        t.nodes().count(),
        t.flag_a,
        t.reserved
    )?;
    let mut items = t.descendants().peekable();
    let mut shown = 0usize;
    if let Some(f) = &t.frill_layer {
        writeln!(s, "frill layer {}", node_line(f))?;
        site_children(s, &mut items, SceneSite::FrillLayer, max, &mut shown)?;
    }
    for (i, g) in t.groups.iter().enumerate().take(max) {
        writeln!(
            s,
            "group {i} (id {}): {} children",
            g.group_id,
            g.children.len()
        )?;
        site_children(s, &mut items, SceneSite::Group(i), max, &mut shown)?;
    }
    if t.groups.len() > max {
        writeln!(s, "... {} more groups", t.groups.len() - max)?;
    }
    more(s, total, shown.min(total))
}

// ------------------------------------------------------------------- 0x16, 0x17

fn water_type_json(e: &WaterType) -> Value {
    json!({
        "key": e.key,
        "wet": e.wet.map(|w| json!({
            "surface": jd(w.surface),
            "extra": w.extra.iter().map(|d| jd(*d)).collect::<Vec<_>>(),
            "volume_entity": jd(w.volume_entity),
            "value": w.value,
        })),
    })
}

fn water_json(t: &WaterTable) -> Value {
    json!({
        "type": "water table",
        "did": jd(t.did),
        "reserved": t.reserved,
        "level_key": t.level_key,
        "type_key": t.type_key,
        "default": t.default.as_ref().map(water_type_json),
        "entries": t.entries().iter().map(water_type_json).collect::<Vec<_>>(),
    })
}

fn fog_zone_json(z: &FogZone) -> Value {
    json!({
        "key": z.key,
        "active": z.active,
        "near": z.near,
        "far": z.far,
        "amount": z.amount,
        "value_60": z.value_60,
        "colour": argb(z.colour),
        "light_a": z.light_a,
        "colour_a1": argb(z.colour_a1),
        "colour_a2": argb(z.colour_a2),
        "light_b": z.light_b,
        "colour_b": argb(z.colour_b),
        "colour_c": argb(z.colour_c),
        "flag_c": z.flag_c,
        "colour_d": argb(z.colour_d),
        "e": z.e.map(|(d, v)| json!({"did": jd(d), "value": v})),
        "flag_f": z.flag_f,
        "value_g": z.value_g,
        "overrides": z.overrides.iter().map(|o| json!({"mode": o.mode, "a": o.a, "b": o.b})).collect::<Vec<_>>(),
    })
}

fn fog_json(t: &FogZoneTable) -> Value {
    json!({
        "type": "fog-zone table",
        "did": jd(t.did),
        "reserved": t.reserved,
        "zone_key": t.zone_key,
        "any_active": t.any_active(),
        "default": t.default.as_ref().map(fog_zone_json),
        "entries": t.entries().iter().map(fog_zone_json).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn water_text(&self, t: &WaterTable, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "water table {}: {} listed entries, level key {:#x}, type key {:#x}, default {}",
            hx(t.did),
            t.entries().len(),
            t.level_key,
            t.type_key,
            if t.default.is_some() { "yes" } else { "no" }
        )?;
        let mut tab = Table::new(&[
            ("KEY", Align::Right),
            ("STATE", Align::Left),
            ("SURFACE", Align::Left),
            ("VOLUME ENTITY", Align::Left),
            ("VALUE", Align::Right),
        ]);
        let row = |e: &WaterType, key: String| match &e.wet {
            None => vec![key, "dry".into()],
            Some(w) => vec![
                key,
                "wet".into(),
                od(w.surface),
                od(w.volume_entity),
                w.value.to_string(),
            ],
        };
        if let Some(d) = &t.default {
            tab.row(row(d, "default".into()));
        }
        let mut shown = 0;
        for e in t.entries().iter().take(self.max) {
            shown += 1;
            tab.row(row(e, e.key.to_string()));
        }
        table_into(s, &tab)?;
        more(s, t.entries().len(), shown)?;
        Ok(())
    }

    fn fog_text(&self, t: &FogZoneTable, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "fog-zone table {}: {} listed entries, zone key {:#x}, default {}, {}",
            hx(t.did),
            t.entries().len(),
            t.zone_key,
            if t.default.is_some() { "yes" } else { "no" },
            if t.any_active() {
                "some zones active"
            } else {
                "no zone active"
            }
        )?;
        let mut tab = Table::new(&[
            ("KEY", Align::Right),
            ("ACTIVE", Align::Right),
            ("NEAR", Align::Right),
            ("FAR", Align::Right),
            ("AMOUNT", Align::Right),
            ("COLOUR", Align::Left),
        ]);
        let row = |z: &FogZone, key: String| {
            vec![
                key,
                z.active.to_string(),
                z.near.to_string(),
                z.far.to_string(),
                z.amount.to_string(),
                argb(z.colour),
            ]
        };
        if let Some(d) = &t.default {
            tab.row(row(d, "default".into()));
        }
        let mut shown = 0;
        for e in t.entries().iter().take(self.max) {
            shown += 1;
            tab.row(row(e, e.key.to_string()));
        }
        table_into(s, &tab)?;
        more(s, t.entries().len(), shown)?;
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x15

fn weighted_json<T: Copy>(l: &WeightedList<T>, f: impl Fn(T) -> Value) -> Value {
    json!({
        "key": l.key,
        "total_weight": l.total_weight(),
        "pairs": l.pairs.iter().map(|p| json!({"value": f(p.value), "weight": p.weight})).collect::<Vec<_>>(),
    })
}

fn listref_json<T: Copy>(r: &ListRef<T>, f: impl Fn(T) -> Value) -> Value {
    match r {
        ListRef::Named(k) => json!({"named": k}),
        ListRef::Inline(l) => json!({"inline": weighted_json(l, f)}),
    }
}

fn listref_text<T: Copy>(r: &ListRef<T>, f: impl Fn(T) -> String) -> String {
    match r {
        ListRef::Named(k) => format!("named {k:#x}"),
        ListRef::Inline(l) => {
            let mut v: Vec<String> = l
                .pairs
                .iter()
                .take(4)
                .map(|p| format!("{}({})", f(p.value), p.weight))
                .collect();
            if l.pairs.len() > 4 {
                v.push(format!("... {} in all", l.pairs.len()));
            }
            if v.is_empty() {
                "empty".into()
            } else {
                v.join(" ")
            }
        }
    }
}

fn sky_json(t: &SkyDescription) -> Value {
    let day = |d: Did| jd(d);
    let num = |k: u32| json!(k);
    json!({
        "type": "sky description",
        "did": jd(t.did),
        "version": t.version,
        "light_interval": [t.light_interval_min, t.light_interval_max],
        "sky_interval": [t.sky_interval_min, t.sky_interval_max],
        "game_time": jd(t.game_time),
        "day_descriptions": t.day_descriptions().count(),
        "groups": t.groups.iter().map(|g| json!({
            "day_type": g.day_type,
            "chance": g.chance,
            "follow": listref_json(&g.follow, num),
            "entries": g.entries.iter().map(|e| json!({
                "sky_key": e.sky_key,
                "list": listref_json(&e.list, day),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "named_follow_lists": t.named_follow_lists.iter().map(|l| weighted_json(l, num)).collect::<Vec<_>>(),
        "named_sky_lists": t.named_sky_lists.iter().map(|l| weighted_json(l, day)).collect::<Vec<_>>(),
        "local_defaults": t.local_defaults.iter().map(|e| json!({
            "sky_key": e.sky_key,
            "list": listref_json(&e.list, day),
        })).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn sky_text(&self, t: &SkyDescription, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "sky description {} (version {}): game time {}, light interval {}..{} s, sky \
             interval {}..{} s",
            hx(t.did),
            t.version,
            od(t.game_time),
            t.light_interval_min,
            t.light_interval_max,
            t.sky_interval_min,
            t.sky_interval_max
        )?;
        writeln!(
            s,
            "{} day groups, {} named follow lists, {} named sky lists, {} local defaults, {} \
             day-description references",
            t.groups.len(),
            t.named_follow_lists.len(),
            t.named_sky_lists.len(),
            t.local_defaults.len(),
            t.day_descriptions().count()
        )?;
        let mut tab = Table::new(&[
            ("DAY TYPE", Align::Right),
            ("CHANCE", Align::Right),
            ("FOLLOW", Align::Left),
            ("SKY KEY", Align::Right),
            ("DAY DESCRIPTIONS(WEIGHT)", Align::Left),
        ]);
        let mut rows = 0usize;
        let mut total = 0usize;
        for g in &t.groups {
            let follow = listref_text(&g.follow, |k| format!("{k:#x}"));
            if g.entries.is_empty() {
                total += 1;
                if rows < self.max {
                    rows += 1;
                    tab.row(vec![
                        format!("{:#x}", g.day_type),
                        g.chance.to_string(),
                        follow.clone(),
                    ]);
                }
            }
            for e in &g.entries {
                total += 1;
                if rows < self.max {
                    rows += 1;
                    tab.row(vec![
                        format!("{:#x}", g.day_type),
                        g.chance.to_string(),
                        follow.clone(),
                        format!("{:#x}", e.sky_key),
                        listref_text(&e.list, od),
                    ]);
                }
            }
        }
        for e in &t.local_defaults {
            total += 1;
            if rows < self.max {
                rows += 1;
                tab.row(vec![
                    "local".into(),
                    String::new(),
                    String::new(),
                    format!("{:#x}", e.sky_key),
                    listref_text(&e.list, od),
                ]);
            }
        }
        table_into(s, &tab)?;
        more(s, total, rows)?;
        for l in t.named_sky_lists.iter().take(self.max) {
            writeln!(
                s,
                "named sky list {:#x}: {}",
                l.key,
                listref_text(&ListRef::Inline(l.clone()), od)
            )?;
        }
        for l in t.named_follow_lists.iter().take(self.max) {
            writeln!(
                s,
                "named follow list {:#x}: {}",
                l.key,
                listref_text(&ListRef::Inline(l.clone()), |k| format!("{k:#x}"))
            )?;
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x1A

fn day_json(t: &DayDescription) -> Value {
    json!({
        "type": "day description",
        "did": jd(t.did),
        "transition_seconds": t.transition_seconds,
        "unknown0": t.unknown0,
        "keyframes_ascending": t.keyframes_ascending(),
        "object_indices_unique": t.object_indices_unique(),
        "keyframes": t.keyframes.iter().map(|k| json!({
            "time": k.time,
            "fog_near": k.fog_near,
            "fog_far": k.fog_far,
            "fog_extra": k.fog_extra,
            "fog_colour": argb(k.fog_colour),
            "sun_brightness": k.sun_brightness,
            "sun_heading": k.sun_heading,
            "sun_pitch": k.sun_pitch,
            "sun_colour": argb(k.sun_colour),
            "sun_specular": argb(k.sun_specular),
            "ambient_brightness": k.ambient_brightness,
            "ambient_colour_1": argb(k.ambient_colour_1),
            "ambient_colour_2": argb(k.ambient_colour_2),
            "colour_3": argb(k.colour_3),
            "colour_4": argb(k.colour_4),
            "values_a": k.values_a,
            "value_b": k.value_b,
            "curves": k.curves.iter().map(curve_json).collect::<Vec<_>>(),
            "effect": k.effect.map(|e| json!({"object": jd(e.object), "extra": e.extra})),
            "object_entries": k.object_entries.iter().map(|o| json!({
                "object_index": o.object_index,
                "replacement": jd(o.replacement),
                "angle_a_offset": o.angle_a_offset,
                "angle_b_offset": o.angle_b_offset,
                "tint": argb(o.tint),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "objects": t.objects.iter().map(|o| json!({
            "index": o.index,
            "object": jd(o.object),
            "time": [o.begin_time, o.end_time],
            "angle_a": [o.angle_a_begin, o.angle_a_end],
            "angle_b": [o.angle_b_begin, o.angle_b_end],
            "reverse": o.reverse,
            "unknown_20": o.unknown_20,
            "flags": [o.flag_25, o.flag_26, o.flag_30],
            "value_28": o.value_28,
            "value_2c": o.value_2c,
            "flag_31": o.flag_31,
            "layers": {
                "value_a": o.layers.value_a,
                "value_b": o.layers.value_b,
                "flag": o.layers.flag,
                "value": o.layers.value,
                "layers": o.layers.layers.iter().map(|l| json!({
                    "key": l.key, "texture": jd(l.texture), "value": l.value,
                })).collect::<Vec<_>>(),
            },
        })).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn day_text(&self, t: &DayDescription, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "day description {}: {} keyframes{}, {} sky objects, transition {} s",
            hx(t.did),
            t.keyframes.len(),
            if t.keyframes_ascending() {
                ""
            } else {
                " (NOT ascending)"
            },
            t.objects.len(),
            t.transition_seconds
        )?;
        let mut tab = Table::new(&[
            ("TIME", Align::Right),
            ("FOG NEAR", Align::Right),
            ("FOG FAR", Align::Right),
            ("FOG COLOUR", Align::Left),
            ("SUN", Align::Right),
            ("HEADING", Align::Right),
            ("PITCH", Align::Right),
            ("SUN COLOUR", Align::Left),
            ("AMBIENT", Align::Right),
            ("AMBIENT COLOURS", Align::Left),
            ("OBJ", Align::Right),
        ]);
        let mut shown = 0;
        for k in t.keyframes.iter().take(self.max) {
            shown += 1;
            tab.row(vec![
                k.time.to_string(),
                k.fog_near.to_string(),
                k.fog_far.to_string(),
                argb(k.fog_colour),
                k.sun_brightness.to_string(),
                k.sun_heading.to_string(),
                k.sun_pitch.to_string(),
                argb(k.sun_colour),
                k.ambient_brightness.to_string(),
                format!("{} {}", argb(k.ambient_colour_1), argb(k.ambient_colour_2)),
                k.object_entries.len().to_string(),
            ]);
        }
        table_into(s, &tab)?;
        more(s, t.keyframes.len(), shown)?;
        if !t.objects.is_empty() {
            writeln!(s)?;
            let mut tab = Table::new(&[
                ("INDEX", Align::Right),
                ("OBJECT", Align::Left),
                ("TIME", Align::Left),
                ("ANGLE A", Align::Left),
                ("ANGLE B", Align::Left),
                ("REV", Align::Left),
                ("LAYERS", Align::Right),
            ]);
            let mut shown = 0;
            for o in t.objects.iter().take(self.max) {
                shown += 1;
                tab.row(vec![
                    o.index.to_string(),
                    od(o.object),
                    format!("{}..{}", o.begin_time, o.end_time),
                    format!("{}..{}", o.angle_a_begin, o.angle_a_end),
                    format!("{}..{}", o.angle_b_begin, o.angle_b_end),
                    if o.reverse { "yes" } else { "no" }.into(),
                    o.layers.layers.len().to_string(),
                ]);
            }
            table_into(s, &tab)?;
            more(s, t.objects.len(), shown)?;
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x35, 0x32

fn game_time_json(t: &GameTime) -> Value {
    json!({
        "type": "game time",
        "did": jd(t.did),
        "day_length": t.day_length,
        "days_per_year": t.days_per_year,
        "zero_offset": t.zero_offset,
        "zero_year": t.zero_year,
        "value_54": t.value_54,
        "value_58": t.value_58,
        "calendar_is_ordered": t.calendar_is_ordered(),
        "periods": t.periods.iter().map(|p| json!({"start": p.start, "name": p.name})).collect::<Vec<_>>(),
        "weekdays": t.weekdays,
        "months": t.months.iter().map(|m| json!({"start_day": m.start_day, "name": m.name})).collect::<Vec<_>>(),
        "unknown_last": t.unknown_last,
    })
}

fn lod_json(t: &LodClassTable) -> Value {
    json!({
        "type": "LOD-class table",
        "did": jd(t.did),
        "classes": t.entries.iter().map(|c| json!({
            "class": c.class,
            "values": c.values,
            "max_distance": c.max_distance(),
        })).collect::<Vec<_>>(),
    })
}

impl View<'_> {
    fn game_time_text(&self, t: &GameTime, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "game time {}: day length {} s, {} days per year, zero offset {}, zero year {}, \
             values {} / {}, last {}",
            hx(t.did),
            t.day_length,
            t.days_per_year,
            t.zero_offset,
            t.zero_year,
            t.value_54,
            t.value_58,
            t.unknown_last
        )?;
        writeln!(
            s,
            "{} day periods, {} weekdays, {} months{}",
            t.periods.len(),
            t.weekdays.len(),
            t.months.len(),
            if t.calendar_is_ordered() {
                ""
            } else {
                " (calendar NOT ordered)"
            }
        )?;
        for p in t.periods.iter().take(self.max) {
            writeln!(s, "  period at {}: name token {:#x}", p.start, p.name)?;
        }
        more(s, t.periods.len(), t.periods.len().min(self.max))?;
        for m in t.months.iter().take(self.max) {
            writeln!(
                s,
                "  month from day {}: name token {:#x}",
                m.start_day, m.name
            )?;
        }
        more(s, t.months.len(), t.months.len().min(self.max))?;
        if !t.weekdays.is_empty() {
            let w: Vec<String> = t.weekdays.iter().map(|w| format!("{w:#x}")).collect();
            writeln!(s, "  weekday tokens: {}", w.join(" "))?;
        }
        Ok(())
    }

    fn lod_text(&self, t: &LodClassTable, s: &mut String) -> Result<()> {
        writeln!(s, "LOD-class table {}: {} classes", hx(t.did), t.len())?;
        let mut tab = Table::new(&[
            ("CLASS", Align::Right),
            ("VALUES", Align::Left),
            ("MAX", Align::Right),
        ]);
        let mut shown = 0;
        for c in t.entries.iter().take(self.max) {
            shown += 1;
            let v: Vec<String> = c.values.iter().map(f32::to_string).collect();
            tab.row(vec![
                c.class.to_string(),
                v.join(" "),
                c.max_distance().to_string(),
            ]);
        }
        table_into(s, &tab)?;
        more(s, t.len(), shown)?;
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x08

fn target_json(t: &TargetBlock) -> Value {
    json!({
        "unresolved": t.unresolved,
        "holding_location": t.holding_location,
        "bone": t.bone,
        "offset": t.offset,
        "yaw_offset": t.yaw_offset,
        "pitch_offset": t.pitch_offset,
        "height_factor": t.height_factor,
        "height_factor_under_water": t.height_factor_under_water,
    })
}

fn camera_body_json(b: &CameraBody) -> Value {
    match b {
        CameraBody::ThirdPerson(b) => json!({
            "angle_smoothing": b.angle_smoothing,
            "zoom_smoothing": b.zoom_smoothing,
            "target_smoothing": b.target_smoothing,
            "unnamed": b.unnamed,
            "target": target_json(&b.target),
            "constrain_pitch": b.constrain_pitch,
            "pitch": [b.pitch_low, b.pitch_high],
            "constrain_yaw": b.constrain_yaw,
            "offset_range": b.offset_range,
            "yaw": [b.yaw_low, b.yaw_high],
            "mouse_look_relative": b.mouse_look_relative,
            "detached_mouse": b.detached_mouse,
            "distance": [b.min_distance, b.max_distance],
            "fov": b.fov,
            "initial_pitch": b.initial_pitch,
            "initial_yaw": b.initial_yaw,
            "initial_distance": b.initial_distance,
            "align_to_slope": b.align_to_slope,
            "min_target_height": b.min_target_height,
            "target_fade_distance": b.target_fade_distance,
            "target_fade_opacity": b.target_fade_opacity,
            "look_at_smoothing": b.look_at_smoothing,
        }),
        CameraBody::FirstPerson(b) => json!({
            "fov": b.fov,
            "constrain_pitch": b.constrain_pitch,
            "pitch": [b.pitch_low, b.pitch_high],
            "target": target_json(&b.target),
            "initial_yaw": b.initial_yaw,
            "look_from_smoothing": b.look_from_smoothing,
        }),
        CameraBody::Cinematic(b) => json!({
            "fov": b.fov,
            "target_smoothing": b.target_smoothing,
            "angle_smoothing": b.angle_smoothing,
        }),
        CameraBody::Flight(b) => json!({
            "fov": b.fov,
            "initial_yaw": b.initial_yaw,
            "constrain_pitch": b.constrain_pitch,
            "pitch": [b.pitch_low, b.pitch_high],
            "constrain_roll": b.constrain_roll,
            "roll": [b.roll_left, b.roll_right],
            "target": target_json(&b.target),
        }),
    }
}

fn camera_json(c: &CameraState) -> Value {
    json!({
        "type": "camera state",
        "entity": jd(c.entity),
        "collision_smoothing": c.collision_smoothing,
        "clip": [c.near_clip, c.far_clip],
        "uses_physics": c.uses_physics,
        "uses_water_physics": c.uses_water_physics,
        "follows_under_water": c.follows_under_water,
        "auto_transitions_disabled": c.auto_transitions_disabled,
        "controller_type": c.body.controller_type(),
        "body": camera_body_json(&c.body),
    })
}

fn camera_text(c: &CameraState, s: &mut String) -> Result<()> {
    let ty = match c.body {
        CameraBody::ThirdPerson(_) => "third person",
        CameraBody::FirstPerson(_) => "first person",
        CameraBody::Cinematic(_) => "cinematic",
        CameraBody::Flight(_) => "flight",
    };
    writeln!(
        s,
        "camera state: entity {}, controller {} ({ty}), fov {}, clip {}..{}, collision \
         smoothing {}",
        od(c.entity),
        c.body.controller_type(),
        c.body.fov(),
        c.near_clip,
        c.far_clip,
        c.collision_smoothing
    )?;
    writeln!(
        s,
        "physics {}, water physics {}, follows under water {}, auto transitions disabled {}",
        c.uses_physics, c.uses_water_physics, c.follows_under_water, c.auto_transitions_disabled
    )?;
    match &c.body {
        CameraBody::ThirdPerson(b) => writeln!(
            s,
            "distance {}..{} (initial {}), pitch {}..{} (constrained {}), initial pitch {}, \
             initial yaw {}",
            b.min_distance,
            b.max_distance,
            b.initial_distance,
            b.pitch_low,
            b.pitch_high,
            b.constrain_pitch,
            b.initial_pitch,
            b.initial_yaw
        )?,
        CameraBody::FirstPerson(b) => writeln!(
            s,
            "pitch {}..{} (constrained {}), initial yaw {}",
            b.pitch_low, b.pitch_high, b.constrain_pitch, b.initial_yaw
        )?,
        CameraBody::Cinematic(_) => {}
        CameraBody::Flight(b) => writeln!(
            s,
            "pitch {}..{} (constrained {}), roll {}..{} (constrained {})",
            b.pitch_low,
            b.pitch_high,
            b.constrain_pitch,
            b.roll_left,
            b.roll_right,
            b.constrain_roll
        )?,
    }
    if let Some(t) = c.body.target() {
        writeln!(
            s,
            "target: holding location {}, bone {}, offset {}, height factor {} ({} under water)",
            t.holding_location,
            t.bone,
            v3(t.offset),
            t.height_factor,
            t.height_factor_under_water
        )?;
    }
    Ok(())
}

// ------------------------------------------------------------------- 0x66, 0x01

fn mesh_json(m: &CollisionMesh) -> Value {
    let tri = u64::try_from(m.triangles.len()).unwrap_or(u64::MAX);
    json!({
        "type": "collision mesh",
        "bounds": m.bounds().map(|(lo, hi)| json!([lo, hi])),
        "layout_rule": rule(m.layout_rule),
        "tree": tree_json(m.tree_code(), m.tree.build_byte, Some(tri)),
        "vertices": m.vertices,
        "triangles": m.triangles,
    })
}

fn mesh_text(m: &CollisionMesh, indent: &str, s: &mut String) -> Result<()> {
    writeln!(
        s,
        "{indent}collision mesh: {} vertices, {} triangles, bounds {}, layout: {}",
        m.vertices.len(),
        m.triangles.len(),
        m.bounds()
            .map_or("-".into(), |(lo, hi)| format!("{}..{}", v3(lo), v3(hi))),
        rule(m.layout_rule)
    )?;
    let tri = u64::try_from(m.triangles.len()).unwrap_or(u64::MAX);
    tree_text(
        s,
        &format!("{indent}tree"),
        m.tree_code(),
        m.tree.build_byte,
        Some(tri),
    )?;
    Ok(())
}

fn cell_json(c: &CellGeometry) -> Value {
    let corners = |v: &[[f32; 3]]| json!(v);
    json!({
        "type": "cell geometry",
        "did": jd(c.did),
        "physics_mesh": c.physics_mesh.as_ref().map(mesh_json),
        "planes": c.planes.iter().map(|p| json!({"normal": p.normal, "distance": p.distance})).collect::<Vec<_>>(),
        "plane_list_end": format!("{:?}", c.plane_list_end),
        "portals": c.portals.iter().map(|p| json!({"lead": p.lead, "corners": corners(&p.corners)})).collect::<Vec<_>>(),
        "walls": c.walls.iter().map(|w| corners(&w.corners)).collect::<Vec<_>>(),
        "hull": c.hull,
        "properties": c.properties.len(),
        "render_mesh_bytes": c.render_mesh.len(),
        "layout_rule": rule(c.layout_rule),
    })
}

impl View<'_> {
    fn cell_text(&self, c: &CellGeometry, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "cell geometry {}: {} planes (ends {:?}), {} portals, {} walls, {} hull vertices, \
             {} properties, render mesh {} bytes, layout: {}",
            hx(c.did),
            c.planes.len(),
            c.plane_list_end,
            c.portals.len(),
            c.walls.len(),
            c.hull.len(),
            c.properties.len(),
            c.render_mesh.len(),
            rule(c.layout_rule)
        )?;
        match &c.physics_mesh {
            Some(m) => mesh_text(m, "physics ", s)?,
            None => writeln!(s, "no physics mesh")?,
        }
        let mut shown = 0;
        for (k, p) in c.planes.iter().enumerate().take(self.max) {
            shown += 1;
            writeln!(s, "  plane {k}: normal {}, d {}", v3(p.normal), p.distance)?;
        }
        more(s, c.planes.len(), shown)?;
        let mut shown = 0;
        for (k, p) in c.portals.iter().enumerate().take(self.max) {
            shown += 1;
            let cs: Vec<String> = p.corners.iter().map(|v| v3(*v)).collect();
            writeln!(s, "  portal {k} (lead {}): {}", p.lead, cs.join(" "))?;
        }
        more(s, c.portals.len(), shown)?;
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x0F

impl View<'_> {
    fn region_json(&self, r: &RegionDescriptor) -> Result<Value> {
        let mut slots = Vec::new();
        for (t, did) in r.tables() {
            let present = match did {
                Some(d) => Some(self.input.locate(d.0)?.is_some()),
                None => None,
            };
            slots.push(json!({
                "slot": t.slot(),
                "table": t.name(),
                "prefix": format!("0x{:02X}", t.prefix()),
                "did": jod(did),
                "present": present,
            }));
        }
        Ok(json!({
            "type": "region descriptor",
            "did": jd(r.did),
            "region": r.region,
            "map_image": jd(r.map_image),
            "value_12": r.value_12,
            "map_origin": r.map_origin,
            "map_origin_units": r.map_origin_units(),
            "map_offset": r.map_offset,
            "map_scale": r.map_scale,
            "slots": slots,
            "first_unusual_field": r.first_unusual_field(),
        }))
    }

    fn region_text(&self, r: &RegionDescriptor, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "region descriptor {}: region {}, map image {}, map origin {:?} (×20 units), \
             offset {:?}, scale {}, value_12 {}",
            hx(r.did),
            r.region,
            od(r.map_image),
            r.map_origin,
            r.map_offset,
            r.map_scale,
            r.value_12
        )?;
        let mut tab = Table::new(&[
            ("SLOT", Align::Right),
            ("TABLE", Align::Left),
            ("DID", Align::Left),
            ("IN INPUT", Align::Left),
        ]);
        for (t, did) in r.tables() {
            let present = match did {
                Some(d) => {
                    if self.input.locate(d.0)?.is_some() {
                        "yes"
                    } else {
                        "no"
                    }
                }
                None => "-",
            };
            tab.row(vec![
                t.slot().to_string(),
                t.name().into(),
                did.map_or("-".into(), hx),
                present.into(),
            ]);
        }
        table_into(s, &tab)?;
        if let Some(u) = r.first_unusual_field() {
            writeln!(s, "unusual: {u}")?;
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x80 kinds 0 and 4

fn chunk_json(c: &SurfaceChunk) -> Value {
    json!({
        "layers": c.layers.iter().map(|l| json!({
            "material": l.material,
            "mask": jd(l.mask),
            "whole": l.whole,
            "start": l.start,
            "prims": l.prims,
            "min_vertex": l.min_vertex,
            "vertex_span": l.vertex_span,
        })).collect::<Vec<_>>(),
        "indices": c.indices.len(),
        "quad_grid": c.quads.is_some(),
    })
}

fn opt_tree_json(t: Option<&CollisionTree>) -> Value {
    t.map_or(Value::Null, |t| {
        tree_json(TreeCode::from(t), t.build_byte, Some(GRID_TRIANGLES))
    })
}

/// Triangles of a 33 × 33 grid: the key limit of terrain and ceiling trees.
const GRID_TRIANGLES: u64 = tessera_formats::landblock::GRID_TRIANGLE_COUNT as u64;

fn landblock_data_json(d: &LandblockData) -> Value {
    let (lo, hi) = d.heights.min_max();
    json!({
        "type": "landblock data",
        "did": jd(d.did),
        "info": jod(d.info),
        "reserved": d.reserved,
        "height_range": [tessera_formats::landblock::height_from_raw(lo), tessera_formats::landblock::height_from_raw(hi)],
        "terrain": chunk_json(&d.terrain),
        "terrain_tree": opt_tree_json(d.terrain_tree.as_ref()),
        "ceiling": d.ceiling.as_ref().map(|c| {
            let samples = (0..tessera_formats::landblock::GRID_SIDE)
                .flat_map(|i| (0..tessera_formats::landblock::GRID_SIDE).map(move |j| (i, j)))
                .filter(|&(i, j)| c.has_sample(i, j))
                .count();
            json!({
                "samples_with_ceiling": samples,
                "tree": opt_tree_json(c.tree.as_ref()),
                "surface": chunk_json(&c.surface),
            })
        }),
        "boxes": d.boxes.as_ref().map(|b| b.iter().map(|x| json!(x.values)).collect::<Vec<_>>()),
        "layout_rule": rule(d.layout_rule),
        "first_unusual_field": d.first_unusual_field(),
    })
}

impl View<'_> {
    fn landblock_data_text(&self, d: &LandblockData, s: &mut String) -> Result<()> {
        let (lo, hi) = d.heights.min_max();
        writeln!(
            s,
            "landblock data {}: info {}, heights {}..{}, {} terrain layers ({} indices{}), \
             layout: {}",
            hx(d.did),
            d.info.map_or("-".into(), hx),
            tessera_formats::landblock::height_from_raw(lo),
            tessera_formats::landblock::height_from_raw(hi),
            d.terrain.layers.len(),
            d.terrain.indices.len(),
            if d.terrain.quads.is_some() {
                ", quad grid"
            } else {
                ""
            },
            rule(d.layout_rule)
        )?;
        match &d.terrain_tree {
            Some(t) => tree_text(
                s,
                "terrain tree",
                TreeCode::from(t),
                t.build_byte,
                Some(GRID_TRIANGLES),
            )?,
            None => writeln!(s, "terrain tree: none")?,
        }
        match &d.ceiling {
            Some(c) => {
                let samples = (0..tessera_formats::landblock::GRID_SIDE)
                    .flat_map(|i| (0..tessera_formats::landblock::GRID_SIDE).map(move |j| (i, j)))
                    .filter(|&(i, j)| c.has_sample(i, j))
                    .count();
                writeln!(
                    s,
                    "ceiling: {samples} of {} samples, {} layers",
                    tessera_formats::landblock::GRID_SAMPLES,
                    c.surface.layers.len()
                )?;
                match &c.tree {
                    Some(t) => tree_text(
                        s,
                        "ceiling tree",
                        TreeCode::from(t),
                        t.build_byte,
                        Some(GRID_TRIANGLES),
                    )?,
                    None => writeln!(s, "ceiling tree: none")?,
                }
            }
            None => writeln!(s, "ceiling: none")?,
        }
        match &d.boxes {
            Some(b) => writeln!(s, "box list: {} boxes", b.len())?,
            None => writeln!(s, "box list: absent")?,
        }
        let mut tab = Table::new(&[
            ("LAYER", Align::Right),
            ("MATERIAL", Align::Right),
            ("MASK", Align::Left),
            ("WHOLE", Align::Left),
            ("PRIMS", Align::Right),
        ]);
        let layers: Vec<_> = d
            .terrain
            .layers
            .iter()
            .map(|l| ("terrain", l))
            .chain(
                d.ceiling
                    .iter()
                    .flat_map(|c| c.surface.layers.iter().map(|l| ("ceiling", l))),
            )
            .collect();
        let mut shown = 0;
        for (k, (which, l)) in layers.iter().enumerate().take(self.max) {
            shown += 1;
            tab.row(vec![
                format!("{which} {k}"),
                l.material.to_string(),
                od(l.mask),
                if l.whole { "yes" } else { "no" }.into(),
                l.prims.to_string(),
            ]);
        }
        if !layers.is_empty() {
            table_into(s, &tab)?;
            more(s, layers.len(), shown)?;
        }
        if let Some(u) = d.first_unusual_field() {
            writeln!(s, "unusual: {u}")?;
        }
        Ok(())
    }

    fn properties_json(&self, p: &LandblockProperties) -> Value {
        let (light_maps, minimap) = match &self.schema.dict {
            Some(dict) => {
                let pids = LandblockPids::from_dictionary(dict);
                let lm = p.light_maps(&pids);
                (
                    json!({"terrain": jod(lm.terrain), "ceiling": jod(lm.ceiling)}),
                    p.minimap(&pids).map(|m| {
                        json!({
                            "image": jd(m.image),
                            "block_offset": m.block_offset,
                            "pixel_offset": m.pixel_offset,
                            "scale": m.scale,
                        })
                    }),
                )
            }
            None => (Value::Null, None),
        };
        json!({
            "type": "landblock properties",
            "did": jd(p.did),
            "properties": p.properties.len(),
            "light_maps": light_maps,
            "minimap": minimap,
            "dictionary": self.schema.source,
        })
    }

    fn properties_text(&self, p: &LandblockProperties, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "landblock properties {}: {} properties",
            hx(p.did),
            p.properties.len()
        )?;
        let Some(dict) = &self.schema.dict else {
            writeln!(s, "light maps: unknown (no property dictionary found)")?;
            return Ok(());
        };
        let pids = LandblockPids::from_dictionary(dict);
        let lm = p.light_maps(&pids);
        writeln!(
            s,
            "light maps: terrain {}, ceiling {}",
            lm.terrain.map_or("-".into(), hx),
            lm.ceiling.map_or("-".into(), hx)
        )?;
        if let Some(m) = p.minimap(&pids) {
            writeln!(
                s,
                "minimap: image {}, block offset {:?}, pixel offset {:?}, scale {}",
                hx(m.image),
                m.block_offset,
                m.pixel_offset,
                m.scale
            )?;
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- 0x805RXXYY

fn sector_mesh_json(m: &SectorMesh) -> Value {
    json!({
        "samples": m.samples.len(),
        "indices": m.indices.len(),
        "bounds_min": m.bounds_min,
        "bounds_max": m.bounds_max,
        "chunks": m.chunks.iter().map(|c| json!({
            "vertex_start": c.vertex_start,
            "vertex_count": c.vertex_count,
            "min": c.min,
            "max": c.max,
            "wet": c.wet,
            "water_index": c.water_index,
            "lods": c.lods.iter().map(|l| [l.first_index, l.triangles]).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn sector_json(g: &SectorRecord) -> Value {
    json!({
        "type": "distant-landscape sector",
        "did": jd(g.did),
        "key": g.key(),
        "terrain": sector_mesh_json(&g.terrain),
        "mode": g.mode,
        "ceiling_mask": g.ceiling_mask,
        "ceiling": g.ceiling.as_ref().map(sector_mesh_json),
        "block_size": g.block_size,
        "water": g.water.iter().map(|p| json!({"type": p.water_type, "height": p.height}))
            .collect::<Vec<_>>(),
        "sprites": g.sprites.iter().map(|p| json!({
            "name_key": p.name_key,
            "east": p.east,
            "height": p.height,
            "north": p.north,
            "scale": p.scale,
            "distance": p.distance,
            "byte": p.byte,
        })).collect::<Vec<_>>(),
        "entities": g.entities.iter().map(|e| json!({
            "type": e.entity_type,
            "object": jd(e.object),
            "iid": format!("0x{:016X}", e.iid.0),
            "scale": e.scale,
            "position_flags": e.position.flags(),
            "landblock": e.position.landblock(),
            "cell": e.position.cell(),
            "offset": e.position.offset(),
            "orientation": e.position.orientation(),
            "property": e.property.as_ref().map(|p| format!("{:#x}", p.pid)),
            "view_distance_sq": e.view_distance_sq,
        })).collect::<Vec<_>>(),
        "colour_list": jd(g.colour_list),
        "ceiling_colour_list": jd(g.ceiling_colour_list),
        "tile": jd(g.tile),
        "ceiling_tile": jd(g.ceiling_tile),
        "flow_map": jd(g.flow_map),
        "imposter_table": jd(g.imposter_table),
        "deviations": g.deviations(),
    })
}

impl View<'_> {
    fn sector_text(&self, g: &SectorRecord, s: &mut String) -> Result<()> {
        writeln!(
            s,
            "distant-landscape sector {} (key {}): mode {}, ceiling mask {:#05x}, block size {}, \
             {} water pairs, {} sprites, {} entities",
            hx(g.did),
            g.key().map_or("-".into(), |(x, y)| format!("{x}, {y}")),
            g.mode,
            g.ceiling_mask,
            g.block_size,
            g.water.len(),
            g.sprites.len(),
            g.entities.len()
        )?;
        writeln!(
            s,
            "terrain: {} samples, {} stored indices, bounds {} – {}",
            g.terrain.samples.len(),
            g.terrain.indices.len(),
            v3(g.terrain.bounds_min),
            v3(g.terrain.bounds_max)
        )?;
        if let Some(c) = &g.ceiling {
            writeln!(
                s,
                "ceiling: {} samples, {} stored indices, bounds {} – {}",
                c.samples.len(),
                c.indices.len(),
                v3(c.bounds_min),
                v3(c.bounds_max)
            )?;
        }
        let mut tab = Table::new(&[
            ("X", Align::Right),
            ("Y", Align::Right),
            ("HEIGHTS", Align::Left),
            ("WATER", Align::Left),
            ("CEILING", Align::Left),
        ]);
        let (x0, y0) = g.origin().unwrap_or((0, 0));
        for y in (0..3usize).rev() {
            for x in 0..3usize {
                let Some(c) = g.terrain.chunk(x, y) else {
                    continue;
                };
                let water = g.water_at(x, y).map_or("-".into(), |p| {
                    format!("type {} at {}", p.water_type, p.height)
                });
                let ceiling = g
                    .ceiling
                    .as_ref()
                    .and_then(|m| m.chunk(x, y))
                    .filter(|c| c.is_present())
                    .map_or("-".into(), |c| format!("{} – {}", c.min[2], c.max[2]));
                tab.row(vec![
                    (usize::from(x0) + x).to_string(),
                    (usize::from(y0) + y).to_string(),
                    format!("{} – {}", c.min[2], c.max[2]),
                    water,
                    ceiling,
                ]);
            }
        }
        table_into(s, &tab)?;
        writeln!(
            s,
            "colour list {}, ceiling colour list {}, tile {}, ceiling tile {}, flow map {}, \
             imposter table {}",
            od(g.colour_list),
            od(g.ceiling_colour_list),
            od(g.tile),
            od(g.ceiling_tile),
            od(g.flow_map),
            od(g.imposter_table)
        )?;
        if !g.sprites.is_empty() {
            let mut tab = Table::new(&[
                ("KEY", Align::Right),
                ("EAST", Align::Right),
                ("NORTH", Align::Right),
                ("HEIGHT", Align::Right),
                ("SCALE", Align::Right),
                ("DISTANCE", Align::Right),
                ("BYTE", Align::Right),
            ]);
            let mut shown = 0;
            for p in g.sprites.iter().take(self.max) {
                shown += 1;
                tab.row(vec![
                    p.name_key.to_string(),
                    p.east.to_string(),
                    p.north.to_string(),
                    p.height.to_string(),
                    p.scale.to_string(),
                    p.distance.to_string(),
                    p.byte.to_string(),
                ]);
            }
            writeln!(s, "sprites:")?;
            table_into(s, &tab)?;
            more(s, g.sprites.len(), shown)?;
        }
        if !g.entities.is_empty() {
            let mut tab = Table::new(&[
                ("TYPE", Align::Right),
                ("OBJECT", Align::Left),
                ("IID", Align::Left),
                ("SCALE", Align::Right),
                ("OFFSET", Align::Left),
                ("PROPERTY", Align::Left),
                ("VIEW", Align::Right),
            ]);
            let mut shown = 0;
            for e in g.entities.iter().take(self.max) {
                shown += 1;
                tab.row(vec![
                    e.entity_type.to_string(),
                    hx(e.object),
                    format!("0x{:016X}", e.iid.0),
                    e.scale[0].to_string(),
                    e.position.offset().map_or("-".into(), v3),
                    e.property
                        .as_ref()
                        .map_or("-".into(), |p| format!("{:#x}", p.pid)),
                    e.view_distance().to_string(),
                ]);
            }
            writeln!(s, "entities:")?;
            table_into(s, &tab)?;
            more(s, g.entities.len(), shown)?;
        }
        let dev = g.deviations();
        if dev.is_empty() {
            writeln!(s, "invariants (distant-landscape.md §7): all hold")?;
        } else {
            for d in dev {
                writeln!(s, "invariant broken: {d}")?;
            }
        }
        Ok(())
    }
}

/// Every `show` text renderer must work on default-built records (no panics on empty
/// lists); the CLI tests cover real layouts through synthetic archives.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_formatters() {
        assert_eq!(od(Did(0)), "-");
        assert_eq!(od(Did(0x4100_0001)), "0x41000001");
        assert_eq!(argb(Argb(0xFF00_8040)), "0xFF008040");
        assert_eq!(curve_text(&FloatCurve::Constant(0.5)), "0.5");
        assert_eq!(curve_json(&FloatCurve::Constant(0.5))["mode"], 1);
        let mut s = String::new();
        more(&mut s, 5, 2).unwrap();
        assert_eq!(s, "... 3 more (use --max-list 0 for all)\n");
        let mut s = String::new();
        more(&mut s, 2, 2).unwrap();
        assert!(s.is_empty());
    }

    /// A scene with many groups prints in time linear in its size, whatever `--max-list`
    /// says (once per group, the old view re-walked every child of the record).
    #[test]
    fn scene_text_is_linear_in_the_groups() {
        use tessera_formats::scenery::SceneGroup;
        const N: usize = 50_000;
        let mut groups = vec![SceneGroup {
            group_id: 1,
            children: vec![SceneChild::Object(Did(0x4700_0001)); N],
        }];
        groups.extend((0..N).map(|_| SceneGroup {
            group_id: 1,
            children: Vec::new(),
        }));
        let scene = Scene {
            did: Did(0x0200_0001),
            groups,
            ..Scene::default()
        };
        // Through the decoder, as `show` sees it.
        let scene = <Scene as tessera_formats::Record>::parse(&scene.to_bytes()).unwrap();
        let started = std::time::Instant::now();
        let mut s = String::new();
        scene_text(&scene, 20, &mut s).unwrap();
        let mut all = String::new();
        scene_text(&scene, usize::MAX, &mut all).unwrap();
        let took = started.elapsed();
        assert!(took.as_secs() < 10, "{took:?}");
        assert_eq!(s.lines().count(), 1 + 20 + 20 + 2, "{s}");
        assert!(
            s.contains(&format!("... {} more groups", N + 1 - 20)),
            "{s}"
        );
        assert!(s.contains(&format!("... {} more (use", N - 20)), "{s}");
        assert_eq!(all.lines().count(), 1 + (N + 1) + N);

        // Children print under their own group, the frill layer's first.
        let small = Scene {
            did: Did(0x0200_0002),
            frill_layer: Some(PlacementNode {
                children: vec![SceneChild::Object(Did(0x4700_0009))],
                ..PlacementNode::default()
            }),
            groups: vec![
                SceneGroup {
                    group_id: 1,
                    children: vec![SceneChild::Object(Did(0x4700_0002))],
                },
                SceneGroup {
                    group_id: 2,
                    children: vec![SceneChild::Scene(Did(0x0200_0003))],
                },
            ],
            ..Scene::default()
        };
        let mut s = String::new();
        scene_text(&small, usize::MAX, &mut s).unwrap();
        let lines: Vec<&str> = s.lines().skip(1).collect();
        assert!(lines[0].starts_with("frill layer"), "{s}");
        assert_eq!(lines[1].trim(), "object 0x47000009", "{s}");
        assert!(lines[2].starts_with("group 0 (id 1): 1 children"), "{s}");
        assert_eq!(lines[3].trim(), "object 0x47000002", "{s}");
        assert!(lines[4].starts_with("group 1 (id 2): 1 children"), "{s}");
        assert_eq!(lines[5].trim(), "scene 0x02000003", "{s}");
        assert_eq!(lines.len(), 6, "{s}");
    }

    #[test]
    fn tree_summary_reports_walks() {
        let code = [0x30u8];
        let t = TreeCode::new([0.0; 3], 65_536.0, &code);
        let v = tree_json(t, Some(1), Some(1));
        assert_eq!(v["walk"]["keys"], 1);
        assert_eq!(v["walk"]["keys_in_range"], true);
        let mut s = String::new();
        tree_text(&mut s, "tree", t, None, Some(0)).unwrap();
        assert!(s.contains("KEYS OUT OF RANGE"), "{s}");
        let bad = [0xFFu8];
        let mut s = String::new();
        tree_text(
            &mut s,
            "tree",
            TreeCode::new([0.0; 3], 1.0, &bad),
            None,
            None,
        )
        .unwrap();
        assert!(s.contains("WALK FAILED"), "{s}");
    }
}
