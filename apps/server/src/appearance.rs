// SPDX-License-Identifier: GPL-3.0-or-later
//! Appearances in server files (`test-server.md` §3.5 `appearance`, §5.4 `appearance`).
//!
//! Appearances hold only opaque references chosen by players' clients (or, in content, at
//! most a handful written by a contributor). The server checks the protocol's limits
//! (`tessera-protocol.md` §5.3) and never interprets a reference (`server-content.md` §4.1).

use tessera_protocol::limits::{MAX_APPEARANCE_SCALE, MAX_EQUIPMENT};
use tessera_protocol::types::{Appearance, Body, Equipped};

use crate::tomlx::{Reader, View, float_text};

/// Whether an appearance meets the protocol's limits (§5.3): scale in (0, 100], at most 32
/// items, item DIDs and object DIDs non-zero, dye codes in [0, 1].
#[must_use]
pub fn is_valid(a: &Appearance) -> bool {
    let body_ok = match a.body {
        Body::Character { .. } => true,
        Body::Object { did } => did != 0,
    };
    let dye_ok = |d: Option<f32>| d.is_none_or(|d| d.is_finite() && (0.0..=1.0).contains(&d));
    body_ok
        && a.scale.is_finite()
        && a.scale > 0.0
        && a.scale <= MAX_APPEARANCE_SCALE
        && a.equipment.len() <= MAX_EQUIPMENT
        && a.equipment
            .iter()
            .all(|e| e.item != 0 && dye_ok(e.dye) && dye_ok(e.secondary_dye))
}

/// The number of opaque references in an appearance (`test-server.md` §5.4): one per body,
/// one for `idle`, one per item.
#[must_use]
pub fn reference_count(a: &Appearance) -> usize {
    1 + usize::from(a.idle.is_some()) + a.equipment.len()
}

/// Reads an appearance table. Findings are pushed to `r` with its range rule; returns `None`
/// when the table is not a valid appearance.
pub(crate) fn parse(r: &mut Reader<'_, '_>, v: View<'_, '_>) -> Option<Appearance> {
    r.check_keys(v, &["body", "scale", "idle", "items"]);
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
        r.check_keys(iv, &["did", "dye", "secondary_dye", "stowed"]);
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
        if (t.contains_key("dye") && dye.is_none())
            || (t.contains_key("secondary_dye") && dye2.is_none())
            || (t.contains_key("stowed") && stowed.is_none())
        {
            ok = false;
        }
        match did {
            Some((d, _)) => equipment.push(Equipped {
                item: u32::try_from(d).unwrap_or(1),
                dye: dye.map(|(f, _)| f as f32),
                secondary_dye: dye2.map(|(f, _)| f as f32),
                stowed: stowed.unwrap_or(false),
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
            out.push_str(&format!(", stowed = {} }}", e.stowed));
        }
        out.push(']');
    }
    out.push_str(" }");
    out
}
