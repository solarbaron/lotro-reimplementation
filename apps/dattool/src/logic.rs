// SPDX-License-Identifier: GPL-3.0-or-later
//! `logic <dir|archive> [DID]`: the landblock-borne logic of one landblock-info record
//! (0x8RR2XXYY), or with `--all` a census over every landblock-info record of the input:
//! Decal SEDs and their parameters (`decals.md` §3), phasing boxes with the empty-journal
//! test (`world-logic.md` §4.9), logic wiring (§6–§7), instance script tables with their link
//! targets (`instance-scripts.md` §3), drama lists and their facets (`npc-drama.md` §3–§4)
//! and riders (`mounts.md` §7).
//!
//! The command reads only. It prints counts, DIDs, enum keys and instance IDs, never names,
//! dialog lines or other text of the install.

use std::collections::{BTreeMap, HashMap};
use std::process::ExitCode;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tessera_formats::gameplay::drama::{DramaKind, DramaLabels, DramaList};
use tessera_formats::gameplay::mounts::{MountLabels, RiderInstance};
use tessera_formats::gameplay::quests::{EntryGates, MapJournal};
use tessera_formats::gameplay::world_logic::{
    CounterBox, CustomCondition, EventLists, GeneratorMapping, LocalPlayer, LogicBoxTimer,
    PHASING_BOX_UNIQUEDB_KEY, PathMapVolume, TravelHooks, WorldLogicLabels,
};
use tessera_formats::landblock::{InstanceId, LandblockInfo, StaticEntityType};
use tessera_formats::script_table::instance::{
    InstanceScriptLabels, InstanceScriptTable, link_targets,
};
use tessera_formats::{DataIdMap, Did, EntityDescriptor, PropertyDictionary, PropertySet, Record};
use tessera_world::decals::{
    DEFAULT_MATERIAL_UNIQUEDB_KEY, DecalBox, DecalParams, DecalPids, marker_chain,
};
use tessera_world::phasing::landblock_boxes;

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::did_hex;
use crate::world::Schema;

/// Options of `logic`.
#[derive(Debug)]
pub struct Opts {
    /// Install directory or archive.
    pub target: String,
    /// A landblock-info DID; `None` with `all`.
    pub did: Option<u32>,
    /// Census over every landblock-info record.
    pub all: bool,
    /// Most rows per list in text output (0 = all).
    pub max_list: usize,
}

struct Labels {
    drama: DramaLabels,
    ist: InstanceScriptLabels,
    logic: WorldLogicLabels,
    mounts: MountLabels,
    decals: DecalPids,
    box_did: Option<Did>,
    default_material: Did,
}

fn uniquedb(input: &Input) -> Result<Option<DataIdMap>> {
    // The root DID map (0x28000000) names UNIQUEDB; read both from the input.
    let Some(root) = input.locate(0x2800_0000)? else {
        return Ok(None);
    };
    let root = DataIdMap::parse(&root.read()?)?;
    let Some(did) = root.by_label("UNIQUEDB") else {
        return Ok(None);
    };
    match input.locate(did.0)? {
        Some(f) => Ok(Some(DataIdMap::parse(&f.read()?)?)),
        None => Ok(None),
    }
}

/// Counters of one record (or of all).
#[derive(Default)]
struct Counts(BTreeMap<&'static str, u64>);

impl Counts {
    fn add(&mut self, k: &'static str, v: u64) {
        *self.0.entry(k).or_default() += v;
    }
    fn json(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|(k, v)| ((*k).to_owned(), json!(v)))
                .collect(),
        )
    }
}

#[allow(clippy::too_many_lines)]
fn landblock(
    input: &Input,
    dict: &PropertyDictionary,
    l: &Labels,
    info: &LandblockInfo,
    c: &mut Counts,
    details: Option<&mut Vec<Value>>,
) {
    let mut rows: Vec<Value> = Vec::new();
    let mut chains: HashMap<Did, Vec<PropertySet>> = HashMap::new();
    for e in info
        .entities
        .iter()
        .filter(|e| e.kind() == Some(StaticEntityType::Decal))
    {
        c.add("decal SEDs", 1);
        let Some(object) = e.object else { continue };
        let chain = chains.entry(object).or_insert_with(|| {
            marker_chain(object, |d| {
                let bytes = input.locate(d.0).ok().flatten()?.read().ok()?;
                let ed = EntityDescriptor::parse(&bytes, dict).ok()?;
                Some((ed.properties, Some(ed.header.template)))
            })
        });
        let refs: Vec<&PropertySet> = chain.iter().collect();
        let p = DecalParams::collect(&l.decals, l.default_material, &refs, e.properties.as_ref());
        if details.is_some() {
            let centre = e
                .placement
                .map(|pl| DecalBox::new(pl.position, pl.rotation, p.ortho_scale).center);
            rows.push(json!({
                "subject": "decal",
                "iid": e.iid.map(|i| format!("{:#018x}", i.0)),
                "material": did_hex(p.material.0),
                "lighting_blend": p.lighting_blend,
                "lod_class": format!("{:#x}", p.lod_class),
                "depth_bias": p.depth_bias,
                "ortho_scale": p.ortho_scale,
                "box_centre": centre,
            }));
        }
    }
    let boxes = landblock_boxes(info, &l.logic.quest, l.box_did);
    let empty = MapJournal::default();
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: &empty,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    for b in boxes.sed_boxes.iter().chain(&boxes.spawn_boxes) {
        c.add("phasing boxes", 1);
        let shown = b.fields.test(true, Some(&player));
        c.add(
            if shown {
                "boxes true (empty journal)"
            } else {
                "boxes false (empty journal)"
            },
            1,
        );
        c.add("box targets", b.targets.len() as u64);
        if details.is_some() {
            rows.push(json!({
                "subject": "phasing box",
                "iid": format!("{:#018x}", b.iid.0),
                "checks": b.fields.checks.as_ref().map_or(0, Vec::len),
                "operator": b.fields.operator(),
                "true_draw": b.fields.true_draw,
                "empty_journal_shows_targets": shown,
                "targets": b.targets.iter().map(|t| format!("{:#018x}", t.0)).collect::<Vec<_>>(),
            }));
        }
    }
    c.add("phasing link halves", boxes.half_links as u64);
    for w in &info.weenies {
        let set = &w.properties;
        let lg = &l.logic;
        c.add(
            "counter boxes",
            u64::from(CounterBox::read(&lg.logic, set).is_some()),
        );
        c.add(
            "logic-box timers",
            u64::from(LogicBoxTimer::read(&lg.logic, set).is_some()),
        );
        if let Some(m) = GeneratorMapping::read_all(&lg.wiring, set) {
            c.add("generator mappings", m.len() as u64);
        }
        c.add(
            "custom conditions",
            CustomCondition::read_all(lg, set).len() as u64,
        );
        c.add(
            "event-list holders",
            u64::from(!EventLists::read(&lg.wiring, set).is_empty()),
        );
        c.add(
            "travel hooks",
            u64::from(TravelHooks::read(&lg.travel, set).is_some()),
        );
        c.add(
            "path-map volumes",
            u64::from(PathMapVolume::read(&lg.travel, set, dict).is_some()),
        );
        if let Some(ist) = InstanceScriptTable::read(&l.ist, set) {
            c.add("instance script tables", 1);
            c.add("instance scripts", ist.scripts.len() as u64);
            for name in ist.link_names() {
                c.add("link targets", 1);
                let to = link_targets(&info.links, w.iid, name);
                c.add(
                    if to.is_empty() {
                        "link targets unresolved"
                    } else {
                        "link targets resolved"
                    },
                    1,
                );
            }
            if details.is_some() {
                rows.push(json!({
                    "subject": "instance script table",
                    "iid": format!("{:#018x}", w.iid.0),
                    "keys": ist.scripts.iter().map(|s| format!("{:#x}", s.key)).collect::<Vec<_>>(),
                    "startup": ist.startup.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>(),
                }));
            }
        }
        if let Some(list) = DramaList::read(&l.drama.drama, set) {
            c.add("drama lists", 1);
            for d in &list.dramas {
                c.add(
                    match d.kind {
                        DramaKind::Named => "named dramas",
                        DramaKind::Instant => "instant dramas",
                        DramaKind::Idle => "idle dramas",
                        DramaKind::Other(_) => "other drama elements",
                    },
                    1,
                );
                for a in d.actions() {
                    c.add("drama actions", 1);
                    c.add("drama facets", a.facets().present().len() as u64);
                }
            }
        }
        if RiderInstance::read(&l.mounts, set).is_some() {
            c.add("riders", 1);
        }
    }
    if let Some(d) = details {
        d.extend(rows);
    }
    let _ = InstanceId(0);
}

/// Runs `logic`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let schema = Schema::load(&input, ctx)?;
    let Some(dict) = &schema.dict else {
        bail!("the logic views need the property dictionary (0x34000000)");
    };
    let udb = uniquedb(&input)?;
    let labels = Labels {
        drama: DramaLabels::resolve(dict),
        ist: InstanceScriptLabels::resolve(dict),
        logic: WorldLogicLabels::resolve(dict),
        mounts: MountLabels::resolve(dict),
        decals: DecalPids::resolve(dict),
        box_did: udb
            .as_ref()
            .and_then(|m| m.get(PHASING_BOX_UNIQUEDB_KEY))
            .map(|e| e.value),
        default_material: udb
            .as_ref()
            .and_then(|m| m.get(DEFAULT_MATERIAL_UNIQUEDB_KEY))
            .map_or(Did(0), |e| e.value),
    };
    let mut counts = Counts::default();
    let mut details: Vec<Value> = Vec::new();
    let mut records = 0u64;
    if o.all {
        for (_, archive) in input.archives() {
            for e in archive.entries_range(0x8000_0000..=0x8FFF_FFFF) {
                let e = e?;
                if (e.did >> 20) & 0xF != 2 {
                    continue;
                }
                let bytes = archive.read_entry(&e)?;
                let info = LandblockInfo::parse_expecting(&bytes, Did(e.did), Some(e.stamp), dict)?;
                records += 1;
                landblock(&input, dict, &labels, &info, &mut counts, None);
            }
        }
    } else {
        let Some(did) = o.did else {
            bail!("give a landblock-info DID or --all");
        };
        if did >> 24 != 0x80 || (did >> 20) & 0xF != 2 {
            bail!(
                "{} is not a landblock-info record (0x8RR2XXYY)",
                did_hex(did)
            );
        }
        let found = input.require(did)?;
        let bytes = found.read()?;
        let info = LandblockInfo::parse_expecting(&bytes, Did(did), Some(found.entry.stamp), dict)?;
        records = 1;
        landblock(
            &input,
            dict,
            &labels,
            &info,
            &mut counts,
            Some(&mut details),
        );
    }
    let doc = json!({
        "records": records,
        "counts": counts.json(),
        "rows": details,
    });
    if out.json {
        out.json(&doc)?;
    } else {
        writeln!(out.w, "{records} landblock-info record(s)")?;
        for (k, v) in &counts.0 {
            writeln!(out.w, "  {k}: {v}")?;
        }
        let limit = if o.max_list == 0 {
            usize::MAX
        } else {
            o.max_list
        };
        for r in details.iter().take(limit) {
            writeln!(out.w, "  {r}")?;
        }
        if details.len() > limit {
            writeln!(
                out.w,
                "  … {} more rows (--max-list 0 for all)",
                details.len() - limit
            )?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

use std::io::Write as _;
