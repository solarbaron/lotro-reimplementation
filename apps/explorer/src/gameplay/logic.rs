// SPDX-License-Identifier: GPL-3.0-or-later
//! Cards of the landblock-borne logic and of mounts: the mount card (`mounts.md` §10), the
//! drama viewer (`npc-drama.md` §11), the per-instance script view (`instance-scripts.md`
//! §9.6), the gating and wiring view of a landblock (`world-logic.md` §13) and the decal
//! list with its preview meshes (`decals.md` §9.3).
//!
//! Object records (templates, mount effects) get sections on their gameplay card; a
//! landblock-info record (0x8RR2XXYY) gets a card of its own with one section per subject.
//! Everything is read from the install: labels by dictionary name, enum labels from the
//! mappers, the box physics object and default decal material from the UNIQUEDB map.

use std::collections::HashMap;

use tessera_formats::gameplay::common::{Pid, PropertyAccess, enum_mapper_of};
use tessera_formats::gameplay::drama::{
    DramaKind, DramaList, KungFu, StartingIdle, dialog_presentation, plan_drama, script_link_port,
};
use tessera_formats::gameplay::mounts::{
    MountEffect, MountGroup, RiderInstance, TemplateMountStats, WAR_STEED_MASK,
};
use tessera_formats::gameplay::quests::{EntryGates, MapJournal};
use tessera_formats::gameplay::world_logic::{
    BranchVisuals, CounterBox, CustomCondition, EventLists, GeneratorMapping, LocalPlayer,
    LogicBoxTimer, PHASING_BOX_UNIQUEDB_KEY, PathMapVolume, QuestStateBranch, QuestStateScript,
    SkirmishValues, TravelHooks,
};
use tessera_formats::landblock::{InstanceId, LandblockInfo, StaticEntityType};
use tessera_formats::script_table::instance::{
    EntityTableSet, InstanceScriptTable, MemberOverride, link_targets,
};
use tessera_formats::{Did, EntityDescriptor, PropertySet};
use tessera_world::decals::{
    DEFAULT_MATERIAL_UNIQUEDB_KEY, DecalBox, DecalParams, ReceiverKind, ReceiverTriangle,
    ReceiverVertex, build_mesh, marker_chain,
};
use tessera_world::phasing::landblock_boxes;

use super::card::{Card, Section, did_text, opt_did_text, opt_text, opt_yes, yes};
use super::{Built, Cx, Family, GameplayProduct, Object, product};
use crate::error::ExplorerError;

/// Most entities given their own rows per landblock section.
const MAX_ENTITIES: usize = 400;

/// The raw mapper label of `key` for property `pid`.
fn enum_name(cx: &Cx<'_>, pid: Pid, key: u32) -> Option<String> {
    let mapper = enum_mapper_of(cx.s.registry.dictionary(), pid)?;
    cx.ctx
        .game
        .enum_label(mapper, key)
        .ok()
        .and_then(|l| l.name.map(|n| n.to_string()))
}

fn iid_text(i: InstanceId) -> String {
    format!("{:#018x}", i.0)
}

// ---------------------------------------------------------------------------------------------
// Mount card (mounts.md §10)
// ---------------------------------------------------------------------------------------------

/// The mount card of a class-2459 effect, recognised by its fields (§2.1).
pub(super) fn mount_card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let m = &cx.s.mounts;
    let e = MountEffect::recognise(m, obj.set)?;
    let l = &cx.labels;
    let p = &m.effect;
    let mut b = Built {
        kind: "mount effect".into(),
        title: e.name().map(|n| l.text(n)),
        ..Built::default()
    };
    let mut s = Section::fields("Mount effect (mounts.md §2)");
    let group = match e.group() {
        MountGroup::Durability => "durability mount",
        MountGroup::WarSteed => "war-steed record",
        MountGroup::Unknown => "unknown",
    };
    s.field("group", group);
    let leak = |o: Option<String>| o.map(|s| &*Box::leak(s.into_boxed_str()));
    let hobby = e.is_hobby_horse(|k| leak(enum_name(cx, p.animation_script, k)));
    let startup = e.is_startup_record(|k| leak(enum_name(cx, p.equivalence_class, k)));
    s.field("hobby-horse", yes(hobby));
    s.field("startup record", yes(startup));
    if let Some(t) = e.mount_type() {
        let war = if t.0 & WAR_STEED_MASK != 0 {
            " — war-steed"
        } else {
            ""
        };
        s.field(
            "mount type",
            format!("{}{war}", l.bits(p.mount_type, u64::from(t.0))),
        );
    }
    let r = e.rider();
    s.did("mount entity", r.entity);
    if let Some(ent) = r.entity.filter(|d| !d.is_null()) {
        let setup = tessera_formats::resolve_setup_did(ent, |d| cx.ctx.game.read(d))
            .ok()
            .and_then(|path| path.last().copied());
        s.did("its setup", setup);
    }
    s.field(
        "holding location",
        l.opt_enum(p.slot_holding_location, r.holding_location),
    );
    s.opt("scale", r.scale.map(|v| format!("{v:?}")));
    s.opt("bone offset", r.bone_offset.map(|v| format!("{v:?}")));
    s.field("behaviour", l.opt_enum(p.slot_behavior, r.behavior));
    s.field("start script", l.opt_enum(p.start_script, r.start_script));
    s.field(
        "animation script",
        l.opt_enum(p.animation_script, r.animation_script),
    );
    s.field(
        "dismount (player)",
        l.opt_enum(p.dismount_by_player, r.dismount_by_player),
    );
    s.field(
        "dismount (other)",
        l.opt_enum(p.dismount_by_other, r.dismount_by_other),
    );
    s.opt("durability base", e.durability_max());
    s.opt("regeneration", e.durability_regen());
    for ev in e.durability_events() {
        s.field(
            format!("durability event {}", l.opt_enum(p.event_id, ev.event)),
            opt_text(ev.change),
        );
    }
    if let Some(pip) = e.pip_type() {
        s.field("pip type", l.enum_key(p.pip_type, pip));
    }
    for g in e.gear_modifiers() {
        s.field(
            format!("gear {}", l.opt_enum(p.gear, g.gear)),
            opt_text(g.modifier),
        );
    }
    for a in e
        .appearances()
        .into_iter()
        .filter(|a| a.appearance.is_some())
    {
        s.did("appearance", a.appearance);
    }
    b.card.add(s);
    b.groups.push(("Mount fields", p.all()));
    Some(b)
}

/// Template mount stats and riders (§6.1, §7.1) on any object card.
pub(super) fn mount_sections(cx: &Cx<'_>, obj: &Object<'_>, card: &mut Card) {
    let m = &cx.s.mounts;
    let st = TemplateMountStats::read(&m.stats, obj.set);
    if !st.is_empty() {
        let mut s = Section::fields("Mount stats (mounts.md §6.1)");
        s.opt("max speed", st.max_speed);
        s.opt("acceleration", st.acceleration);
        s.opt("turn rate", st.turn_rate);
        s.opt("gear-change rate", st.gear_change_rate);
        s.field(
            "cached (speed, accel, turn, gear)",
            format!("{:?}", st.cached),
        );
        s.opt("mount speed multiplier", st.effect_speed);
        card.add(s);
    }
    if let Some(r) = RiderInstance::read(m, obj.set) {
        card.add(rider_section(cx, &r));
    }
}

fn rider_section(cx: &Cx<'_>, r: &RiderInstance) -> Section {
    let m = &cx.s.mounts;
    let l = &cx.labels;
    let mut s = Section::fields("Rider keys (mounts.md §7)");
    s.field("rider type", l.opt_enum(m.state.rider_type, r.rider_type));
    s.field(
        "livery",
        l.opt_enum(m.state.mount_appearance, r.mount_appearance),
    );
    s.field("mounted", opt_yes(r.is_mounted));
    s.field(
        "current mount type",
        l.opt_bits(
            m.state.current_mount_type,
            r.current_mount_type.map(|t| u64::from(t.0)),
        ),
    );
    s.field(
        "mount type",
        l.opt_bits(m.effect.mount_type, r.mount_type.map(|t| u64::from(t.0))),
    );
    s.field(
        "banner",
        l.opt_enum(m.state.banner_appearance, r.banner_appearance),
    );
    s.field(
        "banner wind",
        l.opt_enum(m.state.banner_animation, r.banner_animation),
    );
    s
}

// ---------------------------------------------------------------------------------------------
// Drama viewer (npc-drama.md §11)
// ---------------------------------------------------------------------------------------------

/// The drama sections of a set: the list, its actions and the entity's scripted-fight and
/// starting-idle properties.
pub(super) fn drama_sections(cx: &Cx<'_>, set: &PropertySet, card: &mut Card, title: &str) {
    let d = &cx.s.drama;
    let l = &cx.labels;
    if let Some(list) = DramaList::read(&d.drama, set) {
        let mut s = Section::new(
            format!("Dramas{title} (npc-drama.md §3)"),
            &["kind", "name", "flags", "actions", "plan"],
        );
        for x in &list.dramas {
            let flags = [
                x.interruptible().filter(|b| *b).map(|_| "interruptible"),
                x.become_monster().filter(|b| *b).map(|_| "becomes monster"),
                x.become_npc().filter(|b| *b).map(|_| "becomes NPC"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            let plan = plan_drama(x, |_| 0.0);
            s.push(
                vec![
                    x.kind.name().to_owned(),
                    x.name().unwrap_or("—").to_owned(),
                    flags,
                    x.actions().len().to_string(),
                    format!("{} steps", plan.len()),
                ],
                Vec::new(),
            );
        }
        card.add(s);
        let mut a = Section::new(
            format!("Drama actions{title} (§4, Decision T1 order)"),
            &["drama", "#", "facets", "details"],
        );
        for x in list
            .dramas
            .iter()
            .filter(|x| !matches!(x.kind, DramaKind::Other(_)))
        {
            for (i, act) in x.actions().iter().enumerate() {
                let f = act.facets();
                let mut details: Vec<String> = Vec::new();
                let mut links = Vec::new();
                if let Some(w) = f.wait_before {
                    details.push(format!("before {w} s"));
                }
                if let Some(w) = f.wait_after {
                    details.push(format!("after {w} s"));
                }
                if let Some(v) = f.wait_variance {
                    details.push(format!("± {v} s"));
                }
                if let Some(k) = f.emote.filter(|k| *k != 0) {
                    details.push(format!("emote {}", l.enum_key(d.drama.emote, k)));
                }
                if let Some(dialog) = &f.dialog {
                    let label = dialog
                        .dialog_type
                        .and_then(|t| enum_name(cx, d.drama.dialog_type, t));
                    let pres = dialog_presentation(label.as_deref());
                    let lines: Vec<String> = dialog.lines.iter().map(|si| l.text(si)).collect();
                    details.push(format!(
                        "dialog {} (overhead {}, chat {}): {}",
                        label.unwrap_or_default(),
                        yes(pres.overhead),
                        yes(pres.chat),
                        lines.join(" | ")
                    ));
                }
                if let Some(snd) = &f.sound {
                    details.push(format!("sound {}", opt_did_text(snd.sound)));
                    links.extend(snd.sound.map(|d| d.0));
                }
                if let Some(k) = f.script_link {
                    let label = enum_name(cx, d.drama.script_link, k);
                    let port = label.as_deref().and_then(script_link_port);
                    details.push(format!(
                        "script link {} → port {}",
                        label.unwrap_or_default(),
                        opt_text(port)
                    ));
                }
                if let Some(p) = &f.patrol_start {
                    details.push(format!("patrol {p}"));
                }
                if let Some(bc) = &f.broadcast {
                    details.push(format!(
                        "broadcast {} within {} ({})",
                        bc.name.as_deref().unwrap_or("—"),
                        opt_text(bc.range),
                        l.property(bc.property_to_match)
                    ));
                }
                if let Some(da) = &f.direct_actors {
                    details.push(format!("directs actors {:#x}", da.actors.unwrap_or(0)));
                }
                for did in [f.scriptlet, f.remove_effect, f.add_effect.and_then(|x| x.0)]
                    .into_iter()
                    .flatten()
                {
                    links.push(did.0);
                    details.push(did_text(did));
                }
                let facets: Vec<String> = f.present().iter().map(|x| format!("{x:?}")).collect();
                a.push(
                    vec![
                        x.name().unwrap_or("—").to_owned(),
                        i.to_string(),
                        facets.join(", "),
                        details.join("; "),
                    ],
                    links,
                );
            }
        }
        card.add(a);
    }
    let kf = KungFu::read(d, set);
    let idle = StartingIdle::read(d, set);
    if !kf.is_empty() || idle != StartingIdle::default() {
        let mut s = Section::fields(format!("Scripted fight and starting idle{title} (§7, §8)"));
        s.field("start as NPC", opt_yes(kf.start_as_npc));
        s.field("repeatable", opt_yes(kf.repeatable));
        for t in kf.transitions.iter().flatten() {
            s.field(
                format!("{:?} transition", t.kind),
                format!(
                    "{} at {}",
                    t.drama.as_deref().unwrap_or("—"),
                    opt_text(t.health_ratio)
                ),
            );
        }
        s.opt("starting idle drama", idle.drama.as_deref());
        s.field(
            "starting idle animation",
            l.opt_enum(d.idle.animation, idle.animation),
        );
        s.field("returns to starting idle", opt_yes(idle.returns));
        card.add(s);
    }
}

// ---------------------------------------------------------------------------------------------
// Quest-state script path (world-logic.md §13 item 3)
// ---------------------------------------------------------------------------------------------

/// The script-path test and the visuals of both branches of a set.
pub(super) fn quest_state_sections(cx: &Cx<'_>, set: &PropertySet, card: &mut Card, title: &str) {
    let w = &cx.s.logic;
    let q = &w.quest;
    if !set.has(q.quest_id) && !set.has(q.test) {
        return;
    }
    let l = &cx.labels;
    let sp = QuestStateScript::read(q, set);
    let mut s = Section::fields(format!(
        "Quest-state script path{title} (world-logic.md §5)"
    ));
    s.did("quest", Some(sp.entry.quest));
    s.field("status", format!("{:#010x}", sp.entry.status.0));
    s.field("operator", l.enum_key(q.quest_operator, sp.entry.operator));
    if !sp.entry_2.quest.is_null() {
        s.did("quest 2", Some(sp.entry_2.quest));
    }
    s.field("QuestState_Test", l.opt_enum(q.test, sp.test));
    for br in [QuestStateBranch::True, QuestStateBranch::False] {
        let v = BranchVisuals::read(w, br, set);
        s.field(format!("{br:?} visuals"), format!("{v:?}"));
    }
    card.add(s);
}

// ---------------------------------------------------------------------------------------------
// The landblock card
// ---------------------------------------------------------------------------------------------

/// Whether `did` is a landblock-info record (0x8RR2XXYY).
#[must_use]
pub fn is_landblock_info(did: u32) -> bool {
    did >> 24 == 0x80 && (did >> 20) & 0xF == 2
}

/// A square through the box centre facing −Y′, larger than the box (`decals.md` §9.3).
fn preview(b: &DecalBox) -> Vec<ReceiverTriangle> {
    let [x, y, z] = b.axes;
    let r = b.terrain_half_width() * 2.0;
    let n = [-y[0], -y[1], -y[2]];
    let at = |sx: f32, sz: f32| ReceiverVertex {
        position: [
            b.center[0] + x[0] * sx + z[0] * sz,
            b.center[1] + x[1] * sx + z[1] * sz,
            b.center[2] + x[2] * sx + z[2] * sz,
        ],
        normal: n,
    };
    let (a, bb, c, d) = (at(-r, -r), at(r, -r), at(r, r), at(-r, r));
    vec![
        ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices: [a, bb, c],
        },
        ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices: [a, c, d],
        },
    ]
}

/// The card of a landblock-info record: decals, gating, logic wiring, instance scripts,
/// dramas and riders.
#[allow(clippy::too_many_lines)]
pub(super) fn landblock_product(cx: &Cx<'_>, did: u32) -> Result<GameplayProduct, ExplorerError> {
    let raw = crate::decoders::read_raw(cx.ctx, did, None)?;
    let dict = cx.s.registry.dictionary();
    let info = LandblockInfo::parse_expecting(&raw.bytes, Did(did), Some(raw.entry.stamp), dict)?;
    let mut p = product(did, Family::Logic, "landblock logic");
    let l = &cx.labels;
    let weenies: HashMap<InstanceId, &PropertySet> = info
        .weenies
        .iter()
        .map(|w| (w.iid, &w.properties))
        .collect();
    let uniquedb = cx.s.uniquedb.as_deref();
    let mut card = Card::default();

    // Decals (decals.md §3, §4.2, §9.3).
    let default_material = uniquedb
        .and_then(|m| m.get(DEFAULT_MATERIAL_UNIQUEDB_KEY))
        .map_or(Did(0), |e| e.value);
    let mut decals = Section::new(
        "Decals (decals.md §3, §9.3)",
        &[
            "IID",
            "material",
            "blend",
            "LOD class",
            "scale",
            "box centre",
            "preview triangles",
        ],
    );
    for e in info
        .entities
        .iter()
        .filter(|e| e.kind() == Some(StaticEntityType::Decal))
        .take(MAX_ENTITIES)
    {
        let chain = e
            .object
            .map(|o| {
                marker_chain(o, |d| {
                    let bytes = cx.ctx.game.read(d).ok().flatten()?;
                    let ed = EntityDescriptor::parse(&bytes, dict).ok()?;
                    Some((ed.properties, Some(ed.header.template)))
                })
            })
            .unwrap_or_default();
        let refs: Vec<&PropertySet> = chain.iter().collect();
        let params =
            DecalParams::collect(&cx.s.decals, default_material, &refs, e.properties.as_ref());
        let (centre, tris) = match e.placement {
            Some(pl) => {
                let b = DecalBox::new(pl.position, pl.rotation, params.ortho_scale);
                let m = build_mesh(&params, &b, preview(&b), None);
                (format!("{:.2?}", b.center), m.triangle_count().to_string())
            }
            None => ("—".into(), "—".into()),
        };
        decals.push(
            vec![
                e.iid.map_or_else(|| "—".into(), iid_text),
                did_text(params.material),
                l.enum_key(cx.s.decals.lighting_blend, params.lighting_blend),
                l.enum_key(cx.s.decals.lod_class, params.lod_class),
                format!("{:?}", params.ortho_scale),
                centre,
                tris,
            ],
            vec![params.material.0],
        );
    }
    card.add(decals);

    // Gating (world-logic.md §13 items 1–2) with the empty journal of D1.
    let q = &cx.s.logic.quest;
    let box_did = uniquedb
        .and_then(|m| m.get(PHASING_BOX_UNIQUEDB_KEY))
        .map(|e| e.value);
    let boxes = landblock_boxes(&info, q, box_did);
    let empty = MapJournal::default();
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: &empty,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    let mut gating = Section::new(
        "Phasing boxes (world-logic.md §4, empty journal of D1)",
        &[
            "box",
            "kind",
            "checks",
            "operator",
            "True_Draw",
            "flags",
            "test",
            "targets",
        ],
    );
    for (kind, list) in [
        ("SED", &boxes.sed_boxes),
        ("spawn-only (D2)", &boxes.spawn_boxes),
    ] {
        for b in list.iter().take(MAX_ENTITIES) {
            let checks: Vec<String> = b
                .fields
                .checks
                .iter()
                .flatten()
                .map(|c| {
                    format!(
                        "{} {} {:#010x}",
                        did_text(c.quest),
                        l.enum_key(q.quest_operator, c.operator),
                        c.status.0
                    )
                })
                .collect();
            let shown = b.fields.test(true, Some(&player));
            gating.push(
                vec![
                    iid_text(b.iid),
                    kind.into(),
                    checks.join("; "),
                    l.enum_key(q.check_operator, b.fields.operator()),
                    b.fields.true_draw.map_or_else(
                        || "false (default of the box test)".into(),
                        |t| yes(t).into(),
                    ),
                    format!(
                        "update only on creation {}, delay refresh {}",
                        opt_yes(b.fields.update_only_on_creation),
                        opt_yes(b.fields.delay_refresh)
                    ),
                    if shown {
                        "targets shown"
                    } else {
                        "targets phased out"
                    }
                    .into(),
                    b.targets
                        .iter()
                        .map(|t| iid_text(*t))
                        .collect::<Vec<_>>()
                        .join(", "),
                ],
                b.fields
                    .checks
                    .iter()
                    .flatten()
                    .map(|c| c.quest.0)
                    .collect(),
            );
        }
    }
    if boxes.half_links > 0 {
        gating.push(
            vec![
                format!("{} cross-landblock link halves", boxes.half_links),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            Vec::new(),
        );
    }
    card.add(gating);

    // Per-entity sections (instance sets): logic wiring, scripts, dramas, riders.
    let ist_labels = &cx.s.ist;
    let mut wiring = Section::new(
        "Logic wiring (world-logic.md §6, §7)",
        &["IID", "kind", "values"],
    );
    let mut scripts = Section::new(
        "Instance scripts (instance-scripts.md §9.6)",
        &["IID", "key", "roots", "links → targets", "overrides"],
    );
    let mut drama_card = Card::default();
    let mut riders = Section::new("Riders (mounts.md §7)", &["IID", "keys"]);
    for w in info.weenies.iter().take(4 * MAX_ENTITIES) {
        let set = &w.properties;
        let id = iid_text(w.iid);
        let lg = &cx.s.logic;
        if let Some(c) = CounterBox::read(&lg.logic, set) {
            wiring.push(
                vec![id.clone(), "counter box".into(), format!("{c:?}")],
                Vec::new(),
            );
        }
        if let Some(t) = LogicBoxTimer::read(&lg.logic, set) {
            wiring.push(
                vec![id.clone(), "timer".into(), format!("{t:?}")],
                Vec::new(),
            );
        }
        if let Some(maps) = GeneratorMapping::read_all(&lg.wiring, set) {
            for m in maps {
                wiring.push(
                    vec![
                        id.clone(),
                        "generator event → callback".into(),
                        format!("{} → {}", did_text(m.event), did_text(m.callback)),
                    ],
                    vec![m.event.0, m.callback.0],
                );
            }
        }
        for c in CustomCondition::read_all(lg, set) {
            wiring.push(
                vec![
                    id.clone(),
                    format!("condition link {}", opt_text(c.link)),
                    format!("{:?}", c.forms),
                ],
                Vec::new(),
            );
        }
        let ev = EventLists::read(&lg.wiring, set);
        if !ev.is_empty() {
            wiring.push(
                vec![id.clone(), "event lists".into(), format!("{ev:?}")],
                Vec::new(),
            );
        }
        if let Some(v) = SkirmishValues::read(&lg.skirmish, set) {
            wiring.push(
                vec![id.clone(), "skirmish".into(), format!("{v:?}")],
                Vec::new(),
            );
        }
        if let Some(t) = TravelHooks::read(&lg.travel, set) {
            wiring.push(
                vec![id.clone(), "travel".into(), format!("{t:?}")],
                t.scene.map(|d| d.0).into_iter().collect(),
            );
        }
        if let Some(v) = PathMapVolume::read(&lg.travel, set, dict) {
            wiring.push(
                vec![id.clone(), "path-map volume".into(), format!("{v:?}")],
                Vec::new(),
            );
        }
        if let Some(ist) = InstanceScriptTable::read(ist_labels, set) {
            let layers = [set];
            let ts = EntityTableSet::gather(ist_labels, &layers);
            let overrides: Vec<String> = set
                .entries
                .iter()
                .filter_map(|e| {
                    let name = dict.name(e.pid)?;
                    MemberOverride::parse(name)
                        .map(|m| format!("{}@{}.{}", m.owner, m.holder, m.member))
                })
                .collect();
            for sc in &ist.scripts {
                let links: Vec<String> = sc
                    .roots
                    .iter()
                    .flat_map(|r| r.targets.iter())
                    .filter_map(|t| t.link_name.as_deref())
                    .map(|n| {
                        let to = link_targets(&info.links, w.iid, n);
                        if to.is_empty() {
                            format!("{n} → (none)")
                        } else {
                            format!(
                                "{n} → {}",
                                to.iter()
                                    .map(|t| iid_text(*t))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    })
                    .collect();
                scripts.push(
                    vec![
                        id.clone(),
                        l.enum_key(ist_labels.script_id, sc.key),
                        sc.roots.len().to_string(),
                        links.join("; "),
                        overrides.join(", "),
                    ],
                    Vec::new(),
                );
            }
            if !ts.includes.is_empty() {
                scripts.push(
                    vec![
                        id.clone(),
                        "includes".into(),
                        String::new(),
                        String::new(),
                        ts.includes
                            .iter()
                            .map(|d| did_text(*d))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ],
                    ts.includes.iter().map(|d| d.0).collect(),
                );
            }
        }
        if set.has(cx.s.drama.drama.list) {
            drama_sections(cx, set, &mut drama_card, &format!(" of {id}"));
        }
        quest_state_sections(cx, set, &mut drama_card, &format!(" of {id}"));
        if let Some(r) = RiderInstance::read(&cx.s.mounts, set) {
            let s = rider_section(cx, &r);
            let keys: Vec<String> = s.rows.iter().map(|r| r.cells.join(" ")).collect();
            riders.push(vec![id.clone(), keys.join("; ")], Vec::new());
        }
        let _ = weenies.len();
    }
    card.add(wiring);
    card.add(scripts);
    card.add(riders);
    p.sections = card.sections;
    p.sections.extend(drama_card.sections);
    p.notes.push(format!(
        "{} static entities, {} instance sets, {} links",
        info.entities.len(),
        info.weenies.len(),
        info.links.len()
    ));
    Ok(p)
}
