// SPDX-License-Identifier: GPL-3.0-or-later
//! Creature cards (`creatures.md`): templates of mobs, NPCs and gameplay containers (§4),
//! the AI and generator-profile records (§5, §6.5) and the encounter table (0x14, §7).

use tessera_formats::Record;
use tessera_formats::gameplay::creatures::{
    ActionTable, AttitudeTable, Classification, ClassificationFilterList, ColorEventTable,
    Conjunction, CreatureRecordKind, CreatureTemplate, EncounterTable, Formation, GeneratorProfile,
    ProbabilityTable, PuppetPackage, TemplateKind, displayed_level, immunity_indicators,
};

use super::card::{Section, dump, opt_did_text, opt_text, opt_yes, yes};
use super::{Built, Cx, Family, GameplayProduct, Object, product};
use crate::error::ExplorerError;

fn template_name(k: TemplateKind) -> &'static str {
    match k {
        TemplateKind::Mob => "creature template (mob)",
        TemplateKind::Npc => "creature template (NPC)",
        TemplateKind::Container => "creature template (gameplay container)",
    }
}

/// The template card, when the set's `WeenieType` marks a mob, NPC or container (§2.2).
pub(super) fn template_card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let tp = &cx.s.creatures.template;
    let kind = cx.s.creature_weenie.of_set(obj.set, tp.weenie_type)?;
    let l = &cx.labels;
    let t = CreatureTemplate::read(obj.set, tp);
    let id = &t.identity;
    let mut b = Built {
        kind: template_name(kind).into(),
        title: id.name.as_ref().map(|n| l.text(n)),
        ..Built::default()
    };

    let mut s = Section::fields("Identity (§4.1)");
    s.field(tp.name.label(), l.opt_text(id.name.as_ref()));
    s.field(
        tp.weenie_type.label(),
        l.opt_enum(tp.weenie_type, id.weenie_type),
    );
    s.field(tp.species.label(), l.opt_enum(tp.species, id.species));
    s.field(
        tp.subspecies.label(),
        l.opt_enum(tp.subspecies, id.subspecies),
    );
    s.field(tp.show_subspecies.label(), opt_yes(id.show_subspecies));
    s.field(tp.genus.label(), l.opt_bits(tp.genus, id.genus));
    s.field(tp.alignment.label(), l.opt_enum(tp.alignment, id.alignment));
    s.field(
        tp.agent_class.label(),
        l.opt_enum(tp.agent_class, id.agent_class),
    );
    s.did(tp.classification.label(), id.classification);
    if id.monster_division.is_some() {
        s.field(
            tp.monster_division.label(),
            l.opt_enum(tp.monster_division, id.monster_division),
        );
    }
    if id.examination_rank.is_some() {
        s.field(
            tp.examination_rank.label(),
            l.opt_enum(tp.examination_rank, id.examination_rank),
        );
    }
    if let Some(n) = &id.not_alive_name {
        s.field("not-alive name", l.text(n));
    }
    let c = &t.combat;
    s.field(
        "displayed level (§4.8)",
        displayed_level(obj.set, tp.advancement_level).to_string(),
    );
    let [a, b2, c2] = immunity_indicators(
        c.state_immunities.unwrap_or(0),
        c.conjunction_immune.unwrap_or(false),
    );
    s.field(
        "immunity indicators (§4.8)",
        format!("{} {} {}", yes(a), yes(b2), yes(c2)),
    );
    b.card.add(s);

    let pr = &t.presentation;
    let mut p = Section::fields("Presentation (§4.2)");
    p.did("physical object", pr.phys_obj);
    for (i, ap) in pr.appearances.iter().enumerate() {
        let d = ap.apr_file.filter(|d| !d.is_null());
        p.push(
            vec![
                format!("appearance [{i}]"),
                format!(
                    "key {} file {} modifier {}",
                    opt_text(ap.key),
                    opt_did_text(d),
                    opt_text(ap.modifier)
                ),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    p.opt("render scale", pr.render_scale.map(|v| format!("{v:?}")));
    p.opt("physics scale", pr.physics_scale.map(|v| format!("{v:?}")));
    p.dids("cosmetic loadout", &pr.cosmetic_loadouts);
    b.card.add(p);

    let ai = &t.ai;
    let mut tables = Section::new(
        "AI action tables (§4.3)",
        &["table", "min level", "max level"],
    );
    for at in &ai.action_tables {
        let d = Some(at.table).filter(|d| !d.is_null());
        tables.push(
            vec![
                opt_did_text(d),
                opt_text(at.min_bound()),
                opt_text(at.max_bound()),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    b.card.add(tables);
    let mut a2 = Section::fields("AI settings (§4.3)");
    a2.did("attitude table", ai.attitude_table);
    a2.opt("perception radius", ai.perception_radius);
    a2.opt("call-for-help radius", ai.call_for_help_radius);
    a2.did("call-for-help filter", ai.call_for_help_filter);
    a2.did("colour-event table", ai.color_event_table);
    a2.did("formation group", ai.formation_group);
    a2.did("puppet package", ai.puppet_package);
    a2.field("physically wanders", opt_yes(ai.physically_wanders));
    a2.opt("wandering range", ai.wandering_range);
    a2.did("combat event table", ai.combat_event_table);
    a2.did("conjunction event table", ai.conjunction_event_table);
    a2.field("combat flags (§4.4)", format!("{c:?}"));
    b.card.add(a2);

    let lo = &t.loot;
    let mut loot = Section::fields("Loot (§4.5)");
    loot.did("trophy list override", lo.trophy_list);
    loot.did("treasure list override", lo.treasure_list);
    loot.did("barter trophy list", lo.barter_trophy_list);
    loot.did("reputation trophy list", lo.reputation_trophy_list);
    loot.did("filtered trophy table", lo.filtered_trophy_table);
    loot.field("generates trophies", opt_yes(lo.generates_trophies));
    loot.field("remote lootable", opt_yes(lo.remote_lootable));
    b.card.add(loot);

    let ro = &t.roles;
    let mut roles = Section::new("Roles (§4.6)", &["role", "value"]);
    for q in &ro.active_quests {
        let d = q.quest.filter(|d| !d.is_null());
        roles.push(
            vec![
                format!("quest role {}", q.role_name.as_deref().unwrap_or("—")),
                opt_did_text(d),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    roles.did("related quest", ro.related_quest);
    roles.did("positive faction", ro.positive_faction);
    roles.did("negative faction", ro.negative_faction);
    roles.dids("comment", &ro.comments);
    if let Some(o) = &ro.occupation_title {
        roles.field("occupation title", l.text(o));
    }
    b.card.add(roles);

    let ef = &t.effects;
    let mut effects = Section::new(
        "Effects and use (§4.7)",
        &["list", "effect", "spellcraft", "damage qualifier"],
    );
    for (list, v) in [("startup", &ef.startup_effects), ("init", &ef.init_effects)] {
        for e in v {
            let d = e.effect.filter(|d| !d.is_null());
            effects.push(
                vec![
                    list.into(),
                    opt_did_text(d),
                    opt_text(e.spellcraft),
                    opt_text(e.damage_qualifier),
                ],
                d.map(|d| d.0).into_iter().collect(),
            );
        }
    }
    for (name, d) in [
        ("usage action", ef.usage_action),
        ("action handler", ef.csm_action_handler),
        ("induction action", ef.induction_action),
    ] {
        if let Some(d) = d.filter(|d| !d.is_null()) {
            effects.push(
                vec![
                    name.into(),
                    opt_did_text(Some(d)),
                    String::new(),
                    String::new(),
                ],
                vec![d.0],
            );
        }
    }
    for d in &ef.script_tables {
        effects.push(
            vec![
                "script table".into(),
                opt_did_text(Some(*d)),
                String::new(),
                String::new(),
            ],
            vec![d.0],
        );
    }
    b.card.add(effects);
    b.groups = vec![("Template properties (§4)", tp.all())];
    Some(b)
}

fn record_kind_name(k: CreatureRecordKind) -> &'static str {
    match k {
        CreatureRecordKind::ActionTable => "AI action table",
        CreatureRecordKind::ProbabilityTable => "AI probability table",
        CreatureRecordKind::AttitudeTable => "attitude table",
        CreatureRecordKind::FilterList => "classification filter list",
        CreatureRecordKind::Classification => "classification",
        CreatureRecordKind::ColorEventTable => "colour-event table",
        CreatureRecordKind::Formation => "formation",
        CreatureRecordKind::Conjunction => "conjunction",
        CreatureRecordKind::PuppetPackage => "puppet package",
        CreatureRecordKind::GeneratorProfile => "generator profile",
    }
}

/// The card of an AI or generator-profile record (§5, §6.5), recognised by signature.
pub(super) fn ai_card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let pids = &cx.s.creatures;
    let kind = CreatureRecordKind::of_set(obj.set, pids)?;
    let name = record_kind_name(kind);
    let set = obj.set;
    let a = &pids.ai;
    let x = &pids.exceptions;
    let title = format!("{name} (creatures.md)");
    let decoded = match kind {
        CreatureRecordKind::ActionTable => ActionTable::read(set, a).map(|v| dump(&title, &v)),
        CreatureRecordKind::ProbabilityTable => {
            ProbabilityTable::read(set, a).map(|v| dump(&title, &v))
        }
        CreatureRecordKind::AttitudeTable => Ok(dump(&title, &AttitudeTable::read(set, a, x))),
        CreatureRecordKind::FilterList => {
            ClassificationFilterList::read(set, a, x).map(|v| dump(&title, &v))
        }
        CreatureRecordKind::Classification => Ok(dump(&title, &Classification::read(set, a))),
        CreatureRecordKind::ColorEventTable => {
            ColorEventTable::read(set, a).map(|v| dump(&title, &v))
        }
        CreatureRecordKind::Formation => Formation::read(set, a).map(|v| dump(&title, &v)),
        CreatureRecordKind::Conjunction => Ok(dump(&title, &Conjunction::read(set, a))),
        CreatureRecordKind::PuppetPackage => Ok(dump(&title, &PuppetPackage::read(set, a))),
        CreatureRecordKind::GeneratorProfile => GeneratorProfile::read(set, pids).map(|g| {
            let mut s = dump(&title, &g);
            for issue in g.issues() {
                s.push(vec![format!("issue: {issue:?}")], Vec::new());
            }
            s
        }),
    };
    let mut b = Built {
        kind: name.into(),
        ..Built::default()
    };
    match decoded {
        Ok(s) => b.card.add(s),
        Err(e) => b.notes.push(format!("{name}: {e}")),
    }
    b.groups = vec![
        ("AI record properties (§5)", a.all()),
        ("Generator-profile properties (§6.5)", pids.profile.all()),
    ];
    Some(b)
}

/// The card of an encounter table (prefix 0x14, §7).
pub(super) fn encounter_product(cx: &Cx<'_>, did: u32) -> Result<GameplayProduct, ExplorerError> {
    let raw = crate::decoders::read_raw(cx.ctx, did, None)?;
    let t = EncounterTable::parse_expecting(&raw.bytes, did)?;
    let mut p = product(did, Family::Creature, "encounter table");
    let mut s = Section::fields("Encounter table (§7)");
    s.field("reserved (offset 4)", t.reserved.to_string());
    s.field("block-map key", t.block_map_key.to_string());
    s.field("zones", t.zones.len().to_string());
    s.field(
        "zone keys equal their indices (CR11)",
        yes(t.keys_are_indices()),
    );
    p.sections.push(s);
    let mut z = Section::new("Zones", &["key", "label", "slots", "slot fields"]);
    for zone in &t.zones {
        let fields = zone
            .slots
            .iter()
            .take(4)
            .map(|sl| {
                format!(
                    "[{} {} {} {} {} {}]",
                    sl.index, sl.field_4, sl.field_8, sl.field_12, sl.field_16, sl.field_20
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        z.push(
            vec![
                zone.key.to_string(),
                zone.label.clone(),
                zone.slots.len().to_string(),
                fields,
            ],
            Vec::new(),
        );
    }
    p.sections.push(z);
    Ok(p)
}
