// SPDX-License-Identifier: GPL-3.0-or-later
//! Item tooltips in the client (`item-tooltips.md`): the request for an item of the
//! player's inventory (its instance state and the viewer, §11), the model built by
//! `tessera-gamedata`'s engine (§3–§8), and its conversion into the data-neutral input of
//! `tessera-ui`'s examination tooltip panel ([`ExamTooltip`]), which builds, measures,
//! places and pins it (§4, §5, §7, §9, §10).
//!
//! Conversion rules (the model's content as the panel takes it):
//!
//! - one [`ExamRow`] per row of the model, in template order; when the engine left out an
//!   entry template it could not read, the tooltip names its templates explicitly so rows
//!   and templates stay paired;
//! - texts as plain text (the model has already interpreted markup into styled spans; the
//!   spans' styles are not carried over);
//! - state labels as `UIStateID` keys of the install; colours as given;
//! - an icon row shows the item's composed icon layers ([`item_icon`]: the instance's
//!   checkpointed image override when it carries one, `icon-layers.md` §3.1 step 1; else
//!   the record's, [`super::ItemFacts::icon`]);
//! - the worth row's coins carry their count texts; coin images are not given (the panel
//!   then shows the counts only);
//! - a literal item (`scenario-feed.md` §5.2) gets the minimal tooltip of §11 in the frame
//!   of [`LITERAL_FRAME_LABEL`] with its first name row (Tessera rule, recorded as an OPEN
//!   in `item-tooltips.md` §11).

use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{Did, PropertyEntry};
use tessera_gamedata::inventory::{Container, ItemId, ItemRef};
use tessera_gamedata::tooltip::{
    Condition, Equipped, Instance, Purpose, Row, RowContent as MRow, SkillViewer, Subject,
    TemplateRef, TooltipData, TooltipError, TooltipModel, TooltipRequest, VITAL_TYPE_HEALTH,
    VITAL_TYPE_POWER, Viewer, Widget,
};
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;
use tessera_ui::game::examine::{
    ExamCoin, ExamKey, ExamLine, ExamRow, ExamTemplate, ExamTooltip, RowContent,
};

use super::data::CharacterData;
use super::state::{PlayerState, VitalKind};

/// `UILAYOUT` label of the examination layout (`item-tooltips.md` §2, §10).
pub const EXAMINATION_LAYOUT_LABEL: &str = "examination";
/// The tooltip element a literal item's minimal tooltip uses (Tessera rule, §11 OPEN).
pub const LITERAL_FRAME_LABEL: &str = "Item_ExamineLayout";

/// The slot bit of an equipment key (key *k* is bit *k* − 1; 0 outside 1–32).
#[must_use]
pub fn key_bit(key: u16) -> u32 {
    if (1..=32).contains(&key) {
        1u32 << (key - 1)
    } else {
        0
    }
}

/// A session property value as a record property value (for instance properties, §11).
#[must_use]
pub fn record_value(v: &tessera_net::PropertyValue) -> Option<tessera_formats::PropertyValue> {
    use tessera_formats::PropertyValue as F;
    use tessera_net::PropertyValue as N;
    Some(match v {
        N::Bool(b) => F::Bool(*b),
        N::Int(i) => F::Int(*i),
        N::Int64(i) => F::Int64(*i),
        N::Float(f) => F::Float(*f),
        N::Enum(k) => F::Enum(*k),
        N::Bitfield32(b) => F::Bitfield32(*b),
        N::Bitfield64(b) => F::Bitfield64(*b),
        N::Did(d) => F::DidRef(Did(d.0)),
        N::String(s) => F::String(s.clone()),
        N::Instance(i) => F::InstanceId(*i),
        N::Colour(c) => F::Colour(*c),
        _ => return None,
    })
}

/// The instance state of item `id` (§11): quantity, condition, whether it is equipped,
/// and its reported instance properties.
#[must_use]
pub fn instance(state: &PlayerState, id: ItemId) -> Instance {
    let Some(it) = state.inventory().item(id) else {
        return Instance::default();
    };
    Instance {
        quantity: Some(i64::from(it.quantity)),
        condition: it
            .condition
            .map(|(current, max)| Condition { current, max }),
        equipped: it.place.container == Container::Equipment,
        properties: state
            .item_properties(id)
            .filter_map(|(pid, v)| {
                Some(PropertyEntry {
                    pid,
                    value: record_value(v)?,
                })
            })
            .collect(),
    }
}

/// The instance's `Icon_Layer_ImageOverrideDID_Checkpointed` of item `id`, as "Read *P*"
/// of `icon-layers.md` §3.1 gives it (a DID, or 0 for another value type); `None` when the
/// item carries none.
#[must_use]
pub fn image_override(data: &CharacterData, state: &PlayerState, id: ItemId) -> Option<Did> {
    let pid = data.icon_pid(tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED)?;
    state
        .item_properties(id)
        .find(|(p, _)| *p == pid)
        .map(|(_, v)| match v {
            tessera_net::PropertyValue::Did(d) => Did(d.0),
            _ => Did(0),
        })
}

/// The composed icon of item `id` of the inventory (`icon-layers.md` §3.1): the record's
/// layers with the instance's image override when it carries one; `None` for a literal
/// item, an unknown record or an item without an icon.
#[must_use]
pub fn item_icon(data: &CharacterData, state: &PlayerState, id: ItemId) -> Option<IconImage> {
    let it = state.inventory().item(id)?;
    let ItemRef::Record(did) = &it.what else {
        return None;
    };
    let over = image_override(data, state, id);
    data.instance_icon(*did, &|_| over)
}

/// The viewer of §2 and §11: the player's class, species and level, the comparison
/// option, and the equipped item records with their slot bits.
#[must_use]
pub fn viewer(state: &PlayerState, compare_equipped: bool) -> Viewer {
    let race = match state.body() {
        Some(tessera_net::Body::Character { species, .. }) => Some(species),
        _ => None,
    };
    Viewer {
        class: state.class(),
        race,
        level: state.level().and_then(|l| i32::try_from(l).ok()),
        compare_equipped,
        equipped: state
            .inventory()
            .equipped()
            .filter_map(|it| match &it.what {
                ItemRef::Record(did) => Some(Equipped {
                    slot: key_bit(it.place.slot),
                    did: *did,
                    instance: instance(state, it.id),
                }),
                ItemRef::Literal(_) => None,
            })
            .collect(),
    }
}

/// The viewer's combat state for the skill rows of item tooltips (`skill-tooltips.md` §13):
/// the morale and power maxima as `VitalType` keys 1 and 2 (ST25), so the skill-item vital
/// cost rows (§7.10) and an attached skill tooltip show percentage costs. The level comes from
/// [`viewer`].
#[must_use]
pub fn combat_viewer(state: &PlayerState) -> SkillViewer {
    SkillViewer {
        max_vitals: [
            (VITAL_TYPE_HEALTH, VitalKind::Morale),
            (VITAL_TYPE_POWER, VitalKind::Power),
        ]
        .into_iter()
        .filter_map(|(key, k)| Some((key, state.vital(k)?.max)))
        .filter(|(_, m)| m.is_finite() && *m > 0.0)
        .collect(),
        ..SkillViewer::default()
    }
}

/// The hover (or pinned) tooltip request of item `id`; `None` for an unknown item.
#[must_use]
pub fn request(state: &PlayerState, id: ItemId, purpose: Purpose) -> Option<TooltipRequest> {
    let it = state.inventory().item(id)?;
    Some(TooltipRequest {
        subject: match &it.what {
            ItemRef::Record(d) => Subject::Record(*d),
            ItemRef::Literal(t) => Subject::Literal(t.clone()),
        },
        instance: instance(state, id),
        purpose,
        override_pair: None,
        override_compare_kind: None,
    })
}

/// The pin key of item `id` (`item-tooltips.md` §10: instance and record).
#[must_use]
pub fn exam_key(state: &PlayerState, id: ItemId) -> ExamKey {
    let record = match state.inventory().item(id).map(|i| &i.what) {
        Some(ItemRef::Record(d)) => d.0,
        _ => 0,
    };
    ExamKey {
        instance: id,
        record,
    }
}

/// Builds the tooltip of item `id` for `purpose` and converts it for the panel.
///
/// # Errors
///
/// The engine's errors ([`TooltipError::NoTooltip`] when the item names none), and
/// [`TooltipError::RecordMissing`] for an unknown item.
pub fn exam_tooltip_of(
    data: &CharacterData,
    state: &PlayerState,
    id: ItemId,
    purpose: Purpose,
    compare_equipped: bool,
) -> Result<ExamTooltip, TooltipError> {
    let req = request(state, id, purpose).ok_or(TooltipError::RecordMissing(Did(0)))?;
    let model = data.engine().build_with_combat(
        &req,
        &viewer(state, compare_equipped),
        &combat_viewer(state),
    )?;
    // Icon rows: the hovered item's own icon (with its instance override) for its record;
    // a comparison's for the equipped item in its slot.
    let own = state.inventory().item(id).and_then(|it| match &it.what {
        ItemRef::Record(d) => Some(*d),
        ItemRef::Literal(_) => None,
    });
    let icons = |slot: Option<u32>, did: Did| -> Option<IconImage> {
        let holder = match slot {
            None => (own == Some(did)).then_some(id),
            Some(bits) => state
                .inventory()
                .equipped()
                .find(|it| key_bit(it.place.slot) == bits)
                .filter(|it| it.what == ItemRef::Record(did))
                .map(|it| it.id),
        };
        match holder {
            Some(h) => item_icon(data, state, h),
            None => data.item(did).and_then(|f| f.icon.clone()),
        }
    };
    convert_with(data, &model, exam_key(state, id), &icons).ok_or(TooltipError::NoTooltip)
}

/// Icon rows' pictures: (comparison slot bits, or `None` for the main tooltip; the row's
/// item record) → the composed icon.
pub type IconSource<'a> = dyn Fn(Option<u32>, Did) -> Option<IconImage> + 'a;

/// The `UIStateID` key of a state label.
fn state_id(data: &CharacterData, label: Option<&str>) -> Option<u32> {
    data.engine().data().ui_key(UiEnum::StateId, label?)
}

fn template(t: TemplateRef) -> ExamTemplate {
    ExamTemplate {
        layout: t.layout.0,
        element: t.element,
    }
}

/// The minimal tooltip frame and its name row for literal items (§11 Tessera rule): the
/// [`LITERAL_FRAME_LABEL`] element of the examination layout and the first entry template
/// whose row is a name row. `None` when the install lacks them.
#[must_use]
pub fn literal_frame(data: &CharacterData) -> Option<(ExamTemplate, ExamTemplate)> {
    let d = data.engine().data();
    let layout = d.layout_by_label(EXAMINATION_LAYOUT_LABEL)?;
    let element = d.ui_key(UiEnum::ElementId, LITERAL_FRAME_LABEL)?;
    let at = TemplateRef { layout, element };
    let frame = data.engine().frame(at).ok()?;
    let name = frame.list.templates.iter().flatten().find(|t| {
        data.engine()
            .template(**t)
            .is_ok_and(|r| r.widget == Widget::Name)
    })?;
    Some((template(at), template(*name)))
}

/// Converts a tooltip model for the panel (module docs); `None` for a literal item when
/// the install has no frame for it.
#[must_use]
pub fn convert(data: &CharacterData, model: &TooltipModel, key: ExamKey) -> Option<ExamTooltip> {
    convert_with(data, model, key, &|_, did| {
        data.item(did).and_then(|f| f.icon.clone())
    })
}

/// [`convert`] with the icon rows' pictures from `icons`.
#[must_use]
pub fn convert_with(
    data: &CharacterData,
    model: &TooltipModel,
    key: ExamKey,
    icons: &IconSource<'_>,
) -> Option<ExamTooltip> {
    let Some(frame) = &model.frame else {
        let (frame, name_row) = literal_frame(data)?;
        let text = match &model.subject {
            Subject::Literal(t) => t.clone(),
            Subject::Record(_) => String::new(),
        };
        let colour = model.rows.first().and_then(|r| r.colour);
        let mut t = ExamTooltip::literal(frame, name_row, text, colour);
        t.key = key;
        return Some(t);
    };
    // The templates that gave rows (the engine skips the ones it cannot read).
    let readable: Vec<TemplateRef> = frame
        .list
        .templates
        .iter()
        .flatten()
        .copied()
        .filter(|t| data.engine().template(*t).is_ok())
        .collect();
    let all_readable = readable.len() == frame.list.templates.len();
    Some(ExamTooltip {
        frame: template(frame.at),
        rows: model
            .rows
            .iter()
            .map(|r| row(data, r, &|d| icons(None, d)))
            .collect(),
        comparisons: model
            .comparison
            .tooltips
            .iter()
            .filter_map(|(bits, m)| {
                let record = match m.subject {
                    Subject::Record(d) => d.0,
                    Subject::Literal(_) => 0,
                };
                let bits = *bits;
                convert_with(
                    data,
                    m,
                    ExamKey {
                        instance: 0,
                        record,
                    },
                    &|_, d| icons(Some(bits), d),
                )
            })
            .collect(),
        key,
        templates: (!all_readable).then(|| readable.into_iter().map(template).collect()),
    })
}

fn line(data: &CharacterData, l: &tessera_gamedata::tooltip::Line) -> ExamLine {
    ExamLine {
        text: l.text.plain_text(),
        state: state_id(data, l.state.as_deref()),
        colour: l.colour,
    }
}

/// A worth-row coin's image (`item-tooltips.md` §6.11 answer): the coin icon state of its
/// denomination. The model's currencies are sorted largest first, so with the install's
/// three coins (gold, silver, copper) the currency index is the coin's state; with any
/// other currency list no image is given (Tessera rule, as `npc-windows.md` §9.2 shows only
/// three coins).
fn coin_icon(data: &CharacterData, index: usize) -> Option<ImageRef> {
    data.coin_values()?;
    data.coin_icons()
        .get(index)
        .copied()
        .flatten()
        .map(|d| ImageRef::Did(d.0))
}

fn row(data: &CharacterData, r: &Row, icon: &dyn Fn(Did) -> Option<IconImage>) -> ExamRow {
    let content = match &r.content {
        MRow::Empty => RowContent::Empty,
        MRow::Separator => RowContent::Separator,
        MRow::Text(t) => RowContent::Text(t.plain_text()),
        MRow::Image(d) => RowContent::Image(ImageRef::Did(d.0)),
        MRow::Icon { item, ghosted } => RowContent::Icon {
            image: icon(*item),
            ghosted: *ghosted,
        },
        MRow::Money { label, coins } => RowContent::Money {
            label: label.as_ref().map(tessera_text::RichText::plain_text),
            coins: coins
                .iter()
                .map(|c| ExamCoin {
                    text: c.text.clone(),
                    icon: coin_icon(data, c.index),
                })
                .collect(),
        },
        MRow::Lines { header, lines } => RowContent::Lines {
            header: header.as_ref().map(|h| line(data, h)),
            lines: lines.iter().map(|l| line(data, l)).collect(),
        },
        MRow::Pair { left, right } => {
            let side = |s: &Option<Box<Row>>| {
                Box::new(
                    s.as_ref()
                        .map_or_else(ExamRow::empty, |r| row(data, r, icon)),
                )
            };
            RowContent::Pair {
                left: side(left),
                right: side(right),
            }
        }
        MRow::List(rows) => RowContent::List(rows.iter().map(|r| row(data, r, icon)).collect()),
        _ => RowContent::Empty,
    };
    ExamRow {
        content,
        state: state_id(data, r.state.as_deref()),
        colour: r.colour,
        hidden: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bits() {
        assert_eq!(key_bit(1), 1);
        assert_eq!(key_bit(3), 4);
        assert_eq!(key_bit(32), 1 << 31);
        assert_eq!(key_bit(0), 0);
        assert_eq!(key_bit(33), 0);
    }

    #[test]
    fn session_values_become_record_values() {
        use tessera_formats::PropertyValue as F;
        use tessera_net::PropertyValue as N;
        assert_eq!(record_value(&N::Int(3)), Some(F::Int(3)));
        assert_eq!(
            record_value(&N::Did(tessera_net::DataId(7))),
            Some(F::DidRef(Did(7)))
        );
        assert_eq!(
            record_value(&N::String("x".into())),
            Some(F::String("x".into()))
        );
    }
}
