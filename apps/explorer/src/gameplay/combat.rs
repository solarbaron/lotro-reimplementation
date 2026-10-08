// SPDX-License-Identifier: GPL-3.0-or-later
//! Skill, effect and trait cards and the other combat objects (`combat-data.md`).
//!
//! Families come from class ancestry over the class library (§2.1); without a library or a
//! root class the family signatures decide (§2.1 last column, F2), for skills, effects and
//! traits only.

use tessera_formats::gameplay::combat::{
    CombatFamily, CombatObject, Effect, EffectReference, ModEntry, ModEntryKind, ModifierPids,
    Skill, Trait, VitalCost, read_combat_object,
};
use tessera_formats::gameplay::common::{GameObjectRecord, ObjectRoot};

use super::card::{Section, dump, opt_did_text, opt_text, opt_yes};
use super::{Built, Cx, Object};

/// The combat card, when the object is a combat object.
pub(super) fn card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let pids = &cx.s.combat;
    let by_class = match (&cx.s.library, obj.root) {
        (Some(lib), Some(root)) => Some((lib, root)),
        _ => None,
    };
    let record = |root: u32| GameObjectRecord {
        root: ObjectRoot {
            did: obj.did,
            root_class: root,
        },
        properties: obj.set.clone(),
    };
    let decoded = if let Some((lib, root)) = by_class {
        let classes = &cx.s.combat_classes;
        if classes.family(lib, root).is_none() && classes.related(root).is_none() {
            return None;
        }
        read_combat_object(&record(root), &cx.s.combat_classes, lib, pids)
    } else {
        // §2.1 signatures, when ancestry cannot be checked.
        let family = [
            CombatFamily::Skill,
            CombatFamily::Effect,
            CombatFamily::Trait,
        ]
        .into_iter()
        .find(|&f| pids.has_signature(f, obj.set))?;
        let rec = record(obj.root.unwrap_or(0));
        match family {
            CombatFamily::Skill => {
                Skill::read(&rec, pids).map(|s| Some(CombatObject::Skill(Box::new(s))))
            }
            CombatFamily::Effect => {
                Effect::read(&rec, pids).map(|e| Some(CombatObject::Effect(Box::new(e))))
            }
            _ => Trait::read(&rec, pids).map(|t| Some(CombatObject::Trait(Box::new(t)))),
        }
    };
    let mut b = match decoded {
        Ok(Some(o)) => object_card(cx, &o),
        Ok(None) => return None,
        Err(e) => Built {
            kind: "combat object (unreadable)".into(),
            notes: vec![format!("combat reader: {e}")],
            ..Built::default()
        },
    };
    if by_class.is_none() {
        b.notes
            .push("recognised by its signature (no class ancestry)".into());
    }
    if let (Some((_, root)), true) = (by_class, b.kind == "effect")
        && let Some(first) = b.card.sections.first_mut()
    {
        first.field(
            "role (§6.1)",
            format!("{:?}", cx.s.combat_classes.effect_role(root)),
        );
    }
    Some(b)
}

fn kind_name(o: &CombatObject) -> &'static str {
    match o {
        CombatObject::Effect(_) => "effect",
        CombatObject::Trait(_) => "trait",
        CombatObject::Progression(_) => "progression",
        CombatObject::Skill(_) => "skill",
        CombatObject::MonsterPlayTable(_) => "monster-play table",
        CombatObject::AdvancementLine(_) => "advancement line",
        CombatObject::ClassTable(_) => "class advancement table",
        CombatObject::ThresholdPolicy(_) => "threshold policy",
        CombatObject::BattleStatistic(_) => "battle statistic",
        CombatObject::TraitTree(_) => "trait tree",
        CombatObject::SparseDidProgression(_) => "sparse DID progression",
        CombatObject::CombatControl(_) => "combat control",
        CombatObject::EffectControl(_) => "effect control",
        CombatObject::TraitControl(_) => "trait control",
        CombatObject::DpsTable(_) => "DPS table",
        CombatObject::ValueTable(_) => "value table",
        CombatObject::Induction(_) => "induction",
        CombatObject::Channelling(_) => "channelling",
    }
}

fn object_card(cx: &Cx<'_>, o: &CombatObject) -> Built {
    let kind = kind_name(o);
    match o {
        CombatObject::Skill(s) => skill_card(cx, s),
        CombatObject::Effect(e) => effect_card(cx, e),
        CombatObject::Trait(t) => trait_card(cx, t),
        other => {
            let mut b = Built {
                kind: kind.into(),
                ..Built::default()
            };
            b.card.add(dump(format!("{kind} (combat-data.md)"), other));
            b.groups = vec![("Combat properties", all_pids(cx))];
            b
        }
    }
}

/// Every combat label, for the property groups of the less common objects.
fn all_pids(cx: &Cx<'_>) -> Vec<tessera_formats::gameplay::common::Pid> {
    let p = &cx.s.combat;
    [
        p.progression.all(),
        p.traits.all(),
        p.class_table.all(),
        p.tables.all(),
        p.timing.all(),
        p.effect_extra.all(),
    ]
    .concat()
}

fn vital_rows(cx: &Cx<'_>, s: &mut Section, list: &str, costs: &[VitalCost]) {
    let l = &cx.labels;
    let p = &cx.s.combat.skill;
    for c in costs {
        let prog = c.points_progression.filter(|d| !d.is_null());
        s.push(
            vec![
                list.into(),
                l.opt_enum(p.vital_type, c.vital_type),
                opt_text(c.points),
                opt_did_text(prog),
                opt_text(c.percent),
                opt_yes(c.consumes_all),
                c.modifiers
                    .iter()
                    .map(|m| l.pid_name(*m))
                    .collect::<Vec<_>>()
                    .join(", "),
            ],
            prog.map(|d| d.0).into_iter().collect(),
        );
    }
}

fn skill_card(cx: &Cx<'_>, sk: &Skill) -> Built {
    let l = &cx.labels;
    let p = &cx.s.combat.skill;
    let h = &cx.s.combat.hook;
    let mut b = Built {
        kind: "skill".into(),
        title: Some(l.text(&sk.name)),
        ..Built::default()
    };
    let mut s = Section::fields("Skill (§7)");
    s.field(p.name.label(), l.text(&sk.name));
    if let Some(d) = &sk.desc {
        s.field(p.desc.label(), l.text(d));
    }
    s.field(p.skill_type.label(), l.bits(p.skill_type, sk.skill_type));
    s.field(p.category.label(), l.opt_enum(p.category, sk.category));
    s.field(p.harmful.label(), opt_yes(sk.harmful));
    s.field(p.is_monster_skill.label(), opt_yes(sk.is_monster_skill));
    s.field(
        p.target_selection_mode.label(),
        l.enum_key(p.target_selection_mode, sk.target_selection_mode),
    );
    let [player, pet, npc, monster] = sk.valid_targets;
    s.field(
        "valid targets (player, pet, NPC, monster)",
        format!(
            "{} {} {} {}",
            opt_yes(player),
            opt_yes(pet),
            opt_yes(npc),
            opt_yes(monster)
        ),
    );
    s.opt(p.min_range.label(), sk.min_range);
    s.opt(p.max_range.label(), sk.max_range);
    s.opt(p.area_max_targets.label(), sk.area_max_targets);
    if sk.area_anchor.is_some() {
        s.field(
            p.area_anchor.label(),
            l.opt_enum(p.area_anchor, sk.area_anchor),
        );
    }
    for v in &sk.detection_volumes {
        s.field("detection volume (§7.4)", format!("{v:?}"));
    }
    s.opt(p.heading_offset.label(), sk.heading_offset);
    s.field("hit rules (§7.6)", format!("{:?}", sk.hit));
    let t = &sk.timing;
    s.opt(p.action_duration.label(), t.action_duration);
    s.opt(p.recovery_time.label(), t.recovery_time);
    if t.recovery_channel.is_some() {
        s.field(
            p.recovery_channel.label(),
            l.opt_enum(p.recovery_channel, t.recovery_channel),
        );
    }
    s.field(p.immediate.label(), opt_yes(t.immediate));
    s.did(p.induction.label(), t.induction);
    s.did(p.channelling_state.label(), t.channelling_state);
    s.opt(p.toggle_hook_number.label(), t.toggle_hook_number);
    s.did(p.required_trait.label(), sk.required_trait);
    let icons = [p.icon, p.large_icon, p.small_icon];
    for (pid, d) in icons.iter().zip(sk.icons) {
        s.did(pid.label(), d);
    }
    b.card.add(s);

    let mut costs = Section::new(
        "Vital costs (§7.3)",
        &[
            "list",
            "vital",
            "points",
            "progression",
            "percent",
            "consumes all",
            "modifiers",
        ],
    );
    vital_rows(cx, &mut costs, p.vital_cost_list.label(), &sk.vital_costs);
    vital_rows(cx, &mut costs, p.toggle_cost_list.label(), &sk.toggle_costs);
    b.card.add(costs);

    let mut hooks = Section::new(
        "Attack hooks (§7.7)",
        &[
            "hook",
            "damage max",
            "progression",
            "variance",
            "damage type",
            "critical chance",
            "critical ×",
            "threat",
        ],
    );
    for (i, hk) in sk.attack_hooks.iter().enumerate() {
        let prog = hk.damage_max_progression.filter(|d| !d.is_null());
        let mut links: Vec<u32> = prog.map(|d| d.0).into_iter().collect();
        links.extend(hk.dps_add_progression.map(|d| d.0));
        hooks.push(
            vec![
                opt_text(hk.animation_hook_index.or(i64::try_from(i).ok())),
                opt_text(hk.damage_max),
                opt_did_text(prog),
                opt_text(hk.damage_max_variance),
                l.opt_enum(h.damage_type, hk.damage_type),
                opt_text(hk.critical_chance),
                opt_text(hk.critical_multiplier),
                opt_text(hk.threat_value),
            ],
            links,
        );
    }
    b.card.add(hooks);
    b.card.add(references_section(cx, &sk.references));
    b.groups = vec![
        ("Skill properties (§7)", p.all()),
        ("Attack hook properties (§7.7)", h.all()),
        ("Effect references (§6.3)", cx.s.combat.effect_ref.all()),
    ];
    b
}

fn effect_card(cx: &Cx<'_>, e: &Effect) -> Built {
    let l = &cx.labels;
    let p = &cx.s.combat.effect;
    let mut b = Built {
        kind: "effect".into(),
        title: Some(l.text(&e.name)),
        ..Built::default()
    };
    let mut s = Section::fields("Effect (§6)");
    s.field(p.name.label(), l.text(&e.name));
    s.field(
        p.definition_description.label(),
        l.text(&e.definition_description),
    );
    s.field(
        p.applied_description.label(),
        l.text(&e.applied_description),
    );
    s.did(p.icon.label(), e.icon);
    s.field(p.harmful.label(), opt_yes(e.harmful));
    s.field(p.debuff.label(), opt_yes(e.debuff));
    s.field(
        "application probability (absent ⇒ 1)",
        e.application_probability_or_default().to_string(),
    );
    s.opt(p.probability_variance.label(), e.probability_variance);
    if e.probability_additive_property.is_some() {
        s.field(
            p.probability_additive_property.label(),
            l.property(e.probability_additive_property),
        );
    }
    s.did(p.probability_progression.label(), e.probability_progression);
    if e.resist_category.is_some() {
        s.field(
            p.resist_category.label(),
            l.opt_bits(p.resist_category, e.resist_category),
        );
    }
    s.opt(p.resist_level.label(), e.resist_level);
    s.field("duration group (§6.2)", format!("{:?}", e.duration.group()));
    let d = &e.duration;
    s.opt(p.duration_interval.label(), d.interval);
    s.opt(p.duration_pulse_count.label(), d.pulse_count);
    s.did(
        p.duration_progression_interval.label(),
        d.interval_progression,
    );
    s.did(
        p.duration_progression_pulse_count.label(),
        d.pulse_count_progression,
    );
    s.field(
        p.expires_in_real_time.label(),
        opt_yes(d.expires_in_real_time),
    );
    s.field(p.combat_only.label(), opt_yes(d.combat_only));
    for m in &e.interval_modifiers {
        s.field(p.interval_modifier_list.label(), l.pid_name(*m));
    }
    if e.equivalence_class.is_some() {
        s.field(
            p.equivalence_class.label(),
            l.opt_enum(p.equivalence_class, e.equivalence_class),
        );
    }
    s.opt(p.class_priority.label(), e.class_priority);
    s.opt(p.cap.label(), e.cap);
    s.opt(p.critical_chance.label(), e.critical_chance);
    if e.vital_type.is_some() {
        s.field("vital type", l.opt_enum(p.base_vital_type, e.vital_type));
    }
    if e.damage_type.is_some() {
        s.field(
            p.damage_type.label(),
            l.opt_enum(p.damage_type, e.damage_type),
        );
    }
    s.field(p.multiplicative.label(), opt_yes(e.multiplicative));
    let amounts = [
        ("instant vital (§6.4)", &e.instant_vital),
        ("over time, initial (§6.4)", &e.over_time_initial),
        ("over time, per interval (§6.4)", &e.over_time_per_interval),
    ];
    for (name, a) in amounts {
        if let Some(a) = a {
            let prog = a.progression.filter(|d| !d.is_null());
            s.push(
                vec![name.into(), format!("{a:?}")],
                prog.map(|d| d.0).into_iter().collect(),
            );
        }
    }
    s.opt("aura (§6.5)", e.aura.map(|a| format!("{a:?}")));
    s.opt("area (§6.5)", e.area.map(|a| format!("{a:?}")));
    s.opt("skill proc (§6.5)", e.skill_proc.map(|a| format!("{a:?}")));
    s.opt("absorb bubble (§6.5)", e.bubble.map(|a| format!("{a:?}")));
    s.opt("pips (§6.5)", e.pips.map(|a| format!("{a:?}")));
    b.card.add(s);
    b.card.add(mod_section(
        cx,
        "Modifiers (§5)",
        &e.modifiers,
        &cx.s.combat.modifier,
    ));
    b.card.add(references_section(cx, &e.references));
    b.groups = vec![
        ("Effect properties (§6)", p.all()),
        ("Vital amounts (§6.4)", cx.s.combat.vital.all()),
        ("Effect data (§6.5)", cx.s.combat.effect_extra.all()),
        ("Effect references (§6.3)", cx.s.combat.effect_ref.all()),
    ];
    b
}

fn trait_card(cx: &Cx<'_>, t: &Trait) -> Built {
    let l = &cx.labels;
    let p = &cx.s.combat.traits;
    let mut b = Built {
        kind: "trait".into(),
        title: Some(l.text(&t.name)),
        ..Built::default()
    };
    let mut s = Section::fields("Trait (§8)");
    s.field(p.name.label(), l.text(&t.name));
    if let Some(d) = &t.description {
        s.field(p.description.label(), l.text(d));
    }
    if let Some(d) = &t.tooltip {
        s.field(p.tooltip.label(), l.text(d));
    }
    s.field(p.nature.label(), l.enum_key(p.nature, t.nature));
    s.field(p.category.label(), l.enum_key(p.category, t.category));
    s.field(p.tier.label(), t.tier.to_string());
    s.field(p.cost_to_slot.label(), t.cost_to_slot.to_string());
    s.did(p.icon.label(), t.icon);
    s.opt(p.minimum_level.label(), t.minimum_level);
    s.opt(p.maximum_rank.label(), t.maximum_rank);
    if t.rank_property.is_some() {
        s.field(p.rank_property.label(), l.property(t.rank_property));
    }
    s.did(
        p.rank_overlay_progression.label(),
        t.rank_overlay_progression,
    );
    s.did(p.point_cost_progression.label(), t.point_cost_progression);
    s.opt(
        p.required_character_level.label(),
        t.required_character_level,
    );
    s.dids(p.skill_array.label(), &t.skills);
    b.card.add(s);
    b.card.add(mod_section(
        cx,
        "Modifiers (§5)",
        &t.modifiers,
        &cx.s.combat.modifier,
    ));
    b.card.add(references_section(cx, &t.references));
    b.groups = vec![
        ("Trait properties (§8)", p.all()),
        ("Effect references (§6.3)", cx.s.combat.effect_ref.all()),
    ];
    b
}

/// A table of modifier entries (`combat-data.md` §5.1): the property changed, operator,
/// inline value or progression, level range.
pub(super) fn mod_section(
    cx: &Cx<'_>,
    title: &str,
    entries: &[ModEntry],
    mp: &ModifierPids,
) -> Section {
    let l = &cx.labels;
    let mut s = Section::new(
        title,
        &["property", "op", "value", "progression", "range", "entry"],
    );
    for e in entries {
        if matches!(e.kind, ModEntryKind::FieldLess) {
            continue;
        }
        let (value, links) = match &e.value {
            Some(v) => {
                let (text, label, links) = l.value(e.modified, v);
                (
                    if label.is_empty() {
                        text
                    } else {
                        format!("{text} ({label})")
                    },
                    links,
                )
            }
            None => ("—".to_owned(), Vec::new()),
        };
        let mut links = links;
        let prog = e.progression.filter(|d| !d.is_null());
        links.extend(prog.map(|d| d.0));
        let range = if e.range.is_ranged() {
            format!(
                "{} … {}",
                opt_text(e.range.floor),
                opt_text(e.range.ceiling)
            )
        } else {
            String::new()
        };
        s.push(
            vec![
                l.property(Some(e.modified)),
                l.opt_enum(mp.op, e.op),
                value,
                opt_did_text(prog),
                range,
                format!("{:?}", e.kind),
            ],
            links,
        );
    }
    s
}

/// A table of effect references (`combat-data.md` §6.3).
pub(super) fn references_section(cx: &Cx<'_>, refs: &[EffectReference]) -> Section {
    let l = &cx.labels;
    let mut s = Section::new(
        "Effect references (§6.3)",
        &["list", "kind", "effect", "spellcraft", "duration"],
    );
    for r in refs {
        let d = Some(r.effect).filter(|d| !d.is_null());
        s.push(
            vec![
                l.pid_name(r.top),
                format!("{:?}", r.kind),
                opt_did_text(d).replace('—', "placeholder"),
                opt_text(r.given_spellcraft()),
                opt_text(r.given_duration()),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    s
}
