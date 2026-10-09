// SPDX-License-Identifier: GPL-3.0-or-later
//! `instance <dir|archive> <DID>`: the instanced-content view of one object record
//! (`docs/specs/instances.md`). The DID may be the object record (0x70–0x77) or its property
//! set (0x78–0x7F); the property set is decoded with the dictionary, classified by its
//! characteristic property (§3.4) and shown as the matching typed view with its record-local
//! invariant report (§16).
//!
//! The command reads only. Text output lists at most `--max-list` elements per list (JSON
//! lists every element); StringInfo fields are shown as present or absent, never as text.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tessera_formats::instance::{
    Deviation, InstanceRecord, InstanceVocab, Template, decode_instance_set,
};
use tessera_formats::{Did, object_property_set_did};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::did_hex;
use crate::world::Schema;

/// Options of `instance`.
#[derive(Debug)]
pub struct Opts {
    /// Install directory or archive.
    pub target: String,
    /// Object record or property-set DID.
    pub did: u32,
    /// Most list elements shown in text output (0 = all).
    pub max_list: usize,
}

fn jd(d: Option<Did>) -> Value {
    d.map_or(Value::Null, |d| Value::String(did_hex(d.0)))
}

fn jdids(ds: &[Did]) -> Value {
    Value::Array(ds.iter().map(|d| Value::String(did_hex(d.0))).collect())
}

fn devs(d: &[Deviation]) -> Value {
    Value::Array(
        d.iter()
            .map(|x| Value::String(format!("{} {}", x.invariant, x.detail)))
            .collect(),
    )
}

fn template_json(t: &Template) -> Value {
    json!({
        "kind": format!("{:?}", t.kind()),
        "skirmish_class": t.skirmish_class,
        "name": t.name.is_some(),
        "description": t.description.is_some(),
        "content_layer": t.content_layer,
        "max_players": t.max_players,
        "public_instance": t.public_instance,
        "lock_size": t.lock_size,
        "time_dormant_limit": t.time_dormant_limit,
        "time_grace_limit": t.time_grace_limit,
        "linkdead_timeout_override": t.linkdead_timeout_override,
        "failure_timeout_override": t.failure_timeout_override,
        "length_default": t.length_default,
        "length_ripcord": jd(t.length_ripcord),
        "places": {
            "starting_position": t.places.starting_position,
            "success_location": t.places.success_location,
            "instance_milestone": t.places.instance_milestone,
            "external_milestone": t.places.external_milestone,
            "default_exit": t.places.default_exit,
            "exit_if_door": t.places.exit_if_door,
        },
        "landblocks": t.landblocks.iter().map(|b| json!([b.region, b.x, b.y])).collect::<Vec<_>>(),
        "bad_landblocks": t.bad_landblocks.len(),
        "instance_quest": jd(t.instance_quest),
        "quests_to_bestow": jdids(&t.quests_to_bestow),
        "event_control": jd(t.event_control),
        "teleport_scene": jd(t.teleport_scene),
        "intro_vo": jd(t.intro_vo),
        "permission_blobs": [t.has_default_permission, t.has_creation_permission],
        "species_loot_table": jd(t.species_loot_table),
        "awards": t.awards.iter().map(|a| json!({
            "type": a.award_type,
            "ratios": a.ratios.iter().map(|(c, r)| json!([jd(*c), r])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "tiers": t.tiers,
        "group_sizes": t.group_sizes,
        "tier_minimums": t.tier_minimums.iter().map(|c| json!([c.tier, c.group_size])).collect::<Vec<_>>(),
        "level_scale": [t.min_level_scale, t.max_level_scale],
        "level_scaling_level": t.level_scaling_level,
        "world_join": {
            "type": t.world_join_type,
            "category": t.world_join_category,
            "ticket": jd(t.world_join_ticket),
        },
        "encounter_type": t.encounter_type,
        "distant_battle_script": jd(t.distant_battle_script),
        "deviations": devs(&t.deviations()),
    })
}

fn view_json(v: &InstanceRecord) -> Value {
    let body = match v {
        InstanceRecord::Template(t) => template_json(t),
        InstanceRecord::MatchParams(m) => json!({
            "level": [m.level.0, m.level.1],
            "group_size": [m.group_size.0, m.group_size.1],
            "tier": [m.tier.0, m.tier.1],
        }),
        InstanceRecord::SkillTrainerTraits(t) => json!({"traits": jdids(&t.traits)}),
        InstanceRecord::EncounterControl(c) => json!({
            "default_encounter_timeout": c.default_encounter_timeout,
            "default_linkdead_timeout": c.default_linkdead_timeout,
            "leave_induction": jd(c.leave_induction),
            "locks": c.locks.iter().map(|r| json!({"size": r.size, "reset_days": r.reset_day_keys()})).collect::<Vec<_>>(),
            "lockable": jdids(&c.lockable),
            "restrict_max_players_exclusions": jdids(&c.restrict_max_players_exclusions),
            "pvmp_challenge": jdids(&c.pvmp_challenge),
            "entry_filters": jdids(&c.entry_filters),
            "filter_ignored": jdids(&c.filter_ignored),
        }),
        InstanceRecord::EntryFilter(f) => json!({
            "reject_message": f.has_reject_message,
            "reject_error_type": f.reject_error_type,
            "alert": f.alert,
        }),
        InstanceRecord::SkirmishControl(s) => json!({
            "difficulty_multipliers": s.difficulty_multipliers,
            "group_size_multipliers": s.group_size_multipliers,
            "point_token": jd(s.point_token),
            "trait_gates": s.trait_gates.iter().map(|(r, d)| json!([r, jd(*d)])).collect::<Vec<_>>(),
            "default_max_rank": s.default_max_rank,
            "trait_pools": s.trait_pools,
            "max_skirmishes_per_week": s.max_skirmishes_per_week,
            "blocked_stats": s.blocked_stats,
            "conversion_entries": s.conversion.len(),
            "fastest_possible_time": s.fastest_possible_time,
            "derived_stats": s.derived_stats.iter().map(|d| json!({
                "derived": d.derived, "primary": d.primary, "operations": d.operations, "scale": d.scale,
            })).collect::<Vec<_>>(),
            "deviations": devs(&s.check_conversion()),
        }),
        InstanceRecord::WorldJoinControl(w) => json!({
            "available": jdids(&w.available),
            "groups": w.groups.iter().map(|g| json!({"label": g.label, "templates": jdids(&g.templates)})).collect::<Vec<_>>(),
            "featured": w.featured.iter().map(|d| jd(*d)).collect::<Vec<_>>(),
            "featured_world_event": jd(w.featured_world_event),
            "default_ticket": jd(w.default_ticket),
            "min_level": w.min_level,
            "seasonal": w.seasonal,
            "capped": w.capped,
            "finder_order": jdids(&w.finder_order()),
            "deviations": devs(&w.deviations()),
        }),
        InstanceRecord::WorldJoinTicket(t) => json!({
            "max_encounters": jd(t.max_encounters),
            "min_time_between_launches": jd(t.min_time_between_launches),
        }),
        InstanceRecord::BigBattleControl(b) => json!({
            "initial_trait_points": b.initial_trait_points,
            "refund_progression": jd(b.refund_progression),
            "first_time_medal_bonus": b.first_time_medal_bonus,
            "medals": b.medals.iter().map(|m| json!({
                "medal": m.medal, "trait_points": m.trait_points,
                "cash_out_points_required": m.cash_out_points_required,
                "cash_out_loot": jd(m.cash_out_loot), "alert": m.alert,
            })).collect::<Vec<_>>(),
            "quest_points": b.quest_points.len(),
            "quest_encounters": b.quest_encounters.iter().map(|(q, e)| json!([jd(*q), jd(*e)])).collect::<Vec<_>>(),
            "location_events": b.location_events.iter().map(|(l, e)| json!([l, jd(*e)])).collect::<Vec<_>>(),
        }),
        InstanceRecord::DistantBattleScript(s) => json!({
            "required_event": jd(s.required_event),
            "progress_event": jd(s.progress_event),
            "waves": s.waves.iter().map(|w| json!({
                "flanks": w.flanks, "units": w.unit_count, "siege": w.siege_count,
                "requirements": w.has_requirements, "full_clear_event": jd(w.full_clear_event),
                "no_secondary_quest": w.no_secondary_quest,
            })).collect::<Vec<_>>(),
            "objectives": s.objectives.iter().map(|o| json!({
                "quest": jd(o.quest), "zone_event": jd(o.zone_event), "flank": o.flank,
                "units": o.unit_count, "waves": o.wave_count,
            })).collect::<Vec<_>>(),
            "weighted_units": s.weighted_units.iter().map(|(u, w)| json!([jd(*u), w])).collect::<Vec<_>>(),
            "weighted_siege_units": s.weighted_siege_units.iter().map(|(u, w)| json!([jd(*u), w])).collect::<Vec<_>>(),
        }),
        InstanceRecord::DistantBattleUnit(u) => json!({
            "groups": u.groups.iter().map(|g| json!({
                "unit": jd(g.unit), "cosmetic_entity": jd(g.cosmetic_entity),
                "offsets": g.offsets.len(), "visual_radius": g.visual_radius,
                "jitter": g.jitter, "appearance_entries": g.appearance_entries,
            })).collect::<Vec<_>>(),
            "formation_groups": u.formation.len(),
            "dimensions": u.dimensions,
            "width_class": u.width_class,
            "composition": u.composition.len(),
            "figures": u.figure_count(),
            "ground_clamp": u.ground_clamp(),
            "capture_quality": u.capture_quality(),
            "lane_type": u.lane_type(),
            "deviations": devs(&u.deviations(None)),
        }),
        InstanceRecord::FormationControl(f) => json!({
            "spacing_between_groups": f.spacing_between_groups,
            "widths": f.widths,
        }),
        InstanceRecord::SessionPlayControl(s) => json!({
            "slots": s.slots.iter().map(|(d, n)| json!([jd(*d), n])).collect::<Vec<_>>(),
        }),
        InstanceRecord::FellowshipMakerControl(f) => json!({
            "min_level": f.min_level,
            "valid_sizes": f.valid_sizes,
            "match_time": f.match_time,
            "bonus_thresholds": f.bonus_thresholds.iter().map(|(e, t)| json!([jd(*e), t])).collect::<Vec<_>>(),
            "random_bonus_effect": jd(f.random_bonus_effect),
            "deserter_effect": jd(f.deserter_effect),
            "excluded": jdids(&f.excluded),
        }),
        InstanceRecord::PhasingBox(p) => json!({
            "update_only_on_creation": p.update_only_on_creation,
            "delay_refresh": p.delay_refresh,
            "force_refresh_on_enter": p.force_refresh_on_enter,
        }),
        InstanceRecord::BattleStat(s) => json!({
            "name": s.has_name,
            "description": s.has_description,
            "scope": s.scope,
            "update_policy": s.update_policy,
            "replicate_to_clients": s.replicate_to_clients,
            "forward_to": jd(s.forward_to),
        }),
    };
    json!({"kind": format!("{:?}", v.kind()), "fields": body})
}

/// Writes `v` as indented `key: value` lines, cutting lists at `max` elements.
fn write_text(w: &mut dyn Write, v: &Value, indent: usize, max: usize) -> std::io::Result<()> {
    let pad = " ".repeat(indent);
    if let Value::Object(map) = v {
        for (k, x) in map {
            match x {
                Value::Object(_) => {
                    writeln!(w, "{pad}{k}:")?;
                    write_text(w, x, indent + 2, max)?;
                }
                Value::Array(a) if max > 0 && a.len() > max => {
                    let shown = Value::Array(a.iter().take(max).cloned().collect());
                    writeln!(w, "{pad}{k} ({} items, first {max}): {shown}", a.len())?;
                }
                _ => writeln!(w, "{pad}{k}: {x}")?,
            }
        }
    }
    Ok(())
}

pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let input = Input::open(&o.target, ctx)?;
    let schema = Schema::load(&input, ctx)?;
    let Some(dict) = &schema.dict else {
        bail!("the instanced-content views need the property dictionary (0x34000000)");
    };
    let did = Did(o.did);
    let set_did = match did.prefix() {
        0x70..=0x77 => object_property_set_did(did)
            .with_context(|| format!("{} has no property-set DID", did_hex(o.did)))?,
        0x78..=0x7F => did,
        _ => bail!("{} is not an object record or property set", did_hex(o.did)),
    };
    let found = input.require(set_did.0)?;
    let payload = found.read()?;
    let vocab = InstanceVocab::from_dictionary(dict);
    let (_, view) = decode_instance_set(&payload, Some(set_did), dict, &vocab)
        .with_context(|| format!("{} does not decode as a property set", did_hex(set_did.0)))?;
    let doc = match &view {
        Some(v) => {
            let mut doc = view_json(v);
            doc["did"] = Value::String(did_hex(o.did));
            doc["property_set"] = Value::String(did_hex(set_did.0));
            doc
        }
        None => json!({
            "did": did_hex(o.did),
            "property_set": did_hex(set_did.0),
            "kind": null,
            "note": "no characteristic property of the instanced-content family (instances.md §3.4)",
        }),
    };
    if out.json {
        out.json(&doc)?;
    } else {
        write_text(&mut out.w, &doc, 0, o.max_list)?;
    }
    Ok(ExitCode::SUCCESS)
}
