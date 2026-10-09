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
use std::sync::Arc;

use tessera_formats::gameplay::common::{Pid, PropertyAccess, enum_mapper_of};
use tessera_formats::gameplay::drama::{
    DramaKind, DramaList, KungFu, StartingIdle, dialog_presentation, plan_drama, script_link_port,
};
use tessera_formats::gameplay::mounts::{
    MountEffect, MountGroup, RiderInstance, TemplateMountStats, WAR_STEED_MASK,
};
use tessera_formats::gameplay::quests::{EntryGates, MapJournal, QuestRecord};
use tessera_formats::gameplay::world_logic::{
    BranchVisuals, CounterBox, CustomCondition, EventLists, GeneratorMapping, LocalPlayer,
    LogicBoxTimer, PHASING_BOX_UNIQUEDB_KEY, PathMapVolume, QuestStateBranch, QuestStateScript,
    SkirmishValues, TravelHooks,
};
use tessera_formats::landblock::{InstanceId, LandblockInfo, StaticEntityType};
use tessera_formats::script_table::NodeData;
use tessera_formats::script_table::instance::{
    EntityTableSet, INSTANCE_SWITCH_LABELS, InstanceScriptTable, MemberOverride, NodeFieldLabels,
    link_targets,
};
use tessera_formats::{
    Did, EntityDescriptor, PropertySet, PropertyValue, ScriptTable, Scriptlet, StaticEntity,
};
use tessera_world::decals::{build_mesh, marker_chain};
use tessera_world::lighting::glow::{GlowPids, GlowTableLights, GlowThresholds};
use tessera_world::phasing::landblock_boxes;

use super::card::{Card, Section, did_text, opt_did_text, opt_text, opt_yes, yes};
use super::live::{BoxLive, GlowEntity, GlowLive, JournalLive, LiveData, MAX_LIVE, status_text};
use super::{Built, Cx, Family, GameplayProduct, Object, product};
use crate::drama_play::{DramaPlayArg, DramaSource};
use crate::error::ExplorerError;
use crate::script_view::{NodeNames, dids_in_line, node_lines};

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
    b.live.ride = Some(obj.did.0);
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

/// The dramas of a set that can be played in the character viewer (`npc-drama.md` §11
/// item 4): every named, instant and idle drama of its list.
pub(super) fn drama_plays(
    cx: &Cx<'_>,
    set: &PropertySet,
    figure: Option<u32>,
    source: DramaSource,
    owner: &str,
) -> Vec<DramaPlayArg> {
    let Some(list) = DramaList::read(&cx.s.drama.drama, set) else {
        return Vec::new();
    };
    list.dramas
        .iter()
        .filter(|d| !matches!(d.kind, DramaKind::Other(_)))
        .filter_map(|d| {
            let name = d.name()?;
            Some(DramaPlayArg {
                figure,
                source,
                name: name.to_owned(),
                caption: if owner.is_empty() {
                    name.to_owned()
                } else {
                    format!("{owner}: {name}")
                },
            })
        })
        .take(MAX_LIVE)
        .collect()
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
    let status_labels = mapper_labels(cx, q.quest_status);
    s.did("quest", Some(sp.entry.quest));
    if !sp.entry.quest.is_null() {
        s.field("quest name", quest_name(cx, sp.entry.quest));
    }
    s.field("status", status_text(sp.entry.status.0, &status_labels));
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

/// Most lines of the per-instance script views of one landblock card.
const MAX_SCRIPT_LINES: usize = 20_000;
/// Most lines of one included table in a per-instance script view.
const MAX_TABLE_LINES: usize = 400;
/// Most tables of one entity's closure (include closure).
const MAX_CLOSURE: usize = 64;
/// Most quests named in the simulated journal.
const MAX_QUESTS: usize = 200;

/// The script tables decoded during one card build.
struct Tables<'c, 'a> {
    cx: &'c Cx<'a>,
    cache: HashMap<Did, Option<Arc<ScriptTable>>>,
}

impl<'c, 'a> Tables<'c, 'a> {
    fn new(cx: &'c Cx<'a>) -> Self {
        Self {
            cx,
            cache: HashMap::new(),
        }
    }

    fn get(&mut self, did: Did) -> Option<Arc<ScriptTable>> {
        if let Some(t) = self.cache.get(&did) {
            return t.clone();
        }
        let dict = self.cx.s.registry.dictionary();
        let t = self
            .cx
            .ctx
            .game
            .read(did)
            .ok()
            .flatten()
            .and_then(|b| ScriptTable::parse_expecting(&b, did, dict).ok())
            .map(Arc::new);
        self.cache.insert(did, t.clone());
        t
    }

    /// The include closure of `roots` in pre-order, each table once, bounded.
    fn closure(&mut self, roots: &[Did]) -> Vec<Did> {
        let mut out: Vec<Did> = Vec::new();
        let mut stack: Vec<Did> = roots.iter().rev().copied().collect();
        while let Some(d) = stack.pop() {
            if out.contains(&d) || out.len() >= MAX_CLOSURE {
                continue;
            }
            out.push(d);
            if let Some(t) = self.get(d) {
                for inc in t.includes.iter().rev() {
                    stack.push(*inc);
                }
            }
        }
        out
    }
}

/// The 0x47 chain of a placed object, nearest first.
fn chain_sets(cx: &Cx<'_>, object: Did) -> Vec<PropertySet> {
    let dict = cx.s.registry.dictionary();
    let mut v = marker_chain(object, |d| {
        let bytes = cx.ctx.game.read(d).ok().flatten()?;
        let ed = EntityDescriptor::parse(&bytes, dict).ok()?;
        Some((ed.properties, Some(ed.header.template)))
    });
    v.reverse();
    v
}

/// Whether a node of `table` reads property `pid` (a Switch, When or ScalarBlend on it, or
/// a driven pair with it as source; `instance-scripts.md` §5.3).
fn reads(table: &ScriptTable, pid: u32) -> bool {
    let mut found = false;
    for s in &table.scripts {
        for r in &s.roots {
            r.walk(&mut |n, _| {
                if found {
                    return;
                }
                found = match &n.data {
                    NodeData::Switch(sw) => sw.property == pid,
                    NodeData::ScalarBlend { property, .. } => *property == pid,
                    _ => false,
                } || n.base.driven.iter().any(|&(_, src)| src == pid);
            });
        }
    }
    found
}

/// The values of the entity lookup (per-instance set, then the chain) for `pids`.
fn lookup_values(
    layers: &[&PropertySet],
    pids: impl IntoIterator<Item = u32>,
) -> Vec<(u32, PropertyValue)> {
    pids.into_iter()
        .filter_map(|p| layers.iter().find_map(|l| l.get(p)).map(|v| (p, v.clone())))
        .collect()
}

/// The labels of an enum mapper and its bases.
fn mapper_labels(cx: &Cx<'_>, pid: Pid) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    let mut next = enum_mapper_of(cx.s.registry.dictionary(), pid);
    for _ in 0..16 {
        let Some(m) = next.filter(|d| !d.is_null()) else {
            break;
        };
        let Ok(Some(mapper)) = cx.ctx.game.enum_mapper(m) else {
            break;
        };
        for (k, n) in mapper.names() {
            if !out.iter().any(|(x, _)| x == k) {
                out.push((*k, n.clone()));
            }
        }
        next = Some(mapper.base);
    }
    out
}

/// A quest's name from its record, or its DID.
fn quest_name(cx: &Cx<'_>, quest: Did) -> String {
    cx.object_set(quest)
        .ok()
        .and_then(|set| {
            QuestRecord::new(&cx.s.quests, &set)
                .ok()
                .and_then(|r| r.name().map(|n| cx.labels.text(n)))
        })
        .unwrap_or_else(|| did_text(quest))
}

/// The per-instance script view of one entity (`instance-scripts.md` §9.6 items 1–3):
/// the table set in include order with every script's key, per-script properties and node
/// tree, the instance definitions, the link targets and the member overrides.
#[allow(clippy::too_many_arguments)]
fn ist_view(
    cx: &Cx<'_>,
    tables: &mut Tables<'_, '_>,
    title: &str,
    layers: &[&PropertySet],
    ist: &InstanceScriptTable,
    links: &[String],
    budget: &mut usize,
) -> Section {
    let l = &cx.labels;
    let dict = cx.s.registry.dictionary();
    let labels = &cx.s.ist;
    let mut s = Section::new(
        format!("Per-instance scripts of {title} (instance-scripts.md §9.6)"),
        &["line"],
    );
    s.preformatted = true;
    let pid = |p: u32| l.pid_name(p);
    let key = |k: u32| l.enum_key(labels.script_id, k);
    let names = NodeNames {
        pid: &pid,
        key: &key,
    };
    let mut ts = EntityTableSet::gather(labels, layers);
    ts.merge(ist, &NodeFieldLabels::resolve(dict), dict);
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "table set (§4.1): includes [{}]; start-up keys [{}]",
        ts.includes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        ts.startup_keys()
            .iter()
            .map(|k| key(*k))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let max = (*budget).min(MAX_SCRIPT_LINES);
    for inc in &ts.includes {
        let Some(t) = tables.get(*inc) else {
            lines.push(format!("[table {inc}: missing or undecodable]"));
            continue;
        };
        lines.push(format!(
            "[included table {inc}, kind {}, {} scripts, includes [{}]]",
            t.kind,
            t.scripts.len(),
            t.includes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let table_max = (lines.len() + MAX_TABLE_LINES).min(max);
        for sc in &t.scripts {
            if lines.len() >= table_max {
                lines.push(format!("  … cut at {MAX_TABLE_LINES} lines of this table"));
                break;
            }
            let props = t
                .script_properties
                .iter()
                .find(|(k, _)| *k == sc.key)
                .map(|(_, p)| property_text(l, p))
                .unwrap_or_default();
            lines.push(format!("  script {}{props}", key(sc.key)));
            for r in &sc.roots {
                node_lines(r, &names, 2, &mut lines, table_max);
            }
        }
    }
    lines.push(format!(
        "[instance definitions (§4.3): {} scripts{}]",
        ts.own.len(),
        if ist.replaced > 0 {
            format!(", {} repeated keys replaced", ist.replaced)
        } else {
            String::new()
        }
    ));
    for def in &ts.own {
        if lines.len() >= max {
            break;
        }
        let props = def
            .properties
            .as_ref()
            .map(|p| property_text(l, p))
            .unwrap_or_default();
        lines.push(format!("  script {}{props}", key(def.key)));
        for r in &def.roots {
            node_lines(r, &names, 2, &mut lines, max);
        }
    }
    for link in links {
        lines.push(format!("link target {link}"));
    }
    // Member overrides (§5.1, §5.3): value found and where, and the closure tables reading it.
    let closure = tables.closure(&ts.includes);
    let mut overrides: Vec<(String, String)> = Vec::new();
    if let Some(inst) = layers.first() {
        for e in &inst.entries {
            let Some(name) = dict.name(e.pid) else {
                continue;
            };
            let Some(m) = MemberOverride::parse(name) else {
                continue;
            };
            let readers: Vec<String> = closure
                .iter()
                .filter(|d| tables.get(**d).is_some_and(|t| reads(&t, e.pid)))
                .map(ToString::to_string)
                .collect();
            let (value, _, _) = l.value(e.pid, &e.value);
            overrides.push((
                format!("{}@{}", m.owner, m.holder),
                format!(
                    "  {} = {value} (instance set); read by [{}]",
                    m.member,
                    readers.join(", ")
                ),
            ));
        }
    }
    overrides.sort();
    let mut last = String::new();
    for (group, line) in overrides {
        if group != last {
            lines.push(format!("member overrides of {group} (§5.1)"));
            last = group;
        }
        lines.push(line);
    }
    if lines.len() >= max {
        lines.push(format!("… cut at {max} lines"));
        s.truncated = true;
    }
    *budget = budget.saturating_sub(lines.len());
    for line in lines {
        let links = dids_in_line(&line);
        s.push(vec![line], links);
    }
    s
}

fn property_text(l: &super::card::Labels<'_>, p: &PropertySet) -> String {
    if p.entries.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = p
        .entries
        .iter()
        .take(8)
        .map(|e| {
            let (v, label, _) = l.value(e.pid, &e.value);
            if label.is_empty() {
                format!("{} = {v}", l.pid_name(e.pid))
            } else {
                format!("{} = {v} ({label})", l.pid_name(e.pid))
            }
        })
        .collect();
    format!(" {{{}}}", parts.join(", "))
}

/// The card of a landblock-info record: decals, gating, logic wiring, instance scripts and
/// switches, light-glow objects, dramas and riders; with the interactive parts (decal
/// previews, day and night lights, simulated journal, drama player).
#[allow(clippy::too_many_lines)]
pub(super) fn landblock_product(cx: &Cx<'_>, did: u32) -> Result<GameplayProduct, ExplorerError> {
    let raw = crate::decoders::read_raw(cx.ctx, did, None)?;
    let dict = cx.s.registry.dictionary();
    let info = LandblockInfo::parse_expecting(&raw.bytes, Did(did), Some(raw.entry.stamp), dict)?;
    let mut p = product(did, Family::Logic, "landblock logic");
    let l = &cx.labels;
    let mut live = LiveData::default();
    let sed_by_iid: HashMap<InstanceId, &StaticEntity> = info
        .entities
        .iter()
        .filter_map(|e| e.iid.map(|i| (i, e)))
        .collect();
    let uniquedb = cx.s.uniquedb.as_deref();
    let mut card = Card::default();
    let mut tables = Tables::new(cx);

    // Decals (decals.md §3, §4.2, §9.3): the same sources as the mesh view's preview.
    let decal_sources = crate::decal_preview::landblock_decals(cx.ctx, did)?;
    let mut decals = Section::new(
        "Decals (decals.md §3, §9.3; preview: the buttons above open the mesh view)",
        &[
            "choice",
            "IID",
            "material",
            "blend",
            "LOD class",
            "scale",
            "box centre",
            "preview triangles",
        ],
    );
    for (i, d) in decal_sources.iter().enumerate() {
        if i < crate::decal_preview::MAX_DECAL_CHOICES {
            live.decals.push(d.caption(i));
        }
        let (centre, tris) = match &d.volume {
            Some(b) => {
                let m = build_mesh(
                    &d.params,
                    b,
                    crate::decal_preview::preview_receiver(b),
                    None,
                );
                (format!("{:.2?}", b.center), m.triangle_count().to_string())
            }
            None => ("—".into(), "—".into()),
        };
        decals.push(
            vec![
                (i + 1).to_string(),
                d.iid.map_or_else(|| "—".into(), iid_text),
                did_text(d.params.material),
                l.enum_key(cx.s.decals.lighting_blend, d.params.lighting_blend),
                l.enum_key(cx.s.decals.lod_class, d.params.lod_class),
                format!("{:?}", d.params.ortho_scale),
                centre,
                tris,
            ],
            vec![d.params.material.0],
        );
    }
    card.add(decals);

    // Gating (world-logic.md §13 items 1–2): the boxes, evaluated live with the simulated
    // journal (the empty journal of D1 by default).
    let q = &cx.s.logic.quest;
    let box_did = uniquedb
        .and_then(|m| m.get(PHASING_BOX_UNIQUEDB_KEY))
        .map(|e| e.value);
    let boxes = landblock_boxes(&info, q, box_did);
    let status_labels = mapper_labels(cx, q.quest_status);
    let mut journal = JournalLive {
        status_labels: status_labels.clone(),
        ..JournalLive::default()
    };
    let mut quest_names: HashMap<Did, String> = HashMap::new();
    let mut gating = Section::new(
        "Phasing boxes (world-logic.md §4, §13)",
        &[
            "box",
            "kind",
            "checks",
            "operator",
            "True_Draw",
            "flags",
            "test (empty journal of D1)",
            "targets",
        ],
    );
    let empty = MapJournal::default();
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: &empty,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    for (kind, list) in [
        ("SED", &boxes.sed_boxes),
        ("spawn-only (D2)", &boxes.spawn_boxes),
    ] {
        for b in list.iter().take(MAX_ENTITIES) {
            let mut checks: Vec<String> = Vec::new();
            for c in b.fields.checks.iter().flatten() {
                let name = if let Some(n) = quest_names.get(&c.quest) {
                    n.clone()
                } else if quest_names.len() < MAX_QUESTS && !c.quest.is_null() {
                    let n = quest_name(cx, c.quest);
                    quest_names.insert(c.quest, n.clone());
                    journal.quests.push((c.quest, n.clone()));
                    n
                } else {
                    did_text(c.quest)
                };
                checks.push(format!(
                    "quest {name} [{}] {} {}",
                    did_text(c.quest),
                    l.enum_key(q.quest_operator, c.operator),
                    status_text(c.status.0, &status_labels)
                ));
            }
            let shown = b.fields.test(true, Some(&player));
            let targets: Vec<String> = b
                .targets
                .iter()
                .map(|t| {
                    let object = sed_by_iid
                        .get(t)
                        .and_then(|e| e.object)
                        .map_or_else(String::new, |o| format!(" {o}"));
                    format!("{}{object}", iid_text(*t))
                })
                .collect();
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
                    targets.join(", "),
                ],
                b.fields
                    .checks
                    .iter()
                    .flatten()
                    .map(|c| c.quest.0)
                    .collect(),
            );
            journal.boxes.push(BoxLive {
                iid: iid_text(b.iid),
                kind,
                fields: b.fields.clone(),
                checks,
                targets,
            });
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
    if !journal.boxes.is_empty() {
        live.journal = Some(journal);
    }

    // Light-glow objects (instance-scripts.md §6, §9.4): Physics SEDs whose class table has
    // the day and night Light nodes.
    live.glow = glow_live(cx, &info, &mut tables);

    // Per-entity sections (instance sets): logic wiring, scripts, switches, dramas, riders.
    let ist_labels = &cx.s.ist;
    let mut wiring = Section::new(
        "Logic wiring (world-logic.md §6, §7)",
        &["IID", "kind", "values"],
    );
    let mut switches = Section::new(
        "Instance switches (instance-scripts.md §8, §9.6 item 4)",
        &["IID", "switch", "value", "read by (closure tables)"],
    );
    let switch_pids: Vec<(&str, Pid)> = INSTANCE_SWITCH_LABELS
        .iter()
        .map(|n| (*n, Pid::resolve(dict, n)))
        .collect();
    let mut script_card = Card::default();
    let mut drama_card = Card::default();
    let mut riders = Section::new("Riders (mounts.md §7)", &["IID", "keys"]);
    let mut script_budget = MAX_SCRIPT_LINES;
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
        let sed_object = sed_by_iid.get(&w.iid).and_then(|e| e.object);
        let has_switch = switch_pids
            .iter()
            .any(|(_, pid)| pid.id().is_some_and(|p| set.get(p).is_some()));
        let ist = InstanceScriptTable::read(ist_labels, set);
        if ist.is_some() || has_switch {
            let chain = sed_object.map(|o| chain_sets(cx, o)).unwrap_or_default();
            let mut layers: Vec<&PropertySet> = vec![set];
            layers.extend(chain.iter());
            if has_switch {
                let ts = EntityTableSet::gather(ist_labels, &layers);
                let closure = tables.closure(&ts.includes);
                for (name, pid) in &switch_pids {
                    let (Some(p), Some(v)) = (pid.id(), pid.id().and_then(|p| set.get(p))) else {
                        continue;
                    };
                    let (value, label, _) = l.value(p, v);
                    let readers: Vec<Did> = closure
                        .iter()
                        .copied()
                        .filter(|d| tables.get(*d).is_some_and(|t| reads(&t, p)))
                        .collect();
                    switches.push(
                        vec![
                            id.clone(),
                            (*name).to_owned(),
                            if label.is_empty() {
                                value
                            } else {
                                format!("{value} ({label})")
                            },
                            readers
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", "),
                        ],
                        readers.iter().map(|d| d.0).collect(),
                    );
                }
            }
            if let Some(ist) = &ist {
                let links: Vec<String> = ist
                    .link_names()
                    .into_iter()
                    .map(|n| {
                        let to = link_targets(&info.links, w.iid, n);
                        if to.is_empty() {
                            format!("{n:?} → (none)")
                        } else {
                            format!(
                                "{n:?} → {}",
                                to.iter()
                                    .map(|t| {
                                        let object = sed_by_iid
                                            .get(t)
                                            .and_then(|e| e.object)
                                            .map_or_else(String::new, |o| format!(" ({o})"));
                                        format!("{}{object}", iid_text(*t))
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    })
                    .collect();
                if script_budget > 0 {
                    script_card.add(ist_view(
                        cx,
                        &mut tables,
                        &id,
                        &layers,
                        ist,
                        &links,
                        &mut script_budget,
                    ));
                }
            }
        }
        if set.has(cx.s.drama.drama.list) {
            drama_sections(cx, set, &mut drama_card, &format!(" of {id}"));
            live.dramas.extend(drama_plays(
                cx,
                set,
                sed_object.map(|o| o.0),
                DramaSource::Landblock { did, iid: w.iid.0 },
                &id,
            ));
        }
        quest_state_sections(cx, set, &mut drama_card, &format!(" of {id}"));
        if let Some(r) = RiderInstance::read(&cx.s.mounts, set) {
            let s = rider_section(cx, &r);
            let keys: Vec<String> = s.rows.iter().map(|r| r.cells.join(" ")).collect();
            riders.push(vec![id.clone(), keys.join("; ")], Vec::new());
        }
    }
    card.add(wiring);
    card.add(switches);
    card.add(riders);
    p.sections = card.sections;
    p.sections.extend(script_card.sections);
    p.sections.extend(drama_card.sections);
    live.dramas.truncate(MAX_LIVE);
    p.notes.push(format!(
        "{} static entities, {} instance sets, {} links",
        info.entities.len(),
        info.weenies.len(),
        info.links.len()
    ));
    if !live.is_empty() {
        p.live = Some(Arc::new(live));
    }
    Ok(p)
}

/// The light-glow objects of `info` (§6.1): Physics SEDs whose placed object's class
/// table holds the day and night Light nodes (`GlowTableLights::from_table`), with the
/// thresholds from the table's `Startup` scriptlet (§6.2).
fn glow_live(cx: &Cx<'_>, info: &LandblockInfo, tables: &mut Tables<'_, '_>) -> Option<GlowLive> {
    let dict = cx.s.registry.dictionary();
    let pids = GlowPids::resolve(dict);
    let present = pids.present_time?;
    let labels = &cx.s.ist;
    let weenies: HashMap<InstanceId, &PropertySet> = info
        .weenies
        .iter()
        .map(|w| (w.iid, &w.properties))
        .collect();
    let mut by_object: HashMap<Did, Option<(Did, Vec<PropertySet>)>> = HashMap::new();
    let mut lights: HashMap<Did, Option<GlowTableLights>> = HashMap::new();
    let mut thresholds: Option<Result<GlowThresholds, String>> = None;
    let member_pids: Vec<u32> = pids
        .day
        .iter()
        .chain(&pids.night)
        .chain([&pids.time_override, &pids.ignore_inside_outside])
        .flatten()
        .copied()
        .collect();
    let startup = Pid::resolve(dict, "ScriptTable_ScriptID");
    let startup_key = enum_mapper_of(dict, startup).and_then(|m| {
        tessera_formats::EnumMapper::key_in_chain_from(
            m,
            |d| cx.ctx.game.enum_mapper(d).ok().flatten(),
            "Startup",
        )
    });
    let mut entities = Vec::new();
    for e in info
        .entities
        .iter()
        .filter(|e| e.kind() == Some(StaticEntityType::Physics))
    {
        if entities.len() >= MAX_LIVE {
            break;
        }
        let Some(object) = e.object else { continue };
        let entry = by_object.entry(object).or_insert_with(|| {
            let chain = chain_sets(cx, object);
            let refs: Vec<&PropertySet> = chain.iter().collect();
            let class = refs
                .iter()
                .find_map(|l| l.get_pid(labels.class_table))
                .and_then(PropertyValue::as_struct)
                .and_then(|s| s.did_nonnull(labels.db_script_table));
            class.map(|c| (c, chain))
        });
        let Some((class, chain)) = entry.clone() else {
            continue;
        };
        let stored = lights
            .entry(class)
            .or_insert_with(|| {
                let t = tables.get(class)?;
                let g = GlowTableLights::from_table(&t, &pids);
                (g.day.is_some() && g.night.is_some()).then_some(g)
            })
            .clone();
        let Some(stored) = stored else { continue };
        if thresholds.is_none() {
            // The scriptlet the table's Startup invokes whose day-fraction tests give the
            // thresholds (§6.2).
            let t = tables.get(class);
            let mut found = Err("no light-glow scriptlet in the table's Startup".to_owned());
            if let (Some(t), Some(k)) = (t, startup_key)
                && let Some(sc) = t.script(k)
            {
                let mut files: Vec<Did> = Vec::new();
                for r in &sc.roots {
                    r.walk(&mut |n, _| {
                        if let NodeData::Scriptlet { file, .. } = &n.data
                            && !file.is_null()
                            && !files.contains(file)
                        {
                            files.push(*file);
                        }
                    });
                }
                for f in files {
                    let s = cx
                        .ctx
                        .game
                        .read(f)
                        .ok()
                        .flatten()
                        .and_then(|b| Scriptlet::parse_expecting(&b, f, dict).ok());
                    if let Some(th) = s.and_then(|s| GlowThresholds::from_scriptlet(&s, present)) {
                        found = Ok(th);
                        break;
                    }
                }
            }
            thresholds = Some(found);
        }
        let inst = e
            .iid
            .and_then(|i| weenies.get(&i).copied())
            .or(e.properties.as_ref());
        let mut layers: Vec<&PropertySet> = inst.into_iter().collect();
        layers.extend(chain.iter());
        entities.push(GlowEntity {
            iid: e.iid.map_or_else(|| "—".into(), iid_text),
            object,
            table: class,
            values: lookup_values(&layers, member_pids.iter().copied()),
            stored,
        });
    }
    if entities.is_empty() {
        return None;
    }
    let default_bool = |pid: Option<u32>| {
        pid.and_then(|p| dict.definition(p))
            .and_then(|d| match d.default.as_ref() {
                Some(PropertyValue::Bool(b)) => Some(*b),
                _ => None,
            })
            .unwrap_or(false)
    };
    let time_default = pids
        .time_override
        .and_then(|p| dict.definition(p))
        .and_then(|d| d.default.as_ref())
        .and_then(PropertyValue::as_bits)
        .and_then(|k| u32::try_from(k).ok());
    let time_labels = Pid::resolve(dict, "TimeOfDay@Entity.m_eTimeOverride");
    Some(GlowLive {
        pids,
        thresholds: thresholds.unwrap_or_else(|| Err("no light-glow table".into())),
        time_labels: mapper_labels(cx, time_labels),
        time_default,
        ignore_default: default_bool(pids.ignore_inside_outside),
        activated_defaults: (default_bool(pids.day[7]), default_bool(pids.night[7])),
        entities,
    })
}
