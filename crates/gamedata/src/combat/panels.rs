// SPDX-License-Identifier: GPL-3.0-or-later
//! The data of the combat panels (`combat-ui.md` §4–§8, §10; `game-controls.md` §7.1, §7.5;
//! `interaction.md` §3.8.4, §8).

use std::collections::HashMap;

use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{Did, StringInfo};
use tessera_text::Values;

use super::{CombatData, as_bits, as_bool, as_did, as_info, as_key, elements, field, get, structs};
use crate::tooltip::{SkillIconSize, TemplateRef, TextRef, TooltipData};

/// The consider type `combat-ui.md` TR15 uses for creatures.
pub const CONSIDER_MONSTER: &str = "Monster";
/// The name state of an even consideration, and of a creature without both levels (TR15).
pub const CON_EVEN_STATE: &str = "ConEven_ColorName";
/// The name state of a defeated target (TR15 step 1).
pub const CORPSE_STATE: &str = "Corpse_ColorName";
/// The name state of an NPC (TR15 step 3).
pub const NPC_STATE: &str = "NPC_ColorName";
/// The name state of a player (TR15 step 3).
pub const PLAYER_STATE: &str = "PlayerNormal_ColorName";
/// The name state of an object (TR15 step 3).
pub const OBJECT_STATE: &str = "Object_ColorName";
/// The plain frame state of the vital displays (`combat-ui.md` TR9, `game-controls.md` §7.1).
pub const PLAIN_FRAME_STATE: &str = "normal";

/// The border states of effect icons (`combat-ui.md` §8.1): labels of the resist-category
/// mapper that are states of `TimedEffectBorderField`.
pub const BORDER_STATES: [&str; 11] = [
    "FeignDeath",
    "Disease",
    "Physical",
    "Wound",
    "Cry",
    "Song",
    "Fear",
    "Poison",
    "Magic",
    "Elemental",
    "Corruption",
];

/// The consideration table width (`game-controls.md` §7.5: 31 ranks, index *d* + 15).
const TABLE_CENTRE: i64 = 15;

// ---------------------------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------------------------

/// The kind of entity a frame shows (`combat-ui.md` TR1, TR14, TR15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// A player character.
    Player,
    /// A non-player character.
    NonPlayer,
    /// A creature.
    Creature,
    /// An object.
    Object,
}

/// The health meter's colour state (`combat-ui.md` TR14): green for players and NPCs, red for
/// creatures, grey when defeated; objects take red like the layout's default.
#[must_use]
pub fn health_meter_state(kind: TargetKind, defeated: bool) -> &'static str {
    if defeated {
        return "vitalmeter_grey";
    }
    match kind {
        TargetKind::Player | TargetKind::NonPlayer => "vitalmeter_green",
        TargetKind::Creature | TargetKind::Object => "vitalmeter_red",
    }
}

/// The name colour state of a target (`combat-ui.md` TR15): defeated → corpse; creatures by
/// consideration (`levels` = (viewer, target)), even without both levels; NPCs, players and
/// objects by kind.
#[must_use]
pub fn name_colour_state(
    kind: TargetKind,
    defeated: bool,
    levels: Option<(i32, i32)>,
    consideration: Option<&Consideration>,
) -> String {
    if defeated {
        return CORPSE_STATE.to_owned();
    }
    match kind {
        TargetKind::Creature => levels
            .and_then(|(v, t)| consideration?.state(CONSIDER_MONSTER, v, t))
            .unwrap_or(CON_EVEN_STATE)
            .to_owned(),
        TargetKind::NonPlayer => NPC_STATE.to_owned(),
        TargetKind::Player => PLAYER_STATE.to_owned(),
        TargetKind::Object => OBJECT_STATE.to_owned(),
    }
}

/// The consideration control and its tables (`game-controls.md` §7.5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Consideration {
    /// `RankToUIState`: (rank key, state label).
    pub rank_states: Vec<(u32, String)>,
    /// `CombatTableData`: (consider-type label, table DIDs by element 1, 2, …).
    pub lists: Vec<(String, Vec<Did>)>,
    /// Each table's 31 rank keys.
    pub tables: HashMap<Did, Vec<u32>>,
}

impl Consideration {
    /// The rank a viewer of `viewer_level` sees on a target of `target_level` (the
    /// data-consistent reading of `game-controls.md` §7.5, Q16 OPEN): list element by the
    /// viewer's level (element *L* for level *L*, clamped to the list), table element
    /// *d* + 15 with *d* = target − viewer, clamped to the table.
    #[must_use]
    pub fn rank(&self, consider_type: &str, viewer_level: i32, target_level: i32) -> Option<u32> {
        let (_, list) = self.lists.iter().find(|(t, _)| t == consider_type)?;
        if list.is_empty() {
            return None;
        }
        let element = usize::try_from(viewer_level.max(1))
            .unwrap_or(1)
            .min(list.len());
        let table = self.tables.get(list.get(element - 1)?)?;
        if table.is_empty() {
            return None;
        }
        let d = i64::from(target_level) - i64::from(viewer_level);
        let last = i64::try_from(table.len() - 1).unwrap_or(0);
        let i = usize::try_from((d + TABLE_CENTRE).clamp(0, last)).unwrap_or(0);
        table.get(i).copied()
    }

    /// The name state of [`Consideration::rank`] through `RankToUIState`.
    #[must_use]
    pub fn state(&self, consider_type: &str, viewer_level: i32, target_level: i32) -> Option<&str> {
        let rank = self.rank(consider_type, viewer_level, target_level)?;
        self.rank_states
            .iter()
            .find(|(r, _)| *r == rank)
            .map(|(_, s)| s.as_str())
    }
}

/// One vital-display override (`game-controls.md` §7.1).
#[derive(Debug, Clone, PartialEq)]
pub struct VitalDisplayOverride {
    /// `VitalDisplayOverride_RequiredEnum`.
    pub key: u32,
    /// The UI state label (`VitalDisplayOverride_UIState`).
    pub state: Option<String>,
    /// `VitalDisplayOverride_CSM_Name`.
    pub name: Option<StringInfo>,
    /// For free-people characters.
    pub freep: bool,
    /// For monster characters.
    pub monster: bool,
    /// For PvP characters.
    pub pvp: bool,
}

/// The vital-display overrides (`game-controls.md` §7.1). The player frame uses
/// [`PLAIN_FRAME_STATE`] in P2 (`combat-ui.md` TR9).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VitalDisplayOverrides {
    /// The overrides in control order.
    pub entries: Vec<VitalDisplayOverride>,
}

impl VitalDisplayOverrides {
    /// The UI state of override `key`.
    #[must_use]
    pub fn state(&self, key: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .and_then(|e| e.state.as_deref())
    }
}

/// How the target frame shows the level (`combat-ui.md` §5.2 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelDisplay {
    /// `OpponentLevel` shown.
    #[default]
    Shown,
    /// Hidden, with `OpponentLevelHiddenSkull` (`NPC_HideLevelInDisplay`).
    HiddenWithSkull,
    /// Hidden without the skull (`NPC_HideLevelAndVitalText`).
    Hidden,
}

/// The decoration of a target frame (`combat-ui.md` §5.2, TR13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFrame {
    /// The decoration state of `OpponentPanel_BG`, `OpponentVitals_Animations` and
    /// `OpponentLevel_BG`.
    pub decoration: String,
    /// The level display.
    pub level: LevelDisplay,
}

/// The decoration state of a `MonsterLevel_ExaminationModStatType` label (`combat-ui.md` §5.2
/// step 3 table); every other label and an absent property give `monster_normal`.
#[must_use]
pub fn decoration_state(label: Option<&str>) -> &'static str {
    match label.unwrap_or("") {
        "Swarm" => "monster_swarm",
        "EliteMaster" => "monster_elitemaster",
        "Nemesis" => "monster_nemesis",
        "Signature" => "monster_signature",
        "ArchNemesis" => "monster_archnemesis",
        "Elite" => "monster_elite",
        "GoodElite" => "monster_goodelite",
        "GoodEliteMaster" => "monster_goodelitemaster",
        "GoodArchNemesis" => "monster_goodarchnemesis",
        "GoodNemesis" => "monster_goodnemesis",
        "GoodSignature" => "monster_goodsignature",
        "Rare_Signature" => "monster_signature_rare",
        "Rare_Swarm" => "monster_swarm_rare",
        "Rare_EliteMaster" => "monster_elitemaster_rare",
        "Rare_Nemesis" => "monster_nemesis_rare",
        "Rare_Elite" => "monster_elite_rare",
        "Rare_Normal" => "monster_normal_rare",
        "Rare_ArchNemesis" => "monster_archnemesis_rare",
        "Scion_Swarm" => "monster_swarm_scion",
        "Scion_ArchNemesis" => "monster_arch_scion",
        "Scion_Signature" => "monster_signature_scion",
        "Scion_Elite" => "monster_elite_scion",
        "Scion_Nemesis" => "monster_nemesis_scion",
        "Scion_EliteMaster" => "monster_elitem_scion",
        "Scion_Normal" => "monster_normal_scion",
        _ => "monster_normal",
    }
}

// ---------------------------------------------------------------------------------------------
// Effects, casts, defeat, floating text
// ---------------------------------------------------------------------------------------------

/// The effect-bar row an icon goes to (`combat-ui.md` TR30: row 1 beneficial, row 2 harmful).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectRow {
    /// Row 1.
    Beneficial,
    /// Row 2.
    Harmful,
}

/// What an effect icon shows (`combat-ui.md` §8.1, TR29, TR30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectIcon {
    /// `Effect_Icon` (`None`: the fallback look of §13.5).
    pub icon: Option<Did>,
    /// `Effect_Harmful`.
    pub harmful: bool,
    /// The row.
    pub row: EffectRow,
    /// The border state (TR29), `None` = hidden.
    pub border: Option<String>,
    /// `Effect_Duration_CombatOnly` (the pulse rule of §8.2).
    pub combat_only: bool,
    /// The hover tooltip pair (`UI_Examination_Tooltip_DID` of the image field).
    pub tooltip: Option<TemplateRef>,
}

impl EffectIcon {
    /// The icon of a literal effect (TR29, TR30): harmful when the source is another entity;
    /// no icon, no border.
    #[must_use]
    pub fn literal(source_is_other: bool) -> Self {
        Self {
            icon: None,
            harmful: source_is_other,
            row: if source_is_other {
                EffectRow::Harmful
            } else {
                EffectRow::Beneficial
            },
            border: None,
            combat_only: false,
            tooltip: None,
        }
    }
}

/// A cast bar's label and style (`combat-ui.md` TR25, `interaction.md` §3.8.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastBar {
    /// The label (`None`: no text).
    pub label: Option<String>,
    /// Interruptible: the opponent bar's `AltInit` state and the mini bar's yellow meter.
    pub interruptible: bool,
}

impl CastBar {
    /// The bar of a literal ref (TR25): the literal, interruptible.
    #[must_use]
    pub fn literal(text: &str) -> Self {
        Self {
            label: (!text.trim().is_empty()).then(|| text.to_owned()),
            interruptible: true,
        }
    }
}

/// A defeat record (`interaction.md` §8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefeatRecord {
    /// The record.
    pub did: Did,
    /// `Death_RallyPoint`: a place key of the places table.
    pub rally_point: Option<String>,
    /// `Death_DefeatEffectProgression`.
    pub defeat_effect_progression: Option<Did>,
    /// Number of `WorldEvent_AllConditionList` conditions (when they apply is OPEN).
    pub conditions: usize,
}

/// The floating-text settings (`combat-ui.md` §6.2). [`Default`] gives the client's built-in
/// values; a property overrides its field only when its stored type matches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatyTextSettings {
    /// `UI_FloatyText_LOSFailureFadeTime` (unit not given by §6.2; default 2.5).
    pub los_failure_fade_time: f32,
    /// `UI_FloatyText_FadeTime` (unit not given by §6.2; default 0.6).
    pub fade_time: f32,
    /// `UI_FloatyText_NearRange`, metres: up to it the distance scale is the near scale
    /// (§6.4).
    pub near_range: f32,
    /// `UI_FloatyText_FarRange`, metres: from it the distance scale is the far scale (§6.4).
    pub far_range: f32,
    /// `UI_FloatyText_NearRangeScale`: size multiplier σ at the near range (§6.4).
    pub near_range_scale: f32,
    /// `UI_FloatyText_FarRangeScale`: size multiplier σ at the far range (§6.4).
    pub far_range_scale: f32,
    /// `UI_FloatyText_DialogNearRangeScale`: the near multiplier for dialogue texts.
    pub dialog_near_range_scale: f32,
    /// `UI_FloatyText_DialogFarRangeScale`: the far multiplier for dialogue texts.
    pub dialog_far_range_scale: f32,
    /// `UI_FloatyText_DamageColumns` (not used in the traced path, §6.5 OPEN).
    pub damage_columns: i32,
    /// `UI_FloatyText_DamageColumnSeparation` (not used in the traced path, §6.5 OPEN).
    pub damage_column_separation: f32,
    /// `UI_FloatyText_CriticalDamage_SpeedMod`: rise-speed factor *m* of critical texts
    /// (§6.3 step 3; 0 = they do not rise).
    pub critical_speed_mod: f32,
    /// `UI_FloatyText_CriticalDamage_InitialScale`: start scale factor *i* of critical texts
    /// (§6.3 step 2).
    pub critical_initial_scale: f32,
    /// `UI_FloatyText_CriticalDamage_ShrinkTime`: *c* of §6.3 step 2, a fraction of the
    /// text's life (compared with *t* = elapsed ÷ life).
    pub critical_shrink_time: f32,
    /// `UI_FloatyText_CriticalDamage_HorizontalOffset` *h*: pixels per critical-slot column
    /// step (§6.3), multiplied by σ.
    pub critical_horizontal_offset: f32,
    /// `UI_FloatyText_CriticalDamage_VerticalOffset` *v*: pixels per critical-slot row step
    /// (§6.3), multiplied by σ.
    pub critical_vertical_offset: f32,
    /// `UI_FloatyText_CheckDistance`: hide texts of entities that fail the camera distance
    /// test (§6.3 step 1).
    pub check_distance: bool,
}

impl Default for FloatyTextSettings {
    fn default() -> Self {
        Self {
            los_failure_fade_time: 2.5,
            fade_time: 0.6,
            near_range: 4.0,
            far_range: 30.0,
            near_range_scale: 1.2,
            far_range_scale: 0.5,
            dialog_near_range_scale: 1.2,
            dialog_far_range_scale: 0.8,
            damage_columns: 3,
            damage_column_separation: 36.0,
            critical_speed_mod: 0.5,
            critical_initial_scale: 1.0,
            critical_shrink_time: 0.1,
            critical_horizontal_offset: 50.0,
            critical_vertical_offset: 50.0,
            check_distance: true,
        }
    }
}

impl FloatyTextSettings {
    /// The release spacing of §6.5 (a built-in value).
    pub const RELEASE_SPACING: f32 = 0.35;
    /// The anchor lift above the bounding box in metres (§6.4).
    pub const ANCHOR_LIFT: f32 = 0.05;
    /// Most pending texts per entity (§6.5).
    pub const MAX_PENDING: usize = 50;
    /// Critical slots per entity (§6.3).
    pub const CRITICAL_SLOTS: usize = 6;

    /// The travel of a text for a screen dimension `h` in pixels (§6.2: *h* × 0.125).
    #[must_use]
    pub fn travel(h: f32) -> f32 {
        h * 0.125
    }

    /// The base rise speed in pixels per second (§6.2: *h* × 0.125 ÷ 1.5).
    #[must_use]
    pub fn base_rise_speed(h: f32) -> f32 {
        h * 0.125 / 1.5
    }
}

/// An avoidance result shown as a word (`combat-ui.md` §6.6, TR19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Avoidance {
    /// `Miss`.
    Miss,
    /// `Evade`.
    Evade,
    /// `Block`.
    Block,
    /// `Parry`.
    Parry,
    /// `Resist`.
    Resist,
    /// `Immune`.
    Immune,
}

impl Avoidance {
    /// Key of the `UI` table of the short-form tokens (`combat-ui.md` §6.6).
    pub const TABLE_KEY: u32 = 0x1000_0065;

    /// The short-form token (TR19).
    #[must_use]
    pub fn short_token(self) -> u32 {
        match self {
            Self::Miss => 0x0D9C_B273,
            Self::Evade => 0x09B5_EA75,
            Self::Block => 0x09B2_184B,
            Self::Parry => 0x09CF_A449,
            Self::Resist => 0x0C1E_5D34,
            Self::Immune => 0x0B72_97D5,
        }
    }

    /// Tessera's fallback word without an install (`combat-ui.md` §13.6).
    #[must_use]
    pub fn fallback(self) -> &'static str {
        match self {
            Self::Miss => "Miss",
            Self::Evade => "Evade",
            Self::Block => "Block",
            Self::Parry => "Parry",
            Self::Resist => "Resist",
            Self::Immune => "Immune",
        }
    }
}

impl<D: TooltipData> CombatData<D> {
    /// The consideration control (`game-controls.md` §7.5), read once; `None` when the install
    /// has none. At most [`super::MAX_CONSIDERATION_TABLES`] lists are kept and as many
    /// distinct tables read; list elements naming a table past that bound find no table.
    pub fn consideration(&self) -> Option<&Consideration> {
        self.consideration
            .get_or_init(|| {
                let p = &self.p;
                let control = self.set(self.data.weenie("ConsiderationControl")?)?;
                let mut c = Consideration::default();
                for s in structs(get(&control, p.rank_to_state)) {
                    let (Some(rank), Some(state)) = (
                        as_key(field(s, p.consider_rank)),
                        as_key(field(s, p.consider_state)),
                    ) else {
                        continue;
                    };
                    if let Some(label) = self.data.ui_label(UiEnum::StateId, state) {
                        c.rank_states.push((rank, label));
                    }
                }
                // Distinct table DIDs tried, read or not: at most MAX_CONSIDERATION_TABLES
                // record reads however many lists and elements the control names.
                let mut tried = std::collections::HashSet::new();
                let mut capped = false;
                for s in structs(get(&control, p.combat_tables)) {
                    if c.lists.len() >= super::MAX_CONSIDERATION_TABLES {
                        capped = true;
                        break;
                    }
                    let Some(kind) = as_key(field(s, p.consider_type)) else {
                        continue;
                    };
                    let Some(label) = p
                        .consider_type
                        .and_then(|pid| self.data.enum_label(pid, kind))
                    else {
                        continue;
                    };
                    let list: Vec<Did> = elements(field(s, p.combat_table_list))
                        .iter()
                        .map(|e| match e.value {
                            tessera_formats::PropertyValue::DidRef(d) => d,
                            _ => Did(0),
                        })
                        .collect();
                    for &d in &list {
                        if d.is_null() || tried.contains(&d) {
                            continue;
                        }
                        if tried.len() >= super::MAX_CONSIDERATION_TABLES {
                            capped = true;
                            break;
                        }
                        tried.insert(d);
                        if let Some(t) = self.set(d) {
                            let ranks = elements(get(&t, p.table_ranks))
                                .iter()
                                .filter_map(|e| as_key(Some(&e.value)))
                                .collect();
                            c.tables.insert(d, ranks);
                        }
                    }
                    c.lists.push((label, list));
                }
                if capped {
                    tracing::debug!(
                        limit = super::MAX_CONSIDERATION_TABLES,
                        "consideration control names too many lists or tables; the rest are \
                         not read"
                    );
                }
                Some(c)
            })
            .as_ref()
    }

    /// The name colour state of a target (`combat-ui.md` TR15) with the install's
    /// consideration tables.
    pub fn name_colour_state(
        &self,
        kind: TargetKind,
        defeated: bool,
        levels: Option<(i32, i32)>,
    ) -> String {
        name_colour_state(kind, defeated, levels, self.consideration())
    }

    /// The vital-display overrides (`game-controls.md` §7.1), read once.
    pub fn vital_display_overrides(&self) -> &VitalDisplayOverrides {
        self.overrides.get_or_init(|| {
            let p = &self.p;
            let mut out = VitalDisplayOverrides::default();
            let Some(control) = self
                .data
                .weenie("gmVitalDisplayOverrideControl")
                .and_then(|d| self.set(d))
            else {
                return out;
            };
            for e in elements(get(&control, p.vdo_array)) {
                let tessera_formats::PropertyValue::DidRef(d) = e.value else {
                    continue;
                };
                let Some(s) = self.set(d) else { continue };
                let Some(key) = as_key(get(&s, p.vdo_key)) else {
                    continue;
                };
                out.entries.push(VitalDisplayOverride {
                    key,
                    state: as_key(get(&s, p.vdo_state))
                        .and_then(|k| self.data.ui_label(UiEnum::StateId, k)),
                    name: as_info(get(&s, p.vdo_name)),
                    freep: as_bool(get(&s, p.vdo_freep)) == Some(true),
                    monster: as_bool(get(&s, p.vdo_monster)) == Some(true),
                    pvp: as_bool(get(&s, p.vdo_pvp)) == Some(true),
                });
            }
            out
        })
    }

    /// The target frame of an entity (`combat-ui.md` §5.2 steps 3–4, TR13): from its body
    /// record when it has one; without, `monster_normal` for creatures and NPCs and
    /// [`PLAIN_FRAME_STATE`] for players and objects.
    pub fn target_frame(&self, body: Option<Did>, kind: TargetKind) -> TargetFrame {
        let p = &self.p;
        let set = body.and_then(|d| self.set(d));
        let decoration = match (&set, kind) {
            (_, TargetKind::Player) => PLAIN_FRAME_STATE.to_owned(),
            (Some(s), _) => {
                let label = as_key(get(s, p.decoration))
                    .and_then(|k| p.decoration.and_then(|pid| self.data.enum_label(pid, k)));
                decoration_state(label.as_deref()).to_owned()
            }
            (None, TargetKind::Creature | TargetKind::NonPlayer) => {
                decoration_state(None).to_owned()
            }
            (None, TargetKind::Object) => PLAIN_FRAME_STATE.to_owned(),
        };
        let level = match &set {
            Some(s) if as_bool(get(s, p.hide_level)) == Some(true) => LevelDisplay::HiddenWithSkull,
            Some(s) if as_bool(get(s, p.hide_level_and_vitals)) == Some(true) => {
                LevelDisplay::Hidden
            }
            _ => LevelDisplay::Shown,
        };
        TargetFrame { decoration, level }
    }

    /// The icon, row and border of an effect record (`combat-ui.md` §8.1, TR29, TR30).
    pub fn effect_icon(&self, effect: Did) -> Option<EffectIcon> {
        let p = &self.p;
        let s = self.set(effect)?;
        let harmful = as_bool(get(&s, p.effect_harmful)) == Some(true);
        let border = if harmful {
            let pid = p.effect_resist_category;
            as_bits(get(&s, pid))
                .first()
                .and_then(|b| pid.and_then(|pid| self.data.enum_label(pid, b.checked_add(1)?)))
                .filter(|l| BORDER_STATES.contains(&l.as_str()))
        } else {
            None
        };
        let tooltip = match (
            as_did(get(&s, p.hover_layout)),
            as_key(get(&s, p.hover_element)).filter(|&e| e != 0),
        ) {
            (Some(layout), Some(element)) => Some(TemplateRef { layout, element }),
            _ => None,
        };
        Some(EffectIcon {
            icon: as_did(get(&s, p.effect_icon)),
            harmful,
            row: if harmful {
                EffectRow::Harmful
            } else {
                EffectRow::Beneficial
            },
            border,
            combat_only: as_bool(get(&s, p.effect_combat_only)) == Some(true),
            tooltip,
        })
    }

    /// The icon a skill display shows (`skill-tooltips.md` §11.2).
    pub fn skill_icon(&self, skill: Did, size: SkillIconSize) -> Option<Did> {
        let p = &self.p;
        let s = self.set(skill)?;
        let pid = size.select(p.skill_icon, p.skill_large_icon, p.skill_small_icon);
        as_did(get(&s, pid))
    }

    fn text(&self, info: &StringInfo) -> Option<String> {
        match self.data.render(TextRef::Info(info), &Values::new()) {
            Ok(Some(t)) => {
                let t = t.plain_text();
                (!t.trim().is_empty()).then_some(t)
            }
            _ => None,
        }
    }

    /// The cast bar of a cast whose `what` is record `what` (`combat-ui.md` TR25): a skill's
    /// `Skill_Name` (interruptible by its `Skill_InductionAction` record, else true, even when
    /// it has `Skill_Toggle_ChannelingState`); an induction or
    /// channelling record's title (`interaction.md` §3.8.4) and flag. Other records: no label,
    /// interruptible.
    pub fn cast_bar(&self, what: Did) -> CastBar {
        let p = &self.p;
        let Some(s) = self.set(what) else {
            return CastBar {
                label: None,
                interruptible: true,
            };
        };
        let timing = |t: &tessera_formats::PropertySet| -> (Option<String>, bool) {
            let title = as_info(get(t, p.induction_title))
                .and_then(|i| self.text(&i))
                .or_else(|| as_info(get(t, p.channelling_title)).and_then(|i| self.text(&i)));
            let interruptible = as_bool(get(t, p.induction_interruptible)) == Some(true)
                || as_bool(get(t, p.channelling_interruptible)) == Some(true);
            (title, interruptible)
        };
        if let Some(name) = as_info(get(&s, p.skill_name)) {
            // TR25: only `Skill_InductionAction`; a channelled skill without one is
            // interruptible.
            let induction = as_did(get(&s, p.skill_induction)).and_then(|d| self.set(d));
            return CastBar {
                label: self.text(&name),
                interruptible: induction.is_none_or(|t| timing(&t).1),
            };
        }
        if get(&s, p.induction_duration).is_some() || get(&s, p.channelling_duration).is_some() {
            let (label, interruptible) = timing(&s);
            return CastBar {
                label,
                interruptible,
            };
        }
        CastBar {
            label: None,
            interruptible: true,
        }
    }

    /// A defeat record (`interaction.md` §8.1); `None` when it has none of the three
    /// properties.
    pub fn defeat_record(&self, did: Did) -> Option<DefeatRecord> {
        let p = &self.p;
        let s = self.set(did)?;
        let rally_point = match get(&s, p.rally_point) {
            Some(tessera_formats::PropertyValue::String(r)) => Some(r.clone()),
            _ => None,
        };
        let defeat_effect_progression = as_did(get(&s, p.defeat_effect_progression));
        let conditions = elements(get(&s, p.defeat_conditions)).len();
        (rally_point.is_some() || defeat_effect_progression.is_some() || conditions > 0).then_some(
            DefeatRecord {
                did,
                rally_point,
                defeat_effect_progression,
                conditions,
            },
        )
    }

    /// The defeat effect of a record for a character level (`interaction.md` §8.1: the
    /// progression's element at the level; null elements give none).
    pub fn defeat_effect(&self, record: &DefeatRecord, level: i32) -> Option<Did> {
        let prog = record.defeat_effect_progression?;
        let set = self.data.record(prog).ok()??;
        let pids = tessera_formats::gameplay::combat::ProgressionPids::resolve(&Names(&self.data));
        let p =
            tessera_formats::gameplay::combat::Progression::from_properties(&*set, false, &pids)
                .ok()?;
        use tessera_formats::gameplay::combat::ProgressionValue as V;
        match p.lookup(i64::from(level)).value()? {
            V::Did(d) | V::Property(tessera_formats::PropertyValue::DidRef(d)) if !d.is_null() => {
                Some(d)
            }
            _ => None,
        }
    }

    /// The floating-text settings (`combat-ui.md` §6.2): `distant` for distant-battle units.
    pub fn floaty_settings(&self, distant: bool) -> FloatyTextSettings {
        let both = self.floaty.get_or_init(|| {
            [
                self.read_floaty("UI_FloatyText_Properties"),
                self.read_floaty("UI_DistantBattle_FloatyText_Properties"),
            ]
        });
        both[usize::from(distant)]
    }

    fn read_floaty(&self, label: &str) -> FloatyTextSettings {
        let mut f = FloatyTextSettings::default();
        let Some(s) = self.data.unique(label).and_then(|d| self.set(d)) else {
            return f;
        };
        let p = &self.p;
        let float = |pid: Option<u32>, slot: &mut f32| {
            if let Some(tessera_formats::PropertyValue::Float(x)) = get(&s, pid)
                && x.is_finite()
            {
                *slot = *x;
            }
        };
        float(p.floaty_los_fade, &mut f.los_failure_fade_time);
        float(p.floaty_fade, &mut f.fade_time);
        float(p.floaty_near, &mut f.near_range);
        float(p.floaty_far, &mut f.far_range);
        float(p.floaty_near_scale, &mut f.near_range_scale);
        float(p.floaty_far_scale, &mut f.far_range_scale);
        float(p.floaty_dialog_near_scale, &mut f.dialog_near_range_scale);
        float(p.floaty_dialog_far_scale, &mut f.dialog_far_range_scale);
        float(p.floaty_column_separation, &mut f.damage_column_separation);
        float(p.floaty_critical_speed, &mut f.critical_speed_mod);
        float(p.floaty_critical_scale, &mut f.critical_initial_scale);
        float(p.floaty_critical_shrink, &mut f.critical_shrink_time);
        float(
            p.floaty_critical_horizontal,
            &mut f.critical_horizontal_offset,
        );
        float(p.floaty_critical_vertical, &mut f.critical_vertical_offset);
        if let Some(tessera_formats::PropertyValue::Int(n)) = get(&s, p.floaty_columns) {
            f.damage_columns = *n;
        }
        if let Some(b) = as_bool(get(&s, p.floaty_check_distance)) {
            f.check_distance = b;
        }
        f
    }

    /// The avoidance word of TR19 from the install's `UI` table; `None` when it does not
    /// resolve (the caller uses [`Avoidance::fallback`]).
    pub fn avoidance_text(&self, a: Avoidance) -> Option<String> {
        match self.data.render(
            TextRef::Token {
                table_key: Avoidance::TABLE_KEY,
                token: a.short_token(),
            },
            &Values::new(),
        ) {
            Ok(Some(t)) => Some(t.plain_text()),
            _ => None,
        }
    }
}

/// Names a [`TooltipData`] as a [`tessera_formats::PropertyNames`].
pub(crate) struct Names<'a, D: TooltipData>(pub(crate) &'a D);

impl<D: TooltipData> tessera_formats::PropertyNames for Names<'_, D> {
    fn pid_of(&self, name: &str) -> Option<u32> {
        self.0.pid(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(ranks: &[(u32, usize)]) -> Vec<u32> {
        ranks
            .iter()
            .flat_map(|&(r, n)| std::iter::repeat_n(r, n))
            .collect()
    }

    #[test]
    fn consideration_worked_example_11_1() {
        // `game-controls.md` §11.1.
        let a = Did(0x7000_0001);
        let b = Did(0x7000_0002);
        let mut list = vec![a; 10];
        list.extend(vec![b; 110]);
        let mut c = Consideration {
            rank_states: vec![(60, "S".into()), (50, CON_EVEN_STATE.into())],
            lists: vec![(CONSIDER_MONSTER.into(), list)],
            tables: HashMap::new(),
        };
        c.tables.insert(a, vec![50; 31]);
        c.tables.insert(
            b,
            table(&[
                (10, 7),
                (20, 3),
                (30, 3),
                (40, 2),
                (50, 1),
                (60, 2),
                (70, 2),
                (80, 3),
                (90, 8),
            ]),
        );
        assert_eq!(c.rank(CONSIDER_MONSTER, 20, 22), Some(60));
        assert_eq!(c.state(CONSIDER_MONSTER, 20, 22), Some("S"));
        assert_eq!(c.rank(CONSIDER_MONSTER, 20, 0), Some(10));
        assert_eq!(c.rank(CONSIDER_MONSTER, 20, 40), Some(90));
        assert_eq!(c.rank(CONSIDER_MONSTER, 5, 5), Some(50));
        // Levels beyond the list use its last element; below 1 its first.
        assert_eq!(c.rank(CONSIDER_MONSTER, 500, 500), Some(50));
        assert_eq!(c.rank(CONSIDER_MONSTER, -3, -3), Some(50));
        assert_eq!(c.rank("Player", 20, 22), None);
    }

    #[test]
    fn name_and_meter_states_tr14_tr15() {
        assert_eq!(
            name_colour_state(TargetKind::Creature, true, Some((1, 1)), None),
            CORPSE_STATE
        );
        assert_eq!(
            name_colour_state(TargetKind::Creature, false, None, None),
            CON_EVEN_STATE
        );
        assert_eq!(
            name_colour_state(TargetKind::NonPlayer, false, None, None),
            NPC_STATE
        );
        assert_eq!(
            name_colour_state(TargetKind::Player, false, None, None),
            PLAYER_STATE
        );
        assert_eq!(
            health_meter_state(TargetKind::Player, false),
            "vitalmeter_green"
        );
        assert_eq!(
            health_meter_state(TargetKind::Creature, false),
            "vitalmeter_red"
        );
        assert_eq!(
            health_meter_state(TargetKind::Creature, true),
            "vitalmeter_grey"
        );
    }

    #[test]
    fn decorations_5_2() {
        assert_eq!(decoration_state(Some("Elite")), "monster_elite");
        assert_eq!(
            decoration_state(Some("Scion_EliteMaster")),
            "monster_elitem_scion"
        );
        assert_eq!(decoration_state(Some("Normal")), "monster_normal");
        assert_eq!(decoration_state(None), "monster_normal");
    }

    #[test]
    fn literals_and_floaty_defaults() {
        let e = EffectIcon::literal(true);
        assert_eq!(e.row, EffectRow::Harmful);
        assert!(e.border.is_none());
        assert_eq!(CastBar::literal("x").label.as_deref(), Some("x"));
        assert_eq!(CastBar::literal("  ").label, None);
        let f = FloatyTextSettings::default();
        assert_eq!(f.far_range, 30.0);
        assert!(f.check_distance);
        assert_eq!(FloatyTextSettings::travel(800.0), 100.0);
        assert_eq!(Avoidance::Parry.short_token(), 0x09CF_A449);
    }
}
