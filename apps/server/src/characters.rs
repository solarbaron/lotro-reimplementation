// SPDX-License-Identifier: GPL-3.0-or-later
//! Character files (`test-server.md` §3.5) and the world-state file (§3.6).
//!
//! A character file holds only its own account's characters: each with its name, appearance
//! (opaque references chosen by the player's client) and last position. Nothing lists
//! references or positions across characters (`server-content.md` §11 invariant 6).

use std::collections::BTreeMap;

use tessera_protocol::limits::MAX_LOCATION_COORDINATE;
use tessera_protocol::types::{Anchor, Appearance, Location, Look};

use crate::appearance;
use crate::datadir::DataDir;
use crate::error::ServerError;
use crate::names;
use crate::tomlx::{self, Reader, Rule, Source, View, float_text, quote};

/// Largest character file, bytes (`test-server.md` §3.5): larger files are refused when
/// read, and a change that would make one larger is refused.
pub const MAX_CHARACTER_FILE: usize = 1_048_576;

/// A stored position (§3.5 `position`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoredPosition {
    /// Region-global position.
    pub location: Location,
    /// Heading, radians (protocol convention).
    pub heading: f32,
    /// The anchor that owns the position, if any.
    pub anchor: Option<Anchor>,
}

/// The appearance as stored: valid, or the original text of one that fails the protocol's
/// limits (kept so a rewrite does not lose it; such a character is refused at `EnterWorld`).
#[derive(Debug, Clone, PartialEq)]
pub enum StoredAppearance {
    /// A valid appearance.
    Valid(Appearance),
    /// The original TOML text of an invalid appearance.
    Invalid(String),
}

/// One character.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterRecord {
    /// Display name as created (NFC).
    pub name: String,
    /// Creation time, RFC 3339 UTC.
    pub created: String,
    /// Last world instance, 0 = never entered.
    pub instance: u32,
    /// Last accepted position; `None` when it failed the limits (reset to the default spawn).
    pub position: Option<StoredPosition>,
    /// Appearance.
    pub appearance: StoredAppearance,
    /// The character's look as its client created it (protocol 1.1; empty: none). Stored
    /// and sent unchanged, never judged (`server-content.md` §4.1 R2, R3).
    pub look: Look,
}

/// One account's character file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterFile {
    /// Canonical account name.
    pub account: String,
    /// The characters.
    pub characters: Vec<CharacterRecord>,
}

/// Whether a character file's text fits [`MAX_CHARACTER_FILE`].
#[must_use]
pub fn fits(file: &CharacterFile) -> bool {
    file.to_toml().len() <= MAX_CHARACTER_FILE
}

fn valid_position(p: &StoredPosition) -> bool {
    p.location.is_valid()
        && p.heading.is_finite()
        && p.anchor
            .is_none_or(|a| a.block_x <= 254 && a.block_y <= 254 && a.owns(&p.location))
}

impl CharacterFile {
    /// Parses a character file for `account`. Structural problems (syntax, unknown keys,
    /// another account's name, duplicate names) make it invalid; a position or appearance
    /// that fails the protocol's limits is kept as described on [`CharacterRecord`] and
    /// reported in the returned warnings.
    pub fn parse(text: &str, account: &str) -> Result<(Self, Vec<String>), String> {
        let src = Source::new(text);
        let doc = tomlx::parse(&src).map_err(|f| format!("line {}: {}", f.line, f.message))?;
        let mut r = Reader::new(&src, Rule::Format, Rule::Range);
        let root = View {
            table: doc.get_ref(),
            line: 1,
            path: "",
        };
        r.check_keys(root, &["format", "account", "character"]);
        r.int(root, "format", true, 1, 1);
        match r.string(root, "account", true) {
            Some((a, line)) if a != account => r.push(
                line,
                Rule::Format,
                "the file holds another account's characters",
            ),
            _ => {}
        }
        let mut warnings = Vec::new();
        let mut out = Self {
            account: account.to_owned(),
            characters: Vec::new(),
        };
        for (t, line) in r.tables(root, "character") {
            let v = View {
                table: t,
                line,
                path: "character",
            };
            r.check_keys(
                v,
                &["name", "created", "instance", "position", "appearance"],
            );
            let name = r.string(v, "name", true);
            let created = r.utc_datetime(v, "created", true);
            let instance = r.int(v, "instance", true, 0, i64::from(u32::MAX));
            let position = match r.table(v, "position", true) {
                Some((pt, pv)) => {
                    let pline = r.line(&pv.span());
                    let pview = View {
                        table: pt,
                        line: pline,
                        path: "character.position",
                    };
                    r.check_keys(pview, &["region", "x", "y", "z", "heading", "anchor"]);
                    let region = r.int(pview, "region", true, 0, 65_535);
                    let x = r.float(pview, "x", true);
                    let y = r.float(pview, "y", true);
                    let z = r.float(pview, "z", true);
                    let heading = r.float(pview, "heading", true);
                    // Some(Some(a)): anchor; Some(None): none; None: structural error.
                    let mut anchor_out_of_range = false;
                    let anchor = match r.array(pview, "anchor", false) {
                        Some((arr, aline)) => {
                            let nums: Vec<i64> = arr
                                .iter()
                                .filter_map(|i| tomlx::integer(i.get_ref()))
                                .collect();
                            if nums.len() == 3 && arr.len() == 3 {
                                match (
                                    u8::try_from(nums[0]),
                                    u8::try_from(nums[1]),
                                    u16::try_from(nums[2]),
                                ) {
                                    (Ok(bx), Ok(by), Ok(cell)) => {
                                        Some(Some(Anchor::new(bx, by, cell)))
                                    }
                                    // Indices that cannot be an anchor fail the limits like
                                    // any other stored position: reset to the default spawn
                                    // (§3.5), never kept without their anchor.
                                    _ => {
                                        anchor_out_of_range = true;
                                        Some(None)
                                    }
                                }
                            } else {
                                r.push(
                                    aline,
                                    Rule::Format,
                                    "`anchor` must be [block X, block Y, cell]",
                                );
                                None
                            }
                        }
                        None => Some(None),
                    };
                    match (region, x, y, z, heading, anchor) {
                        (
                            Some((rg, _)),
                            Some((x, _)),
                            Some((y, _)),
                            Some((z, _)),
                            Some((h, _)),
                            Some(anchor),
                        ) => {
                            let p = StoredPosition {
                                location: Location::new(rg as u16, x, y, z),
                                heading: h as f32,
                                anchor,
                            };
                            if !anchor_out_of_range
                                && valid_position(&p)
                                && [x, y, z].iter().all(|c| c.abs() <= MAX_LOCATION_COORDINATE)
                            {
                                Some(Some(p))
                            } else {
                                warnings.push(format!(
                                    "line {pline}: stored position fails the protocol's limits; the character starts at the default spawn"
                                ));
                                Some(None)
                            }
                        }
                        _ => None,
                    }
                }
                None => None,
            };
            let appearance = match r.value(v, "appearance", true) {
                Some(av) => {
                    // Kept as an inline table re-serialised from the parsed value (never the
                    // source span, which for a header-defined table is the header line).
                    let raw = tomlx::inline_value(av.get_ref()).unwrap_or_else(|| "{}".to_owned());
                    let aline = r.line(&av.span());
                    match av.get_ref() {
                        toml::de::DeValue::Table(at) => {
                            // Parse with a scratch reader: limit failures are not structural.
                            let mut scratch = Reader::new(&src, Rule::Range, Rule::Range);
                            let view = View {
                                table: at,
                                line: aline,
                                path: "character.appearance",
                            };
                            match appearance::parse_character(&mut scratch, view) {
                                Some((a, look)) => Some((StoredAppearance::Valid(a), look)),
                                None => {
                                    warnings.push(format!(
                                        "line {aline}: stored appearance fails the protocol's limits; entering the world is refused"
                                    ));
                                    Some((StoredAppearance::Invalid(raw), Look::none()))
                                }
                            }
                        }
                        _ => {
                            r.push(aline, Rule::Format, "`appearance` must be a table");
                            None
                        }
                    }
                }
                None => None,
            };
            let (
                Some((name, nline)),
                Some((created, _)),
                Some((instance, _)),
                Some(position),
                Some((appearance, look)),
            ) = (name, created, instance, position, appearance)
            else {
                continue;
            };
            // A stored name must stay a protocol NAME (1–64 bytes, no control characters):
            // every answer that carries it (`LoginOk`, `WorldEnter`, ...) must be encodable.
            if !tessera_protocol::text::is_valid_name(&name) {
                r.push(
                    nline,
                    Rule::Format,
                    "character name is not a protocol NAME (1–64 bytes, no control characters)",
                );
                continue;
            }
            let key = names::name_key(&name);
            if out
                .characters
                .iter()
                .any(|c| names::name_key(&c.name) == key)
            {
                r.push(nline, Rule::Format, "duplicate character name");
                continue;
            }
            out.characters.push(CharacterRecord {
                name,
                created,
                instance: u32::try_from(instance).unwrap_or(0),
                position,
                appearance,
                look,
            });
        }
        if let Some(f) = r.findings.first() {
            return Err(format!("line {}: {}", f.line, f.message));
        }
        Ok((out, warnings))
    }

    /// The file's text.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut s = format!(
            "# Tessera test server characters (private operator state; never publish or commit)\nformat = 1\naccount = {}\n",
            quote(&self.account)
        );
        for c in &self.characters {
            s.push_str(&format!(
                "\n[[character]]\nname = {}\ncreated = {}\ninstance = {}\n",
                quote(&c.name),
                c.created,
                c.instance
            ));
            if let Some(p) = &c.position {
                s.push_str(&format!(
                    "position = {{ region = {}, x = {}, y = {}, z = {}, heading = {}",
                    p.location.region,
                    float_text(p.location.x),
                    float_text(p.location.y),
                    float_text(p.location.z),
                    float_text(f64::from(p.heading))
                ));
                if let Some(a) = p.anchor {
                    s.push_str(&format!(
                        ", anchor = [{}, {}, {}]",
                        a.block_x, a.block_y, a.cell
                    ));
                }
                s.push_str(" }\n");
            } else {
                // Position unknown: an invalid placeholder the loader resets to the spawn.
                s.push_str("position = { region = 0, x = 0.0, y = 0.0, z = 0.0, heading = 0.0, anchor = [255, 255, 0] }\n");
            }
            let appearance = match &c.appearance {
                StoredAppearance::Valid(a) => appearance::to_toml_with_look(a, &c.look),
                StoredAppearance::Invalid(raw) => raw.clone(),
            };
            s.push_str(&format!("appearance = {appearance}\n"));
        }
        s
    }
}

/// Loads every character file of the data directory. Returns the files by account and the
/// warnings to log. An invalid file stops the load (the caller refuses to start).
pub fn load_all(dir: &DataDir) -> Result<BTreeMap<String, CharacterFile>, ServerError> {
    let mut out = BTreeMap::new();
    let cdir = dir.characters_dir();
    let entries = match std::fs::read_dir(&cdir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(ServerError::io(cdir, e)),
    };
    for e in entries.filter_map(Result::ok).take(1_000_000) {
        let path = e.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|x| x.to_str()) != Some("toml") || stem.starts_with('.') {
            continue;
        }
        let Some(account) = names::canonical_account(stem).filter(|c| c == stem) else {
            tracing::warn!(file = %path.display(), "ignoring a file not named after a canonical account");
            continue;
        };
        let text = match tomlx::read_text(&path) {
            Ok(Ok(t)) => t,
            Ok(Err(f)) => {
                return Err(ServerError::InvalidFile {
                    path,
                    message: f.message,
                });
            }
            Err(e) => return Err(ServerError::io(path, e)),
        };
        if text.len() > MAX_CHARACTER_FILE {
            return Err(ServerError::InvalidFile {
                path,
                message: format!("character file larger than {MAX_CHARACTER_FILE} bytes"),
            });
        }
        let (file, warnings) =
            CharacterFile::parse(&text, &account).map_err(|message| ServerError::InvalidFile {
                path: path.clone(),
                message,
            })?;
        for w in warnings {
            tracing::error!(file = %path.display(), "{w}");
        }
        out.insert(account, file);
    }
    Ok(out)
}

/// Reads one account's character file (empty when missing).
pub fn read_file(dir: &DataDir, account: &str) -> Result<CharacterFile, ServerError> {
    let path = dir.character_path(account);
    match tomlx::read_text(&path) {
        Ok(Ok(text)) if text.len() > MAX_CHARACTER_FILE => Err(ServerError::InvalidFile {
            path,
            message: format!("character file larger than {MAX_CHARACTER_FILE} bytes"),
        }),
        Ok(Ok(text)) => CharacterFile::parse(&text, account)
            .map(|(f, _)| f)
            .map_err(|message| ServerError::InvalidFile { path, message }),
        Ok(Err(f)) => Err(ServerError::InvalidFile {
            path,
            message: f.message,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CharacterFile {
            account: account.to_owned(),
            characters: Vec::new(),
        }),
        Err(e) => Err(ServerError::io(path, e)),
    }
}

/// The world-state file (§3.6): the game clock of each instance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldState {
    /// Game clock in seconds by instance number (1–16).
    pub clocks: BTreeMap<u32, f64>,
}

impl WorldState {
    /// Parses the file; invalid entries are skipped with a warning (the instance then starts
    /// from the content's clock).
    #[must_use]
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let src = Source::new(text);
        let mut out = Self::default();
        let doc = match tomlx::parse(&src) {
            Ok(d) => d,
            Err(f) => return (out, vec![format!("line {}: {}", f.line, f.message)]),
        };
        let mut r = Reader::new(&src, Rule::Format, Rule::Range);
        let root = View {
            table: doc.get_ref(),
            line: 1,
            path: "",
        };
        r.check_keys(root, &["format", "instance"]);
        r.int(root, "format", true, 1, 1);
        for (t, line) in r.tables(root, "instance") {
            let v = View {
                table: t,
                line,
                path: "instance",
            };
            r.check_keys(v, &["number", "game_clock"]);
            let n = r.int(v, "number", true, 1, 16);
            let g = r.float_in(v, "game_clock", true, |f| f >= 0.0, "≥ 0");
            if let (Some((n, _)), Some((g, _))) = (n, g) {
                out.clocks.insert(n as u32, g);
            }
        }
        let warnings = r
            .findings
            .iter()
            .map(|f| format!("line {}: {}", f.line, f.message))
            .collect();
        (out, warnings)
    }

    /// The file's text.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut s = String::from("# Tessera test server world state\nformat = 1\n");
        for (n, g) in &self.clocks {
            s.push_str(&format!(
                "\n[[instance]]\nnumber = {n}\ngame_clock = {}\n",
                float_text(*g)
            ));
        }
        s
    }

    /// Reads the file (empty when missing or unreadable, with a warning).
    #[must_use]
    pub fn load(dir: &DataDir) -> Self {
        let path = dir.world_state_path();
        match tomlx::read_text(&path) {
            Ok(Ok(text)) => {
                let (s, w) = Self::parse(&text);
                for w in w {
                    tracing::warn!(file = %path.display(), "{w}");
                }
                s
            }
            Ok(Err(f)) => {
                tracing::warn!(file = %path.display(), "{}", f.message);
                Self::default()
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "cannot read the world state");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_protocol::types::{Body, Equipped};

    #[test]
    fn round_trip() {
        let f = CharacterFile {
            account: "alice".into(),
            characters: vec![
                CharacterRecord {
                    name: "Aldith".into(),
                    created: "2026-10-06T12:00:00Z".into(),
                    instance: 1,
                    position: Some(StoredPosition {
                        location: Location::new(1, 20_240.5, 22_420.25, 440.1),
                        heading: 1.25,
                        anchor: Some(Anchor::new(126, 140, 0)),
                    }),
                    appearance: StoredAppearance::Valid(Appearance {
                        body: Body::Character {
                            species: 23,
                            sex: 1,
                        },
                        scale: 1.0,
                        idle: Some(7),
                        equipment: vec![Equipped {
                            item: 0x7000_1234,
                            dye: Some(0.25),
                            secondary_dye: None,
                            stowed: true,
                            slot: None,
                        }],
                    }),
                    look: tessera_protocol::types::Look::none(),
                },
                CharacterRecord {
                    name: "Bram".into(),
                    created: "2026-10-06T12:00:00Z".into(),
                    instance: 0,
                    position: None,
                    appearance: StoredAppearance::Invalid(
                        "{ body = { object = 0 }, scale = 1.0 }".into(),
                    ),
                    look: tessera_protocol::types::Look::none(),
                },
            ],
        };
        let text = f.to_toml();
        let (g, warnings) = CharacterFile::parse(&text, "alice").unwrap();
        assert_eq!(g, f, "{text}");
        assert_eq!(warnings.len(), 2);
        assert!(CharacterFile::parse(&text, "bob").is_err());
        assert!(
            CharacterFile::parse(
                &text.replace("instance = 1", "instance = 1\nextra = 2"),
                "alice"
            )
            .is_err()
        );
    }

    fn one_character(position: &str, appearance: &str) -> String {
        format!(
            "format = 1\naccount = \"alice\"\n[[character]]\nname = \"Aldith\"\ncreated = 2026-10-06T12:00:00Z\ninstance = 1\nposition = {position}\n{appearance}"
        )
    }

    const POS: &str = "{ region = 1, x = 100.0, y = 100.0, z = 0.0, heading = 0.0 }";

    #[test]
    fn invalid_appearances_round_trip_in_every_table_form() {
        let forms = [
            // Header form (the one test-server.md §3.5 documents for items).
            "[character.appearance]\nscale = 500.0\n[character.appearance.body]\nspecies = 1\nsex = 1\n[[character.appearance.items]]\ndid = 0\n",
            // Dotted keys.
            "appearance.scale = 500.0\nappearance.body.species = 1\nappearance.body.sex = 1\n",
            // Inline.
            "appearance = { body = { object = 0 }, scale = 1.0, \"odd key\" = [1, 0x1F, -2.5e3, \"t\\\"x\", 1979-05-27T07:32:00Z, true] }\n",
        ];
        for form in forms {
            let text = one_character(POS, form);
            let (f, warnings) = CharacterFile::parse(&text, "alice").expect(form);
            assert!(
                matches!(f.characters[0].appearance, StoredAppearance::Invalid(_)),
                "{form}"
            );
            assert_eq!(warnings.len(), 1, "{form}");
            let written = f.to_toml();
            let (back, _) = CharacterFile::parse(&written, "alice")
                .unwrap_or_else(|e| panic!("{e}\n{written}"));
            assert_eq!(back, f, "{written}");
            assert_eq!(back.to_toml(), written);
        }
    }

    #[test]
    fn out_of_range_anchor_resets_the_position() {
        let pos =
            "{ region = 1, x = 100.0, y = 100.0, z = 0.0, heading = 0.0, anchor = [300, 1, 0] }";
        let text = one_character(
            pos,
            "appearance = { body = { species = 1, sex = 1 }, scale = 1.0 }\n",
        );
        let (f, warnings) = CharacterFile::parse(&text, "alice").unwrap();
        assert_eq!(f.characters[0].position, None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        // A malformed anchor (not three integers) is still a structural error.
        let pos = "{ region = 1, x = 100.0, y = 100.0, z = 0.0, heading = 0.0, anchor = [1, 2] }";
        let text = one_character(
            pos,
            "appearance = { body = { species = 1, sex = 1 }, scale = 1.0 }\n",
        );
        assert!(CharacterFile::parse(&text, "alice").is_err());
    }

    #[test]
    fn stored_names_must_be_protocol_names() {
        let long: String = std::iter::repeat_n('\u{242EE}', 21).collect();
        let text = one_character(
            POS,
            "appearance = { body = { species = 1, sex = 1 }, scale = 1.0 }\n",
        )
        .replace("Aldith", &long);
        assert!(CharacterFile::parse(&text, "alice").is_err());
    }

    #[test]
    fn looks_and_slots_round_trip() {
        use tessera_protocol::types::{LookEntry, LookValue};
        let look = Look {
            entries: vec![
                LookEntry {
                    property: 1,
                    value: LookValue::Bool(true),
                },
                LookEntry {
                    property: 2,
                    value: LookValue::Int32(-5),
                },
                LookEntry {
                    property: 3,
                    value: LookValue::Int64(i64::MIN),
                },
                LookEntry {
                    property: 4,
                    value: LookValue::Float32(0.1),
                },
                LookEntry {
                    property: 5,
                    value: LookValue::Enum(u32::MAX),
                },
                LookEntry {
                    property: 6,
                    value: LookValue::DataId(0x7000_0001),
                },
                LookEntry {
                    property: 7,
                    value: LookValue::String("8080ff".into()),
                },
                LookEntry {
                    property: 8,
                    value: LookValue::Bitfield32(5),
                },
                LookEntry {
                    property: 9,
                    value: LookValue::Bitfield64(u64::MAX),
                },
                LookEntry {
                    property: u32::MAX,
                    value: LookValue::Colour([1, 2, 3, 255]),
                },
            ],
        };
        let mut a = Appearance::new(Body::Character { species: 1, sex: 2 });
        a.equipment.push(Equipped {
            item: 9,
            slot: Some(64),
            ..Equipped::default()
        });
        let f = CharacterFile {
            account: "alice".into(),
            characters: vec![CharacterRecord {
                name: "Aldith".into(),
                created: "2026-10-10T12:00:00Z".into(),
                instance: 1,
                position: None,
                appearance: StoredAppearance::Valid(a),
                look,
            }],
        };
        let text = f.to_toml();
        let (back, _) =
            CharacterFile::parse(&text, "alice").unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(back, f, "{text}");
        assert!(fits(&f));
        // The header form of test-server.md §3.5 reads too.
        let header = one_character(
            POS,
            "[character.appearance]\nscale = 1.0\n[character.appearance.body]\nspecies = 1\nsex = 1\n[[character.appearance.items]]\ndid = 5\nslot = 17\n[[character.appearance.look]]\nproperty = 7\nkind = \"colour\"\nvalue = [1, 2, 3, 4]\n[[character.appearance.look]]\nproperty = 9\nkind = \"bitfield64\"\nvalue = \"0x00000000000000FF\"\n",
        );
        let (h, warnings) = CharacterFile::parse(&header, "alice").unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(h.characters[0].look.entries.len(), 2);
    }

    #[test]
    fn invalid_looks_and_slots_refuse_world_entry() {
        for bad in [
            // Not ascending.
            "look = [{ property = 2, kind = \"bool\", value = true }, { property = 1, kind = \"bool\", value = true }]",
            // A space in a string; an unknown kind; a value of the wrong type.
            "look = [{ property = 1, kind = \"string\", value = \"a b\" }]",
            "look = [{ property = 1, kind = \"array\", value = 1 }]",
            "look = [{ property = 1, kind = \"int32\", value = 3000000000 }]",
            "look = [{ property = 0, kind = \"bool\", value = true }]",
            "items = [{ did = 5, slot = 65 }]",
            "items = [{ did = 5, slot = 0 }]",
        ] {
            let text = one_character(
                POS,
                &format!(
                    "appearance = {{ body = {{ species = 1, sex = 1 }}, scale = 1.0, {bad} }}\n"
                ),
            );
            let (f, warnings) = CharacterFile::parse(&text, "alice").expect(bad);
            assert!(
                matches!(f.characters[0].appearance, StoredAppearance::Invalid(_)),
                "{bad}"
            );
            assert_eq!(warnings.len(), 1, "{bad}");
            // Kept as written.
            let (back, _) = CharacterFile::parse(&f.to_toml(), "alice").unwrap();
            assert_eq!(back, f);
        }
    }

    #[test]
    fn character_files_are_bounded() {
        let mut f = CharacterFile {
            account: "alice".into(),
            characters: Vec::new(),
        };
        let raw = format!(
            "{{ body = {{ object = 0 }}, scale = 1.0, note = \"{}\" }}",
            "x".repeat(20_000)
        );
        for i in 0..60 {
            f.characters.push(CharacterRecord {
                name: format!("C{i}"),
                created: "2026-10-10T12:00:00Z".into(),
                instance: 0,
                position: None,
                appearance: StoredAppearance::Invalid(raw.clone()),
                look: Look::none(),
            });
        }
        assert!(!fits(&f), "{} bytes", f.to_toml().len());
    }

    #[test]
    fn world_state() {
        let mut w = WorldState::default();
        w.clocks.insert(1, 43_200.5);
        w.clocks.insert(16, 0.0);
        let (back, warn) = WorldState::parse(&w.to_toml());
        assert!(warn.is_empty());
        assert_eq!(back, w);
        let (back, warn) =
            WorldState::parse("format = 1\n[[instance]]\nnumber = 17\ngame_clock = 1.0\n");
        assert!(back.clocks.is_empty());
        assert_eq!(warn.len(), 1);
    }
}
