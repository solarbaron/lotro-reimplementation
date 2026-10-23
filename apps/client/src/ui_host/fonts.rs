// SPDX-License-Identifier: GPL-3.0-or-later
//! Font ladders for crisp scaled text (`ui-scaling.md` §6.4.2): the font records whose plugin
//! `Font` enum names (`lua-plugin-api.md` Appendix A.2, interoperability identifiers) are
//! equal after removing the trailing decimal digits form one ladder. The ladder is built at
//! run time from the enum and the records present; a record's line height is read from the
//! record ([`FontFace::line_height`]), never from its name. A font DID the enum does not name
//! is a ladder of its own, which `tessera-ui` already assumes for a face of no registered
//! ladder, so only ladders of two or more loaded records are registered.
//!
//! [`LadderLoader`] asks the environment for every enum font (decoded on the UI asset
//! workers, never on the frame thread) and hands the ladders over once every record has
//! loaded or failed ([`Ui::set_font_ladders`](tessera_ui::Ui::set_font_ladders)).

use std::sync::Arc;

use tessera_ui::text::{FontFace, FontLadder};

use super::env::EnvState;

/// `Turbine.UI.Lotro.Font` (`lua-plugin-api.md` Appendix A.2): name and font DID. The same
/// table the plugin API registers (`tessera-plugins`, which keeps its copy private); a test
/// in `apps/client/tests/ui_host_wiring.rs` checks that the two agree through a plugin's
/// view of the enum. A stop-gap: once `tessera-plugins` exports its table, this copy goes.
pub const FONT_ENUM: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("TrajanPro20", 0x4200_0000),
    ("Verdana14", 0x4200_0001),
    ("TrajanPro13", 0x4200_0002),
    ("TrajanPro15", 0x4200_0007),
    ("Verdana12", 0x4200_0008),
    ("TrajanPro18", 0x4200_0009),
    ("TrajanPro21", 0x4200_000A),
    ("TrajanPro16", 0x4200_000B),
    ("TrajanPro14", 0x4200_000C),
    ("TrajanPro23", 0x4200_000D),
    ("BookAntiqua20", 0x4200_000E),
    ("BookAntiquaBold22", 0x4200_000F),
    ("BookAntiquaBold24", 0x4200_0010),
    ("TrajanProBold30", 0x4200_0011),
    ("TrajanProBold25", 0x4200_0012),
    ("TrajanProBold22", 0x4200_0013),
    ("TrajanProBold36", 0x4200_0014),
    ("TrajanPro24", 0x4200_0015),
    ("Verdana20", 0x4200_0016),
    ("Verdana10", 0x4200_0017),
    ("Verdana16", 0x4200_0018),
    ("Verdana18", 0x4200_0019),
    ("Verdana22", 0x4200_001A),
    ("Verdana23", 0x4200_001B),
    ("TrajanProBold24", 0x4200_0024),
    ("TrajanPro26", 0x4200_0025),
    ("TrajanPro28", 0x4200_0026),
    ("FixedSys15", 0x4200_0027),
    ("LucidaConsole12", 0x4200_0028),
    ("Arial12", 0x4200_0029),
    ("TrajanProBold16", 0x4200_002A),
    ("TrajanPro19", 0x4200_0035),
    ("BookAntiquaBold14", 0x4200_00F0),
    ("BookAntiqua22", 0x4200_00F1),
    ("BookAntiquaBold18", 0x4200_00F2),
    ("BookAntiquaBold12", 0x4200_00F3),
    ("BookAntiqua24", 0x4200_00F4),
    ("BookAntiquaBold19", 0x4200_00F5),
    ("BookAntiqua12", 0x4200_00F6),
    ("TrajanPro25", 0x4200_00F7),
    ("BookAntiqua16", 0x4200_00F8),
    ("BookAntiqua14", 0x4200_00F9),
    ("BookAntiqua18", 0x4200_00FA),
    ("VerdanaBold16", 0x4200_00FD),
    ("BookAntiqua28", 0x4200_0168),
    ("BookAntiqua26", 0x4200_0169),
    ("BookAntiqua32", 0x4200_016A),
    ("BookAntiqua36", 0x4200_016B),
];

/// Most font records one loader asks for (Tessera bound; the enum has 48).
pub const MAX_LADDER_FONTS: usize = 256;

/// Most frames a loader waits for its records before it registers what has loaded
/// (Tessera bound: a record that never answers does not hold the ladders back for ever).
pub const MAX_LADDER_POLLS: u32 = 3_600;

/// The ladder name of an enum name: the name without its trailing decimal digits
/// (`ui-scaling.md` §6.4.2).
#[must_use]
pub fn ladder_name(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// The ladders of an enum table: (ladder name, font DIDs in table order). DID 0
/// (`Undefined`) is left out, a DID is listed once (its first name wins), and at most
/// [`MAX_LADDER_FONTS`] DIDs are taken. Ladders are in order of their first record.
#[must_use]
pub fn ladder_groups(table: &[(&str, u32)]) -> Vec<(String, Vec<u32>)> {
    let mut out: Vec<(String, Vec<u32>)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (name, did) in table {
        if *did == 0 || seen.len() >= MAX_LADDER_FONTS || !seen.insert(*did) {
            continue;
        }
        let stem = ladder_name(name);
        match out.iter_mut().find(|(n, _)| n == stem) {
            Some((_, dids)) => dids.push(*did),
            None => out.push((stem.to_owned(), vec![*did])),
        }
    }
    out
}

/// The state of one font record for a [`LadderLoader`].
#[derive(Debug, Clone)]
pub enum FontState {
    /// Decoded.
    Ready(Arc<FontFace>),
    /// Still loading.
    Pending,
    /// Absent or undecodable: the ladder goes without it.
    Failed,
}

/// Builds the ladders from faces: each group's ready faces, kept when two or more loaded
/// (a single record is a ladder of its own without registration).
#[must_use]
pub fn build_ladders(
    groups: &[(String, Vec<u32>)],
    mut face: impl FnMut(u32) -> Option<Arc<FontFace>>,
) -> Vec<FontLadder> {
    groups
        .iter()
        .filter_map(|(_, dids)| {
            let faces: Vec<Arc<FontFace>> = dids.iter().filter_map(|d| face(*d)).collect();
            (faces.len() >= 2).then(|| FontLadder::new(faces))
        })
        .collect()
}

/// Loads the enum's fonts on the UI asset workers and builds the ladders once (module docs).
#[derive(Debug, Clone)]
pub struct LadderLoader {
    groups: Vec<(String, Vec<u32>)>,
    polls: u32,
}

impl Default for LadderLoader {
    fn default() -> Self {
        Self::new(FONT_ENUM)
    }
}

impl LadderLoader {
    /// A loader for the ladders of `table` ([`ladder_groups`]).
    #[must_use]
    pub fn new(table: &[(&str, u32)]) -> Self {
        Self {
            groups: ladder_groups(table),
            polls: 0,
        }
    }

    /// Whether [`Self::poll`] has run at least once (the records are being loaded).
    #[must_use]
    pub fn started(&self) -> bool {
        self.polls > 0
    }

    /// The ladders of the loader (name, DIDs).
    #[must_use]
    pub fn groups(&self) -> &[(String, Vec<u32>)] {
        &self.groups
    }

    /// One poll (once per frame): asks `state` for every record (a new one starts loading on
    /// the workers) and returns the ladders once no record is pending any more, or once
    /// [`MAX_LADDER_POLLS`] polls have run. `None` while records are still loading.
    pub fn poll(&mut self, mut state: impl FnMut(u32) -> FontState) -> Option<Vec<FontLadder>> {
        self.polls = self.polls.saturating_add(1);
        let mut faces = std::collections::HashMap::new();
        let mut pending = false;
        for (_, dids) in &self.groups {
            for d in dids {
                match state(*d) {
                    FontState::Ready(f) => {
                        faces.insert(*d, f);
                    }
                    FontState::Pending => pending = true,
                    FontState::Failed => {}
                }
            }
        }
        if pending && self.polls < MAX_LADDER_POLLS {
            return None;
        }
        Some(build_ladders(&self.groups, |d| faces.get(&d).cloned()))
    }

    /// [`Self::poll`] over the client's environment: a record is ready when
    /// [`EnvState::prefetch_font`] returns it, failed when it no longer resolves, pending
    /// otherwise. The records are asked for as prefetches, so their arrival re-shapes no text
    /// (only the one [`tessera_ui::Ui::set_font_ladders`] call does).
    pub(crate) fn poll_env(&mut self, st: &mut EnvState) -> Option<Vec<FontLadder>> {
        self.poll(|d| match st.prefetch_font(d) {
            Some(f) => FontState::Ready(f),
            None if st.font_resolves(d) => FontState::Pending,
            None => FontState::Failed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_names_drop_trailing_digits() {
        assert_eq!(ladder_name("TrajanPro20"), "TrajanPro");
        assert_eq!(ladder_name("TrajanProBold36"), "TrajanProBold");
        assert_eq!(ladder_name("Undefined"), "Undefined");
        assert_eq!(ladder_name("12"), "");
    }

    /// `ui-scaling.md` §6.4.2's table: the ladders and their sizes in this enum.
    #[test]
    fn enum_ladders_match_the_spec_table() {
        let g = ladder_groups(FONT_ENUM);
        let count = |n: &str| g.iter().find(|(x, _)| x == n).map_or(0, |(_, d)| d.len());
        assert_eq!(count("TrajanPro"), 13);
        assert_eq!(count("TrajanProBold"), 6);
        assert_eq!(count("BookAntiqua"), 11);
        assert_eq!(count("BookAntiquaBold"), 6);
        assert_eq!(count("Verdana"), 8);
        for one in ["VerdanaBold", "LucidaConsole", "FixedSys", "Arial"] {
            assert_eq!(count(one), 1, "{one}");
        }
        assert_eq!(count("Undefined"), 0, "DID 0 is no record");
        let total: usize = g.iter().map(|(_, d)| d.len()).sum();
        assert_eq!(total, FONT_ENUM.len() - 1);
    }

    #[test]
    fn duplicate_dids_are_listed_once() {
        let g = ladder_groups(&[("A10", 5), ("A12", 6), ("B10", 5), ("A14", 0)]);
        assert_eq!(g, vec![("A".to_owned(), vec![5, 6])]);
    }

    #[test]
    fn loader_waits_for_pending_records_then_gives_up() {
        let mut l = LadderLoader::new(&[("A10", 1), ("A12", 2)]);
        assert!(l.poll(|_| FontState::Pending).is_none());
        // Failed records leave the ladder; one face is no ladder.
        let got = l.poll(|_| FontState::Failed).expect("nothing pending");
        assert!(got.is_empty());
        let mut l = LadderLoader::new(&[("A10", 1)]);
        let mut out = None;
        for _ in 0..MAX_LADDER_POLLS {
            out = l.poll(|_| FontState::Pending);
            if out.is_some() {
                break;
            }
        }
        assert!(out.is_some(), "the loader gives up after its bound");
    }
}
