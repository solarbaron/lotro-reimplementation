// SPDX-License-Identifier: GPL-3.0-or-later
//! Effect lines (`skill-tooltips.md` §9): the texts an effect contributes to a tooltip, shared
//! by the effect tooltip (§8.3), the effect-list rows of skills (§9.6) and items (§9.7,
//! `item-tooltips.md` §6.16), and the durations of §10.

use std::collections::HashMap;

use tessera_formats::gameplay::combat::{
    CombatClasses, EffectReference, EffectRole, ModEntry, ModOp, Progression, ProgressionData,
    ProgressionKind, effect_references, mod_entries,
};
use tessera_formats::gameplay::misc::PropertyMetaDataView;
use tessera_formats::property::PropertyEntry;
use tessera_formats::{Did, PropertySet, PropertyType, PropertyValue, StringInfo};
use tessera_text::{RichText, Value, Values};

use super::data::TextRef;
use super::duration::{self, DurationOptions, DurationParts};
use super::numbers::{NumberOptions, english_float, english_integer};
use super::rules::tokens;
use super::{MAX_LINES, TooltipData, TooltipEngine};

/// Tokens and variables of the effect lines (`skill-tooltips.md` §9.4, §9.5). They are read
/// from the `Examination` table (key [`tokens::EXAMINATION_TABLE_KEY`]).
pub mod line_tokens {
    /// `Add` template.
    pub const ADD: u32 = 0x0E44_7774;
    /// `Subtract` template.
    pub const SUBTRACT: u32 = 0x067F_C704;
    /// `Multiply` of a percentage float, value ≥ 1 (increase).
    pub const MULTIPLY_INCREASE: u32 = 0x09C7_FA07;
    /// `Multiply` of a percentage float, value < 1 (decrease).
    pub const MULTIPLY_DECREASE: u32 = 0x09C7_9707;
    /// Any other `Multiply`.
    pub const MULTIPLY: u32 = 0x07DF_4F79;
    /// `Set` template.
    pub const SET: u32 = 0x0E44_0914;
    /// `Or` template.
    pub const OR: u32 = 0x0DE4_4862;
    /// `ArrayAdd` template.
    pub const ARRAY_ADD: u32 = 0x046C_9024;
    /// Value text of a percentage.
    pub const VALUE_PERCENT: u32 = 0x0CE3_9455;
    /// Value text of a plain value.
    pub const VALUE_PLAIN: u32 = 0x0F5F_0564;
    /// The value variable (templates and value texts).
    pub const VAR_VALUE: u32 = 0x005A_6195;
    /// The property-name variable of the templates.
    pub const VAR_NAME: u32 = 0x0744_A2C9;
    /// The percentage value variable of an override.
    pub const VAR_PERCENT: u32 = 0x06D0_4635;
    /// The chance line.
    pub const CHANCE: u32 = 0x05AF_40D9;
    /// The chance variable.
    pub const VAR_CHANCE: u32 = 0x04A6_7A64;
    /// The combat-only line with a removal delay.
    pub const COMBAT_ONLY: u32 = 0x073C_4A39;
    /// The removal-delay variable.
    pub const VAR_DELAY: u32 = 0x0048_A069;
    /// The combat-only line of another caster's effect.
    pub const COMBAT_ONLY_OTHER: u32 = 0x020E_F2F3;
    /// The duration line.
    pub const DURATION: u32 = 0x04FE_416E;
    /// The duration variable.
    pub const VAR_DURATION: u32 = 0x0A66_8AAE;
}

use line_tokens as lt;

/// The base removal delay of combat-only effects in seconds (`skill-tooltips.md` §9.5).
pub const COMBAT_ONLY_BASE_DELAY: f32 = 9.0;

/// Most effects one effect-list row lists (hostile-input bound).
pub const MAX_LIST_EFFECTS: usize = 64;

/// Whether an effect is offered (by a skill or an item) or held (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectMode {
    /// An effect a skill or an item offers.
    Definition,
    /// An effect an actor holds.
    Applied,
}

/// The state code of a line (§9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineState {
    /// 1: modifier and chance lines of a non-harmful effect.
    Buffed,
    /// 2: the description line (the element's base colour).
    Plain,
    /// 3: modifier and chance lines of a harmful effect.
    Debuffed,
    /// 4: combat-only and duration lines.
    Duration,
}

impl LineState {
    /// The state label of the line element, `None` for [`LineState::Plain`].
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Buffed => Some("buffed"),
            Self::Plain => None,
            Self::Debuffed => Some("debuffed"),
            Self::Duration => Some("duration"),
        }
    }

    /// The state code of §9.3.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Buffed => 1,
            Self::Plain => 2,
            Self::Debuffed => 3,
            Self::Duration => 4,
        }
    }
}

/// One effect line.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectLine {
    /// The text.
    pub text: RichText,
    /// Its state.
    pub state: LineState,
}

/// The lines of one effect and the texts that could not be found while building them
/// (lookup misses are reported, not failures, `skill-tooltips.md` §14.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectLines {
    /// The lines in order.
    pub lines: Vec<EffectLine>,
    /// Lookup misses (names, descriptions, overrides, metadata names, tokens).
    pub misses: u32,
}

/// What a property's metadata object says (`misc-records.md` §13.1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Meta {
    pub(crate) name: Option<StringInfo>,
    pub(crate) percent: bool,
    pub(crate) hide_identity: bool,
}

/// The inputs of one composition (§9.1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineInput<'a> {
    /// The effect's property set.
    pub(crate) set: &'a PropertySet,
    /// The modifier entries to describe (the effect's own, or an item's, §9.7).
    pub(crate) mods: &'a [ModEntry],
    /// The class-734 composition (§9.2) instead of the base one.
    pub(crate) property_mod: bool,
    pub(crate) mode: EffectMode,
    /// Evaluation level *L* (§9.8).
    pub(crate) level: i32,
    /// Applied mode: the effect's caster is the viewer (§9.5 combat-only line).
    pub(crate) caster_is_viewer: bool,
}

/// Rounds half away from zero, saturating (`skill-tooltips.md` §7.10, §9.5 Tessera rules).
pub(crate) fn round_away(x: f32) -> i64 {
    if !x.is_finite() {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let r = f64::from(x).round().clamp(-9.0e15, 9.0e15) as i64;
    r
}

fn as_bool(v: Option<&PropertyValue>) -> Option<bool> {
    match v? {
        PropertyValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn as_f32(v: Option<&PropertyValue>) -> Option<f32> {
    match v? {
        PropertyValue::Float(x) => Some(*x),
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int(i) => Some(*i as f32),
        #[allow(clippy::cast_precision_loss)]
        PropertyValue::Int64(i) => Some(*i as f32),
        _ => None,
    }
}

fn as_did(v: Option<&PropertyValue>) -> Option<Did> {
    match v? {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

/// The identity value of `op` in the type `ty` (`combat-data.md` §5.3); `None` for operators
/// without one.
fn identity(op: ModOp, ty: Option<PropertyType>) -> Option<PropertyValue> {
    let one = matches!(op, ModOp::Multiply | ModOp::Divide);
    if !(one || matches!(op, ModOp::Add | ModOp::Subtract | ModOp::Or)) {
        return None;
    }
    Some(match ty? {
        PropertyType::Float => PropertyValue::Float(if one { 1.0 } else { 0.0 }),
        PropertyType::Int => PropertyValue::Int(i32::from(one)),
        PropertyType::Int64 => PropertyValue::Int64(i64::from(one)),
        PropertyType::Bool => PropertyValue::Bool(false),
        PropertyType::Bitfield32 => PropertyValue::Bitfield32(0),
        PropertyType::Bitfield64 => PropertyValue::Bitfield64(0),
        _ => return None,
    })
}

fn equals_identity(v: &PropertyValue, id: &PropertyValue) -> bool {
    match (v, id) {
        (PropertyValue::Float(a), PropertyValue::Float(b)) => a == b,
        (
            PropertyValue::BitfieldAny(a),
            PropertyValue::Bitfield32(0) | PropertyValue::Bitfield64(0),
        ) => a.set_bits().next().is_none(),
        _ => v == id,
    }
}

impl<D: TooltipData> TooltipEngine<D> {
    /// The class-734 composition applies to `effect` (`skill-tooltips.md` §9.2 Tessera rule:
    /// descendants of the property-modification class); `false` when the class is unknown.
    pub(crate) fn is_property_mod(&self, effect: Did) -> bool {
        let Some(class) = self.data.root_class(effect) else {
            return false;
        };
        let classes = CombatClasses::observed();
        let pm: Vec<u32> = classes
            .effect_roles
            .iter()
            .filter(|(_, r)| *r == EffectRole::PropertyModification)
            .map(|(c, _)| *c)
            .collect();
        self.data
            .class_lineage(class)
            .iter()
            .any(|c| pm.contains(c))
    }

    /// The progression record `did`, nested when its class is the progression-of-progressions
    /// class (`combat-data.md` §4.1).
    pub(crate) fn progression(&self, did: Did) -> Option<Progression> {
        let set = self.data.record(did).ok()??;
        let nested = self.data.root_class(did).is_some_and(|c| {
            let classes = CombatClasses::observed();
            self.data
                .class_lineage(c)
                .contains(&classes.progression_of_progressions)
        });
        Progression::from_properties(&*set, nested, &self.schema.progressions).ok()
    }

    /// The value of a progression at `level` as f32 (non-nested; nested ones use outer 1
    /// and inner `level`, `combat-data.md` §5.6 step 4).
    pub(crate) fn progression_f32(&self, did: Did, level: i32) -> Option<f32> {
        let p = self.progression(did)?;
        let l = i64::from(level);
        let v = if p.kind() == ProgressionKind::ProgressionOfProgressions {
            p.evaluate(&|d| self.progression(d), 1, Some(l))?
        } else {
            p.evaluate(&|d| self.progression(d), l, None)?
        };
        v.as_f32()
    }

    /// The PID → metadata index of `misc-records.md` §13.1, read once.
    pub(crate) fn metadata(&self, pid: u32) -> Option<&Meta> {
        self.metadata
            .get_or_init(|| {
                let mut out = HashMap::new();
                let mut read = || -> Option<()> {
                    let control = self.data.weenie(
                        tessera_formats::gameplay::misc::weenie_content_labels::PROPERTY_METADATA_CONTROL,
                    )?;
                    let control = self.data.record(control).ok()??;
                    let list = match control.get(self.meta_pids.pmd_list.id()?)? {
                        PropertyValue::Array(a) => a,
                        _ => return None,
                    };
                    for e in list.elements.iter().take(super::MAX_METADATA) {
                        let PropertyValue::DidRef(d) = e.value else {
                            continue;
                        };
                        let Ok(Some(set)) = self.data.record(d) else {
                            continue;
                        };
                        let Ok(v) = PropertyMetaDataView::from_set(&set, &self.meta_pids) else {
                            continue;
                        };
                        out.entry(v.property).or_insert(Meta {
                            name: v.name.cloned(),
                            percent: v.display_as_percentage,
                            hide_identity: v.hide_identity_examination_mod == Some(true),
                        });
                    }
                    Some(())
                };
                let _ = read();
                out
            })
            .get(&pid)
    }

    /// Renders `text`; a lookup miss counts in `misses` and gives no text.
    pub(crate) fn render_counted(
        &self,
        text: TextRef<'_>,
        v: &Values,
        misses: &mut u32,
    ) -> Option<RichText> {
        match self.data.render(text, v) {
            Ok(t) => t,
            Err(_) => {
                *misses = misses.saturating_add(1);
                None
            }
        }
    }

    fn exam_token(&self, token: u32, v: &Values, misses: &mut u32) -> Option<RichText> {
        self.render_counted(
            TextRef::Token {
                table_key: tokens::EXAMINATION_TABLE_KEY,
                token,
            },
            v,
            misses,
        )
    }

    /// A StringInfo that renders non-empty (a lookup miss counts and gives none).
    pub(crate) fn info_text(&self, info: &StringInfo, misses: &mut u32) -> Option<RichText> {
        self.render_counted(TextRef::Info(info), &Values::new(), misses)
    }

    // -----------------------------------------------------------------------------------------
    // §10.1 durations
    // -----------------------------------------------------------------------------------------

    /// The number text of one unit (§10.1 step 3), the digits when the token is missing.
    fn unit_text(&self, value: u64, padded: bool) -> String {
        let token = if padded {
            duration::PADDED_NUMBER_TOKEN
        } else {
            duration::NUMBER_TOKEN
        };
        let n = i64::try_from(value).unwrap_or(i64::MAX);
        let mut v = Values::new();
        v.insert(duration::VAR_NUMBER, Value::Integer(n));
        match self.data.render(
            TextRef::Token {
                table_key: duration::UI_TABLE_KEY,
                token,
            },
            &v,
        ) {
            Ok(Some(t)) => t.plain_text(),
            _ if padded => format!("{value:02}"),
            _ => value.to_string(),
        }
    }

    /// The duration text of `seconds` (`skill-tooltips.md` §10.1) with the short or long
    /// template of the `UI` table, or with `template` when given (a timer's format string,
    /// §10.2 step 2). `None` when no unit is shown; the plain form of
    /// [`DurationParts::plain`] when the template cannot be found.
    #[must_use]
    pub fn duration_text(
        &self,
        seconds: f64,
        opts: DurationOptions,
        template: Option<&StringInfo>,
    ) -> Option<RichText> {
        let parts = DurationParts::split(seconds, opts);
        if parts.is_empty() {
            return None;
        }
        let mut v = Values::new();
        for (i, (u, var)) in parts.units.iter().zip(duration::UNIT_VARIABLES).enumerate() {
            let text = match u {
                None => String::new(),
                Some(u) => {
                    let n = self.unit_text(u.value, u.padded);
                    match u.tenths.filter(|_| i == 4) {
                        Some(t) => {
                            let mut tv = Values::new();
                            tv.insert(duration::SECONDS_VARIABLE, Value::Text(n.clone()));
                            tv.insert(duration::TENTHS_VARIABLE, Value::Integer(i64::from(t)));
                            match self.data.render(
                                TextRef::Token {
                                    table_key: duration::UI_TABLE_KEY,
                                    token: duration::TENTHS_TOKEN,
                                },
                                &tv,
                            ) {
                                Ok(Some(r)) => r.plain_text(),
                                _ => format!("{n}.{t}"),
                            }
                        }
                        None => n,
                    }
                }
            };
            v.insert(var, Value::Text(text));
        }
        let rendered = match template {
            Some(info) => self.data.render(TextRef::Info(info), &v),
            None => self.data.render(
                TextRef::Token {
                    table_key: duration::UI_TABLE_KEY,
                    token: if opts.long {
                        duration::LONG_TEMPLATE
                    } else {
                        duration::SHORT_TEMPLATE
                    },
                },
                &v,
            ),
        };
        match rendered {
            Ok(Some(t)) => Some(t),
            _ => Some(RichText::plain(&parts.plain())),
        }
    }

    // -----------------------------------------------------------------------------------------
    // combat-data.md §6.2 duration
    // -----------------------------------------------------------------------------------------

    /// Whether the effect has no duration source (§9.5, ST12): no interval, no pulse count,
    /// no permanent flag and no progression interval.
    fn no_duration_source(&self, e: &PropertySet) -> bool {
        let c = &self.c;
        let has = |p: Option<u32>| p.and_then(|p| e.get(p)).is_some();
        !has(c.effect_interval)
            && !has(c.effect_pulse_count)
            && as_bool(c.effect_permanent.and_then(|p| e.get(p))) != Some(true)
            && as_did(c.effect_interval_progression.and_then(|p| e.get(p))).is_none()
    }

    /// The duration of an effect at `level` without a property source (`combat-data.md`
    /// §6.2 steps 2–4; steps 1 and 5 need a source); `f32::MAX` when permanent.
    #[must_use]
    pub fn effect_duration(&self, e: &PropertySet, level: i32) -> f32 {
        let c = &self.c;
        let get = |p: Option<u32>| p.and_then(|p| e.get(p));
        if as_bool(get(c.effect_permanent)) == Some(true) {
            return f32::MAX;
        }
        let constant = as_f32(get(c.effect_interval)).filter(|x| *x > 0.0);
        let interval = match constant {
            Some(i) => Some(i),
            None => as_did(get(c.effect_interval_progression))
                .and_then(|p| self.progression_f32(p, level)),
        };
        let Some(i) = interval.filter(|x| x.is_finite()) else {
            return 0.0;
        };
        let i = i.max(0.5);
        let pulses = match get(c.effect_pulse_count) {
            Some(PropertyValue::Int(n)) if *n > 0 => Some(i64::from(*n)),
            Some(PropertyValue::Int64(n)) if *n > 0 => Some(*n),
            _ => as_did(get(c.effect_pulse_progression))
                .and_then(|p| self.progression_f32(p, level))
                .map(|x| {
                    #[allow(clippy::cast_possible_truncation)]
                    let n = x as i64;
                    n
                }),
        };
        match pulses {
            #[allow(clippy::cast_precision_loss)]
            Some(n) => i * (n.max(1) as f32),
            None => i,
        }
    }

    // -----------------------------------------------------------------------------------------
    // §9.2 composition
    // -----------------------------------------------------------------------------------------

    /// The effect lines of effect record `effect` in `mode` at evaluation level `level`
    /// (`skill-tooltips.md` §9.2). `caster_is_viewer` is used by the applied combat-only line.
    #[must_use]
    pub fn effect_lines(
        &self,
        effect: Did,
        mode: EffectMode,
        level: i32,
        caster_is_viewer: bool,
    ) -> EffectLines {
        let mut out = EffectLines::default();
        let Ok(Some(set)) = self.data.record(effect) else {
            return out;
        };
        let mods = mod_entries(&*set, &self.schema.modifiers);
        let input = LineInput {
            set: &set,
            mods: &mods,
            property_mod: self.is_property_mod(effect),
            mode,
            level,
            caster_is_viewer,
        };
        out.lines = self.compose(&input, &mut out.misses);
        out
    }

    /// §9.2 for one input.
    pub(crate) fn compose(&self, inp: &LineInput<'_>, misses: &mut u32) -> Vec<EffectLine> {
        let c = &self.c;
        let e = inp.set;
        let get = |p: Option<u32>| p.and_then(|p| e.get(p));
        let harmful = as_bool(get(c.effect_harmful)) == Some(true);
        let state = if harmful {
            LineState::Debuffed
        } else {
            LineState::Buffed
        };
        // Tessera rule (§9.2 OPEN, ST21): an absent flag reads true.
        let procedural = as_bool(get(c.effect_procedural)) != Some(false);
        let mut lines = Vec::new();
        let description = |lines: &mut Vec<EffectLine>, misses: &mut u32| {
            let pid = match inp.mode {
                EffectMode::Definition => c.effect_definition_description,
                EffectMode::Applied => c.effect_applied_description,
            };
            if let Some(PropertyValue::StringInfo(info)) = get(pid)
                && let Some(t) = self.info_text(info, misses)
            {
                lines.push(EffectLine {
                    text: t,
                    state: LineState::Plain,
                });
            }
        };
        if inp.property_mod {
            if !procedural {
                return lines;
            }
            description(&mut lines, misses);
            for m in inp.mods.iter().take(MAX_LINES) {
                if let Some(t) = self.modifier_line(m, inp.level, misses) {
                    lines.push(EffectLine { text: t, state });
                }
            }
            if lines.is_empty() {
                return lines;
            }
            if inp.mode == EffectMode::Definition {
                self.chance_line(e, state, &mut lines, misses);
            }
        } else {
            description(&mut lines, misses);
            match inp.mode {
                EffectMode::Applied if !procedural => return lines,
                EffectMode::Applied => {}
                EffectMode::Definition => {
                    if lines.is_empty() {
                        return lines;
                    }
                    self.chance_line(e, state, &mut lines, misses);
                }
            }
        }
        self.extra_lines(inp, &mut lines, misses);
        lines.truncate(MAX_LINES);
        lines
    }

    /// The chance line (§9.5).
    fn chance_line(
        &self,
        e: &PropertySet,
        state: LineState,
        lines: &mut Vec<EffectLine>,
        misses: &mut u32,
    ) {
        let c = &self.c;
        let get = |p: Option<u32>| p.and_then(|p| e.get(p));
        if as_bool(get(c.effect_suppress_probability)) == Some(true) {
            return;
        }
        // Tessera rule: the constant probability, absent 1.
        let chance = as_f32(get(c.effect_probability)).unwrap_or(1.0);
        if !(chance > 0.0 && chance < 1.0) {
            return;
        }
        let pct = english_float(f64::from(chance * 100.0), 0, NumberOptions::TOOLTIP);
        let mut v = Values::new();
        v.insert(lt::VAR_CHANCE, Value::Text(pct));
        if let Some(t) = self.exam_token(lt::CHANCE, &v, misses) {
            lines.push(EffectLine { text: t, state });
        }
    }

    /// The combat-only and duration lines (§9.5).
    fn extra_lines(&self, inp: &LineInput<'_>, lines: &mut Vec<EffectLine>, misses: &mut u32) {
        let c = &self.c;
        let e = inp.set;
        let get = |p: Option<u32>| p.and_then(|p| e.get(p));
        if as_bool(get(c.effect_combat_only)) == Some(true) {
            let own = inp.mode == EffectMode::Definition || inp.caster_is_viewer;
            let text = if own {
                // The viewer carries no `Effect_CombatOnlyRemovalDelay_AddMod` in the sandbox.
                let delay = round_away(COMBAT_ONLY_BASE_DELAY.max(0.0));
                let mut v = Values::new();
                v.insert(lt::VAR_DELAY, Value::Text(english_integer(delay)));
                self.exam_token(lt::COMBAT_ONLY, &v, misses)
            } else {
                self.exam_token(lt::COMBAT_ONLY_OTHER, &Values::new(), misses)
            };
            if let Some(t) = text {
                lines.push(EffectLine {
                    text: t,
                    state: LineState::Duration,
                });
            }
        }
        if self.no_duration_source(e) || as_bool(get(c.effect_permanent)) == Some(true) {
            return;
        }
        let d = self.effect_duration(e, inp.level);
        if d.is_nan() || d <= 0.0 || d == f32::MAX {
            return;
        }
        let Some(dt) = self.duration_text(f64::from(d), DurationOptions::EFFECT_DURATION, None)
        else {
            return;
        };
        let mut v = Values::new();
        v.insert(lt::VAR_DURATION, Value::Text(dt.plain_text()));
        if let Some(t) = self.exam_token(lt::DURATION, &v, misses) {
            lines.push(EffectLine {
                text: t,
                state: LineState::Duration,
            });
        }
    }

    // -----------------------------------------------------------------------------------------
    // §9.4 modifier lines
    // -----------------------------------------------------------------------------------------

    fn operator(&self, m: &ModEntry) -> Option<ModOp> {
        let key = m.op?;
        self.schema
            .modifiers
            .op
            .id()
            .and_then(|pid| self.data.enum_label(pid, key))
            .and_then(|l| ModOp::from_label(&l))
            .or_else(|| ModOp::from_observed_key(key))
    }

    /// The value of an entry at `level` (`combat-data.md` §5.6 steps 2–4, without a
    /// property source): the identity value when a ranged entry does not apply.
    fn entry_value(&self, m: &ModEntry, op: ModOp, level: i32) -> Option<PropertyValue> {
        if m.is_skipped() {
            return None;
        }
        let l = i64::from(level);
        if !m.applies_at(Some(l)) {
            return identity(op, self.data.property_type(m.modified));
        }
        let source = |d: Did| self.progression(d);
        let nested = m
            .progression
            .and_then(|p| self.progression(p))
            .is_some_and(|p| matches!(p.data, ProgressionData::Progressions(_)));
        if nested {
            m.resolve_value(&source, 1, Some(l))
        } else {
            m.resolve_value(&source, l, None)
        }
    }

    /// One `Mod_Entry`'s line (§9.4), or none.
    pub(crate) fn modifier_line(
        &self,
        m: &ModEntry,
        level: i32,
        misses: &mut u32,
    ) -> Option<RichText> {
        let op = self.operator(m)?;
        let value = self.entry_value(m, op, level)?;
        // Step 3: the metadata object.
        let meta = self.metadata(m.modified)?.clone();
        let name = match &meta.name {
            Some(n) => self.info_text(n, misses).map(|t| t.plain_text()),
            None => None,
        }
        .unwrap_or_default();
        // Step 4: hidden identity values.
        if m.description_override.is_none()
            && meta.hide_identity
            && identity(op, self.data.property_type(m.modified))
                .is_some_and(|id| equals_identity(&value, &id))
        {
            return None;
        }
        // Step 5: the override.
        if let Some(o) = &m.description_override {
            let mut v = Values::new();
            if !matches!(value, PropertyValue::Array(_)) {
                if let Some(t) = self.value_text(&value, op, false, m.modified, misses) {
                    v.insert(lt::VAR_VALUE, Value::Text(t));
                }
                if let Some(t) = self.value_text(&value, op, true, m.modified, misses) {
                    v.insert(lt::VAR_PERCENT, Value::Text(t));
                }
            }
            return self.render_counted(TextRef::Info(o), &v, misses);
        }
        // Step 6: generated line, normalised.
        let (op, value) = match (op, &value) {
            (ModOp::Add | ModOp::Subtract, PropertyValue::Float(x)) if *x < 0.0 => {
                (flip(op), PropertyValue::Float(-x))
            }
            (ModOp::Add | ModOp::Subtract, PropertyValue::Int(x)) if *x < 0 => {
                (flip(op), PropertyValue::Int(x.saturating_neg()))
            }
            (ModOp::Add | ModOp::Subtract, PropertyValue::Int64(x)) if *x < 0 => {
                (flip(op), PropertyValue::Int64(x.saturating_neg()))
            }
            _ => (op, value),
        };
        if !matches!(
            value,
            PropertyValue::String(_)
                | PropertyValue::Enum(_)
                | PropertyValue::Float(_)
                | PropertyValue::StringInfo(_)
                | PropertyValue::Bitfield64(_)
                | PropertyValue::Int(_)
                | PropertyValue::Bitfield32(_)
                | PropertyValue::Int64(_)
                | PropertyValue::Bool(_)
                | PropertyValue::BitfieldAny(_)
        ) {
            return None;
        }
        // Step 7.
        let token = match op {
            ModOp::Add => lt::ADD,
            ModOp::Subtract => lt::SUBTRACT,
            ModOp::Multiply => match value {
                PropertyValue::Float(x) if meta.percent => {
                    if x >= 1.0 {
                        lt::MULTIPLY_INCREASE
                    } else {
                        lt::MULTIPLY_DECREASE
                    }
                }
                _ => lt::MULTIPLY,
            },
            ModOp::Set => lt::SET,
            ModOp::Or => lt::OR,
            ModOp::ArrayAdd => lt::ARRAY_ADD,
            _ => return None,
        };
        let vt = self.value_text(&value, op, meta.percent, m.modified, misses)?;
        let mut v = Values::new();
        v.insert(lt::VAR_VALUE, Value::Text(vt));
        v.insert(lt::VAR_NAME, Value::Text(name));
        self.exam_token(token, &v, misses)
    }

    /// The value text of step 8 (§9.4), or the bare value when its token cannot be found.
    fn value_text(
        &self,
        value: &PropertyValue,
        op: ModOp,
        percent: bool,
        pid: u32,
        misses: &mut u32,
    ) -> Option<String> {
        let number = match value {
            PropertyValue::Int(i) => i.to_string(),
            PropertyValue::Int64(i) => i.to_string(),
            PropertyValue::Float(x) => {
                let x = *x;
                let shown = if percent {
                    // `misc-records.md` §13.1 X4, in single precision.
                    let d = if op == ModOp::Multiply {
                        if x > 1.0 {
                            x - 1.0
                        } else if x < 1.0 {
                            1.0 - x
                        } else {
                            x
                        }
                    } else {
                        x
                    };
                    d * 100.0
                } else {
                    x
                };
                // Tessera rule (§9.4 OPEN): 3 decimal places always.
                english_float(f64::from(shown), 3, NumberOptions::TOOLTIP)
            }
            PropertyValue::Bool(b) => {
                let token = if *b {
                    tokens::UTILS_YES
                } else {
                    tokens::UTILS_NO
                };
                self.render_counted(
                    TextRef::Token {
                        table_key: tokens::UTILS_TABLE_KEY,
                        token,
                    },
                    &Values::new(),
                    misses,
                )?
                .plain_text()
            }
            PropertyValue::Enum(k) => self.data.enum_text(pid, *k)?,
            PropertyValue::StringInfo(i) => self.info_text(i, misses)?.plain_text(),
            PropertyValue::String(s) => s.clone(),
            PropertyValue::Bitfield32(b) => {
                self.bits_text(pid, (0..32).filter(|i| b >> i & 1 == 1))?
            }
            PropertyValue::Bitfield64(b) => {
                self.bits_text(pid, (0..64).filter(|i| b >> i & 1 == 1))?
            }
            PropertyValue::BitfieldAny(b) => self.bits_text(pid, b.set_bits().take(4096))?,
            _ => return None,
        };
        let token = if percent {
            lt::VALUE_PERCENT
        } else {
            lt::VALUE_PLAIN
        };
        let mut v = Values::new();
        v.insert(lt::VAR_VALUE, Value::Text(number.clone()));
        match self.data.render(
            TextRef::Token {
                table_key: tokens::EXAMINATION_TABLE_KEY,
                token,
            },
            &v,
        ) {
            Ok(Some(t)) => Some(t.plain_text()),
            Ok(None) => Some(number),
            Err(_) => {
                *misses = misses.saturating_add(1);
                Some(number)
            }
        }
    }

    /// The list text of a bitfield's set bits (`item-tooltips.md` §6.4 step 7).
    pub(crate) fn bits_text(&self, pid: u32, bits: impl Iterator<Item = u32>) -> Option<String> {
        let mut texts = Vec::new();
        for i in bits {
            texts.push(self.data.enum_text(pid, i.checked_add(1)?)?);
            if texts.len() >= MAX_LINES {
                break;
            }
        }
        super::rules::join_list(&texts, |a, b| {
            let mut v = Values::new();
            v.insert(tokens::VAR_FIRST, Value::Text(a.to_owned()));
            v.insert(tokens::VAR_SECOND, Value::Text(b.to_owned()));
            match self.data.render(
                TextRef::Token {
                    table_key: tokens::UTILS_TABLE_KEY,
                    token: tokens::UTILS_LIST,
                },
                &v,
            ) {
                Ok(Some(r)) => r.plain_text(),
                _ => format!("{a}, {b}"),
            }
        })
    }

    // -----------------------------------------------------------------------------------------
    // §9.6 skill lists, §9.7 item lists
    // -----------------------------------------------------------------------------------------

    /// The effect references of list property `list` inside `holder` (a set or a struct's
    /// fields), placeholders skipped.
    pub(crate) fn refs_of(&self, value: Option<&PropertyValue>, list: u32) -> Vec<EffectReference> {
        let Some(v) = value else {
            return Vec::new();
        };
        let tmp = PropertySet {
            entries: vec![PropertyEntry {
                pid: list,
                value: v.clone(),
            }],
            ..PropertySet::default()
        };
        effect_references(&tmp, &self.effect_ref_pids)
            .into_iter()
            .filter(|r| !r.is_placeholder())
            .take(MAX_LIST_EFFECTS)
            .collect()
    }

    /// The effects of skill list `target` (§9.6 table, with the Tessera rule for the merged
    /// critical lists): (effect, given spellcraft).
    pub(crate) fn skill_list(&self, skill: &PropertySet, target: u32) -> Vec<(Did, Option<f32>)> {
        let c = &self.c;
        let top = |p: Option<u32>| match p {
            Some(p) => self.refs_of(skill.get(p), p),
            None => Vec::new(),
        };
        let hooks = |field: Option<u32>| -> Vec<EffectReference> {
            let (Some(list), Some(field)) = (c.hook_list, field) else {
                return Vec::new();
            };
            let Some(PropertyValue::Array(a)) = skill.get(list) else {
                return Vec::new();
            };
            let mut out = Vec::new();
            for h in &a.elements {
                if let PropertyValue::Struct(s) = &h.value {
                    out.extend(self.refs_of(s.get(field), field));
                }
                if out.len() >= MAX_LIST_EFFECTS {
                    break;
                }
            }
            out.truncate(MAX_LIST_EFFECTS);
            out
        };
        let pick = |r: Vec<EffectReference>| -> Vec<(Did, Option<f32>)> {
            r.into_iter()
                .map(|r| (r.effect, r.given_spellcraft()))
                .collect()
        };
        match target {
            1 => pick(top(c.skill_user_effects)),
            2 => pick(top(c.skill_critical_effects)),
            3 => pick(hooks(c.hook_target_effects)),
            4 => pick(hooks(c.hook_positional_effects)),
            5..=7 => {
                let crit = pick(hooks(c.hook_critical_effects));
                let dev = pick(hooks(c.hook_super_critical_effects));
                let in_list = |l: &[(Did, Option<f32>)], d: Did| l.iter().any(|(x, _)| *x == d);
                match target {
                    5 => crit
                        .iter()
                        .copied()
                        .filter(|(d, _)| !in_list(&dev, *d))
                        .collect(),
                    6 => dev
                        .iter()
                        .copied()
                        .filter(|(d, _)| !in_list(&crit, *d))
                        .collect(),
                    _ => crit
                        .iter()
                        .copied()
                        .filter(|(d, _)| in_list(&dev, *d))
                        .collect(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// The lines of a list of effects in definition mode (§9.6): each at its evaluation level
    /// (§9.8: the given spellcraft truncated, else `list_level`).
    pub(crate) fn list_lines(
        &self,
        effects: &[(Did, Option<f32>)],
        list_level: i32,
        misses: &mut u32,
    ) -> Vec<EffectLine> {
        let mut out = Vec::new();
        for &(effect, spellcraft) in effects.iter().take(MAX_LIST_EFFECTS) {
            #[allow(clippy::cast_possible_truncation)]
            let level = spellcraft
                .filter(|s| s.is_finite())
                .map_or(list_level, |s| {
                    s.trunc().clamp(i32::MIN as f32, i32::MAX as f32) as i32
                });
            let l = self.effect_lines(effect, EffectMode::Definition, level, true);
            *misses = misses.saturating_add(l.misses);
            out.extend(l.lines);
            if out.len() >= MAX_LINES {
                out.truncate(MAX_LINES);
                break;
            }
        }
        out
    }

    /// The item's own `Mod_Array` as the template effect `PropertyModEffect` describes it
    /// (§9.7 step 2): its stat lines.
    pub(crate) fn item_stat_lines(
        &self,
        item: &PropertySet,
        level: i32,
        misses: &mut u32,
    ) -> Vec<EffectLine> {
        let mods = mod_entries(item, &self.schema.modifiers);
        if mods.is_empty() {
            return Vec::new();
        }
        let template = self
            .data
            .weenie(PROPERTY_MOD_EFFECT_LABEL)
            .and_then(|d| self.data.record(d).ok().flatten());
        let empty = PropertySet::default();
        let set = template.as_deref().unwrap_or(&empty);
        let input = LineInput {
            set,
            mods: &mods,
            property_mod: true,
            mode: EffectMode::Definition,
            level,
            caster_is_viewer: true,
        };
        self.compose(&input, misses)
    }
}

/// WEENIECONTENT label of the template effect that describes an item's own modifiers
/// (`skill-tooltips.md` §9.7 step 2).
pub const PROPERTY_MOD_EFFECT_LABEL: &str = "PropertyModEffect";

fn flip(op: ModOp) -> ModOp {
    match op {
        ModOp::Add => ModOp::Subtract,
        ModOp::Subtract => ModOp::Add,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities() {
        assert_eq!(
            identity(ModOp::Add, Some(PropertyType::Float)),
            Some(PropertyValue::Float(0.0))
        );
        assert_eq!(
            identity(ModOp::Multiply, Some(PropertyType::Int)),
            Some(PropertyValue::Int(1))
        );
        assert_eq!(identity(ModOp::Set, Some(PropertyType::Int)), None);
        assert_eq!(identity(ModOp::Add, None), None);
        assert!(equals_identity(
            &PropertyValue::Float(0.0),
            &PropertyValue::Float(0.0)
        ));
        assert!(!equals_identity(
            &PropertyValue::Float(0.5),
            &PropertyValue::Float(0.0)
        ));
    }

    #[test]
    fn rounding_and_states() {
        assert_eq!(round_away(2.5), 3);
        assert_eq!(round_away(-2.5), -3);
        assert_eq!(round_away(f32::NAN), 0);
        assert_eq!(LineState::Debuffed.label(), Some("debuffed"));
        assert_eq!(LineState::Plain.label(), None);
        assert_eq!(LineState::Duration.code(), 4);
        assert_eq!(flip(ModOp::Add), ModOp::Subtract);
    }
}
