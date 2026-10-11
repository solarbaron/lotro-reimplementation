// SPDX-License-Identifier: GPL-3.0-or-later
//! The data-neutral input of the combat widgets. The client fills these from the session
//! (scenario feed or server backend) and `tessera-gamedata`'s lookups; `tessera-ui` knows
//! neither. Values are display values: fractions, texts, state choices.

use crate::game::IconImage;

/// An entity as the client names it (the session's entity ID).
pub type EntityId = u64;

/// Most characters of a name or label the widgets keep (Tessera cap, hostile input).
pub const MAX_LABEL_CHARS: usize = 256;

/// Cuts a label to [`MAX_LABEL_CHARS`] characters.
#[must_use]
pub fn cap_label(s: &str) -> String {
    s.chars().take(MAX_LABEL_CHARS).collect()
}

/// One vital as last reported (`VitalChanged { current, max }`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vital {
    /// The current value.
    pub current: f64,
    /// The maximum.
    pub max: f64,
}

impl Vital {
    /// A vital.
    #[must_use]
    pub fn new(current: f64, max: f64) -> Self {
        Self { current, max }
    }

    /// The meter position of TR10: current ÷ max clamped to [0, 1]; 0 when the maximum is
    /// 0 (or either value is not finite) (CUI2).
    #[must_use]
    pub fn fraction(&self) -> f32 {
        if !self.current.is_finite() || !self.max.is_finite() || self.max == 0.0 {
            return 0.0;
        }
        ((self.current / self.max) as f32).clamp(0.0, 1.0)
    }

    /// The vital text of TR10: "*current*/*maximum*" in whole numbers rounded half away
    /// from zero.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "{}/{}",
            round_half_away(self.current),
            round_half_away(self.max)
        )
    }
}

/// Rounds half away from zero to an integer, saturating; NaN gives 0.
#[must_use]
pub fn round_half_away(v: f64) -> i64 {
    if v.is_nan() {
        return 0;
    }
    // `f64::round` rounds half away from zero; the cast saturates.
    v.round() as i64
}

/// The nine consideration ranks (`combat-ui.md` §5.4, `game-controls.md` §7.5), easiest
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConRank {
    /// `ConTrivial_ColorName`.
    Trivial,
    /// `ConInsanelyEasy_ColorName`.
    InsanelyEasy,
    /// `ConVeryEasy_ColorName`.
    VeryEasy,
    /// `ConEasy_ColorName`.
    Easy,
    /// `ConEven_ColorName`.
    Even,
    /// `ConTough_ColorName`.
    Tough,
    /// `ConVeryTough_ColorName`.
    VeryTough,
    /// `ConInsanelyTough_ColorName`.
    InsanelyTough,
    /// `ConImpossible_ColorName`.
    Impossible,
}

impl ConRank {
    /// The rank by its index 0–8 (easiest first).
    #[must_use]
    pub fn from_index(i: usize) -> Option<Self> {
        [
            Self::Trivial,
            Self::InsanelyEasy,
            Self::VeryEasy,
            Self::Easy,
            Self::Even,
            Self::Tough,
            Self::VeryTough,
            Self::InsanelyTough,
            Self::Impossible,
        ]
        .get(i)
        .copied()
    }

    /// The index 0–8.
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The name colour state of TR15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameColour {
    /// A consideration state (creatures with known levels).
    Con(ConRank),
    /// `NPC_ColorName`.
    Npc,
    /// `PlayerNormal_ColorName`.
    Player,
    /// `Object_ColorName`.
    Object,
    /// `Corpse_ColorName` (defeated).
    Corpse,
    /// `Tapped_ColorName`.
    Tapped,
}

/// The health meter's colour state (TR14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HealthColour {
    /// `vitalmeter_red` (the meter's default).
    #[default]
    Red,
    /// `vitalmeter_green`.
    Green,
    /// `vitalmeter_grey`.
    Grey,
    /// `vitalmeter_yellow`.
    Yellow,
}

/// What the level element shows (§5.2 step 4, TR16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelShown {
    /// Nothing (no level reported).
    #[default]
    Nothing,
    /// The level as a decimal integer.
    Level(i32),
    /// The level hidden with the skull (`NPC_HideLevelInDisplay`).
    Skull,
    /// The level hidden without the skull (`NPC_HideLevelAndVitalText`).
    Hidden,
}

/// The class resource of the player (TR10, TR11, §13.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClassResource {
    /// A pip record of type `Wrath`: the wrath meter replaces the power meter.
    Wrath(Vital),
    /// A pip record of another type: the P3 `PipDisplay` of that `PipType` key shows the
    /// state of its list entry for round(current).
    Pips {
        /// The `PipType` key.
        pip_type: u32,
        /// The value.
        value: Vital,
    },
    /// A literal ref or one without a pip record: the fallback resource bar (§13.4).
    Plain(Vital),
}

/// What a unit frame shows (E5 player frame, E6 target frame, E7 selection's target).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrameView {
    /// The entity shown.
    pub entity: EntityId,
    /// Its name.
    pub name: String,
    /// The name colour (target frames; the player frame's name has a fixed colour).
    pub name_colour: Option<NameColour>,
    /// The level display.
    pub level: LevelShown,
    /// Whether the entity has vitals for display (TR12); without, the target frame shows
    /// its non-vital field.
    pub has_vitals: bool,
    /// Health.
    pub health: Option<Vital>,
    /// Power.
    pub power: Option<Vital>,
    /// The health meter's colour (target frames).
    pub health_colour: HealthColour,
    /// The class resource (player frame).
    pub resource: Option<ClassResource>,
    /// The frame decoration state (a `UIStateID` value the client resolved from the label
    /// of §5.2 step 3 / TR13; install looks only).
    pub decoration: Option<u32>,
    /// `UI_ClassDisplay_Type` of a player target (§5.2 step 5).
    pub class_display: Option<i32>,
}

/// One effect instance on an entity (`EffectApplied`, §8, TR29–TR31).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectView {
    /// The instance (the feed's effect instance ID).
    pub instance: u64,
    /// The effect record DID (0 for a literal ref).
    pub effect_id: u32,
    /// The icon (`Effect_Icon`); `None` shows the fallback look of §13.5.
    pub icon: Option<IconImage>,
    /// The literal text (or name) whose first character the fallback icon shows.
    pub label: String,
    /// Whether the effect is harmful (`Effect_Harmful`, or the literal rule of TR30).
    pub harmful: bool,
    /// The border state (a `UIStateID` value from TR29), when shown.
    pub border: Option<u32>,
    /// Start time (the event's arrival), seconds of the UI clock.
    pub start: f64,
    /// The duration in seconds; `None` for an effect without one.
    pub duration: Option<f64>,
    /// `Effect_Duration_CombatOnly`.
    pub combat_only: bool,
    /// The effect's tooltip element (`UI_Examination_Tooltip_DID`), when known.
    pub tooltip: Option<u32>,
}

impl EffectView {
    /// The timeout (start + duration), when timed.
    #[must_use]
    pub fn timeout(&self) -> Option<f64> {
        self.duration
            .filter(|d| d.is_finite())
            .map(|d| self.start + d.max(0.0))
    }
}

/// How a cast bar's cast is shown (TR25–TR27).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CastView {
    /// The label.
    pub label: String,
    /// Whether the cast is interruptible (the opponent bar's `AltInit`).
    pub interruptible: bool,
    /// Whether the player may cancel it (TR28: a craft cannot be cancelled by Escape).
    pub cancellable: bool,
    /// The total duration in seconds (`duration_ms` ÷ 1000).
    pub duration: f64,
}

/// How a cast ended (`CastEnded { outcome }`, TR28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastOutcome {
    /// `Completed` → `Succeeded`.
    Completed,
    /// `Interrupted` → `Interrupted`.
    Interrupted,
    /// `Cancelled` → `Abandoned`.
    Cancelled,
}

/// An entity's floating-text anchor in screen space, as the client projects it each frame
/// (§6.4: the top of the bounding box plus 0.05 m; §13.6 without a world).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenAnchor {
    /// Screen x, pixels.
    pub x: f32,
    /// Screen y, pixels.
    pub y: f32,
    /// The camera's distance to the anchor, metres.
    pub distance: f32,
    /// Whether the anchor is on screen and in front of the camera.
    pub on_screen: bool,
    /// Whether the entity is drawn and passes the distance test (§6.3 step 1).
    pub drawn: bool,
}

/// The client's answer for anchors.
pub trait AnchorSource {
    /// The anchor of `entity` this frame, or `None` when it has none (no position, behind
    /// the camera).
    fn anchor(&self, entity: EntityId) -> Option<ScreenAnchor>;
}

impl<F: Fn(EntityId) -> Option<ScreenAnchor>> AnchorSource for F {
    fn anchor(&self, entity: EntityId) -> Option<ScreenAnchor> {
        self(entity)
    }
}

/// A neutral UI action the client maps to a session command (CUI8: commands come only from
/// these and the targeting keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatAction {
    /// Send `CancelCast` (TR28: the player's cast bar was clicked; TR5: Escape).
    CancelCast,
    /// Send `SelectTarget { target: None }` (TR5: Escape with a selection).
    ClearSelection,
    /// Use the quickslot with this shortcut index (TR33: a click on its button).
    UseShortcut(i32),
    /// The defeat panel's OK button: the panel closed (TR38: nothing is sent).
    DefeatAcknowledged,
}
