// SPDX-License-Identifier: GPL-3.0-or-later
//! Item cards (`items.md`): identity, binding, the stats the value, DPS and armour tables
//! give (§9.2–§9.5), stat modifiers, effect lists, requirements, sockets and links. The
//! icon layers come from the icon view's composer (`icon-layers.md`), requested by the UI.

use tessera_formats::Did;
use tessera_formats::gameplay::combat::Progression;
use tessera_formats::gameplay::items::{
    DpsTable, EffectListKind, ItemKind, ItemView, ValueTable, armour_value, damage_range,
    item_value, weapon_damage,
};

use super::card::{Section, opt_text, opt_yes, yes};
use super::{Built, Cx, Object, combat};

/// A short name of an item kind.
fn kind_name(k: ItemKind) -> &'static str {
    match k {
        ItemKind::Weapon => "weapon",
        ItemKind::RecipeScroll => "recipe scroll",
        ItemKind::MultiRecipeScroll => "multi-recipe scroll",
        ItemKind::Package => "package",
        ItemKind::TextItem => "text item",
        ItemKind::PropertyToken => "property-modifying token",
        ItemKind::Door => "door",
        ItemKind::InteractiveObject => "interactive object",
        ItemKind::Key => "key",
        ItemKind::Jewellery => "jewellery",
        ItemKind::Shield => "shield",
        ItemKind::TravelPoint => "travel point",
        ItemKind::Milestone => "milestone",
        ItemKind::Wearable => "wearable",
        ItemKind::General => "general item",
    }
}

fn list_name(k: EffectListKind) -> &'static str {
    match k {
        EffectListKind::Usage => "usage",
        EffectListKind::Equipper => "equipper",
        EffectListKind::Target => "target",
        EffectListKind::Hotspot => "hotspot",
    }
}

/// The item card, when the set carries the item signature (IT1) or a `WeenieType` that
/// gives an item kind (§2.1).
pub(super) fn card(cx: &Cx<'_>, obj: &Object<'_>) -> Option<Built> {
    let schema = &cx.s.items;
    let view = ItemView::new(obj.set, schema);
    let weenie_kind = view.weenie_type().and_then(|k| cx.s.item_weenie.kind(k));
    if !view.has_item_signature() && weenie_kind.is_none() {
        return None;
    }
    let l = &cx.labels;
    let p = &schema.item;
    let dict = cx.s.registry.dictionary();
    let kind = view.kind(&cx.s.item_weenie);
    let level = view.level_or_default(dict);
    let quality = view.quality();
    let mut b = Built {
        kind: format!("item ({})", kind_name(kind)),
        title: view.name().map(|n| l.text(n)),
        ..Built::default()
    };

    let mut sum = Section::fields("Item");
    sum.field("kind", kind_name(kind));
    sum.field("item signature (IT1)", yes(view.has_item_signature()));
    if let Some(n) = view.name() {
        sum.field(p.name.label(), l.text(n));
    }
    if let Some(d) = view.description() {
        sum.field(p.description.label(), l.text(d));
    }
    sum.field(
        p.weenie_type.label(),
        l.opt_enum(p.weenie_type, view.weenie_type()),
    );
    sum.field(
        p.item_class.label(),
        l.opt_enum(p.item_class, view.item_class()),
    );
    sum.field(p.quality.label(), l.opt_enum(p.quality, quality));
    sum.field(
        p.equipment_category.label(),
        l.opt_bits(p.equipment_category, view.equipment_category()),
    );
    sum.field("level (stored or default)", opt_text(level));
    sum.opt(p.min_level.label(), view.min_level_or_default(dict));
    sum.opt(p.max_level.label(), view.max_level());
    sum.field("binding (§5.4)", format!("{:?}", view.binding(dict)));
    sum.opt(p.max_quantity.label(), view.max_quantity());
    sum.did(p.phys_obj.label(), view.phys_obj());
    sum.did(p.external_appearance.label(), view.external_appearance());
    sum.did(p.property_set.label(), view.item_set());
    sum.did(p.granted_skill.label(), view.granted_skill());
    sum.did(p.granted_trait.label(), view.granted_trait());
    sum.did(p.recipe.label(), view.recipe());
    b.card.add(sum);

    b.card.add(stats(cx, &view, level, quality, &mut b.notes));

    b.card.add(combat::mod_section(
        cx,
        "Stat modifiers (Mod_Array)",
        &view.mods(),
        &schema.modifiers,
    ));

    let mut effects = Section::new(
        "Effects (§5.9)",
        &[
            "list",
            "effect",
            "spellcraft",
            "floor",
            "ceiling",
            "arguments",
        ],
    );
    for k in EffectListKind::ALL {
        for e in view.effects(k) {
            let d = e.effect.filter(|d| !d.is_null());
            effects.push(
                vec![
                    list_name(k).into(),
                    super::card::opt_did_text(d),
                    opt_text(e.spellcraft),
                    opt_text(e.floor),
                    opt_text(e.ceiling),
                    e.data.len().to_string(),
                ],
                d.map(|d| d.0).into_iter().collect(),
            );
        }
    }
    b.card.add(effects);

    b.card.add(requirements(cx, &view));

    let mut sockets = Section::new(
        "Sockets (§5.11)",
        &["socket", "type", "unlock item level", "well formed"],
    );
    for (i, sk) in view.sockets().iter().enumerate() {
        sockets.push(
            vec![
                i.to_string(),
                l.opt_bits(p.socket_type, sk.socket_type),
                opt_text(sk.unlock_item_level),
                yes(sk.is_well_formed()).into(),
            ],
            Vec::new(),
        );
    }
    b.card.add(sockets);

    let sb = view.socketable();
    if sb != Default::default() {
        let mut s = Section::fields("Socketable (§5.11)");
        s.field(
            p.socket_type.label(),
            l.opt_bits(p.socket_type, sb.socket_type),
        );
        s.opt(p.gem_min_level.label(), sb.gem_min_level);
        s.opt(p.gem_max_level.label(), sb.gem_max_level);
        s.opt(p.socket_levelup_increment.label(), sb.levelup_increment);
        s.did(p.socket_operation.label(), sb.operation);
        s.did(p.property_set.label(), sb.item_set);
        b.card.add(s);
    }

    let bh = view.behaviours();
    let mut beh = Section::fields("Behaviour objects (§5.10)");
    beh.did(p.usage_action.label(), bh.action);
    beh.did(p.usage_permission.label(), bh.permission);
    beh.did(p.take_permission.label(), bh.take_permission);
    beh.did(p.action_handler.label(), bh.action_handler);
    beh.opt(p.cooldown.label(), view.cooldown());
    beh.opt(p.equip_cooldown.label(), view.equip_cooldown());
    beh.opt(p.decay.label(), view.decay());
    beh.field(p.destroy_on_use.label(), opt_yes(view.destroy_on_use(dict)));
    beh.field(
        p.allowed_in_combat.label(),
        opt_yes(view.allowed_in_combat(dict)),
    );
    beh.field(
        p.allowed_while_mounted.label(),
        opt_yes(view.allowed_while_mounted(dict)),
    );
    for (i, d) in view.script_tables().iter().enumerate() {
        beh.did(format!("{} [{i}]", p.script_tables.label()), Some(*d));
    }
    b.card.add(beh);

    b.groups = vec![
        ("Item properties (items.md §5)", p.all()),
        ("Usage requirements (§5.10)", schema.requirements.all()),
    ];
    Some(b)
}

/// Value, DPS, damage and armour, read from the tables the item names (§9.2–§9.5).
fn stats(
    cx: &Cx<'_>,
    view: &ItemView<'_>,
    level: Option<i32>,
    quality: Option<u32>,
    notes: &mut Vec<String>,
) -> Section {
    let l = &cx.labels;
    let p = view.pids();
    let tables = &cx.s.items.tables;
    let mut s = Section::fields("Stats (§9)");
    s.opt(p.value.label(), view.value());
    let value_table = view.value_table().filter(|d| !d.is_null());
    s.did(p.value_table.label(), value_table);
    match (value_table, level, quality) {
        (None, Some(lv), Some(q)) => {
            s.field("value (§9.2, no table)", opt_text(item_value(None, lv, q)))
        }
        (Some(t), Some(lv), Some(q)) => {
            match cx
                .object_set(t)
                .and_then(|set| ValueTable::from_set(&set, tables).map_err(|e| e.to_string()))
            {
                Ok(vt) => {
                    s.field("value (§9.2)", opt_text(item_value(Some(&vt), lv, q)));
                    s.field("value table levels", vt.base_values.len().to_string());
                }
                Err(e) => notes.push(format!("value table {t}: {e}")),
            }
        }
        _ => {}
    }

    if let Some(w) = view.weapon() {
        s.field(
            p.damage_type.label(),
            l.opt_enum(p.damage_type, w.damage_type),
        );
        s.field(p.weapon_speed.label(), l.opt_enum(p.weapon_speed, w.speed));
        s.opt(p.base_dps.label(), w.base_dps);
        s.opt(p.damage.label(), w.damage);
        s.opt(p.damage_variance.label(), w.variance);
        s.opt(p.base_action_duration.label(), w.action_duration);
        s.opt("DPS level (§9.3)", w.dps_level);
        if let (Some(d), Some(v)) = (w.damage, w.variance) {
            let (lo, hi) = damage_range(d, v);
            s.field("damage per hit (§9.4)", format!("{lo} … {hi}"));
        }
        let dps_table = w.dps_table.filter(|d| !d.is_null());
        s.did(p.dps_table.label(), dps_table);
        if let (Some(t), Some(lv), Some(q)) = (dps_table, w.dps_level, quality) {
            match cx
                .object_set(t)
                .and_then(|set| DpsTable::from_set(&set, tables).map_err(|e| e.to_string()))
            {
                Ok(dt) => {
                    let dps = dt.dps(lv, q);
                    s.field("DPS (§9.3)", opt_text(dps));
                    if let (Some(dps), Some(dur), Some(var)) = (dps, w.action_duration, w.variance)
                    {
                        s.field("damage (§9.4)", opt_text(weapon_damage(dps, dur, var)));
                    }
                }
                Err(e) => notes.push(format!("DPS table {t}: {e}")),
            }
        }
    }

    s.opt(p.armour_value.label(), view.armour_value());
    let armour_table: Option<Did> = view.armour_table().filter(|d| !d.is_null());
    s.did(p.armour_table.label(), armour_table);
    if let (Some(t), Some(lv)) = (armour_table, level) {
        match cx.object_set(t).and_then(|set| {
            Progression::from_properties(&set, false, &cx.s.items.progressions)
                .map_err(|e| e.to_string())
        }) {
            Ok(pr) => s.field("armour (§9.5)", opt_text(armour_value(&pr, lv))),
            Err(e) => notes.push(format!("armour table {t}: {e}")),
        }
    }

    let du = view.durability();
    s.field(
        p.durability_enum.label(),
        l.opt_enum(p.durability_enum, du.durability_key),
    );
    s.opt(p.durability_value.label(), du.durability_value);
    s.field(
        p.max_structure_points_enum.label(),
        l.opt_enum(p.max_structure_points_enum, du.max_structure_points_key),
    );
    s.opt(p.max_structure_points.label(), du.max_structure_points);
    s.did(
        p.repair_cost_progression.label(),
        du.repair_cost_progression,
    );
    let dis = view.disenchant();
    s.opt(p.disenchant_value.label(), dis.value);
    s.did(p.disenchant_component.label(), dis.component);
    s.did(p.disenchant_trophy_list.label(), dis.trophy_list);
    s
}

fn requirements(cx: &Cx<'_>, view: &ItemView<'_>) -> Section {
    let l = &cx.labels;
    let r = &cx.s.items.requirements;
    let req = view.requirements();
    let mut s = Section::fields("Requirements (§5.10)");
    s.opt("min level", req.min_level);
    s.opt("max level", req.max_level);
    for (i, c) in req.classes.iter().enumerate() {
        s.field(
            format!("{} [{i}]", r.required_classes.label()),
            l.enum_key(r.required_classes, *c),
        );
    }
    for (i, race) in req.races.iter().enumerate() {
        s.field(
            format!("{} [{i}]", r.required_races.label()),
            l.enum_key(r.required_races, *race),
        );
    }
    if let Some(f) = req.faction {
        s.did(r.required_faction_did.label(), f.faction);
        s.field(
            r.required_faction_tier.label(),
            l.opt_enum(r.required_faction_tier, f.tier),
        );
    }
    s.did(r.required_trait.label(), req.required_trait);
    for (i, t) in req.required_traits.iter().enumerate() {
        let d = t.required_trait.filter(|d| !d.is_null());
        s.push(
            vec![
                format!("{} [{i}]", r.required_trait_array.label()),
                format!(
                    "{} rank {}",
                    super::card::opt_did_text(d),
                    opt_text(t.virtue_rank)
                ),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    s.did(r.barring_trait.label(), req.barring_trait);
    s.did(r.barring_skill.label(), req.barring_skill);
    s.did("barring recipe", req.barring_recipe);
    s.did(r.barring_emote.label(), req.barring_emote);
    s.dids(r.barring_trait_array.label(), &req.barring_traits);
    s.dids(r.barring_skill_array.label(), &req.barring_skills);
    s.opt(r.min_glory_rank.label(), req.min_glory_rank);
    s.did(r.required_craft_profession.label(), req.craft_profession);
    if req.craft_tier.is_some() {
        s.field(
            r.required_craft_tier.label(),
            l.opt_enum(r.required_craft_tier, req.craft_tier),
        );
    }
    if req.craft_mastery.is_some() {
        s.field(
            r.required_craft_mastery.label(),
            l.opt_enum(r.required_craft_mastery, req.craft_mastery),
        );
    }
    for (i, q) in req.quests.iter().enumerate() {
        let d = q.quest.filter(|d| !d.is_null());
        s.push(
            vec![
                format!("{} [{i}]", r.quest_requirements.label()),
                format!(
                    "operator {} quest {} status {}",
                    l.enum_key(r.quest_operator, q.operator),
                    super::card::opt_did_text(d),
                    q.status.map_or_else(|| "—".into(), |st| format!("{st:?}"))
                ),
            ],
            d.map(|d| d.0).into_iter().collect(),
        );
    }
    s.did(r.restricted_effect.label(), req.restricted_effect);
    s.did(r.required_effect.label(), req.required_effect);
    s
}
