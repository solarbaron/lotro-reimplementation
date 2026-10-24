// SPDX-License-Identifier: GPL-3.0-or-later
//! Row content (`item-tooltips.md` §6) with the Tessera rules of P1.

use std::collections::BTreeSet;

use tessera_formats::gameplay::combat::{ModEntryKind, Progression, ProgressionValue};
use tessera_formats::gameplay::items::{
    DpsTable, DurationMap, ItemSchema, ItemSetView, ItemView, control_labels, damage_range,
    equipment_category_keys, weapon_damage,
};
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_text::{RichText, Style, Value, Values};

use super::data::{TextRef, TooltipData};
use super::layout::{RowTemplate, Widget};
use super::rules::{self, BindingInputs, SaleFlags, tokens};
use super::{
    Coin, CurrencyUnit, Equipped, Examined, Kind, Line, MAX_LINES, MAX_SET_MEMBERS, Row,
    RowContent, RowFailure, TooltipEngine, Viewer,
};

/// Context of one fill: the template, the examined object and the failure list.
pub(crate) struct Fill<'a, 'b> {
    pub(crate) t: &'a RowTemplate,
    pub(crate) ex: &'a Examined<'b>,
    pub(crate) viewer: &'a Viewer,
    pub(crate) failures: &'a mut Vec<RowFailure>,
}

impl Fill<'_, '_> {
    pub(crate) fn fail(&mut self, reason: impl Into<String>) {
        self.failures.push(RowFailure {
            template: self
                .t
                .label
                .clone()
                .unwrap_or_else(|| format!("{:#010x}", self.t.at.element)),
            reason: reason.into(),
        });
    }

    fn get(&self, pid: Option<u32>) -> Option<&PropertyValue> {
        self.ex.set.get(pid?)
    }

    fn flag(&self, pid: Option<u32>) -> bool {
        matches!(self.get(pid), Some(PropertyValue::Bool(true)))
    }

    fn int(&self, pid: Option<u32>) -> Option<i32> {
        match self.get(pid)? {
            PropertyValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn key(&self, pid: Option<u32>) -> Option<u32> {
        match self.get(pid)? {
            PropertyValue::Enum(k) => Some(*k),
            _ => None,
        }
    }

    fn did(&self, pid: Option<u32>) -> Option<Did> {
        match self.get(pid)? {
            PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
            _ => None,
        }
    }
}

fn values(pairs: impl IntoIterator<Item = (u32, Value)>) -> Values {
    let mut v = Values::new();
    for (id, value) in pairs {
        v.insert(id, value);
    }
    v
}

fn text_of(rt: &RichText) -> String {
    rt.plain_text()
}

/// Placeholder characters (private use) standing for the two parts of a joined text.
const PART_A: char = '\u{F8F0}';
const PART_B: char = '\u{F8F1}';

/// `inner` on top of `outer`: the attributes `inner` sets win.
fn overlay_style(outer: &Style, inner: &Style) -> Style {
    Style {
        color: inner.color.or(outer.color),
        bold: outer.bold || inner.bold,
        italic: outer.italic || inner.italic,
        underline: outer.underline || inner.underline,
        list_item: outer.list_item || inner.list_item,
        link: inner.link.clone().or_else(|| outer.link.clone()),
        placeholder: outer.placeholder || inner.placeholder,
    }
}

/// Replaces each placeholder character of `rendered` by the spans of its part, keeping the
/// part's own styles (markup colours) over the surrounding span's.
fn splice(rendered: RichText, parts: &[(char, &RichText)]) -> RichText {
    let mut out = RichText::new();
    for span in rendered.into_spans() {
        let mut rest = span.text.as_str();
        while let Some((i, ch)) = rest
            .char_indices()
            .find(|(_, c)| parts.iter().any(|(p, _)| p == c))
        {
            out.push_str(&rest[..i], &span.style);
            if let Some((_, part)) = parts.iter().find(|(p, _)| *p == ch) {
                for inner in part.spans() {
                    out.push_str(&inner.text, &overlay_style(&span.style, &inner.style));
                }
            }
            rest = &rest[i + ch.len_utf8()..];
        }
        out.push_str(rest, &span.style);
    }
    out
}

impl<D: TooltipData> TooltipEngine<D> {
    fn render(&self, f: &mut Fill<'_, '_>, text: TextRef<'_>, v: &Values) -> Option<RichText> {
        match self.data.render(text, v) {
            Ok(t) => t,
            Err(e) => {
                f.fail(e);
                None
            }
        }
    }

    /// Renders the row's format; a row without one fails.
    fn render_format(&self, f: &mut Fill<'_, '_>, v: &Values) -> Option<RichText> {
        let t = f.t;
        match &t.format {
            Some(info) => self.render(f, TextRef::Info(info), v),
            None => {
                f.fail("row has no format string");
                None
            }
        }
    }

    fn row(&self, t: &RowTemplate, content: RowContent, state: Option<&str>) -> Row {
        let colour = state.and_then(|s| t.state_colour(s)).or(t.colour);
        Row {
            label: t.label.clone(),
            widget: t.widget.clone(),
            size: t.size,
            stretch_wide: t.stretch_wide,
            horizontal: t.horizontal,
            content,
            state: state.map(str::to_owned),
            colour,
        }
    }

    fn text_row(&self, t: &RowTemplate, text: Option<RichText>, state: Option<&str>) -> Row {
        self.row(t, text.map_or(RowContent::Empty, RowContent::Text), state)
    }

    /// Fills one row (§6).
    pub(crate) fn fill(
        &self,
        t: &RowTemplate,
        ex: &Examined<'_>,
        viewer: &Viewer,
        failures: &mut Vec<RowFailure>,
    ) -> Row {
        // `skill-tooltips.md` §6.1 step 1: an item tooltip's skill-item rows show the item's
        // skill. Only rows whose provider is a child of the skill examination base do this
        // (ST23); the other providers always examine the examined object.
        if t.for_skill_item && ex.kind == Kind::Item && t.skill_provider() {
            return self.fill_skill_item(t, ex, viewer, failures);
        }
        let mut f = Fill {
            t,
            ex,
            viewer,
            failures,
        };
        let item = ex.kind == Kind::Item;
        match &t.widget {
            Widget::Separator => self.row(t, RowContent::Separator, None),
            Widget::Name if !item => self.combat_name(&mut f),
            Widget::Name => self.name(&mut f),
            Widget::Icon if !item => self.combat_icon(&mut f),
            Widget::Icon => match ex.did {
                Some(item) => self.row(
                    t,
                    RowContent::Icon {
                        item,
                        ghosted: false,
                    },
                    None,
                ),
                None => self.row(t, RowContent::Empty, None),
            },
            Widget::Binding => self.binding(&mut f),
            Widget::Uniqueness => {
                let on = f.flag(self.p.unique);
                self.fixed_flag(&mut f, on)
            }
            Widget::Indestructible => {
                let on = f.flag(self.p.indestructible);
                self.fixed_flag(&mut f, on)
            }
            Widget::Generic => self.generic(&mut f),
            Widget::ItemLevel => self.item_level(&mut f),
            Widget::Armour => {
                let a = f.int(self.p.armour).unwrap_or(0);
                let text = (a != 0)
                    .then(|| self.render_format(&mut f, &values([(tokens::VAR_VALUE, a.into())])))
                    .flatten();
                self.text_row(t, text, None)
            }
            Widget::Durability => self.durability(&mut f),
            Widget::Worth => self.worth(&mut f),
            Widget::Damage | Widget::Dps => self.damage(&mut f),
            Widget::EquipmentCategory => self.category(&mut f),
            Widget::AllowedSlots => self.allowed_slots(&mut f),
            Widget::Requirements => self.requirements(&mut f),
            Widget::LevelRange => self.level_range(&mut f),
            Widget::EffectList => match ex.kind {
                Kind::Item => self.effect_list(&mut f),
                Kind::Skill => self.skill_effect_list(&mut f),
                Kind::Effect => self.empty_row(t),
            },
            Widget::SkillQualifier if ex.kind == Kind::Skill => self.qualifier(&mut f),
            Widget::SkillMaxRange if ex.kind == Kind::Skill => self.max_range(&mut f),
            Widget::SkillMaxTargets if ex.kind == Kind::Skill => self.max_targets(&mut f),
            Widget::SkillCastingTime if ex.kind == Kind::Skill => self.casting_time(&mut f),
            Widget::SkillVitalCost if ex.kind == Kind::Skill => self.vital_cost(&mut f),
            Widget::SkillSpecialClass if ex.kind == Kind::Skill => self.special_class(&mut f),
            Widget::SkillTrait if ex.kind == Kind::Skill => self.required_trait(&mut f),
            Widget::SkillRecoveryTime if ex.kind == Kind::Skill => self.recovery(&mut f),
            Widget::EffectResistance if ex.kind == Kind::Effect => self.effect_resistance(&mut f),
            Widget::EffectDescription if ex.kind == Kind::Effect => self.effect_description(&mut f),
            Widget::EffectTime if ex.kind == Kind::Effect => self.effect_time(&mut f),
            Widget::SkillQualifier
            | Widget::SkillMaxRange
            | Widget::SkillMaxTargets
            | Widget::SkillCastingTime
            | Widget::SkillVitalCost
            | Widget::SkillSpecialClass
            | Widget::SkillTrait
            | Widget::SkillRecoveryTime
            | Widget::EffectResistance
            | Widget::EffectDescription
            | Widget::EffectTime => self.empty_row(t),
            Widget::SetMembers | Widget::SetBonuses => self.set_lines(&mut f),
            Widget::Cooldown | Widget::Decay => self.duration_row(&mut f),
            Widget::RecipeOneShot | Widget::RecipeCooldown if item => self.recipe_row(&mut f),
            Widget::RecipeOneShot | Widget::RecipeCooldown => self.empty_row(t),
            Widget::QuestId => self.quest(&mut f),
            Widget::Pair => {
                let mut sides = t.children.iter().map(|c| {
                    c.as_ref()
                        .map(|c| Box::new(self.fill(c, ex, viewer, f.failures)))
                });
                let left = sides.next().flatten();
                let right = sides.next().flatten();
                self.row(t, RowContent::Pair { left, right }, None)
            }
            Widget::List => {
                let rows = t
                    .children
                    .iter()
                    .flatten()
                    .map(|c| self.fill(c, ex, viewer, f.failures))
                    .collect();
                self.row(t, RowContent::List(rows), None)
            }
            Widget::StaticText => {
                let text = match &t.fixed_text {
                    Some(info) => self.render(&mut f, TextRef::Info(info), &Values::new()),
                    None => None,
                };
                self.text_row(t, text, None)
            }
            Widget::Deferred(_) => self.row(t, RowContent::Empty, None),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §6.2 Name
    // -----------------------------------------------------------------------------------------

    fn name(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let state = rules::quality_state_label(f.key(self.p.quality));
        let colour = self.name_colour(state);
        let ex = f.ex;
        let text = match self.p.name.and_then(|p| ex.set.get(p)) {
            Some(PropertyValue::StringInfo(info)) => {
                let info: &StringInfo = info;
                match &t.format {
                    Some(format) => match self.data.text_value(info) {
                        Ok(v) => {
                            self.render(f, TextRef::Info(format), &values([(tokens::VAR_NAME, v)]))
                        }
                        Err(e) => {
                            f.fail(e);
                            None
                        }
                    },
                    None => self.render(f, TextRef::Info(info), &Values::new()),
                }
            }
            _ => None,
        };
        let mut row = self.text_row(t, text, Some(state));
        row.colour = colour.or(t.colour);
        row
    }

    // -----------------------------------------------------------------------------------------
    // §6.5 Binding, §6.6 uniqueness, §6.8 indestructible
    // -----------------------------------------------------------------------------------------

    fn binding(&self, f: &mut Fill<'_, '_>) -> Row {
        let bound_to_id = match f.get(self.p.bound_to_id) {
            Some(PropertyValue::InstanceId(id)) => *id,
            Some(PropertyValue::Int64(i)) => i.cast_unsigned(),
            _ => 0,
        };
        let inputs = BindingInputs {
            bound_to_name: match f.get(self.p.bound_to_name) {
                Some(PropertyValue::String(s)) => Some(s.clone()),
                _ => None,
            },
            bound_to_id,
            known_name: None,
            to_account: f.flag(self.p.bind_to_account),
            on_raid: f.flag(self.p.bind_on_raid),
            on_equip: f.flag(self.p.bind_on_equip),
            on_acquire: f.flag(self.p.bind_on_acquire),
        };
        let text = rules::binding_text(&inputs).and_then(|b| {
            let v = match b.name {
                Some(n) => values([(tokens::VAR_BOUND_NAME, Value::Text(n))]),
                None => Values::new(),
            };
            self.render(
                f,
                TextRef::Token {
                    table_key: tokens::EXAMINATION_TABLE_KEY,
                    token: b.token,
                },
                &v,
            )
        });
        self.text_row(f.t, text, None)
    }

    fn fixed_flag(&self, f: &mut Fill<'_, '_>, on: bool) -> Row {
        let t = f.t;
        let text = if on {
            match &t.fixed_text {
                Some(info) => self.render(f, TextRef::Info(info), &Values::new()),
                None => {
                    f.fail("row has no text property");
                    None
                }
            }
        } else {
            None
        };
        self.text_row(t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §6.4 Generic property rows
    // -----------------------------------------------------------------------------------------

    fn generic(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let empty = |s: &Self| s.row(t, RowContent::Empty, None);
        let Some(pid) = t.generic else {
            return empty(self);
        };
        // Provider step 1–2: instance first, then record (the overlay); nothing pending.
        let Some(v) = f.ex.set.get(pid).cloned() else {
            return empty(self);
        };
        // Widget step 3.
        let is_did = matches!(v, PropertyValue::DidRef(_));
        if t.format.is_none() && !is_did {
            return empty(self);
        }
        // Step 4.
        if !t.display_default {
            if self.data.default_value(pid).as_ref() == Some(&v) {
                return empty(self);
            }
            if let PropertyValue::StringInfo(info) = &v {
                // An unresolved text (a lookup miss) resolves to empty, as an empty text
                // does: the row is empty, not failed (descriptions naming missing tokens
                // are ordinary in the data).
                match self.data.render(TextRef::Info(info), &Values::new()) {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => return empty(self),
                }
            }
        }
        // Step 5: a row holding its own value of the comparison property shows V only
        // when the two are equal.
        if t.comparison.is_some()
            && let Some(own) = &t.comparison_value
            && *own != v
        {
            return empty(self);
        }
        // Step 6.
        if let PropertyValue::DidRef(d) = v {
            return if d.is_null() {
                empty(self)
            } else {
                self.row(t, RowContent::Image(d), None)
            };
        }
        // Step 7: the value variable, or unset.
        let value: Option<Value> = match &v {
            PropertyValue::String(s) => Some(Value::Text(s.clone())),
            PropertyValue::Enum(k) => self.data.enum_text(pid, *k).map(Value::Text),
            PropertyValue::Float(x) => Some(Value::Text(if t.as_duration {
                rules::duration_text(f64::from(*x))
            } else {
                let prec = usize::try_from(t.float_precision.clamp(0, 9)).unwrap_or(1);
                format!("{x:.prec$}")
            })),
            PropertyValue::Int(i) => Some(if t.as_duration {
                Value::Text(rules::duration_text(f64::from(*i)))
            } else {
                Value::Integer(i64::from(*i))
            }),
            PropertyValue::Int64(i) => Some(Value::Integer(*i)),
            PropertyValue::StringInfo(info) => match self.data.text_value(info) {
                Ok(v) => Some(v),
                Err(e) => {
                    f.fail(e);
                    None
                }
            },
            PropertyValue::Bool(b) => {
                let token = if *b {
                    tokens::UTILS_YES
                } else {
                    tokens::UTILS_NO
                };
                self.render(
                    f,
                    TextRef::Token {
                        table_key: tokens::UTILS_TABLE_KEY,
                        token,
                    },
                    &Values::new(),
                )
                .map(|r| Value::Text(text_of(&r)))
            }
            PropertyValue::Bitfield32(b) => {
                self.bit_list(f, pid, (0..32).filter(|i| b >> i & 1 == 1))
            }
            PropertyValue::Bitfield64(b) => {
                self.bit_list(f, pid, (0..64).filter(|i| b >> i & 1 == 1))
            }
            PropertyValue::BitfieldAny(b) => self.bit_list(f, pid, b.set_bits().take(4096)),
            _ => None,
        };
        let v = match value {
            Some(x) => values([(tokens::VAR_VALUE, x)]),
            None => Values::new(),
        };
        // Steps 8–9: G25 leaves the row empty when the format needs the unset variable.
        let text = self.render_format(f, &v);
        self.text_row(t, text, None)
    }

    /// The list text of a bitfield (§6.4 step 7): display texts of the set bits' keys in
    /// ascending bit order, joined pairwise by the `Utils` list format.
    fn bit_list(
        &self,
        f: &mut Fill<'_, '_>,
        pid: u32,
        bits: impl Iterator<Item = u32>,
    ) -> Option<Value> {
        let mut texts = Vec::new();
        for i in bits {
            texts.push(self.data.enum_text(pid, i.checked_add(1)?)?);
            if texts.len() >= MAX_LINES {
                break;
            }
        }
        let joined = rules::join_list(&texts, |a, b| {
            let v = values([
                (tokens::VAR_FIRST, Value::Text(a.to_owned())),
                (tokens::VAR_SECOND, Value::Text(b.to_owned())),
            ]);
            match self.data.render(
                TextRef::Token {
                    table_key: tokens::UTILS_TABLE_KEY,
                    token: tokens::UTILS_LIST,
                },
                &v,
            ) {
                Ok(Some(r)) => text_of(&r),
                Ok(None) => format!("{a}, {b}"),
                Err(e) => {
                    f.fail(e);
                    format!("{a}, {b}")
                }
            }
        })?;
        Some(Value::Text(joined))
    }

    // -----------------------------------------------------------------------------------------
    // §6.7 Item level
    // -----------------------------------------------------------------------------------------

    fn item_level(&self, f: &mut Fill<'_, '_>) -> Row {
        let level = f.int(self.p.level).unwrap_or(0);
        // Tessera rule (§6.7 OPEN): the viewer-effective level equals the item level.
        let (token, vars) = rules::item_level_text(level, level);
        let v = values(
            vars.into_iter()
                .map(|(id, n)| (id, Value::Integer(i64::from(n)))),
        );
        let exam = |token| TextRef::Token {
            table_key: tokens::EXAMINATION_TABLE_KEY,
            token,
        };
        let level_text = self.render(f, exam(token), &v);
        let marker = if f.flag(self.p.level_bonus_pending) {
            self.render(f, exam(tokens::LEVEL_BONUS_PENDING), &Values::new())
        } else {
            None
        };
        // Step 3: 0x5425 = the level text, 0x59BF = the marker; an empty side yields the
        // other. Both parts keep their own styles (the marker's markup colour).
        let text = match (level_text, marker) {
            (Some(a), Some(b)) => {
                let v = values([
                    (tokens::VAR_FIRST, Value::Text(PART_A.to_string())),
                    (tokens::VAR_SECOND, Value::Text(PART_B.to_string())),
                ]);
                let pair = TextRef::Token {
                    table_key: tokens::UTILS_TABLE_KEY,
                    token: tokens::UTILS_PAIR,
                };
                Some(match self.render(f, pair, &v) {
                    Some(r) => splice(r, &[(PART_A, &a), (PART_B, &b)]),
                    None => {
                        let mut out = a;
                        out.push_str(" ", &Style::default());
                        out.append(b);
                        out
                    }
                })
            }
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        };
        let text = text.filter(|t| !t.plain_text().trim().is_empty());
        self.text_row(f.t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §6.10 Durability (with §11 condition)
    // -----------------------------------------------------------------------------------------

    fn durability(&self, f: &mut Fill<'_, '_>) -> Row {
        let (cur, max, wear) = match f.ex.instance.condition {
            Some(c) => (
                i32::try_from(c.current).ok(),
                i32::try_from(c.max).ok(),
                rules::condition_wear_state(c.current, c.max, f.key(self.p.wear_state)),
            ),
            None => (
                f.int(self.p.cur_points),
                f.int(self.p.max_points),
                f.key(self.p.wear_state),
            ),
        };
        let Some((cur, max, state)) = rules::durability(cur, max, wear) else {
            return self.row(f.t, RowContent::Empty, None);
        };
        let v = values([
            (tokens::VAR_DURABILITY_CURRENT, cur.into()),
            (tokens::VAR_DURABILITY_MAX, max.into()),
        ]);
        let text = self.render_format(f, &v);
        self.text_row(f.t, text, state.label())
    }

    // -----------------------------------------------------------------------------------------
    // §6.11 Worth
    // -----------------------------------------------------------------------------------------

    fn currencies(&self) -> &[CurrencyUnit] {
        self.currencies.get_or_init(|| {
            let read = || -> Option<Vec<CurrencyUnit>> {
                let dir = self.data.weenie("CurrencyDirectory")?;
                let dir = self.data.record(dir).ok()??;
                let PropertyValue::Array(list) = dir.get(self.p.currency_list?)? else {
                    return None;
                };
                let mut out = Vec::new();
                for e in list.elements.iter().take(16) {
                    let PropertyValue::DidRef(did) = e.value else {
                        continue;
                    };
                    let Some(c) = self.data.record(did).ok().flatten() else {
                        continue;
                    };
                    if let Some(PropertyValue::Int(v)) =
                        self.p.currency_value.and_then(|p| c.get(p))
                    {
                        out.push(CurrencyUnit {
                            did,
                            value: i64::from(*v),
                        });
                    }
                }
                // `item-tooltips.md` §6.11 order caveat (`npc-windows.md` §9.3): the
                // property set does not store the coins largest first, so a reader of it
                // sorts them by `Currency_Value`, largest first (stable for equal values).
                out.sort_by_key(|c| std::cmp::Reverse(c.value));
                Some(out)
            };
            read().unwrap_or_default()
        })
    }

    fn worth(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let value = f.int(self.p.value);
        let quantity = f.ex.instance.quantity;
        let q = i32::try_from(quantity.unwrap_or(1).clamp(1, i64::from(i32::MAX))).unwrap_or(1);
        let flags = SaleFlags {
            force_not_sellable: f.flag(self.p.force_not_sellable),
            zero_sale_value: value.unwrap_or(0).wrapping_mul(q) == 0,
            quest_item: f.flag(self.p.quest_item),
            indestructible: f.flag(self.p.indestructible),
            barter_item: f.flag(self.p.barter),
            legendary: f.flag(self.p.legendary),
            store_item: f.flag(self.p.store_item),
        };
        let amount = rules::worth_amount(value, quantity).filter(|_| flags.sellable());
        let Some(amount) = amount else {
            return self.row(t, RowContent::Empty, None);
        };
        let currencies = self.currencies();
        if currencies.is_empty() {
            f.fail("no currency directory");
            return self.row(t, RowContent::Empty, None);
        }
        let mut coins = Vec::new();
        for c in rules::coin_breakdown(i64::from(amount), currencies) {
            let v = values([(tokens::VAR_COIN_COUNT, Value::Integer(c.count))]);
            // A failed count format falls back to the digits (and is a row failure).
            let text = self
                .render(
                    f,
                    TextRef::Token {
                        table_key: tokens::NUMBER_TABLE_KEY,
                        token: tokens::COIN_COUNT,
                    },
                    &v,
                )
                .map_or_else(|| c.count.to_string(), |r| text_of(&r));
            let Some(unit) = currencies.get(c.currency) else {
                continue;
            };
            coins.push(Coin {
                currency: unit.did,
                index: c.currency,
                count: c.count,
                text,
            });
        }
        let label = self.render(
            f,
            TextRef::Token {
                table_key: tokens::EXAMINATION_TABLE_KEY,
                token: tokens::WORTH_LABEL,
            },
            &Values::new(),
        );
        self.row(t, RowContent::Money { label, coins }, None)
    }

    // -----------------------------------------------------------------------------------------
    // §6.12 Damage and DPS
    // -----------------------------------------------------------------------------------------

    fn damage(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let view = ItemView::new(&f.ex.set, &self.schema);
        let Some(w) = view.weapon() else {
            return self.row(t, RowContent::Empty, None);
        };
        let quality = view.quality().unwrap_or(0);
        let table_dps = w.dps_table.and_then(|d| {
            let set = self.data.record(d).ok()??;
            DpsTable::from_set(&*set, &self.schema.tables)
                .ok()?
                .dps(w.dps_level?, quality)
        });
        let Some(dps) = table_dps.or(w.base_dps).filter(|d| d.is_finite()) else {
            return self.row(t, RowContent::Empty, None);
        };
        let v = if t.widget == Widget::Dps {
            values([(tokens::VAR_DPS, Value::Text(format!("{dps:.1}")))])
        } else {
            let variance = w.variance.unwrap_or(0.0);
            let max = w
                .damage
                .or_else(|| weapon_damage(dps, w.action_duration?, variance));
            let Some(max) = max.filter(|m| m.is_finite()) else {
                return self.row(t, RowContent::Empty, None);
            };
            let (lo, hi) = damage_range(max, variance);
            let kind = w
                .damage_type
                .and_then(|k| self.data.enum_text(self.p.damage_type?, k))
                .unwrap_or_default();
            values([
                (tokens::VAR_DAMAGE[0], Value::Integer(round(lo))),
                (tokens::VAR_DAMAGE[1], Value::Integer(round(hi))),
                (tokens::VAR_DAMAGE[2], Value::Text(kind)),
            ])
        };
        let text = self.render_format(f, &v);
        self.text_row(t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §6.13 Equipment category, §6.14 allowed slots
    // -----------------------------------------------------------------------------------------

    fn category(&self, f: &mut Fill<'_, '_>) -> Row {
        let bits = match f.get(self.p.equipment_category) {
            Some(PropertyValue::Bitfield64(b)) => *b,
            Some(PropertyValue::Bitfield32(b)) => u64::from(*b),
            _ => 0,
        };
        let text = equipment_category_keys(bits)
            .next()
            .and_then(|k| self.data.enum_text(self.p.equipment_category?, k));
        let text = text
            .and_then(|c| self.render_format(f, &values([(tokens::VAR_CATEGORY, Value::Text(c))])));
        // Tessera rule (§6.13 OPEN): never `cantuse`.
        self.text_row(f.t, text, None)
    }

    fn excluded_slot_keys(&self) -> &[u32] {
        self.slot_exclusions.get_or_init(|| {
            let Some(pid) = self.p.compatible_slot else {
                return Vec::new();
            };
            ["Backpack", "Overflow", "Equipment"]
                .iter()
                .filter_map(|n| self.data.enum_key(pid, n))
                .collect()
        })
    }

    fn allowed_slots(&self, f: &mut Fill<'_, '_>) -> Row {
        let bits = self.slot_bits(&f.ex.set);
        let excluded = self.excluded_slot_keys();
        let lines = (0..32u32)
            .filter(|i| bits >> i & 1 == 1)
            .map(|i| i + 1)
            .filter(|k| !excluded.contains(k))
            .filter_map(|k| self.data.enum_text(self.p.compatible_slot?, k))
            .take(MAX_LINES)
            .map(|s| Line {
                text: RichText::plain(&s),
                state: None,
                colour: f.t.line_colour(None),
            })
            .collect();
        self.row(
            f.t,
            RowContent::Lines {
                header: None,
                lines,
            },
            None,
        )
    }

    // -----------------------------------------------------------------------------------------
    // §6.15 Requirements (Tessera rule)
    // -----------------------------------------------------------------------------------------

    fn requirement_line(&self, t: &RowTemplate, text: String, fails: bool) -> Line {
        let state = fails.then(|| "cantuse".to_owned());
        let colour = t.line_colour(state.as_deref());
        Line {
            text: RichText::plain(&text),
            state,
            colour,
        }
    }

    fn names(&self, pid: Option<u32>, keys: &[u32]) -> String {
        keys.iter()
            .take(MAX_LINES)
            .map(|&k| {
                pid.and_then(|p| self.data.enum_text(p, k))
                    .unwrap_or_else(|| format!("#{k}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn requirements(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let r = ItemView::new(&f.ex.set, &self.schema).requirements();
        let v = f.viewer;
        let mut lines = Vec::new();
        if let Some(n) = r.min_level.filter(|&n| n > 0) {
            let fails = v.level.is_some_and(|l| l < n);
            lines.push(self.requirement_line(t, format!("Minimum level {n}"), fails));
        }
        if let Some(n) = r.max_level.filter(|&n| n > 0) {
            let fails = v.level.is_some_and(|l| l > n);
            lines.push(self.requirement_line(t, format!("Maximum level {n}"), fails));
        }
        if !r.classes.is_empty() {
            let fails = v.class.is_some_and(|c| !r.classes.contains(&c));
            let text = format!("Class: {}", self.names(self.p.class_type, &r.classes));
            lines.push(self.requirement_line(t, text, fails));
        }
        if !r.races.is_empty() {
            let fails = v.race.is_some_and(|c| !r.races.contains(&c));
            let text = format!("Race: {}", self.names(self.p.race_entry, &r.races));
            lines.push(self.requirement_line(t, text, fails));
        }
        self.row(
            t,
            RowContent::Lines {
                header: None,
                lines,
            },
            None,
        )
    }

    fn level_range(&self, f: &mut Fill<'_, '_>) -> Row {
        let r = ItemView::new(&f.ex.set, &self.schema).requirements();
        let text = match (r.min_level, r.max_level) {
            (Some(lo), Some(hi)) if hi > 0 => self.render_format(
                f,
                &values([
                    (tokens::VAR_RANGE[0], lo.into()),
                    (tokens::VAR_RANGE[1], hi.into()),
                ]),
            ),
            _ => None,
        };
        self.text_row(f.t, text, None)
    }

    // -----------------------------------------------------------------------------------------
    // §6.16 Effect and stat lines (Tessera rule)
    // -----------------------------------------------------------------------------------------

    /// The `Effect_Name` text of an effect record; a record or text that cannot be read is
    /// a row failure.
    fn effect_name(&self, f: &mut Fill<'_, '_>, effect: Did) -> Option<String> {
        let set = match self.data.record(effect) {
            Ok(s) => s?,
            Err(e) => {
                f.fail(e);
                return None;
            }
        };
        let PropertyValue::StringInfo(info) = set.get(self.p.effect_name?)? else {
            return None;
        };
        self.content_text(info).map(|r| text_of(&r))
    }

    /// A text the examined content names (an effect, set or member name): a lookup miss
    /// resolves to no text, like an empty text (§6.4 step 4), and is not a row failure —
    /// records naming missing tokens are ordinary in the data (§12 OPEN).
    fn content_text(&self, info: &StringInfo) -> Option<RichText> {
        self.data
            .render(TextRef::Info(info), &Values::new())
            .ok()
            .flatten()
    }

    fn progression_value(&self, did: Did, level: i32) -> Option<String> {
        let set = self.data.record(did).ok()??;
        let p = Progression::from_properties(&*set, false, &self.schema.progressions).ok()?;
        match p.value_at(i64::from(level))? {
            ProgressionValue::Float(x) => Some(stat_number(x)),
            ProgressionValue::Property(v) => value_text(&v),
            _ => None,
        }
    }

    fn mod_lines(
        &self,
        mods: &[tessera_formats::gameplay::combat::ModEntry],
        level: i32,
        colour: Option<u32>,
    ) -> Vec<Line> {
        mods.iter()
            .filter(|m| {
                matches!(
                    m.kind,
                    ModEntryKind::Inline
                        | ModEntryKind::ProgressionDriven
                        | ModEntryKind::Conflicting
                )
            })
            .take(MAX_LINES)
            .map(|m| {
                let name = self
                    .data
                    .property_name(m.modified)
                    .unwrap_or_else(|| format!("{:#x}", m.modified));
                let value = match (m.progression, &m.value) {
                    (Some(p), _) => self.progression_value(p, level),
                    (None, Some(v)) => value_text(v),
                    (None, None) => None,
                };
                let text = match value {
                    Some(v) => format!("{name}: {v}"),
                    None => name,
                };
                Line {
                    text: RichText::plain(&text),
                    state: None,
                    colour,
                }
            })
            .collect()
    }

    /// The item effect-list rows (`item-tooltips.md` §6.16, P2 revision: the effect lines of
    /// `skill-tooltips.md` §9 over the item's lists of §9.7, filtered by the row's
    /// `UI_Examination_Tooltip_EffectCategory`; a row without one is matched by its label as in
    /// P1).
    fn effect_list(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let mask = t.effect_category.unwrap_or_else(|| {
            let label = t.label.as_deref().unwrap_or("");
            if label.contains("TargetEffect") {
                0x20
            } else if label.contains("UsageEffect") {
                0x08
            } else {
                0x10
            }
        });
        let lines = self.item_effect_lines(f, mask);
        self.lines_row(f, lines)
    }

    // -----------------------------------------------------------------------------------------
    // §6.17 Item sets (Tessera rule)
    // -----------------------------------------------------------------------------------------

    fn set_lines(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let empty = |s: &Self| s.row(t, RowContent::Empty, None);
        let set_did = f
            .did(self.p.item_property_set)
            .or_else(|| f.ex.did.and_then(|d| self.data.set_of(d)));
        let Some(set_did) = set_did else {
            return empty(self);
        };
        let set = match self.data.record(set_did) {
            Ok(Some(s)) => s,
            Ok(None) => return empty(self),
            Err(e) => {
                f.fail(e);
                return empty(self);
            }
        };
        let view = ItemSetView::new(&set, &self.schema);
        let members: Vec<Did> = view.members().into_iter().take(MAX_SET_MEMBERS).collect();
        let worn = |f: &Fill<'_, '_>, d: Did| {
            f.viewer.equipped.iter().any(|e| e.did == d)
                || (f.ex.did == Some(d) && f.ex.instance.equipped)
        };
        let count = members
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|&d| worn(f, d))
            .count();
        let state = |active: bool| {
            if active {
                "gameset_active"
            } else {
                "gameset_inactive"
            }
        };
        let line = |text: String, active: bool| {
            let s = state(active);
            Line {
                text: RichText::plain(&text),
                state: Some(s.to_owned()),
                colour: t.line_colour(Some(s)),
            }
        };
        let header = view
            .name()
            .and_then(|n| self.content_text(n))
            .map(Line::plain);
        let mut lines = Vec::new();
        if t.widget == Widget::SetMembers {
            for &m in &members {
                let record = match self.data.record(m) {
                    Ok(r) => r,
                    Err(e) => {
                        f.fail(e);
                        None
                    }
                };
                let name = record
                    .and_then(|r| match self.p.name.and_then(|p| r.get(p)) {
                        Some(PropertyValue::StringInfo(i)) => self.content_text(i),
                        _ => None,
                    })
                    .map_or_else(|| m.to_string(), |r| text_of(&r));
                let active = worn(f, m);
                lines.push(line(name, active));
            }
        } else {
            let n = if members.is_empty() {
                usize::try_from(max_active(&set, &self.schema).unwrap_or(0)).unwrap_or(0)
            } else {
                members.len()
            };
            let gate = match view.max_player_level() {
                None | Some(-1) => true,
                Some(max) => f.viewer.level.unwrap_or(1) <= max,
            };
            let tiers = view.tiers();
            for c in 2..=n.min(MAX_SET_MEMBERS) {
                let Some(tier) = tiers
                    .iter()
                    .find(|t| t.active_count.and_then(|a| usize::try_from(a).ok()) == Some(c))
                else {
                    continue;
                };
                // The tier's effects are described at the set level for c (`items.md` §7.1).
                let level = self.set_level(f, &set, &members, c);
                let mut parts: Vec<String> = Vec::new();
                for e in tier.effects.iter().take(MAX_LINES) {
                    if let Some(name) = e.effect.and_then(|d| self.effect_name(f, d)) {
                        parts.push(name);
                    }
                }
                let room = MAX_LINES.saturating_sub(parts.len());
                parts.extend(
                    self.mod_lines(&tier.mods, level, None)
                        .into_iter()
                        .take(room)
                        .map(|l| text_of(&l.text)),
                );
                lines.push(line(
                    format!("{c}: {}", parts.join(", ")),
                    gate && count >= c,
                ));
            }
        }
        if lines.is_empty() {
            return empty(self);
        }
        self.row(t, RowContent::Lines { header, lines }, None)
    }

    /// The item level of an equipped item: its instance's `Item_Level`, else its record's.
    fn equipped_level(&self, e: &Equipped) -> Option<i32> {
        let pid = self.p.level?;
        if let Some(p) = e.instance.properties.iter().rev().find(|p| p.pid == pid) {
            return match p.value {
                PropertyValue::Int(l) => Some(l),
                _ => None,
            };
        }
        match self.data.record(e.did).ok()??.get(pid)? {
            PropertyValue::Int(l) => Some(*l),
            _ => None,
        }
    }

    /// The set level for count `c` (`items.md` §7.1 "Set level for count c"), with the
    /// Tessera rules of the §6.17 OPEN: the request's given level is the examined item's
    /// level, the character's level is the viewer's (else the examined item's level), and an
    /// average is truncated to an integer level.
    fn set_level(&self, f: &Fill<'_, '_>, set: &PropertySet, members: &[Did], c: usize) -> i32 {
        let given = ItemView::new(&f.ex.set, &self.schema).level().unwrap_or(0);
        let average = self
            .schema
            .sets
            .use_average_item_level
            .id()
            .and_then(|p| set.get(p))
            .is_some_and(|v| matches!(v, PropertyValue::Bool(true)));
        if average {
            if members.is_empty() || c == 0 {
                // Member-less sets read character properties the viewer does not carry:
                // the count property reads 0, so the given level.
                return given;
            }
            // Distinct active members with the level of the copy added last.
            let mut levels: Vec<(Did, i32)> = Vec::new();
            for e in f
                .viewer
                .equipped
                .iter()
                .filter(|e| members.contains(&e.did))
            {
                let l = self.equipped_level(e).unwrap_or(given);
                match levels.iter_mut().find(|(d, _)| *d == e.did) {
                    Some(slot) => slot.1 = l,
                    None => levels.push((e.did, l)),
                }
            }
            if let Some(d) = f.ex.did.filter(|d| members.contains(d))
                && f.ex.instance.equipped
                && !levels.iter().any(|(x, _)| *x == d)
            {
                levels.push((d, given));
            }
            let mut levels: Vec<i32> = levels.into_iter().map(|(_, l)| l).collect();
            levels.sort_unstable_by(|a, b| b.cmp(a));
            levels.resize(c, given);
            #[allow(clippy::cast_precision_loss)]
            let sum: f32 = levels.iter().take(c).map(|&l| l as f32).sum();
            #[allow(clippy::cast_precision_loss)]
            let avg = sum / c as f32;
            #[allow(clippy::cast_possible_truncation)]
            return if avg.is_finite() {
                avg.clamp(i32::MIN as f32, i32::MAX as f32) as i32
            } else {
                given
            };
        }
        match ItemSetView::new(set, &self.schema).level() {
            Some(l) if l != -1 => l,
            _ => f.viewer.level.unwrap_or(given),
        }
    }

    // -----------------------------------------------------------------------------------------
    // §6.18 Cooldown, decay, quest
    // -----------------------------------------------------------------------------------------

    fn duration_map(&self, cooldown: bool) -> Option<&DurationMap> {
        let cell = if cooldown {
            &self.cooldowns
        } else {
            &self.decays
        };
        cell.get_or_init(|| {
            let label = if cooldown {
                control_labels::COOLDOWN_CONTROL
            } else {
                control_labels::ITEM_DECAY_CONTROL
            };
            let set = self.data.record(self.data.weenie(label)?).ok()??;
            Some(if cooldown {
                DurationMap::cooldown(&*set, &self.schema.tables)
            } else {
                DurationMap::decay(&*set, &self.schema.tables)
            })
        })
        .as_ref()
    }

    fn duration_row(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let cooldown = t.widget == Widget::Cooldown;
        let view = ItemView::new(&f.ex.set, &self.schema);
        let key = if cooldown {
            view.cooldown()
        } else {
            view.decay()
        };
        let seconds = key
            .filter(|&k| k != 0)
            .and_then(|k| self.duration_map(cooldown)?.seconds(k))
            .filter(|s| s.is_finite() && *s > 0.0);
        let Some(seconds) = seconds else {
            return self.row(t, RowContent::Empty, None);
        };
        let d = rules::duration_text(f64::from(seconds));
        let v = values([
            (tokens::VAR_DECAY, Value::Text(d.clone())),
            (tokens::VAR_VALUE, Value::Text(d.clone())),
        ]);
        // Tessera rule: the row's format with the duration, or the duration alone when
        // the format has no slot for it.
        let text = match &t.format {
            Some(info) => match self.data.render(TextRef::Info(info), &v) {
                Ok(Some(r)) if text_of(&r).contains(&d) => Some(r),
                Ok(Some(mut r)) => {
                    r.push_str(&format!(" {d}"), &Style::default());
                    Some(r)
                }
                Ok(None) => Some(RichText::plain(&d)),
                Err(e) => {
                    f.fail(e);
                    Some(RichText::plain(&d))
                }
            },
            None => Some(RichText::plain(&d)),
        };
        self.text_row(t, text, None)
    }

    /// The recipe rows of a recipe scroll (`crafting-panels.md` §7 Tessera rule, replacing
    /// the empty rows of §6.20 for these two): for an item with `RecipeItem_Recipe`, the
    /// one-use row shows its text when the recipe is one-use, the cooldown row its text with
    /// the recipe's cooldown duration (the duration text of §6.18) when it has one; any
    /// other case gives an empty row (removed by §5).
    fn recipe_row(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let empty = |s: &Self| s.row(t, RowContent::Empty, None);
        let Some(recipe) = f.did(self.p.recipe) else {
            return empty(self);
        };
        let set = match self.data.record(recipe) {
            Ok(Some(set)) => set,
            Ok(None) => return empty(self),
            Err(e) => {
                f.fail(e);
                return empty(self);
            }
        };
        let (token, values) = if t.widget == Widget::RecipeOneShot {
            let one_use = matches!(
                self.p.recipe_one_time.and_then(|p| set.get(p)),
                Some(PropertyValue::Bool(true))
            );
            if !one_use {
                return empty(self);
            }
            (tokens::RECIPE_ONE_SHOT, Values::new())
        } else {
            let key = match self.p.recipe_cooldown.and_then(|p| set.get(p)) {
                Some(PropertyValue::Enum(k)) => *k,
                _ => return empty(self),
            };
            let seconds = (key != 0)
                .then(|| self.duration_map(true)?.seconds(key))
                .flatten()
                .filter(|s| s.is_finite() && *s > 0.0);
            let Some(seconds) = seconds else {
                return empty(self);
            };
            let d = rules::duration_text(f64::from(seconds));
            (
                tokens::RECIPE_COOLDOWN,
                values([(tokens::VAR_DECAY, Value::Text(d))]),
            )
        };
        let text = match &t.fixed_text {
            Some(info) => self.render(f, TextRef::Info(info), &values),
            None => self.render(
                f,
                TextRef::Token {
                    table_key: tokens::EXAMINATION_TABLE_KEY,
                    token,
                },
                &values,
            ),
        };
        self.text_row(t, text, None)
    }

    fn quest(&self, f: &mut Fill<'_, '_>) -> Row {
        let t = f.t;
        let name = f.did(self.p.quest).and_then(|q| {
            let set = self.data.record(q).ok()??;
            match set.get(self.p.quest_name?)? {
                PropertyValue::StringInfo(i) => self.data.text_value(i).ok(),
                _ => None,
            }
        });
        let text = name.and_then(|n| {
            let v = values([(tokens::VAR_QUEST, n)]);
            match &t.format {
                Some(info) => self.render(f, TextRef::Info(info), &v),
                None => self.render(
                    f,
                    TextRef::Token {
                        table_key: tokens::EXAMINATION_TABLE_KEY,
                        token: tokens::QUEST_ID,
                    },
                    &v,
                ),
            }
        });
        self.text_row(t, text, None)
    }
}

/// `Set_MaxActiveMembers` of a member-less set.
fn max_active(set: &PropertySet, schema: &ItemSchema) -> Option<i32> {
    match set.get(schema.sets.max_active_members.id()?)? {
        PropertyValue::Int(i) => Some(*i),
        _ => None,
    }
}

/// A plain value as text (stat lines).
fn value_text(v: &PropertyValue) -> Option<String> {
    Some(match v {
        PropertyValue::Int(i) => i.to_string(),
        PropertyValue::Int64(i) => i.to_string(),
        PropertyValue::Float(x) => stat_number(*x),
        PropertyValue::Enum(k) => k.to_string(),
        PropertyValue::Bool(b) => b.to_string(),
        _ => return None,
    })
}

/// A stat value as text (Tessera rule of §6.16): at most two decimals, trailing zeros
/// dropped.
fn stat_number(x: f32) -> String {
    if !x.is_finite() {
        return String::new();
    }
    let s = format!("{:.2}", f64::from(x));
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Rounds to the nearest integer, halves away from zero; non-finite or out-of-range values
/// clamp (never in the data).
#[allow(clippy::cast_possible_truncation)]
fn round(x: f32) -> i64 {
    if !x.is_finite() {
        return 0;
    }
    f64::from(x).round().clamp(-9.0e15, 9.0e15) as i64
}
