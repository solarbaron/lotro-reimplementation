// SPDX-License-Identifier: GPL-3.0-or-later
//! What the combat widgets show, from the kept combat state and, with an install,
//! `tessera-gamedata`'s combat data (`combat-ui.md` §4–§10): frame views, effect views,
//! cast views, floating texts, selection views and quickslot views. Without an install
//! every value takes the literal and fallback rules of the spec (§13, TR25, TR29, TR30).

use std::sync::Arc;

use tessera_formats::{Did, PropertyValue};
use tessera_gamedata::combat::{Avoidance, CombatData, TargetKind};
use tessera_gamedata::tooltip::{InstallTooltipData, SkillIconSize, TextRef, TooltipData};
use tessera_net::domain::{CastKind, DeltaKind, HitResult, Ref, Shortcut};
use tessera_net::{Body, EntityId, EntityKind};
use tessera_text::Values;
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;
use tessera_ui::game::combat::{
    CastView, ClassResource, ConRank, EffectView, FloatySettings, FloatyText, FrameView,
    HealthColour, LevelShown, NameColour, SelectionView, Vital,
};
use tessera_ui::game::quickslot::{SlotKind, SlotView};

use super::state::{CombatState, EntityCombat, KeptCast, KeptEffect, KeptVital};
use crate::character::CharacterData;

/// The install's combat data for the widgets: the records, strings and tooltip engine of
/// [`CharacterData`] and `tessera-gamedata`'s [`CombatData`] over them.
pub struct CombatInstall {
    data: Arc<CharacterData>,
    combat: CombatData<InstallTooltipData<'static>>,
    pip_type: Option<u32>,
}

impl std::fmt::Debug for CombatInstall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatInstall").finish_non_exhaustive()
    }
}

impl CombatInstall {
    /// The combat data over the install facts `data`.
    ///
    /// # Errors
    ///
    /// When the property dictionary cannot be read.
    pub fn new(data: Arc<CharacterData>) -> Result<Self, tessera_gamedata::tooltip::TooltipError> {
        let td = InstallTooltipData::new(data.store(), Arc::clone(data.strings()))?;
        let pip_type = td.pid("Pip_Type");
        Ok(Self {
            combat: CombatData::new(td),
            data,
            pip_type,
        })
    }

    /// The install facts (tooltip engine, strings).
    #[must_use]
    pub fn data(&self) -> &Arc<CharacterData> {
        &self.data
    }

    /// `tessera-gamedata`'s combat data.
    #[must_use]
    pub fn combat(&self) -> &CombatData<InstallTooltipData<'static>> {
        &self.combat
    }

    fn tooltip_data(&self) -> &InstallTooltipData<'static> {
        self.combat.data()
    }

    /// The `UIStateID` key of a state label (decoration, border and name states).
    #[must_use]
    pub fn state_id(&self, label: &str) -> Option<u32> {
        self.tooltip_data()
            .ui_key(tessera_formats::ui_layout::UiEnum::StateId, label)
    }

    /// A StringInfo-valued property of record `did`, rendered as plain text; `None` when
    /// absent or empty.
    #[must_use]
    pub fn text_property(&self, did: Did, name: &str) -> Option<String> {
        let d = self.tooltip_data();
        let set = d.record(did).ok()??;
        let PropertyValue::StringInfo(info) = set.get(d.pid(name)?)? else {
            return None;
        };
        match d.render(TextRef::Info(info), &Values::new()) {
            Ok(Some(t)) => {
                let t = t.plain_text();
                (!t.trim().is_empty()).then_some(t)
            }
            _ => None,
        }
    }

    /// A DID-valued property of record `did` (null as `None`).
    #[must_use]
    pub fn did_property(&self, did: Did, name: &str) -> Option<Did> {
        let d = self.tooltip_data();
        let set = d.record(did).ok()??;
        match set.get(d.pid(name)?)? {
            PropertyValue::DidRef(x) if !x.is_null() => Some(*x),
            _ => None,
        }
    }

    /// A Bool-valued property of record `did`.
    #[must_use]
    pub fn bool_property(&self, did: Did, name: &str) -> Option<bool> {
        let d = self.tooltip_data();
        let set = d.record(did).ok()??;
        match set.get(d.pid(name)?)? {
            PropertyValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// A number-valued property of record `did` (Int, Int64, Float; enum and bitfield
    /// values as their number).
    #[must_use]
    pub fn number_property(&self, did: Did, name: &str) -> Option<f64> {
        let d = self.tooltip_data();
        let set = d.record(did).ok()??;
        match set.get(d.pid(name)?)? {
            PropertyValue::Int(i) => Some(f64::from(*i)),
            #[allow(clippy::cast_precision_loss)]
            PropertyValue::Int64(i) => Some(*i as f64),
            PropertyValue::Float(f) if f.is_finite() => Some(f64::from(*f)),
            PropertyValue::Enum(k) | PropertyValue::Bitfield32(k) => Some(f64::from(*k)),
            #[allow(clippy::cast_precision_loss)]
            PropertyValue::Bitfield64(k) => Some(*k as f64),
            _ => None,
        }
    }

    /// The pip type of a pip record: (`Pip_Type` key, its label) (TR10, TR11).
    #[must_use]
    pub fn pip_type(&self, did: Did) -> Option<(u32, Option<String>)> {
        let d = self.tooltip_data();
        let pid = self.pip_type?;
        let set = d.record(did).ok()??;
        let key = match set.get(pid)? {
            PropertyValue::Enum(k) => *k,
            PropertyValue::Int(i) => u32::try_from(*i).ok()?,
            _ => return None,
        };
        Some((key, d.enum_label(pid, key)))
    }

    /// The floating-text settings of `UI_FloatyText_Properties` (§6.2).
    #[must_use]
    pub fn floaty_settings(&self) -> FloatySettings {
        let f = self.combat.floaty_settings(false);
        FloatySettings {
            near_range: f.near_range,
            far_range: f.far_range,
            near_scale: f.near_range_scale,
            far_scale: f.far_range_scale,
            crit_speed_mod: f.critical_speed_mod,
            crit_initial_scale: f.critical_initial_scale,
            crit_shrink_time: f.critical_shrink_time,
            crit_h_offset: f.critical_horizontal_offset,
            crit_v_offset: f.critical_vertical_offset,
        }
        .sanitized()
    }
}

/// The record a ref names.
#[must_use]
pub fn record(r: &Ref) -> Option<Did> {
    match r {
        Ref::Record(d) => Some(Did(d.0)),
        Ref::Literal(_) => None,
    }
}

/// The text of a ref for fallback looks: the literal, or the record's name (or its DID).
#[must_use]
pub fn ref_label(r: &Ref, install: Option<&CombatInstall>, name_property: &str) -> String {
    match r {
        Ref::Literal(s) => s.clone(),
        Ref::Record(d) => install
            .and_then(|i| i.text_property(Did(d.0), name_property))
            .unwrap_or_default(),
    }
}

/// The examination icon of a skill or effect record (`skill-tooltips.md` §11.1:
/// `Skill_SmallIcon` or `Effect_Icon`), for the icon rows of its tooltip.
#[must_use]
pub fn exam_icon(data: &CharacterData, did: Did) -> Option<IconImage> {
    use tessera_gamedata::tooltip::CombatSubject;
    let e = data.engine();
    e.examination_icon(&CombatSubject::Skill(did))
        .or_else(|| e.examination_icon(&CombatSubject::Effect(did)))
        .map(surface_icon)
}

/// The skill or effect tooltip of `model` converted for the examination panel, with the
/// record's examination icon in its icon rows; `None` for a literal when the install has
/// no literal frame.
#[must_use]
pub fn exam_tooltip(
    data: &CharacterData,
    model: &tessera_gamedata::tooltip::TooltipModel,
    key: tessera_ui::game::examine::ExamKey,
) -> Option<tessera_ui::game::examine::ExamTooltip> {
    crate::character::tooltip::convert_with(data, model, key, &|_, did| exam_icon(data, did))
}

/// A single-surface icon.
#[must_use]
pub fn surface_icon(did: Did) -> IconImage {
    IconImage::Layers(vec![ImageRef::Did(did.0)])
}

fn ui_vital(v: KeptVital) -> Vital {
    Vital::new(f64::from(v.current), f64::from(v.max))
}

/// The target kind of `gamedata` for a session kind.
#[must_use]
pub fn target_kind(k: EntityKind) -> TargetKind {
    match k {
        EntityKind::Player => TargetKind::Player,
        EntityKind::NonPlayer => TargetKind::NonPlayer,
        EntityKind::Creature => TargetKind::Creature,
        _ => TargetKind::Object,
    }
}

/// The name colour of a state label (`combat-ui.md` §5.4).
#[must_use]
pub fn name_colour_of_label(label: &str) -> NameColour {
    const CON: [&str; 9] = [
        "ConTrivial_ColorName",
        "ConInsanelyEasy_ColorName",
        "ConVeryEasy_ColorName",
        "ConEasy_ColorName",
        "ConEven_ColorName",
        "ConTough_ColorName",
        "ConVeryTough_ColorName",
        "ConInsanelyTough_ColorName",
        "ConImpossible_ColorName",
    ];
    if let Some(i) = CON.iter().position(|c| *c == label) {
        return ConRank::from_index(i).map_or(NameColour::Con(ConRank::Even), NameColour::Con);
    }
    match label {
        "Corpse_ColorName" => NameColour::Corpse,
        "NPC_ColorName" => NameColour::Npc,
        "PlayerNormal_ColorName" => NameColour::Player,
        "Tapped_ColorName" => NameColour::Tapped,
        "Object_ColorName" => NameColour::Object,
        _ => NameColour::Con(ConRank::Even),
    }
}

/// TR15: the name colour of `e` seen by a player of level `viewer_level`.
#[must_use]
pub fn name_colour(
    e: &EntityCombat,
    viewer_level: Option<u32>,
    install: Option<&CombatInstall>,
) -> NameColour {
    let levels = viewer_level
        .zip(e.level)
        .and_then(|(v, t)| Some((i32::try_from(v).ok()?, i32::try_from(t).ok()?)));
    let label = match install {
        Some(i) => i
            .combat
            .name_colour_state(target_kind(e.kind), e.defeated, levels),
        None => tessera_gamedata::combat::name_colour_state(
            target_kind(e.kind),
            e.defeated,
            levels,
            None,
        ),
    };
    name_colour_of_label(&label)
}

/// TR14: the health meter's colour.
#[must_use]
pub fn health_colour(e: &EntityCombat) -> HealthColour {
    match tessera_gamedata::combat::health_meter_state(target_kind(e.kind), e.defeated) {
        "vitalmeter_green" => HealthColour::Green,
        "vitalmeter_grey" => HealthColour::Grey,
        "vitalmeter_yellow" => HealthColour::Yellow,
        _ => HealthColour::Red,
    }
}

/// The class resource of the player (TR10, TR11, §13.4).
#[must_use]
pub fn class_resource(e: &EntityCombat, install: Option<&CombatInstall>) -> Option<ClassResource> {
    let (r, v) = e.resource.as_ref()?;
    let v = ui_vital(*v);
    let pip = record(r).and_then(|d| install?.pip_type(d));
    Some(match pip {
        Some((_, Some(label))) if label == "Wrath" => ClassResource::Wrath(v),
        Some((key, _)) => ClassResource::Pips {
            pip_type: key,
            value: v,
        },
        None => ClassResource::Plain(v),
    })
}

/// The frame view of entity `id` (E5 for the player, E6 and E7 for others; TR9–TR16).
#[must_use]
pub fn frame_view(
    state: &CombatState,
    id: EntityId,
    install: Option<&CombatInstall>,
) -> Option<FrameView> {
    let e = state.entity(id)?;
    let is_player = state.player() == Some(id);
    let mut level = e.level.map_or(LevelShown::Nothing, |l| {
        LevelShown::Level(i32::try_from(l).unwrap_or(i32::MAX))
    });
    let body = match e.body {
        Some(Body::Object(d)) => Some(Did(d.0)),
        _ => None,
    };
    let mut decoration = None;
    if let Some(i) = install {
        let tf = i.combat.target_frame(body, target_kind(e.kind));
        decoration = i.state_id(&tf.decoration);
        if body.is_some() {
            // §5.2 step 4 applies to targets with a body record (TR16).
            level = match tf.level {
                tessera_gamedata::combat::LevelDisplay::HiddenWithSkull => LevelShown::Skull,
                tessera_gamedata::combat::LevelDisplay::Hidden => LevelShown::Hidden,
                tessera_gamedata::combat::LevelDisplay::Shown => level,
            };
        }
    }
    Some(FrameView {
        entity: id.0,
        name: e.name.clone(),
        name_colour: (!is_player).then(|| name_colour(e, state.player_level(), install)),
        level,
        has_vitals: e.has_vitals(),
        health: e.health.map(ui_vital),
        power: e.power.map(ui_vital),
        health_colour: health_colour(e),
        resource: if is_player {
            class_resource(e, install)
        } else {
            None
        },
        decoration,
        class_display: None,
    })
}

/// The selection display of entity `id` (E1, E3; TR7, TR8).
#[must_use]
pub fn selection_view(
    state: &CombatState,
    id: EntityId,
    install: Option<&CombatInstall>,
    now: f64,
) -> Option<SelectionView> {
    let e = state.entity(id)?;
    let cast = e.cast.as_ref();
    Some(SelectionView {
        name: e.name.clone(),
        colour: Some(name_colour(e, state.player_level(), install)),
        health: e.health.filter(|_| e.has_vitals()).map(ui_vital),
        power: e.power.filter(|_| e.has_vitals()).map(ui_vital),
        health_colour: health_colour(e),
        cast: cast.map(|c| cast_position(c, now)),
        cast_interruptible: cast.is_some_and(|c| cast_view(c, install).interruptible),
        in_combat: state.in_combat(),
    })
}

/// TR26: the fill of a cast at `now`.
#[must_use]
pub fn cast_position(c: &KeptCast, now: f64) -> f32 {
    if c.total <= 0.0 {
        return 1.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let p = ((now - c.start) / c.total).clamp(0.0, 1.0) as f32;
    p
}

/// TR25–TR28: the cast bar of a kept cast.
#[must_use]
pub fn cast_view(c: &KeptCast, install: Option<&CombatInstall>) -> CastView {
    let (label, interruptible) = match (&c.what, install) {
        (Ref::Record(d), Some(i)) => {
            let b = i.combat.cast_bar(Did(d.0));
            (b.label.unwrap_or_default(), b.interruptible)
        }
        (Ref::Record(_), None) => (String::new(), true),
        (Ref::Literal(s), _) => {
            let b = tessera_gamedata::combat::CastBar::literal(s);
            (b.label.unwrap_or_default(), b.interruptible)
        }
    };
    CastView {
        label,
        interruptible,
        cancellable: c.kind != CastKind::Craft,
        duration: c.total,
    }
}

/// TR29, TR30: the effect view of a kept effect on `entity`.
#[must_use]
pub fn effect_view(
    entity: EntityId,
    k: &KeptEffect,
    install: Option<&CombatInstall>,
) -> EffectView {
    let source_is_other = k.source.is_some_and(|s| s != entity);
    let (icon, harmful, border_state, combat_only, effect_id) = match (record(&k.what), install) {
        (Some(d), Some(i)) => match i.combat.effect_icon(d) {
            Some(ic) => (
                ic.icon.map(surface_icon),
                ic.harmful,
                ic.border.as_deref().and_then(|b| i.state_id(b)),
                ic.combat_only,
                d.0,
            ),
            None => (None, source_is_other, None, false, d.0),
        },
        (Some(d), None) => (None, source_is_other, None, false, d.0),
        (None, _) => (None, source_is_other, None, false, 0),
    };
    EffectView {
        instance: u64::from(k.instance),
        effect_id,
        icon,
        label: ref_label(&k.what, install, "Effect_Name"),
        harmful,
        border: border_state,
        start: k.start,
        duration: k.duration,
        combat_only,
        tooltip: (effect_id != 0).then_some(effect_id),
    }
}

/// Every kept effect of `entity`, as views.
#[must_use]
pub fn effect_views(
    state: &CombatState,
    entity: EntityId,
    install: Option<&CombatInstall>,
) -> Vec<EffectView> {
    state.entity(entity).map_or_else(Vec::new, |e| {
        e.effects
            .iter()
            .map(|k| effect_view(entity, k, install))
            .collect()
    })
}

/// The avoidance of a hit result (TR19).
#[must_use]
pub fn avoidance(r: HitResult) -> Option<Avoidance> {
    Some(match r {
        HitResult::Miss => Avoidance::Miss,
        HitResult::Evade => Avoidance::Evade,
        HitResult::Parry => Avoidance::Parry,
        HitResult::Block => Avoidance::Block,
        HitResult::Resist => Avoidance::Resist,
        HitResult::Immune => Avoidance::Immune,
        _ => return None,
    })
}

/// TR19–TR21: the floating text of a `VitalDelta` (`None`: no text).
#[must_use]
pub fn floating_text(
    kind: DeltaKind,
    amount: f32,
    result: HitResult,
    on_local_player: bool,
    install: Option<&CombatInstall>,
) -> Option<FloatyText> {
    if let Some(a) = avoidance(result) {
        let word = install
            .and_then(|i| i.combat.avoidance_text(a))
            .filter(|w| !w.trim().is_empty())
            .unwrap_or_else(|| a.fallback().to_owned());
        return Some(FloatyText::avoidance(word));
    }
    let critical = result == HitResult::Critical;
    match kind {
        DeltaKind::Damage => FloatyText::damage(f64::from(amount), critical, on_local_player),
        DeltaKind::Heal => FloatyText::heal(f64::from(amount), critical),
    }
}

/// The client's key for a skill ref (equal refs, equal keys; TR35).
#[must_use]
pub fn skill_key(r: &Ref) -> u64 {
    use std::hash::{Hash, Hasher};
    // A fixed hasher so keys are the same in every run (CUI9).
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    r.hash(&mut h);
    h.finish()
}

struct Fnv(u64);

impl std::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// What an item slot shows, by item ID: its name, icon and tooltip record.
pub type ItemSlotSource<'a> = dyn Fn(u64) -> Option<(String, Option<IconImage>, Option<u32>)> + 'a;

/// TR32: the slot view of a quickslot's content.
#[must_use]
pub fn slot_view(
    content: &Shortcut,
    install: Option<&CombatInstall>,
    item_label: &ItemSlotSource<'_>,
) -> SlotView {
    match content {
        Shortcut::Skill(r) => {
            let icon = record(r)
                .and_then(|d| install?.combat.skill_icon(d, SkillIconSize::Small))
                .map(surface_icon);
            SlotView {
                kind: SlotKind::Skill(skill_key(r)),
                label: ref_label(r, install, "Skill_Name"),
                icon,
                tooltip: record(r).map(|d| d.0),
            }
        }
        Shortcut::Item(id) => {
            let (label, icon, tooltip) = item_label(id.0).unwrap_or_default();
            SlotView {
                kind: SlotKind::Item(id.0),
                label,
                icon,
                tooltip,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::domain::EffectId;
    use tessera_net::{CombatEvent, Event, Transform, Vec3};

    #[test]
    fn literal_rules_without_an_install() {
        let mut s = CombatState::new();
        let mut out = Vec::new();
        s.apply(
            &Event::WorldEntered {
                player: EntityId(1),
                name: "P".into(),
                transform: Transform::new(Vec3::ZERO, 0.0),
            },
            0.0,
            &mut out,
        );
        s.apply(
            &Event::EntitySpawned {
                id: EntityId(2),
                kind: EntityKind::Creature,
                name: "Boar".into(),
                transform: Transform::new(Vec3::new(0.0, 8.0, 0.0), 0.0),
            },
            0.0,
            &mut out,
        );
        let f = frame_view(&s, EntityId(2), None).expect("frame");
        assert!(!f.has_vitals, "TR12");
        assert_eq!(f.name_colour, Some(NameColour::Con(ConRank::Even)));
        assert_eq!(f.health_colour, HealthColour::Red);
        s.apply(
            &Event::Combat(CombatEvent::EffectApplied {
                entity: EntityId(2),
                effect: EffectId(5),
                what: Ref::Literal("Bleeding".into()),
                duration_ms: Some(4000),
                stacks: 1,
                source: Some(EntityId(1)),
            }),
            3.4,
            &mut out,
        );
        let v = effect_views(&s, EntityId(2), None);
        assert_eq!(v.len(), 1);
        assert!(v[0].harmful, "TR30: another source");
        assert_eq!(v[0].timeout(), Some(7.4));
        assert_eq!(
            floating_text(DeltaKind::Damage, 210.0, HitResult::Critical, false, None)
                .map(|t| (t.text, t.critical)),
            Some(("\u{2212}210".to_owned(), true))
        );
        assert!(floating_text(DeltaKind::Damage, 0.0, HitResult::Normal, false, None).is_none());
        assert_eq!(
            floating_text(DeltaKind::Damage, 0.0, HitResult::Parry, false, None).map(|t| t.text),
            Some("Parry".to_owned())
        );
        let c = KeptCast {
            kind: CastKind::Craft,
            what: Ref::Literal("Jab".into()),
            target: None,
            start: 2.2,
            total: 0.8,
        };
        assert!((cast_position(&c, 2.5) - 0.375).abs() < 1e-6, "§17.1");
        let cv = cast_view(&c, None);
        assert!(cv.interruptible && !cv.cancellable && cv.label == "Jab");
        assert_eq!(
            skill_key(&Ref::Literal("Jab".into())),
            skill_key(&Ref::Literal("Jab".into()))
        );
    }
}
