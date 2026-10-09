// SPDX-License-Identifier: GPL-3.0-or-later
//! Vendor, loot and recipe cards (`economy.md`): loot lists and pick-one tables with their
//! weights (§4), recipes (§5.1), barter profiles (§6.1), vendor inventories and the vendor
//! properties of NPCs (§6.3, §6.4), currency (§7); the other kinds as decoded. Invariant
//! warnings (§12) are listed with the card.

use tessera_formats::Did;
use tessera_formats::gameplay::economy::{
    BarterElement, BarterReward, EconomyKind, EconomyRecord, Ingredient, WeightedRef,
};

use super::card::{Card, Section, dump, opt_did_text, opt_text, yes};
use super::{Built, Cx, Object};

fn kind_name(k: EconomyKind) -> String {
    // `TrophyList` → "trophy list".
    let mut out = String::new();
    for (i, c) in format!("{k:?}").chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn one(d: Did) -> Vec<u32> {
    Some(d)
        .filter(|d| !d.is_null())
        .map(|d| d.0)
        .into_iter()
        .collect()
}

fn link_text(d: Did) -> String {
    opt_did_text(Some(d))
}

/// A weight's share of `total` as a percentage.
fn share(weight: i32, total: u64) -> String {
    if total == 0 || weight <= 0 {
        return "—".into();
    }
    format!("{:.2} %", f64::from(weight) * 100.0 / total as f64)
}

fn weighted(title: &str, entries: &[WeightedRef]) -> Section {
    let total: u64 = entries
        .iter()
        .map(|e| u64::try_from(e.weight).unwrap_or(0))
        .sum();
    let mut s = Section::new(title, &["target", "weight", "share"]);
    for e in entries {
        s.push(
            vec![
                link_text(e.target),
                e.weight.to_string(),
                share(e.weight, total),
            ],
            one(e.target),
        );
    }
    s
}

fn ingredients(s: &mut Section, kind: &str, list: &[Ingredient]) {
    for i in list {
        s.push(
            vec![
                kind.into(),
                link_text(i.item),
                opt_text(i.quantity),
                yes(i.consumed).into(),
                i.crit_bonus.to_string(),
            ],
            one(i.item),
        );
    }
}

/// The economy card, when the set holds a characteristic property (§3.4).
pub(super) fn card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let reader = &cx.s.economy;
    let kind = reader.kind(obj.set)?;
    let mut b = Built {
        kind: kind_name(kind),
        ..Built::default()
    };
    let p = &reader.pids;
    b.groups = vec![
        ("Loot properties (§4)", p.loot.all()),
        ("Crafting properties (§5)", p.craft.all()),
        (
            "Vendor, barter and currency properties (§6–§8)",
            p.vendor.all(),
        ),
    ];
    match reader.decode_as(kind, obj.set) {
        Ok(rec) => {
            record_sections(cx, &rec, &mut b);
            let mut warn = Section::new("Invariant warnings (§12)", &["warning"]);
            for v in rec.check() {
                warn.push(vec![format!("{v:?}")], Vec::new());
            }
            b.card.add(warn);
        }
        Err(e) => b.notes.push(format!("{}: {e}", kind_name(kind))),
    }
    Some(b)
}

fn record_sections(cx: &Cx<'_>, rec: &EconomyRecord, b: &mut Built) {
    let l = &cx.labels;
    let c = &mut b.card;
    match rec {
        EconomyRecord::TrophyList(t) => {
            let mut s = Section::fields("Trophy list (§4.1)");
            if let Some(d) = &t.description {
                s.field("description", l.text(d));
            }
            s.did("icon", t.icon);
            s.opt("max quantity", t.max_quantity);
            c.add(s);
            let mut e = Section::new(
                "Drops",
                &["target", "drop frequency", "quantity", "group drop"],
            );
            let freq = cx.s.economy.pids.loot.trophy_drop_frequency;
            for x in &t.entries {
                e.push(
                    vec![
                        link_text(x.target),
                        l.opt_enum(freq, x.drop_frequency),
                        format!(
                            "{}{}",
                            x.quantity,
                            if x.quantity_stored { "" } else { " (default)" }
                        ),
                        yes(x.group_drop).into(),
                    ],
                    one(x.target),
                );
            }
            c.add(e);
        }
        EconomyRecord::TreasureGroupProfile(t) => {
            let total: u64 = t.weights().iter().map(|&w| u64::from(w)).sum();
            let mut e = Section::new(
                "Treasure group profile (§4.2)",
                &["item", "weight", "share", "quantity"],
            );
            for x in &t.entries {
                e.push(
                    vec![
                        link_text(x.item),
                        x.weight.to_string(),
                        share(x.weight, total),
                        x.quantity.to_string(),
                    ],
                    one(x.item),
                );
            }
            c.add(e);
        }
        EconomyRecord::TreasureList(t) => c.add(weighted("Treasure list (§4.3)", &t.entries)),
        EconomyRecord::WeightedTreasureTable(t) => {
            c.add(weighted("Weighted treasure table (§4.4)", &t.entries));
        }
        EconomyRecord::Recipe(r) => {
            let mut s = Section::fields("Recipe (§5.1)");
            if let Some(n) = &r.name {
                s.field("name", l.text(n));
            }
            s.opt("variant", r.variant().map(|v| format!("{v:?}")));
            s.did("profession", r.profession);
            s.opt("tier", r.tier);
            if let Some(t) = &r.tier_array {
                s.field("tiers", format!("{t:?}"));
            }
            s.push(
                vec![
                    "result".into(),
                    format!("{} × {}", link_text(r.result), r.result_quantity),
                ],
                one(r.result),
            );
            if let Some(cr) = r.critical_result {
                s.push(
                    vec![
                        "critical result".into(),
                        format!("{} × {}", link_text(cr), r.critical_quantity),
                    ],
                    one(cr),
                );
            }
            s.opt("critical success chance", r.critical_chance);
            s.did("ingredient pack", r.ingredient_pack);
            s.opt("ingredient pack quantity", r.ingredient_pack_quantity);
            s.opt("execution time", r.execution_time);
            s.field("one-time recipe", yes(r.one_time));
            s.did("required craft guild", r.required_guild);
            s.did("required facility effect", r.required_facility);
            s.did("XP progression", r.xp_progression);
            s.did("result icon", r.result_icon);
            s.did("critical result icon", r.critical_icon);
            c.add(s);
            let mut ing = Section::new(
                "Ingredients",
                &["kind", "item", "quantity", "consumed", "critical bonus"],
            );
            ingredients(&mut ing, "required", &r.ingredients);
            ingredients(&mut ing, "optional", &r.optional_ingredients);
            c.add(ing);
            let mut mo = Section::new(
                "Multi-output (§5.1)",
                &["ingredient", "result", "critical result"],
            );
            for m in &r.multi_outputs {
                let mut links = one(m.ingredient);
                links.extend(one(m.result));
                links.extend(m.critical_result.map(|d| d.0));
                mo.push(
                    vec![
                        link_text(m.ingredient),
                        link_text(m.result),
                        opt_did_text(m.critical_result),
                    ],
                    links,
                );
            }
            c.add(mo);
            let mut ce = Section::new("Critical-success effects", &["effect", "spellcraft"]);
            for e in &r.critical_effects {
                ce.push(
                    vec![link_text(e.effect), opt_text(e.spellcraft)],
                    one(e.effect),
                );
            }
            c.add(ce);
        }
        EconomyRecord::BarterProfile(bp) => {
            let mut s = Section::fields("Barter profile (§6.1)");
            if let Some(n) = &bp.name {
                s.field("name", l.text(n));
            }
            s.field("munge level override", bp.munge_level_override.to_string());
            s.field("trades", bp.lists.len().to_string());
            c.add(s);
            let mut t = Section::new("Trades", &["trade", "costs", "reward"]);
            for (i, list) in bp.lists.iter().enumerate() {
                let mut links = Vec::new();
                let costs = list
                    .costs()
                    .map(|cost| {
                        links.extend(one(cost.item));
                        format!("{} × {}", link_text(cost.item), opt_text(cost.quantity))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let reward = match list.reward() {
                    Some(BarterReward::Item { item, quantity, .. }) => {
                        links.extend(one(*item));
                        format!("{} × {}", link_text(*item), opt_text(*quantity))
                    }
                    Some(BarterReward::Reputation { faction, tier }) => {
                        links.extend(one(*faction));
                        format!(
                            "reputation {} tier {}",
                            link_text(*faction),
                            opt_text(*tier)
                        )
                    }
                    None => "—".into(),
                };
                let others = list
                    .elements
                    .iter()
                    .filter(|e| matches!(e, BarterElement::Other(_)))
                    .count();
                t.push(
                    vec![
                        format!("{i}{}", if others > 0 { " (+other)" } else { "" }),
                        costs,
                        reward,
                    ],
                    links,
                );
            }
            c.add(t);
        }
        EconomyRecord::VendorInventory(v) => {
            let mut s = Section::new("Vendor inventory (§6.3)", &["item"]);
            for d in &v.items {
                s.push(vec![link_text(*d)], one(*d));
            }
            c.add(s);
        }
        EconomyRecord::PerkVendor(v) => {
            let mut s = Section::new("Perk vendor (§6.4)", &["perk"]);
            for d in &v.perks {
                s.push(vec![link_text(*d)], one(*d));
            }
            c.add(s);
        }
        EconomyRecord::Currency(cu) => {
            let mut s = Section::fields("Currency (§7)");
            s.opt("value (smallest units)", cu.value);
            s.did("coin item", cu.object);
            for (name, t) in [
                ("name", &cu.name),
                ("singular", &cu.singular),
                ("plural", &cu.plural),
            ] {
                if let Some(t) = t {
                    s.field(name, l.text(t));
                }
            }
            c.add(s);
        }
        EconomyRecord::CurrencyDirectory(d) => {
            let mut s = Section::new("Currency directory (§7)", &["currency"]);
            for x in &d.currencies {
                s.push(vec![link_text(*x)], one(*x));
            }
            c.add(s);
        }
        other => c.add(dump(
            format!("{} (economy.md)", kind_name(other.kind())),
            other,
        )),
    }
}

/// The vendor properties of an NPC (§6.3), on any object that carries them.
pub(super) fn vendor_sections(cx: &Cx<'_>, obj: &Object<'_>, card: &mut Card) {
    match cx.s.economy.npc_vendor(obj.set) {
        Ok(Some(v)) => {
            let mut s = Section::fields("Vendor (§6.3)");
            s.field("sell modifier", v.sell_modifier.to_string());
            s.field("sells wearable items", yes(v.sells_wearable_items));
            s.field("buys items", yes(v.buys_items));
            s.field("barter uses tabs", yes(v.barter_use_tabs));
            s.dids("inventory", &v.inventories);
            s.dids("barter profile", &v.barter_profiles);
            s.dids("offered profession", &v.offered_professions);
            s.dids("offered craft guild", &v.offered_guilds);
            s.dids("offered vocation", &v.offered_vocations);
            for d in &v.discounts {
                s.field("discount", cx.labels.pid_name(*d));
            }
            card.add(s);
            let mut items = Section::new("Vendor item list (§6.3)", &["item"]);
            for d in &v.item_list {
                items.push(vec![link_text(*d)], one(*d));
            }
            card.add(items);
        }
        Ok(None) => {}
        Err(e) => {
            let mut s = Section::fields("Vendor (§6.3)");
            s.field("error", e.to_string());
            card.add(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_names_are_words() {
        assert_eq!(kind_name(EconomyKind::TrophyList), "trophy list");
        assert_eq!(kind_name(EconomyKind::Recipe), "recipe");
        assert_eq!(share(1, 4), "25.00 %");
        assert_eq!(share(0, 4), "—");
        assert_eq!(share(3, 0), "—");
    }
}
