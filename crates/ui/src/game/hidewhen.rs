// SPDX-License-Identifier: GPL-3.0-or-later
//! Conditional fields, the game type `HideWhenField` (`game-panels.md` §6).

use tessera_formats::property::PropertyValue;

use crate::props::{array_elements, as_bool, as_key};
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::data::{GameData, HideCondition};
use super::vocab::GameVocab;

/// PS §3 type codes the comparison distinguishes.
mod code {
    pub const TRI_STATE: u32 = 5;
    pub const ENUM: u32 = 8;
    pub const FLOAT: u32 = 9;
    pub const PROPERTY_ID: u32 = 10;
    pub const INT: u32 = 15;
    pub const BITFIELD32: u32 = 18;
    pub const DID_REF: u32 = 20;
    pub const BOOL: u32 = 21;
}

/// The comparison of row 13 (§6.2): whether operator `op` holds for *A* (the data value)
/// against *B* (the player's value), compared by the property's type code. Operators 0 and
/// above 6 never hide. A type the table's last row names (its own comparison routine, not
/// traced) never hides (Tessera rule of the §6.2 OPEN), and so does a value that does not
/// fit its type.
#[must_use]
pub fn compare(op: u32, type_code: u32, a: &PropertyValue, b: &PropertyValue) -> bool {
    if op == 0 || op > 6 {
        return false;
    }
    match type_code {
        code::INT | code::ENUM => {
            let (Some(a), Some(b)) = (word(a), word(b)) else {
                return false;
            };
            // Equality on the stored 32 bits; ordering signed 32-bit (keys of an enum too).
            let (a, b) = (a.cast_signed(), b.cast_signed());
            ordered(op, a.cmp(&b))
        }
        code::FLOAT => {
            let (Some(a), Some(b)) = (float(a), float(b)) else {
                return false;
            };
            match op {
                1 => a >= b,
                2 => a != b,
                3 => a == b,
                4 => a < b,
                5 => a <= b,
                6 => a > b,
                _ => false,
            }
        }
        code::BOOL | code::TRI_STATE | code::PROPERTY_ID | code::BITFIELD32 | code::DID_REF => {
            let (Some(a), Some(b)) = (word(a), word(b)) else {
                return false;
            };
            match op {
                2 => a != b,
                3 => a == b,
                _ => false,
            }
        }
        _ => false,
    }
}

fn ordered(op: u32, o: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match op {
        1 => o != Less,
        2 => o != Equal,
        3 => o == Equal,
        4 => o == Less,
        5 => o != Greater,
        6 => o == Greater,
        _ => false,
    }
}

/// The stored 32-bit value of a scalar (a boolean is 0 or 1, `property-system.md` §4.5).
fn word(v: &PropertyValue) -> Option<u32> {
    Some(match v {
        PropertyValue::Int(i) => i.cast_unsigned(),
        PropertyValue::Enum(k)
        | PropertyValue::PropertyId(k)
        | PropertyValue::Bitfield32(k)
        | PropertyValue::StringToken(k) => *k,
        PropertyValue::DidRef(d) => d.0,
        PropertyValue::Bool(b) => u32::from(*b),
        PropertyValue::TriState(t) => u32::from(*t),
        _ => return None,
    })
}

fn float(v: &PropertyValue) -> Option<f32> {
    match v {
        PropertyValue::Float(f) => Some(*f),
        _ => None,
    }
}

/// The zero value of a type (§6.2: *B* when the player lacks *P* and the dictionary has no
/// default): 0 for int, enum, float, property ID, 32-bit bitfield and DID; false for a
/// boolean; 255 ("undefined") for a tri-state; `None` for the other codes, which never
/// hide anyway.
#[must_use]
pub fn zero_value(type_code: u32) -> Option<PropertyValue> {
    Some(match type_code {
        code::INT => PropertyValue::Int(0),
        code::ENUM => PropertyValue::Enum(0),
        code::FLOAT => PropertyValue::Float(0.0),
        code::PROPERTY_ID => PropertyValue::PropertyId(0),
        code::BITFIELD32 => PropertyValue::Bitfield32(0),
        code::DID_REF => PropertyValue::DidRef(tessera_formats::did::Did(0)),
        code::BOOL => PropertyValue::Bool(false),
        code::TRI_STATE => PropertyValue::TriState(255),
        _ => return None,
    })
}

/// The hide test of §6.2 for the element `k`. The conditions are tested in order and the
/// first that applies hides; a boolean condition applies only when present and true.
#[must_use]
pub fn hide_test(ui: &Ui, gv: &GameVocab, k: ElementKey, data: &dyn GameData) -> bool {
    let p = gv.pids;
    let flag = |pid: u32| ui.prop(k, pid).bool();
    let value = |pid: u32| ui.prop(k, pid).value().map(as_key);
    // Rows 1–6: account tests.
    if flag(p.hw_webstore_disabled) && data.hide_condition(HideCondition::StoreDisabled) {
        return true;
    }
    if let Some(v) = value(p.hw_rip_cord)
        && data.hide_condition(HideCondition::RipCordFails(v))
    {
        return true;
    }
    if let Some(v) = value(p.hw_rip_cord_enabled)
        && data.hide_condition(HideCondition::RipCordPasses(v))
    {
        return true;
    }
    for (pid, c) in [
        (p.hw_non_admin, HideCondition::NotAdmin),
        (p.hw_non_vip, HideCondition::NotSubscriber),
        (p.hw_vip, HideCondition::Subscriber),
    ] {
        if flag(pid) && data.hide_condition(c) {
            return true;
        }
    }
    if !data.has_local_player() {
        return false;
    }
    // Rows 7–10: player tests.
    for (pid, c) in [
        (p.hw_monster_player, HideCondition::MonsterPlayer),
        (p.hw_session_player, HideCondition::SessionPlayer),
        (p.hw_pvp_player, HideCondition::PvP),
        (p.hw_player, HideCondition::PlayerKindFails),
    ] {
        if flag(pid) && data.hide_condition(c) {
            return true;
        }
    }
    // Row 11.
    if let Some(pid) = value(p.hw_property_is_true)
        && data.player_property(pid).as_ref().is_some_and(as_bool)
    {
        return true;
    }
    // Row 12: absent or the invalid DID (0).
    if let Some(pid) = value(p.hw_invalid_data_id) {
        let did = data.player_property(pid).as_ref().map_or(0, as_key);
        if did == 0 {
            return true;
        }
    }
    // Row 13, else row 14.
    if let Some(v) = ui.prop(k, p.hw_compare_value).value() {
        let op = ui.prop(k, p.hw_compare_operator).key();
        return compare_with_player(v, op, data);
    }
    flag(p.hw_not_on_warsteed) && data.hide_condition(HideCondition::NotOnWarsteed)
}

/// Row 13: the first element of the value array is (*P*, *A*); *B* is the player's *P*,
/// else its dictionary default, else the zero value of its type. The type is the
/// dictionary's, or — without a dictionary entry — the type of *A* as stored.
#[must_use]
pub fn compare_with_player(value: &PropertyValue, op: u32, data: &dyn GameData) -> bool {
    let Some(first) = array_elements(value).first() else {
        return false;
    };
    let pid = first.pid;
    let a = &first.value;
    let type_code = data
        .property_type(pid)
        .unwrap_or_else(|| a.property_type().code());
    let b = data
        .player_property(pid)
        .or_else(|| data.property_default(pid))
        .or_else(|| zero_value(type_code));
    let Some(b) = b else { return false };
    compare(op, type_code, a, &b)
}

/// Whether `pid` is one of the condition properties the hide test reads on the element
/// (§6.2), whose change re-evaluates the field (§6.1).
#[must_use]
pub fn is_condition_property(gv: &GameVocab, pid: u32) -> bool {
    let p = gv.pids;
    [
        p.hw_webstore_disabled,
        p.hw_rip_cord,
        p.hw_rip_cord_enabled,
        p.hw_non_admin,
        p.hw_non_vip,
        p.hw_vip,
        p.hw_monster_player,
        p.hw_session_player,
        p.hw_pvp_player,
        p.hw_player,
        p.hw_property_is_true,
        p.hw_invalid_data_id,
        p.hw_compare_value,
        p.hw_compare_operator,
        p.hw_not_on_warsteed,
    ]
    .contains(&pid)
}

/// One `HideWhenField` (§6.1): visible exactly when the hide test is false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HideWhenField {
    /// The element.
    pub key: ElementKey,
}

impl HideWhenField {
    /// After creation and whenever a condition property or the relevant player property
    /// changes: sets the visible flag to "not hidden".
    pub fn evaluate(&self, ui: &mut Ui, gv: &GameVocab, data: &dyn GameData) {
        let hide = hide_test(ui, gv, self.key, data);
        ui.set_shown(self.key, !hide);
    }

    /// A request to show or hide the field (§6.1): showing happens only when the hide test
    /// is false; hiding always hides.
    pub fn request_shown(&self, ui: &mut Ui, gv: &GameVocab, data: &dyn GameData, show: bool) {
        if show && hide_test(ui, gv, self.key, data) {
            return;
        }
        ui.set_shown(self.key, show);
    }
}
