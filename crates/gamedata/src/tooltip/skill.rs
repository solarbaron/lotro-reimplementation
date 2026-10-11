// SPDX-License-Identifier: GPL-3.0-or-later
//! Skill and effect examination tooltips (`skill-tooltips.md` §3–§8, §11–§14): the request,
//! the rows of the skill and effect layouts, the timers, the icons and the census categories
//! of the real-data test. The effect lines (§9) live in `effects.rs`, durations (§10) in
//! [`super::duration`].
//!
//! Tessera rules applied (ST21): every skill row is filled from the **record** and the viewer
//! of §13 (level, maximum vitals, running cooldowns); actor modifier lists contribute nothing;
//! the damage and class-mechanics rows are built empty (they stay [`Widget::Deferred`]); travel
//! skills show `Skill_Name` and use the icon size rule; the required-trait row is `cantuse`
//! unless the viewer reports the trait; a literal subject shows a name-only tooltip.

use std::borrow::Cow;
use std::collections::BTreeMap;

use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_text::{RichText, Value, Values};

use super::data::TextRef;
use super::duration::{DurationOptions, TimerOptions, WRAP_VARIABLE};
use super::effects::{EffectMode, LineInput};
use super::layout::{TemplateRef, Widget};
use super::numbers::{NumberOptions, english_float, english_float_auto, english_integer};
use super::rows::Fill;
use super::rules::tokens;
use super::{
    Comparison, Ctx, Examined, Instance, Kind, Line, MAX_LINES, Purpose, Row, RowContent,
    RowFailure, Subject, TooltipData, TooltipEngine, TooltipError, TooltipModel, Viewer, assemble,
};

/// Tokens and variables of the skill and effect rows (`skill-tooltips.md` §7, §8). Read from
/// the `Examination` table (key [`tokens::EXAMINATION_TABLE_KEY`]).
pub mod skill_tokens {
    /// §7.2: no qualifier, immediate.
    pub const QUALIFIER_IMMEDIATE: u32 = 0x0A9A_9C99;
    /// §7.2: no qualifier, ignores the reset time.
    pub const QUALIFIER_IGNORES_RESET: u32 = 0x099F_1219;
    /// §7.2: joins two qualifier texts.
    pub const QUALIFIER_JOIN: u32 = 0x041F_53AE;
    /// §7.2: qualifier text, immediate.
    pub const QUALIFIED_IMMEDIATE: u32 = 0x00DF_AA95;
    /// §7.2: qualifier text, ignores the reset time.
    pub const QUALIFIED_IGNORES_RESET: u32 = 0x0655_2994;
    /// §7.2: the text-so-far variable.
    pub const VAR_QUALIFIER: u32 = 0x0005_0E84;
    /// §7.2: the next-entry variable of the join.
    pub const VAR_QUALIFIER_NEXT: u32 = 0x0ACA_64B5;
    /// §7.3, §7.4: the value variable.
    pub const VAR_VALUE: u32 = 0x005A_6195;
    /// §7.4: the maximum-targets text.
    pub const MAX_TARGETS: u32 = 0x0B22_6945;
    /// §7.6: casting time of an induction.
    pub const CASTING_INDUCTION: u32 = 0x0875_B0DE;
    /// §7.6: casting time of a channelled toggle.
    pub const CASTING_CHANNEL: u32 = 0x0308_1A4E;
    /// §7.6, §7.15: the duration variable (also the wrap-string variable of §10.2).
    pub const VAR_DURATION: u32 = 0x0005_8E15;
    /// §7.10: one vital-cost line.
    pub const VITAL_COST: u32 = 0x078C_A1B4;
    /// §7.10: one per-second vital-cost line.
    pub const VITAL_COST_PER_SECOND: u32 = 0x0D80_8C44;
    /// §7.10: the cost variable.
    pub const VAR_COST: u32 = 0x0004_8484;
    /// §7.10: the vital-name variable.
    pub const VAR_VITAL: u32 = 0x005A_E85C;
    /// §7.10: joins two cost lines.
    pub const COST_JOIN: u32 = 0x05BA_07CE;
    /// §7.10: the text-so-far variable of the join.
    pub const VAR_COST_SO_FAR: u32 = 0x0603_E6F1;
    /// §7.10: the new-line variable of the join.
    pub const VAR_COST_NEXT: u32 = 0x0603_E6F2;
    /// §7.10: the row-format variable.
    pub const VAR_COSTS: u32 = 0x0860_3E64;
    /// §7.11: a channelled toggle.
    pub const SPECIAL_CHANNELLED: u32 = 0x042D_933C;
    /// §7.11: a toggle.
    pub const SPECIAL_TOGGLE: u32 = 0x0EA9_60CC;
    /// §7.11: the row-format variable.
    pub const VAR_SPECIAL: u32 = 0x0048_0683;
    /// §7.13: the row-format variable.
    pub const VAR_TRAIT: u32 = 0x0059_65E4;
    /// §7.15: wrap string of a static recovery time.
    pub const RECOVERY_TOTAL: u32 = 0x0EDB_B215;
    /// §7.15: wrap string of a counting recovery time.
    pub const RECOVERY_REMAINING: u32 = 0x085F_0B57;
    /// §8.2: the category variable.
    pub const VAR_RESIST_CATEGORY: u32 = 0x0689_C009;
    /// §8.2: the spellcraft variable.
    pub const VAR_RESIST_SPELLCRAFT: u32 = 0x096A_2908;
    /// §7.1, §8.1: the name state of a skill.
    pub const SKILL_NAME_STATE: &str = "Skill_ColorName";
    /// §8.1: the name state of an effect.
    pub const EFFECT_NAME_STATE: &str = "Effect_ColorName";
}

use skill_tokens as st;

/// Most running cooldowns, maximum vitals and traits a [`SkillViewer`] is read for.
pub const MAX_VIEWER_ENTRIES: usize = 1024;

/// What a skill or effect tooltip examines (`skill-tooltips.md` §3, §13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatSubject {
    /// A skill record.
    Skill(Did),
    /// An effect record.
    Effect(Did),
    /// A literal skill (`Ref::Literal`): a name-only tooltip (§13 Tessera rule).
    SkillLiteral(String),
    /// A literal effect.
    EffectLiteral(String),
}

/// A live effect instance the feed reported (`EffectApplied`, §8.2–§8.4, §13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveEffect {
    /// The effect's runtime ID (`UI_Examination_Tooltip_EffectID`).
    pub effect_id: u64,
    /// The instance ID of the object holding it (`UI_Examination_Tooltip_EffectInstanceID`).
    pub holder: u64,
    /// Seconds since the effect was applied (its arrival).
    pub elapsed: f32,
    /// Its duration in seconds (`duration_ms`), `None` without one.
    pub duration: Option<f32>,
    /// The applied level (`EffectApplied` level), `None` when the feed gives none: the
    /// viewer's level is used (§8.2 Tessera rule).
    pub spellcraft: Option<f32>,
    /// The effect's `source` is the viewer (§9.5 combat-only line).
    pub caster_is_viewer: bool,
}

/// The local character as the skill rows see it (`skill-tooltips.md` §13).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillViewer {
    /// Level (`CharacterInfo`, `LevelChanged`); `None` reads as 1.
    pub level: Option<i32>,
    /// Maximum vitals: (vital type key, maximum), from the feed's vitals (§7.10).
    pub max_vitals: Vec<(u32, f32)>,
    /// Running cooldowns: (skill, remaining seconds) (§7.15).
    pub cooldowns: Vec<(Did, f32)>,
    /// Traits the viewer has (none in P2, §7.13).
    pub traits: Vec<Did>,
}

impl SkillViewer {
    /// The level, 1 when unknown (§14.1 viewer).
    #[must_use]
    pub fn level_or_one(&self) -> i32 {
        self.level.unwrap_or(1)
    }

    fn max_vital(&self, v: u32) -> f32 {
        self.max_vitals
            .iter()
            .take(MAX_VIEWER_ENTRIES)
            .find(|(k, _)| *k == v)
            .map_or(0.0, |(_, m)| *m)
            .max(0.0)
    }

    fn cooldown(&self, skill: Did) -> Option<f32> {
        self.cooldowns
            .iter()
            .take(MAX_VIEWER_ENTRIES)
            .find(|(d, _)| *d == skill)
            .map(|(_, r)| *r)
            .filter(|r| r.is_finite() && *r > 0.0)
    }
}

/// A skill or effect tooltip request (§3, §12).
#[derive(Debug, Clone, PartialEq)]
pub struct CombatRequest {
    /// What is examined.
    pub subject: CombatSubject,
    /// The purpose (hover; one-click uses the hover pair, the pairs are equal in the data).
    pub purpose: Purpose,
    /// The live effect an effect-bar icon names (§12.2).
    pub live: Option<LiveEffect>,
    /// An override pair of the requesting element, when both values are non-zero.
    pub override_pair: Option<TemplateRef>,
}

impl CombatRequest {
    /// The hover tooltip of `subject` without a live effect.
    #[must_use]
    pub fn hover(subject: CombatSubject) -> Self {
        Self {
            subject,
            purpose: Purpose::Hover,
            live: None,
            override_pair: None,
        }
    }
}

/// Which timer a [`TimerRow`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    /// The recovery-time row (§7.15).
    Recovery,
    /// The effect time row (§8.4).
    EffectTime,
}

/// A timer row of a built tooltip (§7.15, §8.4, §10.2): the text changes while shown;
/// [`TooltipEngine::timer_text`] gives it for a later moment. A tooltip lists a timer only
/// when its row had text at build time. A counting timer's text becomes `None` once less than
/// a second is left (the default format has no unit for it, §7.15 OPEN); the UI then shows
/// the row without text (as a row built at that moment would be empty) until the tooltip
/// closes or is rebuilt.
#[derive(Debug, Clone, PartialEq)]
pub struct TimerRow {
    /// Index of the (top-level) row in [`TooltipModel::rows`].
    pub row: usize,
    /// Which timer.
    pub kind: TimerKind,
    /// The value in seconds when the tooltip was built.
    pub seconds: f32,
    /// Whether it counts down (otherwise it shows `seconds` unchanged).
    pub counting: bool,
    /// The timer element's properties.
    pub timer: TimerOptions,
    /// The wrap string (an `Examination` token) whose variable receives the timer text.
    pub wrap: Option<u32>,
}

impl TimerRow {
    /// The value `elapsed` seconds after the build.
    #[must_use]
    pub fn seconds_after(&self, elapsed: f32) -> f32 {
        if self.counting {
            (self.seconds - elapsed.max(0.0)).max(0.0)
        } else {
            self.seconds
        }
    }
}

/// The icon size of a skill display (`skill-tooltips.md` §11.2, mapper of
/// `UI_SkillDisplay_IconSize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillIconSize {
    /// Key 2: `Skill_LargeIcon`.
    Large,
    /// Key 3, or absent: `Skill_SmallIcon`.
    #[default]
    Small,
    /// Any other key (1 `Medium`): `Skill_Icon`.
    Other,
}

impl SkillIconSize {
    /// The size of a stored key (`None` = absent).
    #[must_use]
    pub fn from_key(key: Option<u32>) -> Self {
        match key {
            Some(2) => Self::Large,
            None | Some(3) => Self::Small,
            Some(_) => Self::Other,
        }
    }

    /// The §11.2 rule, the one place it is written: of a skill's `Skill_Icon`,
    /// `Skill_LargeIcon` and `Skill_SmallIcon` (values, or the properties that hold them), the
    /// one a display of this size shows.
    #[must_use]
    pub fn select<T>(self, icon: T, large: T, small: T) -> T {
        match self {
            Self::Large => large,
            Self::Small => small,
            Self::Other => icon,
        }
    }
}

/// The tooltip category of the real-data census (`skill-tooltips.md` §14.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CombatCategory {
    /// The tooltip element was built and its name row shows text.
    Rendered,
    /// Built, but the name renders empty (see [`NameState`]).
    RenderedNoName,
    /// No hover pair: the original shows none.
    NoTooltip,
    /// The layout record is missing or fails to decode.
    LayoutUnreadable,
    /// The element is not a top-level element of the layout.
    ElementMissing,
}

/// Why a name is not shown (§14.1 `RenderedNoName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NameState {
    /// The name row shows text.
    Shown,
    /// `NameMissing`: the name's token is not in its table.
    Missing,
    /// `NameEmpty`: the entry renders empty (or there is no name).
    Empty,
}

/// The icon category of the real-data census (§14.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IconCategory {
    /// The icon names an existing 0x41 surface that decodes.
    Rendered,
    /// No icon property (or 0).
    Absent,
    /// The named surface is missing or fails to decode.
    Unreadable,
}

impl IconCategory {
    /// The category of `icon` given a surface check.
    pub fn of(icon: Option<Did>, decodes: impl FnOnce(Did) -> bool) -> Self {
        match icon {
            None => Self::Absent,
            Some(d) if decodes(d) => Self::Rendered,
            Some(_) => Self::Unreadable,
        }
    }
}

/// The outcome of one skill or effect record (§14.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CombatOutcome {
    /// The tooltip category, or the error that escaped the categories.
    pub category: Result<CombatCategory, TooltipError>,
    /// The name state of a built tooltip ([`NameState::Empty`] otherwise).
    pub name: NameState,
    /// Row failures (non-empty: the record also counts as `RowFailure`).
    pub row_failures: Vec<RowFailure>,
    /// Labels of the row templates that were non-empty.
    pub non_empty: Vec<String>,
    /// Rows left empty by the Tessera rules (damage, class mechanics, other widget types).
    pub deferred: usize,
    /// Lookup misses of the texts the record names.
    pub lookup_misses: u32,
    /// The examination icon (§11.1).
    pub icon: Option<Did>,
    /// The quickslot icon of a skill (§11.2, `Small`).
    pub quickslot_icon: Option<Did>,
}

/// Most examples a [`CombatCensus`] keeps.
pub const MAX_COMBAT_EXAMPLES: usize = 32;

/// Counts over many records (§14.1 report).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatCensus {
    /// Records per tooltip category.
    pub tooltips: BTreeMap<CombatCategory, u64>,
    /// `RenderedNoName` split: (missing, empty).
    pub no_name: (u64, u64),
    /// Records with a row failure (`RowFailure`, counted in addition).
    pub row_failures: u64,
    /// Records per icon category (examination icon).
    pub icons: BTreeMap<IconCategory, u64>,
    /// Records per icon category (quickslot icon, skills).
    pub quickslot_icons: BTreeMap<IconCategory, u64>,
    /// Errors that escaped the categories.
    pub escaped: u64,
    /// Per row-template label: tooltips with it non-empty.
    pub non_empty: BTreeMap<String, u64>,
    /// Rows left empty by the Tessera rules.
    pub deferred: u64,
    /// Lookup misses over all records.
    pub lookup_misses: u64,
    /// Examples of escaped errors and failures (bounded).
    pub examples: Vec<String>,
}

impl CombatCensus {
    /// Adds one record's outcome with its icon categories (the quickslot one for skills).
    pub fn add(
        &mut self,
        did: Did,
        o: &CombatOutcome,
        icon: IconCategory,
        quickslot: Option<IconCategory>,
    ) {
        match &o.category {
            Ok(c) => {
                *self.tooltips.entry(*c).or_default() += 1;
                if *c == CombatCategory::RenderedNoName {
                    match o.name {
                        NameState::Missing => self.no_name.0 += 1,
                        _ => self.no_name.1 += 1,
                    }
                }
            }
            Err(e) => {
                self.escaped += 1;
                self.example(format!("{did}: {e}"));
            }
        }
        if let Some(f) = o.row_failures.first() {
            self.row_failures += 1;
            self.example(format!("{did}: row {}: {}", f.template, f.reason));
        }
        *self.icons.entry(icon).or_default() += 1;
        if let Some(q) = quickslot {
            *self.quickslot_icons.entry(q).or_default() += 1;
        }
        for l in &o.non_empty {
            *self.non_empty.entry(l.clone()).or_default() += 1;
        }
        self.deferred += o.deferred as u64;
        self.lookup_misses += u64::from(o.lookup_misses);
    }

    /// Adds another census.
    pub fn merge(&mut self, other: CombatCensus) {
        for (k, v) in other.tooltips {
            *self.tooltips.entry(k).or_default() += v;
        }
        self.no_name.0 += other.no_name.0;
        self.no_name.1 += other.no_name.1;
        self.row_failures += other.row_failures;
        for (k, v) in other.icons {
            *self.icons.entry(k).or_default() += v;
        }
        for (k, v) in other.quickslot_icons {
            *self.quickslot_icons.entry(k).or_default() += v;
        }
        self.escaped += other.escaped;
        for (k, v) in other.non_empty {
            *self.non_empty.entry(k).or_default() += v;
        }
        self.deferred += other.deferred;
        self.lookup_misses += other.lookup_misses;
        for e in other.examples {
            self.example(e);
        }
    }

    /// The count of one tooltip category.
    #[must_use]
    pub fn count(&self, c: CombatCategory) -> u64 {
        self.tooltips.get(&c).copied().unwrap_or(0)
    }

    fn example(&mut self, s: String) {
        if self.examples.len() < MAX_COMBAT_EXAMPLES {
            self.examples.push(s);
        }
    }
}

fn as_bool(v: Option<&PropertyValue>) -> bool {
    matches!(v, Some(PropertyValue::Bool(true)))
}

fn as_f32(v: Option<&PropertyValue>) -> Option<f32> {
    match v? {
        PropertyValue::Float(x) if x.is_finite() => Some(*x),
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn as_did(v: Option<&PropertyValue>) -> Option<Did> {
    match v? {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

fn as_key(v: Option<&PropertyValue>) -> Option<u32> {
    match v? {
        PropertyValue::Enum(k) => Some(*k),
        PropertyValue::Int(i) => u32::try_from(*i).ok(),
        _ => None,
    }
}

fn values(pairs: impl IntoIterator<Item = (u32, Value)>) -> Values {
    let mut v = Values::new();
    for (id, x) in pairs {
        v.insert(id, x);
    }
    v
}

fn text(t: &RichText) -> Value {
    Value::Text(t.plain_text())
}

impl<D: TooltipData> TooltipEngine<D> {
    // -----------------------------------------------------------------------------------------
    // Build
    // -----------------------------------------------------------------------------------------

    /// Builds the skill or effect tooltip of `request` for `viewer` (`skill-tooltips.md`
    /// §3–§8).
    ///
    /// # Errors
    ///
    /// [`TooltipError::NoTooltip`] when the record names no tooltip (every monster skill),
    /// the layout and element errors of §14.1, and the record errors.
    pub fn build_combat(
        &self,
        request: &CombatRequest,
        viewer: &SkillViewer,
    ) -> Result<TooltipModel, TooltipError> {
        let (did, kind) = match &request.subject {
            CombatSubject::SkillLiteral(t) => {
                return Ok(self.combat_literal(t, st::SKILL_NAME_STATE, request.purpose));
            }
            CombatSubject::EffectLiteral(t) => {
                return Ok(self.combat_literal(t, st::EFFECT_NAME_STATE, request.purpose));
            }
            CombatSubject::Skill(d) => (*d, Kind::Skill),
            CombatSubject::Effect(d) => (*d, Kind::Effect),
        };
        let record = match self.data.record(did) {
            Ok(Some(r)) => r,
            Ok(None) => return Err(TooltipError::RecordMissing(did)),
            Err(reason) => return Err(TooltipError::RecordUnreadable { did, reason }),
        };
        let at = match request.override_pair {
            Some(p) if !p.layout.is_null() && p.element != 0 => p,
            _ => self
                .pair(&record, request.purpose)
                .ok_or(TooltipError::NoTooltip)?,
        };
        let frame = self.frame(at)?;
        let ctx = Ctx {
            viewer: viewer.clone(),
            live: request.live,
            ..Ctx::default()
        };
        let instance = Instance::default();
        let ex = Examined {
            did: Some(did),
            set: Cow::Borrowed(record.as_ref()),
            instance: &instance,
            kind,
            ctx: &ctx,
        };
        let item_viewer = Viewer {
            level: viewer.level,
            ..Viewer::default()
        };
        let (rows, failures, timers) = self.fill_rows(&frame, &ex, &item_viewer);
        let assembly = assemble(
            &rows.iter().map(Row::assembly_row).collect::<Vec<_>>(),
            frame.list.design_width,
            Some(frame.list.width_limit()),
        );
        Ok(TooltipModel {
            subject: Subject::Record(did),
            purpose: request.purpose,
            frame: Some(frame),
            rows,
            assembly,
            comparison: Comparison::default(),
            failures,
            timers,
            lookup_misses: ctx.misses.get(),
        })
    }

    /// The minimal tooltip of a literal skill or effect (§13 Tessera rule).
    fn combat_literal(&self, text: &str, state: &str, purpose: Purpose) -> TooltipModel {
        let mut m = self.literal(text, purpose);
        if let Some(row) = m.rows.first_mut() {
            row.state = Some(state.to_owned());
            row.colour = self.name_colour(state);
        }
        m
    }

    /// The text of a timer row `elapsed` seconds after its tooltip was built (§10.2): the
    /// duration text, wrapped in the row's wrap string. `None` when the duration has no text
    /// at that moment (under one second with the default format, §7.15 OPEN): the row is then
    /// shown without text, see [`TimerRow`].
    #[must_use]
    pub fn timer_text(&self, timer: &TimerRow, elapsed: f32) -> Option<RichText> {
        let s = f64::from(timer.seconds_after(elapsed));
        let format = timer.timer.format.as_ref().filter(|f| {
            matches!(
                self.data.render(TextRef::Info(f), &Values::new()),
                Ok(Some(_))
            )
        });
        let opts = if format.is_some() {
            timer.timer.format_options()
        } else {
            timer.timer.options()
        };
        let d = self.duration_text(s, opts, format)?;
        match timer.wrap {
            None => Some(d),
            Some(token) => {
                let v = values([(WRAP_VARIABLE, text(&d))]);
                match self.data.render(
                    TextRef::Token {
                        table_key: tokens::EXAMINATION_TABLE_KEY,
                        token,
                    },
                    &v,
                ) {
                    Ok(Some(t)) => Some(t),
                    _ => Some(d),
                }
            }
        }
    }

    /// Classifies one skill or effect record for the census of §14.1 (hover tooltip, viewer
    /// of level 1 without vitals or live effects).
    pub fn classify_combat(&self, subject: &CombatSubject) -> CombatOutcome {
        let viewer = SkillViewer::default();
        let (icon, quickslot_icon) = match subject {
            CombatSubject::Skill(d) => (
                self.examination_icon(subject),
                self.skill_icon(*d, SkillIconSize::Small),
            ),
            CombatSubject::Effect(_) => (self.examination_icon(subject), None),
            _ => (None, None),
        };
        match self.build_combat(&CombatRequest::hover(subject.clone()), &viewer) {
            Ok(m) => {
                let mut non_empty = Vec::new();
                let mut deferred = 0;
                for r in &m.rows {
                    if matches!(r.widget, Widget::Deferred(_)) {
                        deferred += 1;
                    } else if !r.is_empty() && !matches!(r.content, RowContent::Separator) {
                        non_empty.push(r.label.clone().unwrap_or_else(|| "(unlabelled)".into()));
                    }
                }
                let name = if m.has_name() {
                    NameState::Shown
                } else {
                    self.name_state(subject)
                };
                CombatOutcome {
                    category: Ok(if name == NameState::Shown {
                        CombatCategory::Rendered
                    } else {
                        CombatCategory::RenderedNoName
                    }),
                    name,
                    row_failures: m.failures,
                    non_empty,
                    deferred,
                    lookup_misses: m.lookup_misses,
                    icon,
                    quickslot_icon,
                }
            }
            Err(e) => CombatOutcome {
                category: match e {
                    TooltipError::NoTooltip => Ok(CombatCategory::NoTooltip),
                    TooltipError::LayoutUnreadable { .. } => Ok(CombatCategory::LayoutUnreadable),
                    TooltipError::ElementMissing { .. } => Ok(CombatCategory::ElementMissing),
                    other => Err(other),
                },
                name: NameState::Empty,
                row_failures: Vec::new(),
                non_empty: Vec::new(),
                deferred: 0,
                lookup_misses: 0,
                icon,
                quickslot_icon,
            },
        }
    }

    /// Why the name of a record is not shown: its StringInfo is a missing token, or renders
    /// empty (or is absent).
    fn name_state(&self, subject: &CombatSubject) -> NameState {
        let (did, pid) = match subject {
            CombatSubject::Skill(d) => (*d, self.c.skill_name),
            CombatSubject::Effect(d) => (*d, self.c.effect_name),
            _ => return NameState::Empty,
        };
        let Some(set) = self.data.record(did).ok().flatten() else {
            return NameState::Empty;
        };
        match pid.and_then(|p| set.get(p)) {
            Some(PropertyValue::StringInfo(info)) => {
                match self.data.render(TextRef::Info(info), &Values::new()) {
                    Ok(Some(_)) => NameState::Shown,
                    Ok(None) => NameState::Empty,
                    Err(_) => NameState::Missing,
                }
            }
            _ => NameState::Empty,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §11 Icons
    // -----------------------------------------------------------------------------------------

    /// The examination icon of a skill (`Skill_SmallIcon`) or an effect (`Effect_Icon`)
    /// (§11.1); `None` without one and for literals.
    #[must_use]
    pub fn examination_icon(&self, subject: &CombatSubject) -> Option<Did> {
        let (did, pid) = match subject {
            CombatSubject::Skill(d) => (*d, self.c.skill_small_icon),
            CombatSubject::Effect(d) => (*d, self.c.effect_icon),
            CombatSubject::SkillLiteral(_) | CombatSubject::EffectLiteral(_) => return None,
        };
        let set = self.data.record(did).ok()??;
        as_did(set.get(pid?))
    }

    /// The icon a skill display shows (§11.2): by size, `Skill_LargeIcon`, `Skill_SmallIcon`
    /// or `Skill_Icon` (travel skills use the size rule, Tessera rule).
    #[must_use]
    pub fn skill_icon(&self, skill: Did, size: SkillIconSize) -> Option<Did> {
        let set = self.data.record(skill).ok()??;
        let pid = size.select(
            self.c.skill_icon,
            self.c.skill_large_icon,
            self.c.skill_small_icon,
        );
        as_did(set.get(pid?))
    }

    // -----------------------------------------------------------------------------------------
    // Row helpers
    // -----------------------------------------------------------------------------------------

    fn exam(&self, f: &mut Fill<'_, '_>, token: u32, v: &Values) -> Option<RichText> {
        let mut misses = 0;
        let t = self.render_counted(
            TextRef::Token {
                table_key: tokens::EXAMINATION_TABLE_KEY,
                token,
            },
            v,
            &mut misses,
        );
        f.ex.ctx.miss(misses);
        t
    }

    /// The row's format with `v`; a row without one shows nothing.
    fn format_row(&self, f: &mut Fill<'_, '_>, v: &Values) -> Option<RichText> {
        let t = f.t;
        let info = t.format.as_ref()?;
        let mut misses = 0;
        let r = self.render_counted(TextRef::Info(info), v, &mut misses);
        f.ex.ctx.miss(misses);
        r
    }

    fn get<'e>(ex: &'e Examined<'_>, pid: Option<u32>) -> Option<&'e PropertyValue> {
        ex.set.get(pid?)
    }

    pub(crate) fn line_of(&self, t: &super::RowTemplate, l: super::EffectLine) -> Line {
        let state = l.state.label();
        Line {
            text: l.text,
            state: state.map(str::to_owned),
            colour: t.line_colour(state),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §6.1 skill-item rows
    // -----------------------------------------------------------------------------------------

    /// A row of an item tooltip marked `UI_Examination_Tooltip_For_SkillItem`: the row of the
    /// item's `Usage_SkillToExecute`, empty without one (§6.1 step 1, §12.3).
    pub(crate) fn fill_skill_item(
        &self,
        t: &super::RowTemplate,
        ex: &Examined<'_>,
        viewer: &Viewer,
        failures: &mut Vec<RowFailure>,
    ) -> Row {
        let skill = self
            .c
            .usage_skill
            .and_then(|p| ex.set.get(p))
            .and_then(|v| as_did(Some(v)));
        let set = skill.and_then(|d| match self.data.record(d) {
            Ok(s) => s,
            Err(e) => {
                failures.push(RowFailure {
                    template: t.label.clone().unwrap_or_default(),
                    reason: e,
                });
                None
            }
        });
        let Some(set) = set else {
            return self.empty_row(t);
        };
        let kex = Examined {
            did: skill,
            set: Cow::Borrowed(set.as_ref()),
            instance: ex.instance,
            kind: Kind::Skill,
            ctx: ex.ctx,
        };
        self.fill(t, &kex, viewer, failures)
    }

    pub(crate) fn empty_row(&self, t: &super::RowTemplate) -> Row {
        Row {
            label: t.label.clone(),
            widget: t.widget.clone(),
            size: t.size,
            stretch_wide: t.stretch_wide,
            horizontal: t.horizontal,
            content: RowContent::Empty,
            state: None,
            colour: t.colour,
        }
    }

    fn text_row_of(
        &self,
        t: &super::RowTemplate,
        text: Option<RichText>,
        state: Option<&str>,
    ) -> Row {
        let mut r = self.empty_row(t);
        if let Some(text) = text.filter(|x| !x.plain_text().trim().is_empty()) {
            r.content = RowContent::Text(text);
        }
        if let Some(s) = state {
            r.state = Some(s.to_owned());
            r.colour = t.state_colour(s).or(t.colour);
        }
        r
    }

    // -----------------------------------------------------------------------------------------
    // §7.1, §8.1 name and icon
    // -----------------------------------------------------------------------------------------

    pub(crate) fn combat_name(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let (pid, state) = match f.ex.kind {
            Kind::Effect => (self.c.effect_name, st::EFFECT_NAME_STATE),
            _ => (self.c.skill_name, st::SKILL_NAME_STATE),
        };
        let text = match Self::get(f.ex, pid) {
            Some(PropertyValue::StringInfo(info)) => {
                let info: &StringInfo = info;
                let mut misses = 0;
                let name = self.info_text(info, &mut misses);
                f.ex.ctx.miss(misses);
                match (name, &t.format) {
                    (None, _) => None,
                    (Some(n), None) => Some(n),
                    (Some(n), Some(format)) => {
                        let v = match self.data.text_value(info) {
                            Ok(v) => v,
                            Err(_) => text(&n),
                        };
                        let r = self
                            .data
                            .render(TextRef::Info(format), &values([(tokens::VAR_NAME, v)]));
                        match r {
                            Ok(Some(x)) => Some(x),
                            Ok(None) => None,
                            Err(e) => {
                                f.fail(e);
                                None
                            }
                        }
                    }
                }
            }
            _ => None,
        };
        let mut row = self.text_row_of(t, text, Some(state));
        row.colour = self.name_colour(state).or(t.colour);
        row
    }

    pub(crate) fn combat_icon(&self, f: &mut Fill<'_, '_>) -> Row {
        let pid = match f.ex.kind {
            Kind::Effect => self.c.effect_icon,
            _ => self.c.skill_small_icon,
        };
        let mut row = self.empty_row(f.t);
        if let Some(d) = as_did(Self::get(f.ex, pid)) {
            row.content = RowContent::Image(d);
        }
        row
    }

    // -----------------------------------------------------------------------------------------
    // §7.2–§7.15 skill rows
    // -----------------------------------------------------------------------------------------

    pub(crate) fn qualifier(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let mut keys: Vec<u32> = Vec::new();
        if let (Some(list), Some(q)) = (self.c.hook_list, self.c.hook_qualifier)
            && let Some(PropertyValue::Array(a)) = f.ex.set.get(list)
        {
            for h in a.elements.iter().take(MAX_LINES) {
                if let PropertyValue::Struct(s) = &h.value
                    && let Some(k) = as_key(s.get(q)).filter(|&k| k != 0)
                    && !keys.contains(&k)
                {
                    keys.push(k);
                }
            }
        }
        let fl = as_bool(Self::get(f.ex, self.c.skill_ignores_reset))
            || as_bool(Self::get(f.ex, self.c.skill_immediate));
        let imm = as_bool(Self::get(f.ex, self.c.skill_immediate));
        let q = if keys.is_empty() {
            match (fl, imm) {
                (true, true) => self.exam(f, st::QUALIFIER_IMMEDIATE, &Values::new()),
                (true, false) => self.exam(f, st::QUALIFIER_IGNORES_RESET, &Values::new()),
                _ => None,
            }
        } else {
            let pid = self.c.hook_qualifier;
            let name = |k: u32| pid.and_then(|p| self.data.enum_text(p, k));
            let mut acc: Option<String> = name(keys[0]);
            for &k in &keys[1..] {
                let (Some(so_far), Some(next)) = (acc.clone(), name(k)) else {
                    continue;
                };
                let v = values([
                    (st::VAR_QUALIFIER, Value::Text(so_far.clone())),
                    (st::VAR_QUALIFIER_NEXT, Value::Text(next.clone())),
                ]);
                acc = Some(
                    self.exam(f, st::QUALIFIER_JOIN, &v)
                        .map_or_else(|| format!("{so_far}, {next}"), |r| r.plain_text()),
                );
            }
            match acc {
                None => None,
                Some(a) if fl => {
                    let token = if imm {
                        st::QUALIFIED_IMMEDIATE
                    } else {
                        st::QUALIFIED_IGNORES_RESET
                    };
                    self.exam(f, token, &values([(st::VAR_QUALIFIER, Value::Text(a))]))
                }
                Some(a) => Some(RichText::plain(&a)),
            }
        };
        self.text_row_of(t, q, None)
    }

    pub(crate) fn max_range(&self, f: &mut Fill<'_, '_>) -> Row {
        let r = as_f32(Self::get(f.ex, self.c.skill_max_range)).filter(|r| *r > 0.0);
        let text = r.and_then(|r| {
            let v = values([(st::VAR_VALUE, Value::Text(english_float_auto(f64::from(r))))]);
            self.format_row(f, &v)
        });
        self.text_row_of(f.t, text, None)
    }

    pub(crate) fn max_targets(&self, f: &mut Fill<'_, '_>) -> Row {
        let n = match Self::get(f.ex, self.c.skill_max_targets) {
            Some(PropertyValue::Int(n)) => Some(i64::from(*n)),
            Some(PropertyValue::Int64(n)) => Some(*n),
            _ => None,
        }
        .filter(|n| *n > 1);
        let text = n.and_then(|n| {
            let s = self.exam(
                f,
                st::MAX_TARGETS,
                &values([(st::VAR_VALUE, Value::Text(english_integer(n)))]),
            )?;
            self.format_row(f, &values([(st::VAR_VALUE, text(&s))]))
        });
        self.text_row_of(f.t, text, None)
    }

    pub(crate) fn casting_time(&self, f: &mut Fill<'_, '_>) -> Row {
        let read = |d: Did| self.data.record(d).ok().flatten();
        let induction = as_did(Self::get(f.ex, self.c.skill_induction)).and_then(read);
        let toggle = Self::get(f.ex, self.c.skill_toggle_hook).is_some();
        let channel = as_did(Self::get(f.ex, self.c.skill_channelling));
        // (duration, the text: an override StringInfo or a token).
        let found: Option<(f32, Result<StringInfo, u32>)> = if let Some(ind) = induction {
            let d = as_f32(self.c.induction_duration.and_then(|p| ind.get(p)));
            let over = match self.c.induction_tooltip_override.and_then(|p| ind.get(p)) {
                Some(PropertyValue::StringInfo(s)) => Ok((**s).clone()),
                _ => Err(st::CASTING_INDUCTION),
            };
            d.map(|d| (d, over))
        } else if toggle && let Some(ch) = channel {
            let over =
                as_f32(Self::get(f.ex, self.c.skill_channelling_override)).filter(|d| *d > 0.0);
            let d = over.or_else(|| {
                let set = read(ch)?;
                as_f32(self.c.channelling_duration.and_then(|p| set.get(p)))
            });
            d.map(|d| (d, Err(st::CASTING_CHANNEL)))
        } else {
            None
        };
        let text = found.and_then(|(d, t)| {
            let dt = self.duration_text(f64::from(d), DurationOptions::CASTING_TIME, None)?;
            let v = values([(st::VAR_DURATION, text(&dt))]);
            match t {
                Ok(info) => {
                    let mut misses = 0;
                    let r = self.render_counted(TextRef::Info(&info), &v, &mut misses);
                    f.ex.ctx.miss(misses);
                    r
                }
                Err(token) => self.exam(f, token, &v),
            }
        });
        self.text_row_of(f.t, text, None)
    }

    /// The cost lines of one cost list (§7.10 steps 1–3).
    fn cost_lines(&self, f: &mut Fill<'_, '_>, list: Option<u32>, token: u32) -> Vec<RichText> {
        let mut out = Vec::new();
        let Some(PropertyValue::Array(a)) = list.and_then(|p| f.ex.set.get(p)) else {
            return out;
        };
        let level = f.ex.ctx.viewer.level_or_one();
        let entries: Vec<_> = a
            .elements
            .iter()
            .take(MAX_LINES)
            .filter_map(|e| match &e.value {
                PropertyValue::Struct(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        for v in 1..=4u32 {
            for s in entries
                .iter()
                .filter(|s| as_key(self.c.vital_type.and_then(|p| s.get(p))) == Some(v))
            {
                if as_bool(self.c.vital_consumes_all.and_then(|p| s.get(p))) {
                    continue;
                }
                let percent = as_f32(self.c.vital_percent.and_then(|p| s.get(p))).unwrap_or(0.0);
                // A non-null progression wins over the stored points: the install stores
                // `Skill_Vital_Points` = 0 beside every progression (§7.10 OPEN).
                let points = as_did(self.c.vital_points_progression.and_then(|p| s.get(p)))
                    .and_then(|p| self.progression_f32(p, level))
                    .or_else(|| as_f32(self.c.vital_points.and_then(|p| s.get(p))));
                let m = f.ex.ctx.viewer.max_vital(v);
                let cost = super::effects::round_away(m * percent + points.unwrap_or(0.0));
                if cost <= 0 {
                    continue;
                }
                let Some(name) = self.c.vital_type.and_then(|p| self.data.enum_text(p, v)) else {
                    continue;
                };
                let vals = values([
                    (st::VAR_COST, Value::Text(english_integer(cost))),
                    (st::VAR_VITAL, Value::Text(name)),
                ]);
                if let Some(t) = self.exam(f, token, &vals) {
                    out.push(t);
                }
            }
        }
        out
    }

    pub(crate) fn vital_cost(&self, f: &mut Fill<'_, '_>) -> Row {
        let mut lines = self.cost_lines(f, self.c.skill_vital_costs, st::VITAL_COST);
        lines.extend(self.cost_lines(f, self.c.skill_toggle_costs, st::VITAL_COST_PER_SECOND));
        let mut joined: Option<String> = None;
        for l in lines {
            let l = l.plain_text();
            joined = Some(match joined {
                None => l,
                Some(so_far) => {
                    let v = values([
                        (st::VAR_COST_SO_FAR, Value::Text(so_far.clone())),
                        (st::VAR_COST_NEXT, Value::Text(l.clone())),
                    ]);
                    self.exam(f, st::COST_JOIN, &v)
                        .map_or_else(|| format!("{so_far}, {l}"), |r| r.plain_text())
                }
            });
        }
        let text =
            joined.and_then(|j| self.format_row(f, &values([(st::VAR_COSTS, Value::Text(j))])));
        self.text_row_of(f.t, text, None)
    }

    pub(crate) fn special_class(&self, f: &mut Fill<'_, '_>) -> Row {
        let toggle = Self::get(f.ex, self.c.skill_toggle_hook).is_some();
        let text = if toggle {
            let channelled = as_did(Self::get(f.ex, self.c.skill_channelling)).is_some();
            let token = if channelled {
                st::SPECIAL_CHANNELLED
            } else {
                st::SPECIAL_TOGGLE
            };
            self.exam(f, token, &Values::new())
                .and_then(|s| self.format_row(f, &values([(st::VAR_SPECIAL, text(&s))])))
        } else {
            None
        };
        self.text_row_of(f.t, text, None)
    }

    pub(crate) fn required_trait(&self, f: &mut Fill<'_, '_>) -> Row {
        let Some(tr) = as_did(Self::get(f.ex, self.c.skill_required_trait)) else {
            return self.empty_row(f.t);
        };
        let name = self.data.record(tr).ok().flatten().and_then(|set| {
            match self.c.trait_name.and_then(|p| set.get(p)) {
                Some(PropertyValue::StringInfo(i)) => {
                    let mut misses = 0;
                    let t = self.info_text(i, &mut misses);
                    f.ex.ctx.miss(misses);
                    t
                }
                _ => None,
            }
        });
        let text = name.and_then(|n| self.format_row(f, &values([(st::VAR_TRAIT, text(&n))])));
        let has =
            f.ex.ctx
                .viewer
                .traits
                .iter()
                .take(MAX_VIEWER_ENTRIES)
                .any(|d| *d == tr);
        let state = (!has).then_some("cantuse");
        self.text_row_of(f.t, text, state)
    }

    pub(crate) fn recovery(&self, f: &mut Fill<'_, '_>) -> Row {
        let total = as_f32(Self::get(f.ex, self.c.skill_recovery)).filter(|t| *t > 0.0);
        let Some(total) = total else {
            return self.empty_row(f.t);
        };
        let remaining = f.ex.did.and_then(|d| f.ex.ctx.viewer.cooldown(d));
        let timer = TimerRow {
            row: 0,
            kind: super::TimerKind::Recovery,
            seconds: remaining.unwrap_or(total),
            counting: remaining.is_some(),
            timer: f.t.timer.clone(),
            wrap: Some(if remaining.is_some() {
                st::RECOVERY_REMAINING
            } else {
                st::RECOVERY_TOTAL
            }),
        };
        let text = self.timer_text(&timer, 0.0);
        // An empty row (for example under one second, §7.15 OPEN) carries no timer.
        if text.is_some() {
            *f.ex.ctx.timer.borrow_mut() = Some(timer);
        }
        self.text_row_of(f.t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §8 effect rows
    // -----------------------------------------------------------------------------------------

    /// The live effect of an effect tooltip (§8.2: both request IDs set).
    fn live(f: &Fill<'_, '_>) -> Option<LiveEffect> {
        f.ex.ctx.live.filter(|l| l.effect_id != 0 || l.holder != 0)
    }

    pub(crate) fn effect_resistance(&self, f: &mut Fill<'_, '_>) -> Row {
        let Some(live) = Self::live(f) else {
            return self.empty_row(f.t);
        };
        let pid = self.c.effect_resist_category;
        let bits: Vec<u32> = match pid.and_then(|p| f.ex.set.get(p)) {
            Some(PropertyValue::Bitfield32(b)) => (0..32).filter(|i| b >> i & 1 == 1).collect(),
            Some(PropertyValue::Bitfield64(b)) => (0..64).filter(|i| b >> i & 1 == 1).collect(),
            Some(PropertyValue::BitfieldAny(b)) => b.set_bits().take(4096).collect(),
            _ => Vec::new(),
        };
        let category = bits
            .into_iter()
            .find_map(|b| pid.and_then(|p| self.data.enum_text(p, b.checked_add(1)?)));
        let Some(category) = category else {
            return self.empty_row(f.t);
        };
        #[allow(clippy::cast_precision_loss)]
        let spellcraft = live
            .spellcraft
            .filter(|s| s.is_finite())
            .unwrap_or(f.ex.ctx.viewer.level_or_one() as f32)
            .max(0.0);
        let sc = english_float(f64::from(spellcraft), 0, NumberOptions::PLAIN_KIND);
        let v = values([
            (st::VAR_RESIST_CATEGORY, Value::Text(category)),
            (st::VAR_RESIST_SPELLCRAFT, Value::Text(sc)),
        ]);
        let text = self.format_row(f, &v);
        self.text_row_of(f.t, text, None)
    }

    /// The evaluation level of the examined effect (§9.8): the live effect's spellcraft
    /// truncated, else the viewer's level.
    fn effect_level(f: &Fill<'_, '_>) -> i32 {
        #[allow(clippy::cast_possible_truncation)]
        Self::live(f)
            .and_then(|l| l.spellcraft)
            .filter(|s| s.is_finite())
            .map_or(f.ex.ctx.viewer.level_or_one(), |s| {
                s.trunc().clamp(i32::MIN as f32, i32::MAX as f32) as i32
            })
    }

    pub(crate) fn effect_description(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let caster = Self::live(f).is_none_or(|l| l.caster_is_viewer);
        let mods = tessera_formats::gameplay::combat::mod_entries(
            f.ex.set.as_ref(),
            &self.schema.modifiers,
        );
        let property_mod = f.ex.did.is_some_and(|d| self.is_property_mod(d));
        let input = LineInput {
            set: f.ex.set.as_ref(),
            mods: &mods,
            property_mod,
            mode: EffectMode::Applied,
            level: Self::effect_level(f),
            caster_is_viewer: caster,
        };
        let mut misses = 0;
        let lines = self.compose(&input, &mut misses);
        f.ex.ctx.miss(misses);
        let lines: Vec<Line> = lines.into_iter().map(|l| self.line_of(t, l)).collect();
        let mut row = self.empty_row(t);
        if !lines.is_empty() {
            row.content = RowContent::Lines {
                header: None,
                lines,
            };
        }
        row
    }

    pub(crate) fn effect_time(&self, f: &mut Fill<'_, '_>) -> Row {
        let Some(live) = Self::live(f) else {
            return self.empty_row(f.t);
        };
        let Some(duration) = live.duration.filter(|d| d.is_finite() && *d > 0.0) else {
            return self.empty_row(f.t);
        };
        let timer = TimerRow {
            row: 0,
            kind: super::TimerKind::EffectTime,
            seconds: (duration - live.elapsed.max(0.0)).max(0.0),
            counting: true,
            timer: f.t.timer.clone(),
            wrap: None,
        };
        let text = self.timer_text(&timer, 0.0);
        // An empty row (for example under one second, §7.15 OPEN) carries no timer.
        if text.is_some() {
            *f.ex.ctx.timer.borrow_mut() = Some(timer);
        }
        self.text_row_of(f.t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §9.6 skill effect lists
    // -----------------------------------------------------------------------------------------

    pub(crate) fn skill_effect_list(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let Some(target) = t.effect_target else {
            return self.empty_row(t);
        };
        let effects = self.skill_list(f.ex.set.as_ref(), target);
        let mut misses = 0;
        let lines = self.list_lines(&effects, f.ex.ctx.viewer.level_or_one(), &mut misses);
        f.ex.ctx.miss(misses);
        self.lines_row(f, lines)
    }

    /// A list row with the §9.6 header rule (Tessera rule: the row's format, in its base
    /// style, when the list has lines).
    pub(crate) fn lines_row(&self, f: &mut Fill<'_, '_>, lines: Vec<super::EffectLine>) -> Row {
        let t = f.t;
        if lines.is_empty() {
            return self.empty_row(t);
        }
        let header = self.format_row(f, &Values::new()).map(|h| Line {
            text: h,
            state: None,
            colour: t.colour,
        });
        let lines = lines.into_iter().map(|l| self.line_of(t, l)).collect();
        let mut row = self.empty_row(t);
        row.content = RowContent::Lines { header, lines };
        row
    }

    /// The level and lines of an item's effect lists for a row's categories (§9.7).
    pub(crate) fn item_effect_lines(
        &self,
        f: &mut Fill<'_, '_>,
        mask: u32,
    ) -> Vec<super::EffectLine> {
        use tessera_formats::gameplay::items::{EffectListKind, ItemView};
        let set: &PropertySet = f.ex.set.as_ref();
        let level = match self.p.level.and_then(|p| set.get(p)) {
            Some(PropertyValue::Int(l)) => *l,
            _ => 1,
        };
        let view = ItemView::new(set, &self.schema);
        let refs = |kind| -> Vec<(Did, Option<f32>)> {
            view.effects(kind)
                .into_iter()
                .filter_map(|r| {
                    let d = r.effect.filter(|d| !d.is_null())?;
                    let sc = r
                        .spellcraft
                        .filter(|s| s.to_bits() != (-1.0f32).to_bits() && s.is_finite());
                    Some((d, sc))
                })
                .take(super::MAX_LIST_EFFECTS)
                .collect()
        };
        let mut misses = 0;
        let mut out = Vec::new();
        if mask & 0x10 != 0 {
            out.extend(self.list_lines(&refs(EffectListKind::Equipper), level, &mut misses));
            out.extend(self.item_stat_lines(set, level, &mut misses));
        }
        if mask & 0x20 != 0 {
            out.extend(self.list_lines(&refs(EffectListKind::Target), level, &mut misses));
        }
        if mask & 0x08 != 0 {
            out.extend(self.list_lines(&refs(EffectListKind::Usage), level, &mut misses));
        }
        f.ex.ctx.miss(misses);
        out.truncate(MAX_LINES);
        out
    }
}
