// SPDX-License-Identifier: GPL-3.0-or-later
//! Appearances in server files (`test-server.md` §3.5 `appearance`, §5.4 `appearance`).
//!
//! Appearances hold only opaque references chosen by players' clients (or, in content, at
//! most a handful written by a contributor). The server checks the protocol's limits
//! (`tessera-protocol.md` §5.3) and never interprets a reference (`server-content.md` §4.1).

use tessera_protocol::limits::{
    MAX_APPEARANCE_SCALE, MAX_EQUIP_SLOT, MAX_EQUIPMENT, MAX_LOOK_ENTRIES, MIN_EQUIP_SLOT,
};
use tessera_protocol::types::{Appearance, Body, Equipped, Look, LookEntry, LookValue};
use toml::de::DeValue;

use crate::tomlx::{self, Reader, View, float_text, quote};

/// Whether an appearance meets the protocol's limits (§5.3): scale in (0, 100], at most 32
/// items, item DIDs and object DIDs non-zero, dye codes in [0, 1].
#[must_use]
pub fn is_valid(a: &Appearance) -> bool {
    let body_ok = match a.body {
        Body::Character { .. } => true,
        Body::Object { did } => did != 0,
    };
    let dye_ok = |d: Option<f32>| d.is_none_or(|d| d.is_finite() && (0.0..=1.0).contains(&d));
    let slot_ok = |s: Option<u8>| s.is_none_or(|s| (MIN_EQUIP_SLOT..=MAX_EQUIP_SLOT).contains(&s));
    body_ok
        && a.scale.is_finite()
        && a.scale > 0.0
        && a.scale <= MAX_APPEARANCE_SCALE
        && a.equipment.len() <= MAX_EQUIPMENT
        && a.equipment
            .iter()
            .all(|e| e.item != 0 && dye_ok(e.dye) && dye_ok(e.secondary_dye) && slot_ok(e.slot))
}

/// The number of opaque references in an appearance (`test-server.md` §5.4): one per body,
/// one for `idle`, one per item, one per item `slot`.
#[must_use]
pub fn reference_count(a: &Appearance) -> usize {
    1 + usize::from(a.idle.is_some())
        + a.equipment.len()
        + a.equipment.iter().filter(|e| e.slot.is_some()).count()
}

/// Reads a character file's appearance table (`test-server.md` §3.5), look entries
/// included. `None` when the appearance or its look breaks the protocol's limits.
pub(crate) fn parse_character(
    r: &mut Reader<'_, '_>,
    v: View<'_, '_>,
) -> Option<(Appearance, Look)> {
    let a = parse_keys(r, v, &["body", "scale", "idle", "items", "look"]);
    let look = parse_look(r, v);
    Some((a?, look?))
}

/// The look kinds of §3.5 and their names.
fn look_kind_name(v: &LookValue) -> &'static str {
    match v {
        LookValue::Bool(_) => "bool",
        LookValue::Int32(_) => "int32",
        LookValue::Int64(_) => "int64",
        LookValue::Float32(_) => "float32",
        LookValue::Enum(_) => "enum",
        LookValue::DataId(_) => "data-id",
        LookValue::String(_) => "string",
        LookValue::Bitfield32(_) => "bitfield32",
        LookValue::Bitfield64(_) => "bitfield64",
        LookValue::Colour(_) => "colour",
    }
}

/// Reads `[[…appearance.look]]` (§3.5): ascending property IDs, at most 48 entries and 768
/// bytes, each value of its kind. `None` (with a finding) when any rule is broken.
fn parse_look(r: &mut Reader<'_, '_>, v: View<'_, '_>) -> Option<Look> {
    if !v.table.contains_key("look") {
        return Some(Look::none());
    }
    let tables = r.tables(v, "look");
    let mut ok = true;
    if tables.len() > MAX_LOOK_ENTRIES {
        let line = tables.get(MAX_LOOK_ENTRIES).map_or(v.line, |(_, l)| *l);
        r.push(
            line,
            r.range_rule,
            format!("at most {MAX_LOOK_ENTRIES} look entries per character"),
        );
        return None;
    }
    let mut look = Look::none();
    for (t, line) in tables {
        let lv = View {
            table: t,
            line,
            path: "appearance.look",
        };
        r.check_keys(lv, &["property", "kind", "value"]);
        let property = r.int(lv, "property", true, 1, i64::from(u32::MAX));
        let kind = r.string(lv, "kind", true);
        let value = r.value(lv, "value", true).map(|s| s.get_ref());
        let (Some((property, _)), Some((kind, _)), Some(value)) = (property, kind, value) else {
            ok = false;
            continue;
        };
        let int = |lo: i64, hi: i64| tomlx::integer(value).filter(|n| (lo..=hi).contains(n));
        let value = match kind.as_str() {
            "bool" => match value {
                DeValue::Boolean(b) => Some(LookValue::Bool(*b)),
                _ => None,
            },
            "int32" => int(i64::from(i32::MIN), i64::from(i32::MAX))
                .map(|n| LookValue::Int32(i32::try_from(n).unwrap_or(0))),
            "int64" => tomlx::integer(value).map(LookValue::Int64),
            "enum" => {
                int(0, i64::from(u32::MAX)).map(|n| LookValue::Enum(u32::try_from(n).unwrap_or(0)))
            }
            "data-id" => int(0, i64::from(u32::MAX))
                .map(|n| LookValue::DataId(u32::try_from(n).unwrap_or(0))),
            "bitfield32" => int(0, i64::from(u32::MAX))
                .map(|n| LookValue::Bitfield32(u32::try_from(n).unwrap_or(0))),
            "float32" => match value {
                DeValue::Float(_) | DeValue::Integer(_) => tomlx::float(value)
                    .filter(|f| f.is_finite())
                    .map(|f| {
                        #[allow(clippy::cast_possible_truncation)] // written from an f32
                        let x = f as f32;
                        x
                    })
                    .filter(|f| f.is_finite())
                    .map(LookValue::Float32),
                _ => None,
            },
            "string" => match value {
                DeValue::String(s) => Some(LookValue::String(s.to_string())).filter(|v| match v {
                    LookValue::String(s) => tessera_protocol::types::is_look_string(s),
                    _ => false,
                }),
                _ => None,
            },
            "bitfield64" => match value {
                DeValue::String(s) => s
                    .strip_prefix("0x")
                    .filter(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                    .and_then(|h| u64::from_str_radix(h, 16).ok())
                    .map(LookValue::Bitfield64),
                _ => None,
            },
            "colour" => match value {
                DeValue::Array(a) if a.len() == 4 => {
                    let c: Vec<u8> = a
                        .iter()
                        .filter_map(|i| tomlx::integer(i.get_ref()))
                        .filter_map(|n| u8::try_from(n).ok())
                        .collect();
                    (c.len() == 4).then(|| LookValue::Colour([c[0], c[1], c[2], c[3]]))
                }
                _ => None,
            },
            _ => None,
        };
        match value {
            Some(value) => look.entries.push(LookEntry {
                property: u32::try_from(property).unwrap_or(1),
                value,
            }),
            None => {
                r.push(
                    line,
                    r.range_rule,
                    "look entry: unknown kind, or a value that does not fit it",
                );
                ok = false;
            }
        }
    }
    if let Err(e) = look.check() {
        r.push(v.line, r.range_rule, format!("look: {e}"));
        ok = false;
    }
    ok.then_some(look)
}

/// A look as an inline TOML array of tables (§3.5); `None` for the empty look.
#[must_use]
pub fn look_to_toml(look: &Look) -> Option<String> {
    if look.is_empty() {
        return None;
    }
    let mut out = String::from("[");
    for (i, e) in look.entries.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let value = match &e.value {
            LookValue::Bool(b) => b.to_string(),
            LookValue::Int32(n) => n.to_string(),
            LookValue::Int64(n) => n.to_string(),
            LookValue::Float32(f) => float_text(f64::from(*f)),
            LookValue::Enum(n) | LookValue::DataId(n) | LookValue::Bitfield32(n) => n.to_string(),
            LookValue::String(s) => quote(s),
            LookValue::Bitfield64(n) => format!("\"0x{n:016X}\""),
            LookValue::Colour([r, g, b, a]) => format!("[{r}, {g}, {b}, {a}]"),
        };
        out.push_str(&format!(
            "{{ property = {}, kind = \"{}\", value = {value} }}",
            e.property,
            look_kind_name(&e.value)
        ));
    }
    out.push(']');
    Some(out)
}

/// Reads an appearance table. Findings are pushed to `r` with its range rule; returns `None`
/// when the table is not a valid appearance.
pub(crate) fn parse(r: &mut Reader<'_, '_>, v: View<'_, '_>) -> Option<Appearance> {
    parse_keys(r, v, &["body", "scale", "idle", "items"])
}

fn parse_keys(r: &mut Reader<'_, '_>, v: View<'_, '_>, keys: &[&str]) -> Option<Appearance> {
    r.check_keys(v, keys);
    let mut ok = true;
    let body = match r.table(v, "body", true) {
        Some((t, sv)) => {
            let line = r.line(&sv.span());
            let bv = View {
                table: t,
                line,
                path: "appearance.body",
            };
            if t.contains_key("object") {
                r.check_keys(bv, &["object"]);
                r.int(bv, "object", true, 1, i64::from(u32::MAX))
                    .map(|(did, _)| Body::Object {
                        did: u32::try_from(did).unwrap_or(1),
                    })
            } else {
                r.check_keys(bv, &["species", "sex"]);
                let species = r.int(bv, "species", true, 0, i64::from(u32::MAX));
                let sex = r.int(bv, "sex", true, 0, i64::from(u32::MAX));
                match (species, sex) {
                    (Some((s, _)), Some((x, _))) => Some(Body::Character {
                        species: u32::try_from(s).unwrap_or(0),
                        sex: u32::try_from(x).unwrap_or(0),
                    }),
                    _ => None,
                }
            }
        }
        None => None,
    };
    let scale = r
        .float_in(
            v,
            "scale",
            true,
            |f| f > 0.0 && f <= f64::from(MAX_APPEARANCE_SCALE),
            "(0, 100]",
        )
        .map(|(f, _)| f as f32);
    let idle = match r.int(v, "idle", false, 0, i64::from(u32::MAX)) {
        Some((i, _)) => Some(u32::try_from(i).unwrap_or(0)),
        None => {
            if v.table.contains_key("idle") {
                ok = false;
            }
            None
        }
    };
    let mut equipment = Vec::new();
    let items = r.tables(v, "items");
    if items.len() > MAX_EQUIPMENT {
        let line = items.get(MAX_EQUIPMENT).map_or(v.line, |(_, l)| *l);
        r.push(
            line,
            r.range_rule,
            format!("at most {MAX_EQUIPMENT} items per appearance"),
        );
        ok = false;
    }
    for (t, line) in items.into_iter().take(MAX_EQUIPMENT) {
        let iv = View {
            table: t,
            line,
            path: "appearance.items",
        };
        r.check_keys(iv, &["did", "dye", "secondary_dye", "stowed", "slot"]);
        let did = r.int(iv, "did", true, 1, i64::from(u32::MAX));
        let dye = r.float_in(iv, "dye", false, |f| (0.0..=1.0).contains(&f), "[0, 1]");
        let dye2 = r.float_in(
            iv,
            "secondary_dye",
            false,
            |f| (0.0..=1.0).contains(&f),
            "[0, 1]",
        );
        let stowed = r.bool(iv, "stowed", false).map(|(b, _)| b);
        let slot = r.int(
            iv,
            "slot",
            false,
            i64::from(MIN_EQUIP_SLOT),
            i64::from(MAX_EQUIP_SLOT),
        );
        if (t.contains_key("dye") && dye.is_none())
            || (t.contains_key("secondary_dye") && dye2.is_none())
            || (t.contains_key("stowed") && stowed.is_none())
            || (t.contains_key("slot") && slot.is_none())
        {
            ok = false;
        }
        match did {
            Some((d, _)) => equipment.push(Equipped {
                item: u32::try_from(d).unwrap_or(1),
                dye: dye.map(|(f, _)| f as f32),
                secondary_dye: dye2.map(|(f, _)| f as f32),
                stowed: stowed.unwrap_or(false),
                slot: slot.and_then(|(s, _)| u8::try_from(s).ok()),
            }),
            None => ok = false,
        }
    }
    let a = Appearance {
        body: body?,
        scale: scale?,
        idle,
        equipment,
    };
    (ok && is_valid(&a)).then_some(a)
}

/// An appearance as an inline TOML table (the keys of §3.5).
#[must_use]
pub fn to_toml(a: &Appearance) -> String {
    to_toml_with_look(a, &Look::none())
}

/// An appearance with its look as an inline TOML table (§3.5).
#[must_use]
pub fn to_toml_with_look(a: &Appearance, look: &Look) -> String {
    let body = match a.body {
        Body::Character { species, sex } => format!("{{ species = {species}, sex = {sex} }}"),
        Body::Object { did } => format!("{{ object = {did} }}"),
    };
    let mut out = format!(
        "{{ body = {body}, scale = {}",
        float_text(f64::from(a.scale))
    );
    if let Some(idle) = a.idle {
        out.push_str(&format!(", idle = {idle}"));
    }
    if !a.equipment.is_empty() {
        out.push_str(", items = [");
        for (i, e) in a.equipment.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{{ did = {}", e.item));
            if let Some(d) = e.dye {
                out.push_str(&format!(", dye = {}", float_text(f64::from(d))));
            }
            if let Some(d) = e.secondary_dye {
                out.push_str(&format!(", secondary_dye = {}", float_text(f64::from(d))));
            }
            if let Some(s) = e.slot {
                out.push_str(&format!(", slot = {s}"));
            }
            out.push_str(&format!(", stowed = {} }}", e.stowed));
        }
        out.push(']');
    }
    if let Some(l) = look_to_toml(look) {
        out.push_str(&format!(", look = {l}"));
    }
    out.push_str(" }");
    out
}
