// SPDX-License-Identifier: GPL-3.0-or-later
//! The bridge between `tessera-ui`'s in-memory store of saved panel placements
//! ([`SavedStore::placements`], `ui-scaling.md` §5.8) and their persisted form, the
//! `[UI.panels]` table of Tessera's settings ([`PanelPlacement`]). `tessera-ui` does not
//! depend on `tessera-settings`, so the field-for-field copy lives here, in the UI host
//! (the §5.8 implementer OPEN).
//!
//! - **Start:** [`store_from_prefs`] fills the UI's store from `[UI.panels]` when the host
//!   is made ([`super::UiHost::new`]). §5.8 "Applying" runs in the host
//!   ([`super::UiHost::apply_saved_placements`]): for each saved root when it is attached
//!   (once per frame, before painting) and for all of them after every change of *F* or *A*
//!   (`tessera-ui` itself applies no saved placement).
//! - **Save:** at the end of a user move or resize of a saved element (`UiEvent::
//!   LayoutChanged`) the host records the element's placement (`Ui::record_saved`) and hands
//!   the whole store, as [`prefs_from_store`], to the settings writer
//!   ([`super::settings_writer`]), which writes it off the frame thread.
//! - **Reset** (`ui-scaling.md` §7.1 "Reset panel positions"): the in-memory store is
//!   emptied together with `[UI.panels]`, and the saved roots are laid out again at their
//!   layout positions ([`super::UiHost::reset_panel_positions`]).
//!
//! Keys are written `"0xLLLLLLLL/0xEEEEEEEE"` (the root's layout DID and element ID, §5.8);
//! a key that does not read back that way is skipped (logged at debug level).

use std::collections::BTreeMap;

use tessera_settings::{AnchorX, AnchorY, PanelPlacement, PanelRef};
use tessera_ui::SavedStore;
use tessera_ui::scaling::{Anchor, RefBox, SavedPlacement};

/// The `[UI.panels]` key of (layout DID, element ID).
#[must_use]
pub fn panel_key(layout: u32, element: u32) -> String {
    format!("0x{layout:08X}/0x{element:08X}")
}

/// The (layout DID, element ID) of a `[UI.panels]` key; `None` when it is not
/// `0xLLLLLLLL/0xEEEEEEEE` (hex digits of either case, `0x` or `0X`).
#[must_use]
pub fn parse_panel_key(key: &str) -> Option<(u32, u32)> {
    let (l, e) = key.trim().split_once('/')?;
    let hex = |s: &str| {
        let d = s
            .trim()
            .strip_prefix("0x")
            .or_else(|| s.trim().strip_prefix("0X"))?;
        if d.is_empty() || d.len() > 8 {
            return None;
        }
        u32::from_str_radix(d, 16).ok()
    };
    Some((hex(l)?, hex(e)?))
}

/// A settings placement as the UI's.
#[must_use]
pub fn ui_placement(p: &PanelPlacement) -> SavedPlacement {
    let size = match (p.w, p.h) {
        (Some(w), Some(h)) => Some((
            i32::try_from(w).unwrap_or(i32::MAX),
            i32::try_from(h).unwrap_or(i32::MAX),
        )),
        _ => None,
    };
    SavedPlacement {
        reference: match p.reference {
            PanelRef::Safe => RefBox::Safe,
            PanelRef::Screen => RefBox::Screen,
        },
        ax: match p.ax {
            AnchorX::Left => Anchor::Start,
            AnchorX::Centre => Anchor::Centre,
            AnchorX::Right => Anchor::End,
        },
        dx: f64::from(p.dx),
        ay: match p.ay {
            AnchorY::Top => Anchor::Start,
            AnchorY::Centre => Anchor::Centre,
            AnchorY::Bottom => Anchor::End,
        },
        dy: f64::from(p.dy),
        size,
        shown: p.shown,
    }
}

/// Rounds an offset to the two decimals of the file form (§5.8) as the settings field's
/// `f32`; a non-finite value is 0.
fn offset(v: f64) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let r = ((v * 100.0).round() / 100.0) as f32;
    r
}

/// A UI placement as the settings'.
#[must_use]
pub fn settings_placement(p: &SavedPlacement) -> PanelPlacement {
    let dim = |v: i32| u32::try_from(v.max(0)).unwrap_or(0);
    PanelPlacement {
        reference: match p.reference {
            RefBox::Safe => PanelRef::Safe,
            RefBox::Screen => PanelRef::Screen,
        },
        ax: match p.ax {
            Anchor::Start => AnchorX::Left,
            Anchor::Centre => AnchorX::Centre,
            Anchor::End => AnchorX::Right,
        },
        dx: offset(p.dx),
        ay: match p.ay {
            Anchor::Start => AnchorY::Top,
            Anchor::Centre => AnchorY::Centre,
            Anchor::End => AnchorY::Bottom,
        },
        dy: offset(p.dy),
        w: p.size.map(|(w, _)| dim(w)),
        h: p.size.map(|(_, h)| dim(h)),
        shown: p.shown,
    }
}

/// The UI's store from `[UI.panels]` (start of the run).
#[must_use]
pub fn store_from_prefs(panels: &BTreeMap<String, PanelPlacement>) -> SavedStore {
    let mut store = SavedStore::default();
    for (key, p) in panels {
        match parse_panel_key(key) {
            Some(k) => {
                store.placements.insert(k, ui_placement(p));
            }
            None => tracing::debug!(key = %key, "[UI.panels]: key not 0xL/0xE; skipped"),
        }
    }
    store
}

/// `[UI.panels]` from the UI's store (the anchored placements; the legacy normalized-corner
/// entries are not persisted).
#[must_use]
pub fn prefs_from_store(store: &SavedStore) -> BTreeMap<String, PanelPlacement> {
    store
        .placements
        .iter()
        .map(|((l, e), p)| (panel_key(*l, *e), settings_placement(p)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        assert_eq!(panel_key(0x2200_0064, 0x1000_0024), "0x22000064/0x10000024");
        assert_eq!(
            parse_panel_key("0x22000064/0x10000024"),
            Some((0x2200_0064, 0x1000_0024))
        );
        assert_eq!(parse_panel_key("0X2200aBcD/0x1"), Some((0x2200_ABCD, 1)));
        for bad in [
            "",
            "0x1",
            "0x/0x1",
            "22000064/10000024",
            "0x123456789/0x1",
            "0xZZ/0x1",
        ] {
            assert_eq!(parse_panel_key(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn placements_round_trip_through_the_settings_form() {
        let p = SavedPlacement {
            reference: RefBox::Screen,
            ax: Anchor::End,
            dx: 40.256,
            ay: Anchor::Centre,
            dy: -12.0,
            size: Some((300, 200)),
            shown: Some(false),
        };
        let mut store = SavedStore::default();
        store.placements.insert((0x2200_0001, 0x1000_0002), p);
        let prefs = prefs_from_store(&store);
        let s = &prefs["0x22000001/0x10000002"];
        assert_eq!(s.reference, PanelRef::Screen);
        assert_eq!((s.ax, s.ay), (AnchorX::Right, AnchorY::Centre));
        assert!((s.dx - 40.26).abs() < 1e-4, "two decimals");
        assert_eq!((s.w, s.h, s.shown), (Some(300), Some(200), Some(false)));
        let back = store_from_prefs(&prefs);
        let q = back.placements[&(0x2200_0001, 0x1000_0002)];
        assert_eq!(
            (q.reference, q.ax, q.ay, q.size),
            (p.reference, p.ax, p.ay, p.size)
        );
        assert!((q.dx - 40.26).abs() < 1e-4 && (q.dy + 12.0).abs() < 1e-9);
    }
}
