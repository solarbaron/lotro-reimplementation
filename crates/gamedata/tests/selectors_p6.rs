// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the "P6 progression" selector attributes and families
//! (`scenario-feed.md` §8.3, §8.6 contexts `TraitBranch` and `RecipeCategory`) over a
//! `MemorySource` install built by `common`. Every DID is invented.

mod common;

use common::*;
use tessera_gamedata::{
    Alt, Attribute, Candidate, Family, FilterValue, GameDataStore, LabelContext, SelectorQuery,
    attribute,
};
use tessera_props::MemorySource;

const C_ECON: u32 = 302;

fn s_struct(fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    v_struct(
        &fields
            .iter()
            .map(|(l, v)| (pid(l), v.clone()))
            .collect::<Vec<_>>(),
    )
}

fn structs(element: &str, items: &[Vec<u8>]) -> Vec<u8> {
    let e = pid(element);
    v_array(&items.iter().map(|v| (e, v.clone())).collect::<Vec<_>>())
}

fn placement(t: u32, branch: u32) -> Vec<u8> {
    s_struct(&[
        ("Trait_TraitTree_Trait", v_u32(t)),
        ("Trait_TraitTree_Branch", v_u32(branch)),
    ])
}

#[allow(clippy::too_many_lines)]
fn store() -> GameDataStore<MemorySource> {
    let mut s = Synth::new();
    // Traits: two point-based tree cells (tier 1, 3), one plain trait (tier 2).
    s.object(
        0x7000_0321,
        C_TRAIT,
        Some(&[
            ("Trait_Nature", v_u32(1)),
            ("Trait_Tier", v_i32(1)),
            (
                "Trait_PointBasedTrait_PointCostProgression",
                v_u32(0x7000_0E01),
            ),
        ]),
    );
    s.object(
        0x7000_0322,
        C_TRAIT,
        Some(&[
            ("Trait_Nature", v_u32(1)),
            ("Trait_Tier", v_i32(3)),
            (
                "Trait_PointBasedTrait_PointCostProgression",
                v_u32(0x7000_0E01),
            ),
        ]),
    );
    s.object(
        0x7000_0323,
        C_TRAIT,
        Some(&[("Trait_Nature", v_u32(2)), ("Trait_Tier", v_i32(2))]),
    );
    // Trait trees: tree 1 (type 1) places 321 (branch 5) and 322 (branch 6); tree 2 (type
    // 2) places 322 (branch 7).
    s.object(
        0x7000_0341,
        C_TRAIT_TREE,
        Some(&[
            ("Trait_TraitTree_TreeType", v_u32(1)),
            (
                "Trait_TraitTree_TraitArray",
                structs(
                    "Trait_TraitTree_TraitStruct",
                    &[placement(0x7000_0321, 5), placement(0x7000_0322, 6)],
                ),
            ),
        ]),
    );
    s.object(
        0x7000_0342,
        C_TRAIT_TREE,
        Some(&[
            ("Trait_TraitTree_TreeType", v_u32(2)),
            (
                "Trait_TraitTree_TraitArray",
                structs("Trait_TraitTree_TraitStruct", &[placement(0x7000_0322, 7)]),
            ),
        ]),
    );
    // Skills, inductions, effects.
    s.object(
        0x7000_0301,
        C_SKILL,
        Some(&[("Skill_InductionAction", v_u32(0x7000_0901))]),
    );
    s.object(0x7000_0901, C_INDUCTION, Some(&[]));
    s.object(0x7000_0902, C_INDUCTION, Some(&[]));
    s.object(0x7000_0903, C_INDUCTION, Some(&[]));
    s.object(0x7000_0331, C_EFFECT_INSTANT_VITAL, Some(&[]));
    s.object(0x7000_0332, C_EFFECT_INSTANT_VITAL, Some(&[]));
    // Professions, recipes, vocations, guilds.
    s.object(
        0x7000_0601,
        C_ECON,
        Some(&[("CraftProfession_TierArray", v_array(&[]))]),
    );
    s.object(
        0x7000_0602,
        C_ECON,
        Some(&[("CraftProfession_TierArray", v_array(&[]))]),
    );
    s.object(
        0x7000_0611,
        C_ECON,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0700)),
            ("CraftRecipe_Profession", v_u32(0x7000_0601)),
            ("CraftRecipe_Tier", v_u32(1)),
            ("CraftRecipe_UICategory", v_u32(7)),
            ("CraftRecipe_OneTimeRecipe", v_bool(true)),
            ("CraftRecipe_CriticalResultItem", v_u32(0x7000_0701)),
            ("CraftRecipe_RequiredFacilityEffect", v_u32(0x7000_0331)),
            ("CraftRecipe_InductionAction", v_u32(0x7000_0902)),
        ]),
    );
    s.object(
        0x7000_0612,
        C_ECON,
        Some(&[
            ("CraftRecipe_ResultItem", v_u32(0x7000_0700)),
            ("CraftRecipe_Profession", v_u32(0x7000_0602)),
            ("CraftRecipe_Tier", v_u32(2)),
            ("CraftRecipe_UICategory", v_u32(8)),
            ("CraftRecipe_CooldownDuration", v_u32(35)),
            ("CraftRecipe_CriticalResultItem", v_u32(0)),
        ]),
    );
    s.object(
        0x7000_0621,
        C_ECON,
        Some(&[(
            "CraftVocation_ProfessionArray",
            v_dids("CraftVocation_Profession", &[0x7000_0602]),
        )]),
    );
    s.object(
        0x7000_0631,
        C_ECON,
        Some(&[("CraftGuild_Profession", v_u32(0x7000_0601))]),
    );
    // Creatures: a gathering node (container) of profession 601 with an induction, a mob.
    s.object(
        0x7000_0201,
        C_MOB,
        Some(&[
            ("WeenieType", v_u32(3)),
            ("Usage_RequiredCraftProfession", v_u32(0x7000_0601)),
            ("Induction_ActionDID", v_u32(0x7000_0903)),
        ]),
    );
    s.object(0x7000_0202, C_MOB, Some(&[("WeenieType", v_u32(1))]));
    s.build()
}

fn dids(r: &[Candidate]) -> Vec<u32> {
    r.iter().map(|c| c.did.0).collect()
}

fn run(st: &GameDataStore<MemorySource>, q: &SelectorQuery, b: &[Vec<Candidate>]) -> Vec<u32> {
    dids(&st.resolve(q, b).expect("resolves").records)
}

fn all(f: Family) -> SelectorQuery {
    SelectorQuery::all(f)
}

#[test]
fn p6_families_and_attribute_table() {
    assert_eq!(Family::from_keyword("vocation"), Some(Family::Vocation));
    assert_eq!(
        Family::from_keyword("craft-guild"),
        Some(Family::CraftGuild)
    );
    assert!(Family::RECORD_FAMILIES.contains(&Family::Vocation));
    for (f, a) in [
        (Family::Trait, "point-based"),
        (Family::Trait, "tier"),
        (Family::Trait, "in-tree"),
        (Family::TraitTree, "type"),
        (Family::TraitTree, "branch"),
        (Family::Recipe, "category"),
        (Family::Recipe, "one-use"),
        (Family::Recipe, "cooldown"),
        (Family::Recipe, "critical"),
        (Family::Recipe, "facility"),
        (Family::Profession, "of"),
        (Family::CraftGuild, "profession"),
        (Family::Creature, "gathering"),
        (Family::Creature, "craft-profession"),
        (Family::Effect, "facility"),
        (Family::Hotspot, "selectable"),
        (Family::Hotspot, "facility"),
        (Family::Induction, "of"),
    ] {
        assert!(attribute(f, a).is_some(), "{f:?} {a}");
    }
    assert!(attribute(Family::Vocation, "of").is_none());
}

#[test]
fn trait_and_tree_attributes() {
    let st = store();
    let q = all(Family::Trait).filter(Attribute::PointBased, FilterValue::Bool(true));
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0321, 0x7000_0322]);
    let q = all(Family::Trait).filter(Attribute::PointBased, FilterValue::Bool(false));
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0323]);
    let q = all(Family::Trait).filter(
        Attribute::Tier,
        FilterValue::Range {
            lo: Some(2),
            hi: None,
        },
    );
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0322, 0x7000_0323]);
    // Trees by type label and by branch value (list rule).
    let q = all(Family::TraitTree).filter(
        Attribute::Type,
        FilterValue::Alternatives(vec![Alt::Label("@Hunter".into())]),
    );
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0342]);
    let q = all(Family::TraitTree).filter(
        Attribute::Branch,
        FilterValue::Alternatives(vec![Alt::Value(6), Alt::Label("@Yellow".into())]),
    );
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0341, 0x7000_0342]);
    // The cells of one tree: `trait point-based=yes in-tree=$t`.
    let trees = st
        .resolve(
            &all(Family::TraitTree).filter(
                Attribute::Type,
                FilterValue::Alternatives(vec![Alt::Value(2)]),
            ),
            &[],
        )
        .expect("trees")
        .records;
    let q = all(Family::Trait)
        .filter(Attribute::PointBased, FilterValue::Bool(true))
        .filter(Attribute::InTree, FilterValue::Binding(0));
    assert_eq!(run(&st, &q, &[trees]), vec![0x7000_0322]);
}

#[test]
fn recipe_attributes() {
    let st = store();
    let q = all(Family::Recipe).filter(
        Attribute::Category,
        FilterValue::Alternatives(vec![Alt::Label("@Swords".into())]),
    );
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0611]);
    for (a, yes) in [
        (Attribute::OneUse, 0x7000_0611),
        (Attribute::Cooldown, 0x7000_0612),
        (Attribute::Critical, 0x7000_0611),
        (Attribute::Facility, 0x7000_0611),
    ] {
        let q = all(Family::Recipe).filter(a, FilterValue::Bool(true));
        assert_eq!(run(&st, &q, &[]), vec![yes], "{a:?}=yes");
        let q = all(Family::Recipe).filter(a, FilterValue::Bool(false));
        assert_eq!(run(&st, &q, &[]).len(), 1, "{a:?}=no");
    }
    // Effects some recipe needs as its facility.
    let q = all(Family::Effect).filter(Attribute::Facility, FilterValue::Bool(true));
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0331]);
    let q = all(Family::Effect).filter(Attribute::Facility, FilterValue::Bool(false));
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0332]);
}

#[test]
fn vocation_guild_profession_and_bindings() {
    let st = store();
    assert_eq!(run(&st, &all(Family::Vocation), &[]), vec![0x7000_0621]);
    assert_eq!(run(&st, &all(Family::CraftGuild), &[]), vec![0x7000_0631]);
    let vocations = st.resolve(&all(Family::Vocation), &[]).expect("v").records;
    let q = all(Family::Profession).filter(Attribute::Of, FilterValue::Binding(0));
    assert_eq!(run(&st, &q, &[vocations]), vec![0x7000_0602]);
    let profs = st
        .resolve(
            &SelectorQuery::dids([tessera_formats::Did(0x7000_0601)]),
            &[],
        )
        .expect("p")
        .records;
    let q = all(Family::CraftGuild).filter(Attribute::Profession, FilterValue::Binding(0));
    assert_eq!(
        run(&st, &q, std::slice::from_ref(&profs)),
        vec![0x7000_0631]
    );
    // Gathering nodes of a profession.
    let q = all(Family::Creature).filter(Attribute::Gathering, FilterValue::Bool(true));
    assert_eq!(run(&st, &q, &[]), vec![0x7000_0201]);
    let q = all(Family::Creature).filter(Attribute::CraftProfession, FilterValue::Binding(0));
    assert_eq!(
        run(&st, &q, std::slice::from_ref(&profs)),
        vec![0x7000_0201]
    );
    // Inductions named by a node, a recipe and a skill.
    let bound = vec![
        Candidate::record(tessera_formats::Did(0x7000_0201)),
        Candidate::record(tessera_formats::Did(0x7000_0611)),
        Candidate::record(tessera_formats::Did(0x7000_0301)),
    ];
    let q = all(Family::Induction).filter(Attribute::Of, FilterValue::Binding(0));
    assert_eq!(
        run(&st, &q, &[bound]),
        vec![0x7000_0901, 0x7000_0902, 0x7000_0903]
    );
    // Hotspots stay unsupported: empty with a note.
    let r = st.resolve(&all(Family::Hotspot), &[]).expect("hotspots");
    assert!(r.records.is_empty() && !r.notes.is_empty());
}

#[test]
fn p6_label_contexts() {
    let st = store();
    assert_eq!(st.enum_value(LabelContext::TraitBranch, "@Red"), Some(6));
    assert_eq!(
        st.enum_value(LabelContext::RecipeCategory, "@Axes"),
        Some(8)
    );
    assert_eq!(st.enum_value(LabelContext::RecipeCategory, "@Nope"), None);
}
