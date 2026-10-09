// SPDX-License-Identifier: GPL-3.0-or-later
//! `Turbine.UI.Color` (`lua-plugin-api.md` §13.9): the constructor's argument rules, the
//! single-precision components and the 141 named colours.

use mlua::{MultiValue, Table, Value};

use super::enums::NAMED_COLOURS;
use super::model::num;

/// Store (A, R, G, B) as the fields `A`, `R`, `G`, `B`, each rounded to single precision.
pub(crate) fn store(obj: &Table, c: [f32; 4]) -> mlua::Result<()> {
    obj.raw_set("A", f64::from(c[0]))?;
    obj.raw_set("R", f64::from(c[1]))?;
    obj.raw_set("G", f64::from(c[2]))?;
    obj.raw_set("B", f64::from(c[3]))?;
    Ok(())
}

/// The components a `Color(...)` call gives (§13.9): three numbers are (1, r, g, b), four
/// are (a, r, g, b), any other count is transparent black; non-numbers count as 0.
#[must_use]
pub(crate) fn from_args(args: &MultiValue) -> [f32; 4] {
    let n = |i: usize| num(args.get(i)).unwrap_or(0.0) as f32;
    match args.len() {
        3 => [1.0, n(0), n(1), n(2)],
        4 => [n(0), n(1), n(2), n(3)],
        _ => [0.0; 4],
    }
}

/// A named colour's components (§13.9): each byte / 255 rounded to 3 decimals, then to
/// single precision; alpha 1, except `Transparent` (alpha 0).
#[must_use]
pub(crate) fn named(name: &str, rgb: u32) -> [f32; 4] {
    let ch = |shift: u32| {
        let b = f64::from((rgb >> shift) & 0xFF);
        ((b / 255.0 * 1000.0).round() / 1000.0) as f32
    };
    let a = if name == "Transparent" { 0.0 } else { 1.0 };
    [a, ch(16), ch(8), ch(0)]
}

/// Read a colour value's (A, R, G, B): a table's `A`, `R`, `G`, `B` fields (normal
/// indexing; non-numbers count as 0). `None` for anything that is not a table.
pub(crate) fn read(v: Option<&Value>) -> Option<[f32; 4]> {
    let Some(Value::Table(t)) = v else {
        return None;
    };
    let f = |k: &str| num(t.get::<Value>(k).ok().as_ref()).unwrap_or(0.0) as f32;
    Some([f("A"), f("R"), f("G"), f("B")])
}

/// Is `v` an instance of the `Color` class (its `__index` chain reaches `class`)?
pub(crate) fn is_colour(v: Option<&Value>, class: &Table) -> bool {
    let Some(Value::Table(t)) = v else {
        return false;
    };
    let mut cur = t.clone();
    for _ in 0..64 {
        if cur.to_pointer() == class.to_pointer() {
            return true;
        }
        match cur.metatable().map(|mt| mt.raw_get::<Value>("__index")) {
            Some(Ok(Value::Table(next))) => cur = next,
            _ => return false,
        }
    }
    false
}

/// The named colours as (name, components).
pub(crate) fn all_named() -> impl Iterator<Item = (&'static str, [f32; 4])> {
    NAMED_COLOURS.iter().map(|(n, rgb)| (*n, named(n, *rgb)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_components_follow_the_rounding_rule() {
        let alice = named("AliceBlue", 0xF0F8FF);
        assert_eq!(alice, [1.0, 0.941, 0.973, 1.0]);
        assert_eq!(f64::from(alice[1]), f64::from(0.941_f32));
        let gray = named("Gray", 0x808080);
        assert_eq!(gray[1], 0.502);
        assert_eq!(named("Transparent", 0xFFFFFF), [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(all_named().count(), 141);
    }
}
