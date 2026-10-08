// SPDX-License-Identifier: GPL-3.0-or-later
//! Quest and deed cards (`quests.md`): identity, level and repetition (§4), objectives and
//! roles (§5, §7), rewards (§9) and deed data (§10); the other kinds of the quest family
//! by their properties (§2.1).

use tessera_formats::gameplay::quests::{QuestFamilyKind, QuestRecord, RewardList};

use super::card::{Section, opt_did_text, opt_text, opt_yes};
use super::{Built, Cx, Object};

/// The quest-family card, when the record is of a kind §2.1–§2.3 recognises.
pub(super) fn card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let labels = &cx.s.quests;
    let kind = labels.classify_record(obj.did, obj.set, &cx.s.singletons)?;
    let q = labels;
    let mut b = Built {
        kind: format!("quest family: {kind:?}"),
        ..Built::default()
    };
    b.groups = vec![
        ("Quest properties (§4)", q.quest.all()),
        ("Rewards (§9)", q.reward.all()),
        ("Deeds (§10)", q.deed.all()),
        ("Usage (§8)", q.usage.all()),
        ("Arcs, titles, emotes, reputation (§11–§13)", q.lore.all()),
        ("World events (§14)", q.world.all()),
        ("Dialogue and directories (§7, §15)", q.npc.all()),
        ("Quest treasure (§9.4)", q.treasure.all()),
    ];
    if kind != QuestFamilyKind::Quest {
        return Some(b);
    }
    let rec = match QuestRecord::new(labels, obj.set) {
        Ok(r) => r,
        Err(e) => {
            b.notes.push(format!("quest record: {e}"));
            return Some(b);
        }
    };
    quest_sections(cx, &rec, &mut b);
    Some(b)
}

fn quest_sections(cx: &Cx<'_>, rec: &QuestRecord<'_>, b: &mut Built) {
    let l = &cx.labels;
    let q = &cx.s.quests.quest;
    let dict = cx.s.registry.dictionary();
    b.kind = if rec.is_deed() { "deed" } else { "quest" }.into();
    b.title = rec.name().map(|n| l.text(n));
    let mut s = Section::fields(if rec.is_deed() {
        "Deed (§4, §10)"
    } else {
        "Quest (§4)"
    });
    s.field(q.name.label(), l.opt_text(rec.name()));
    if let Some(d) = rec.description() {
        s.field(q.description.label(), l.text(d));
    }
    s.field(q.category.label(), l.opt_enum(q.category, rec.category()));
    if rec.scope().is_some() {
        s.field(q.scope.label(), l.opt_enum(q.scope, rec.scope()));
    }
    s.opt(q.challenge_level.label(), rec.challenge_level());
    if rec.challenge_level_override().is_some() {
        s.field(
            q.challenge_level_override.label(),
            l.property(rec.challenge_level_override()),
        );
    }
    s.field(
        "challenge level hidden",
        opt_yes(Some(rec.hides_challenge_level())),
    );
    s.opt(
        q.max_times_completable.label(),
        rec.max_times_completable(dict),
    );
    if rec.lock_type().is_some() {
        s.field(
            q.lock_type.label(),
            l.opt_enum(q.lock_type, rec.lock_type()),
        );
    }
    s.opt(q.lock_reset_days.label(), rec.lock_reset_days());
    s.opt(q.time_limit.label(), rec.time_limit());
    s.did(q.next_quest.label(), rec.next_quest());
    s.did(q.precluded_quest.label(), rec.precluded_quest());
    s.dids(q.quests_to_complete.label(), &rec.quests_to_complete());
    s.did(q.web_store_item.label(), rec.web_store_item());
    let numbering = rec.number_events_lenient();
    s.field("numbered events (§5.7)", numbering.len().to_string());
    if let Some(u) = rec.permissions() {
        s.field(
            "permissions (§8)",
            format!("{u:?}").chars().take(300).collect::<String>(),
        );
    }
    let top = rec.top_level_usage();
    s.opt("usage min level", top.min_level());
    b.card.add(s);

    let mut objectives = Section::new("Objectives (§5.1)", &["index", "description", "NPC roles"]);
    for o in rec.objectives() {
        objectives.push(
            vec![
                opt_text(o.index()),
                l.opt_text(o.description()),
                o.npc_roles().len().to_string(),
            ],
            Vec::new(),
        );
    }
    b.card.add(objectives);

    let mut roles = Section::new(
        "Roles (§7.1)",
        &["role", "action", "NPC", "objective", "success text"],
    );
    for r in rec.roles() {
        let npc = r.npc().filter(|d| !d.is_null());
        roles.push(
            vec![
                r.role_name().unwrap_or("—").to_owned(),
                r.action().map_or_else(|| "—".into(), |a| format!("{a:?}")),
                opt_did_text(npc),
                opt_text(r.objective_index()),
                l.opt_text(r.success_text()),
            ],
            npc.map(|d| d.0).into_iter().collect(),
        );
    }
    b.card.add(roles);

    let mut rw = Section::fields("Rewards (§9)");
    for list in RewardList::ALL {
        if let Some(t) = rec.tier(list) {
            rw.field(format!("{list:?} tier"), t.to_string());
        }
    }
    rw.opt("negative tier", rec.negative_tier());
    rw.did("reward-level table", rec.reward_level());
    rw.did("treasure", rec.treasure());
    rw.did("craft profession", rec.craft_profession());
    for (name, f) in [
        ("positive faction", rec.positive_faction()),
        ("negative faction", rec.negative_faction()),
    ] {
        if let Some(f) = f {
            let d = f.faction.filter(|d| !d.is_null());
            rw.push(
                vec![
                    name.into(),
                    format!("{} tier {}", opt_did_text(d), opt_text(f.tier)),
                ],
                d.map(|d| d.0).into_iter().collect(),
            );
        }
    }
    let fixed = rec.fixed_amounts();
    rw.opt("fixed experience", fixed.exp);
    rw.opt("fixed money", fixed.gold);
    rw.opt("fixed glory", fixed.glory);
    rw.opt("fixed reputation", fixed.positive_reputation);
    for g in rec.give_items() {
        let d = g.item.filter(|d| !d.is_null());
        rw.push(
            vec![
                "give item".into(),
                format!("{} × {}", opt_did_text(d), g.quantity),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    for m in rec.medal_loot() {
        let d = m.treasure.filter(|d| !d.is_null());
        rw.push(
            vec![format!("medal {}", opt_text(m.medal)), opt_did_text(d)],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    for p in rec.paper_items() {
        let d = p.item.filter(|d| !d.is_null());
        rw.push(
            vec![
                format!("paper item, tier {}", opt_text(p.tier)),
                opt_did_text(d),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    for e in rec.effects() {
        let d = e.effect.filter(|d| !d.is_null());
        rw.push(
            vec![
                format!("effect, role {}", opt_text(e.apply_role)),
                format!("{} spellcraft {}", opt_did_text(d), opt_text(e.spellcraft)),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    rw.field("loot-table entries", rec.loot_table().len().to_string());
    b.card.add(rw);

    if let Some(d) = rec.deed() {
        let dp = &cx.s.quests.deed;
        let mut s = Section::fields("Deed (§10)");
        s.field(dp.ui_tab.label(), l.opt_enum(dp.ui_tab, d.ui_tab));
        s.field(dp.category.label(), l.opt_enum(dp.category, d.category));
        s.opt(dp.min_level_to_start.label(), d.min_level_to_start);
        s.field(dp.show_progress_bar.label(), opt_yes(d.show_progress_bar));
        s.field(dp.can_be_accelerated.label(), opt_yes(d.can_be_accelerated));
        s.did(dp.required_trait.label(), d.required_trait);
        s.field(dp.is_hidden.label(), opt_yes(d.is_hidden));
        b.card.add(s);
    }
}
