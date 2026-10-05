// SPDX-License-Identifier: GPL-3.0-or-later
//! Text forms of the character viewer's command-line options (`--character`, `--wear`,
//! `--clip`, `--time`, `--sliders`), so screenshots of the character view are
//! reproducible.
//!
//! This module depends on nothing but `std` (the fuzz target `explorer_character_spec`
//! includes it by path). Labels are kept as text here; the character view resolves them
//! against the install at run time (`RaceTable_Race` labels for species), so nothing in
//! this file names a game value.
//!
//! | Option | Grammar | Examples |
//! |---|---|---|
//! | `--character` | `SPECIES,SEX` or a DID (8 hex digits, prefix 0x47, 0x70–0x76 or 0x04) | `human,m`, `0x17,0x1000`, `riverhobbit,female`, `0x4700060A` |
//! | `--wear` | `DID[@OPTION]…`, DID prefix 0x70–0x76 or 0x79; options `dye=CODE`, `dye2=CODE`, `sheathed`, `wielded`, `slot=primary\|secondary\|ranged\|BITS` | `0x790006C3`, `0x7900066F@sheathed`, `0x79000420@dye=0.06` |
//! | `--clip` | `idle`, `walk`, `run`, `none` or a 0x05 DID | `walk`, `0x05008D08` |
//! | `--time` | seconds, finite, ≥ 0 | `0.5` |
//! | `--sliders` | hexadecimal byte pairs, one per visible slider | `80808080…` |
//!
//! Every parser bounds its input ([`MAX_ARG_LEN`], [`MAX_OPTIONS`]) and never panics.

use std::fmt;

/// Longest option text accepted.
pub const MAX_ARG_LEN: usize = 1024;
/// Most `@` options on one `--wear` item.
pub const MAX_OPTIONS: usize = 8;
/// Most items of a character (`--wear` given more often is refused).
pub const MAX_ITEMS: usize = 32;
/// Most slider bytes (`--sliders`).
pub const MAX_SLIDER_BYTES: usize = 256;
/// Largest `--time` in seconds.
pub const MAX_TIME: f32 = 86_400.0;
/// Largest dye code accepted: dye codes are small fractions (0.00 to 0.80 observed) and
/// 1.0 is the base look (`appearance.md` §4.9, `material.md` §12.4). The view's dye
/// control uses the same range.
pub const MAX_DYE_CODE: f32 = 1.0;
/// Longest species label.
pub const MAX_LABEL_LEN: usize = 64;

/// The male `RaceTable_GenderType` value (`appearance.md` §2.1).
pub const SEX_MALE: u32 = 0x1000;
/// The female `RaceTable_GenderType` value (`appearance.md` §2.1).
pub const SEX_FEMALE: u32 = 0x2000;

/// Why an option did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError(pub String);

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SpecError {}

fn err<T>(msg: impl Into<String>) -> Result<T, SpecError> {
    Err(SpecError(msg.into()))
}

fn bounded(s: &str) -> Result<&str, SpecError> {
    if s.len() > MAX_ARG_LEN {
        return err(format!("option text longer than {MAX_ARG_LEN} bytes"));
    }
    Ok(s.trim())
}

/// Parses a DID given as exactly 8 hex digits, with an optional `0x` and `_` separators.
#[must_use]
pub fn parse_did(s: &str) -> Option<u32> {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let mut v: u32 = 0;
    let mut n = 0usize;
    for c in t.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(16)?;
        n += 1;
        if n > 8 {
            return None;
        }
        v = (v << 4) | d;
    }
    (n == 8).then_some(v)
}

/// Parses a non-negative integer, decimal or `0x` hex (at most 16 hex or 20 decimal
/// digits; `_` separators allowed).
#[must_use]
pub fn parse_number(s: &str) -> Option<u64> {
    let t = s.trim();
    let (digits, radix) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => (h, 16),
        None => (t, 10),
    };
    let clean: String = digits.chars().filter(|c| *c != '_').collect();
    if clean.is_empty() || clean.len() > 20 || !clean.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u64::from_str_radix(&clean, radix).ok()
}

/// Parses a finite float (`f32`). At most 64 characters: enough for the shortest form
/// Rust prints of any `f32`, so printed values parse back.
fn parse_float(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    t.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// A species given on the command line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpeciesArg {
    /// A `SpeciesType` key.
    Key(u32),
    /// A `RaceTable_Race` label, matched case-insensitively against the data.
    Label(String),
}

impl fmt::Display for SpeciesArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(k) => write!(f, "{k:#x}"),
            Self::Label(l) => f.write_str(l),
        }
    }
}

/// What `--character` names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FigureArg {
    /// A playable body: species and `RaceTable_GenderType`.
    Playable {
        /// The species.
        species: SpeciesArg,
        /// [`SEX_MALE`], [`SEX_FEMALE`] or another gender value.
        sex: u32,
    },
    /// A placed figure: a 0x47 entity or a 0x70–0x76 game object (a weenie NPC).
    Object(u32),
    /// A bare 0x04 setup (its default parts and skeleton).
    Setup(u32),
}

impl fmt::Display for FigureArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Playable { species, sex } => {
                let s = match *sex {
                    SEX_MALE => "m".to_owned(),
                    SEX_FEMALE => "f".to_owned(),
                    other => format!("{other:#x}"),
                };
                write!(f, "{species},{s}")
            }
            Self::Object(d) | Self::Setup(d) => write!(f, "{d:#010x}"),
        }
    }
}

/// Parses `--character` (see the module table).
///
/// # Errors
///
/// A [`SpecError`] naming what is wrong.
pub fn parse_figure(s: &str) -> Result<FigureArg, SpecError> {
    let t = bounded(s)?;
    if t.is_empty() {
        return err("empty character (give SPECIES,SEX or a DID)");
    }
    if let Some((species, sex)) = t.split_once(',') {
        let species = species.trim();
        let sex = sex.trim();
        let species = match parse_number(species) {
            Some(k) => SpeciesArg::Key(
                u32::try_from(k).map_err(|_| SpecError(format!("species key too large: {k}")))?,
            ),
            None => {
                if species.is_empty()
                    || species.len() > MAX_LABEL_LEN
                    || !species
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return err(format!(
                        "not a species label or key: {species:?} (letters, digits, _ and -)"
                    ));
                }
                SpeciesArg::Label(species.to_owned())
            }
        };
        let sex = match sex.to_ascii_lowercase().as_str() {
            "m" | "male" => SEX_MALE,
            "f" | "female" => SEX_FEMALE,
            other => match parse_number(other).and_then(|v| u32::try_from(v).ok()) {
                Some(v) => v,
                None => return err(format!("not a sex: {sex:?} (m, f or a number)")),
            },
        };
        return Ok(FigureArg::Playable { species, sex });
    }
    let Some(did) = parse_did(t) else {
        return err(format!(
            "not a character: {t:?} (give SPECIES,SEX like human,m or an 8-digit DID)"
        ));
    };
    match did >> 24 {
        0x47 | 0x70..=0x76 => Ok(FigureArg::Object(did)),
        0x04 => Ok(FigureArg::Setup(did)),
        p => err(format!(
            "{did:#010x}: a character is a 0x47 entity, a 0x70–0x76 object or a 0x04 setup, \
             not a {p:#04x} record"
        )),
    }
}

/// The equipment slot an item is put in (`--wear …@slot=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotArg {
    /// `Weapon_Primary` (looked up in the data).
    Primary,
    /// `Weapon_Secondary`.
    Secondary,
    /// `Weapon_Ranged`.
    Ranged,
    /// Raw `ContainerSlot` bits.
    Bits(u64),
}

impl fmt::Display for SlotArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => f.write_str("primary"),
            Self::Secondary => f.write_str("secondary"),
            Self::Ranged => f.write_str("ranged"),
            Self::Bits(b) => write!(f, "{b:#x}"),
        }
    }
}

/// One `--wear` item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemArg {
    /// The item: a game object (0x70–0x76) or its property set (0x79).
    pub did: u32,
    /// First-channel dye code (`None`: the item's default).
    pub dye: Option<f32>,
    /// Second-channel dye code.
    pub dye2: Option<f32>,
    /// Stowed instead of in hand (for held items).
    pub sheathed: bool,
    /// The slot it is equipped in (`None`: its default slot).
    pub slot: Option<SlotArg>,
}

impl ItemArg {
    /// An item with its defaults.
    #[must_use]
    pub fn new(did: u32) -> Self {
        Self {
            did,
            dye: None,
            dye2: None,
            sheathed: false,
            slot: None,
        }
    }
}

impl fmt::Display for ItemArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.did)?;
        if let Some(d) = self.dye {
            write!(f, "@dye={d}")?;
        }
        if let Some(d) = self.dye2 {
            write!(f, "@dye2={d}")?;
        }
        if self.sheathed {
            f.write_str("@sheathed")?;
        }
        if let Some(s) = self.slot {
            write!(f, "@slot={s}")?;
        }
        Ok(())
    }
}

fn dye_code(v: &str, what: &str) -> Result<f32, SpecError> {
    match parse_float(v) {
        Some(c) if (0.0..=MAX_DYE_CODE).contains(&c) => Ok(c),
        _ => err(format!(
            "{what}: not a dye code: {v:?} (a number from 0 to {MAX_DYE_CODE})"
        )),
    }
}

/// Parses one `--wear` item (see the module table).
///
/// # Errors
///
/// A [`SpecError`] naming what is wrong.
pub fn parse_item(s: &str) -> Result<ItemArg, SpecError> {
    let t = bounded(s)?;
    let mut parts = t.split('@');
    let did_text = parts.next().unwrap_or_default();
    let Some(did) = parse_did(did_text) else {
        return err(format!("not an item DID: {did_text:?} (8 hex digits)"));
    };
    if !matches!(did >> 24, 0x70..=0x76 | 0x79) {
        return err(format!(
            "{did:#010x}: an item is a 0x70–0x76 game object or a 0x79 property set"
        ));
    }
    let mut item = ItemArg::new(did);
    for (n, opt) in parts.enumerate() {
        if n >= MAX_OPTIONS {
            return err(format!("more than {MAX_OPTIONS} options on one item"));
        }
        let opt = opt.trim();
        let (key, value) = match opt.split_once('=') {
            Some((k, v)) => (k.trim().to_ascii_lowercase(), Some(v.trim())),
            None => (opt.to_ascii_lowercase(), None),
        };
        match (key.as_str(), value) {
            ("dye", Some(v)) => item.dye = Some(dye_code(v, "dye")?),
            ("dye2", Some(v)) => item.dye2 = Some(dye_code(v, "dye2")?),
            ("sheathed" | "stowed", None) => item.sheathed = true,
            ("wielded", None) => item.sheathed = false,
            ("slot", Some(v)) => {
                item.slot = Some(match v.to_ascii_lowercase().as_str() {
                    "primary" => SlotArg::Primary,
                    "secondary" => SlotArg::Secondary,
                    "ranged" => SlotArg::Ranged,
                    other => match parse_number(other) {
                        Some(b) if b != 0 => SlotArg::Bits(b),
                        _ => {
                            return err(format!(
                                "not a slot: {v:?} (primary, secondary, ranged or bits)"
                            ));
                        }
                    },
                });
            }
            _ => {
                return err(format!(
                    "unknown item option {opt:?} (dye=, dye2=, sheathed, wielded, slot=)"
                ));
            }
        }
    }
    Ok(item)
}

/// The motion states whose clips the script tables select (`script-table.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateArg {
    /// Standing.
    Idle,
    /// Walking forward.
    Walk,
    /// Running forward.
    Run,
}

impl StateArg {
    /// Every state, in display order.
    pub const ALL: [Self; 3] = [Self::Idle, Self::Walk, Self::Run];

    /// The lowercase name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
        }
    }
}

/// What `--clip` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipArg {
    /// No clip: the reference pose.
    None,
    /// The clip the script tables select for a motion state.
    State(StateArg),
    /// A 0x05 clip.
    Did(u32),
}

impl fmt::Display for ClipArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::State(s) => f.write_str(s.name()),
            Self::Did(d) => write!(f, "{d:#010x}"),
        }
    }
}

/// Parses `--clip` (see the module table).
///
/// # Errors
///
/// A [`SpecError`] naming what is wrong.
pub fn parse_clip(s: &str) -> Result<ClipArg, SpecError> {
    let t = bounded(s)?;
    match t.to_ascii_lowercase().as_str() {
        "none" | "bind" | "reference" => return Ok(ClipArg::None),
        "idle" => return Ok(ClipArg::State(StateArg::Idle)),
        "walk" => return Ok(ClipArg::State(StateArg::Walk)),
        "run" => return Ok(ClipArg::State(StateArg::Run)),
        _ => {}
    }
    match parse_did(t) {
        Some(d) if d >> 24 == 0x05 => Ok(ClipArg::Did(d)),
        Some(d) => err(format!("{d:#010x} is not a 0x05 clip")),
        None => err(format!(
            "not a clip: {t:?} (idle, walk, run, none or a 0x05 DID)"
        )),
    }
}

/// Parses `--time` (seconds, finite, 0 to [`MAX_TIME`]).
///
/// # Errors
///
/// A [`SpecError`] for anything else.
pub fn parse_time(s: &str) -> Result<f32, SpecError> {
    let t = bounded(s)?;
    match parse_float(t) {
        Some(v) if (0.0..=MAX_TIME).contains(&v) => Ok(v),
        _ => err(format!("not a time: {t:?} (seconds from 0 to {MAX_TIME})")),
    }
}

/// Parses `--sliders`: hexadecimal byte pairs (at most [`MAX_SLIDER_BYTES`] bytes).
///
/// # Errors
///
/// A [`SpecError`] for an odd length, a non-hex digit or too many bytes.
pub fn parse_sliders(s: &str) -> Result<Vec<u8>, SpecError> {
    let t = bounded(s)?;
    let b = t.as_bytes();
    if !b.len().is_multiple_of(2) {
        return err("slider bytes need an even number of hex digits");
    }
    if b.len() / 2 > MAX_SLIDER_BYTES {
        return err(format!("more than {MAX_SLIDER_BYTES} slider bytes"));
    }
    b.chunks_exact(2)
        .map(|p| {
            let d = |c: u8| char::from(c).to_digit(16);
            match (d(p[0]), d(p[1])) {
                (Some(h), Some(l)) => Ok((h * 16 + l) as u8),
                _ => err("slider bytes must be hex digits"),
            }
        })
        .collect()
}

/// Most times of `--times`.
pub const MAX_TIMES: usize = 16;
/// Most camera angles of `--angles`.
pub const MAX_ANGLES: usize = 8;
/// Smallest and largest tile side of `--tile` (pixels).
pub const TILE_SIDES: (u32, u32) = (64, 2048);

/// Parses `--times`: comma-separated play times in seconds (at most [`MAX_TIMES`]).
///
/// # Errors
///
/// A [`SpecError`] for an empty list, a bad time or too many.
pub fn parse_times(s: &str) -> Result<Vec<f32>, SpecError> {
    let t = bounded(s)?;
    if t.is_empty() {
        return err("no times");
    }
    let mut out = Vec::new();
    for part in t.split(',') {
        if out.len() >= MAX_TIMES {
            return err(format!("more than {MAX_TIMES} times"));
        }
        out.push(parse_time(part)?);
    }
    Ok(out)
}

/// Parses `--angles`: `YAW,PITCH` pairs in degrees separated by `;` or `/` (at most
/// [`MAX_ANGLES`]); pitch within ±89°.
///
/// # Errors
///
/// A [`SpecError`] for an empty list, a bad pair or too many.
pub fn parse_angle_list(s: &str) -> Result<Vec<[f32; 2]>, SpecError> {
    let t = bounded(s)?;
    if t.is_empty() {
        return err("no angles");
    }
    let mut out = Vec::new();
    for pair in t.split([';', '/']) {
        if out.len() >= MAX_ANGLES {
            return err(format!("more than {MAX_ANGLES} angles"));
        }
        let Some((y, p)) = pair.split_once(',') else {
            return err(format!("not YAW,PITCH: {pair:?}"));
        };
        match (parse_float(y), parse_float(p)) {
            (Some(y), Some(p)) if y.abs() <= 3600.0 && p.abs() <= 89.0 => out.push([y, p]),
            _ => return err(format!("not YAW,PITCH in degrees: {pair:?}")),
        }
    }
    Ok(out)
}

/// Parses `--tile`: `WIDTHxHEIGHT` in pixels, each within [`TILE_SIDES`].
///
/// # Errors
///
/// A [`SpecError`] for anything else.
pub fn parse_tile(s: &str) -> Result<[u32; 2], SpecError> {
    let t = bounded(s)?;
    let Some((w, h)) = t.split_once(['x', 'X']) else {
        return err(format!("not WIDTHxHEIGHT: {t:?}"));
    };
    let side = |v: &str| {
        parse_number(v)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (TILE_SIDES.0..=TILE_SIDES.1).contains(v))
    };
    match (side(w), side(h)) {
        (Some(w), Some(h)) => Ok([w, h]),
        _ => err(format!(
            "tile sides must be {} to {} pixels: {t:?}",
            TILE_SIDES.0, TILE_SIDES.1
        )),
    }
}

/// Parses `--focus`: `HEIGHT,RADIUS` in metres (a sphere on the figure's vertical axis to
/// frame instead of the whole figure); height within ±100 m, radius 0.01 to 100 m.
///
/// # Errors
///
/// A [`SpecError`] for anything else.
pub fn parse_focus(s: &str) -> Result<[f32; 2], SpecError> {
    let t = bounded(s)?;
    let Some((z, r)) = t.split_once(',') else {
        return err(format!("not HEIGHT,RADIUS: {t:?}"));
    };
    match (parse_float(z), parse_float(r)) {
        (Some(z), Some(r)) if z.abs() <= 100.0 && (0.01..=100.0).contains(&r) => Ok([z, r]),
        _ => err(format!("not HEIGHT,RADIUS in metres: {t:?}")),
    }
}

/// The whole character set-up of a command line, in one value (what screenshot mode
/// reproduces).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterArgs {
    /// `--character`.
    pub figure: Option<FigureArg>,
    /// `--wear`, in order.
    pub items: Vec<ItemArg>,
    /// `--clip`.
    pub clip: Option<ClipArg>,
    /// `--time`: pause at this clip time.
    pub time: Option<f32>,
    /// `--sliders`.
    pub sliders: Option<Vec<u8>>,
    /// `--bones`: draw the bone overlay.
    pub bones: bool,
    /// `--trail`: draw the root-motion trail.
    pub trail: bool,
}

impl CharacterArgs {
    /// Whether any character option was given.
    #[must_use]
    pub fn any(&self) -> bool {
        self.figure.is_some()
            || !self.items.is_empty()
            || self.clip.is_some()
            || self.time.is_some()
            || self.sliders.is_some()
            || self.bones
            || self.trail
    }

    /// The command-line form of these options (the inverse of the parsers), for notes and
    /// checklists.
    #[must_use]
    pub fn to_command_line(&self) -> String {
        let mut out = Vec::new();
        if let Some(f) = &self.figure {
            out.push(format!("--character {f}"));
        }
        for i in &self.items {
            out.push(format!("--wear {i}"));
        }
        if let Some(c) = self.clip {
            out.push(format!("--clip {c}"));
        }
        if let Some(t) = self.time {
            out.push(format!("--time {t}"));
        }
        if let Some(s) = &self.sliders {
            let hex: String = s.iter().map(|b| format!("{b:02x}")).collect();
            out.push(format!("--sliders {hex}"));
        }
        if self.bones {
            out.push("--bones".into());
        }
        if self.trail {
            out.push("--trail".into());
        }
        out.join(" ")
    }

    /// Parses the options from command-line words (`--character X --wear Y …`), ignoring
    /// words that are not character options. Used by the fuzz target and tests.
    ///
    /// # Errors
    ///
    /// The first [`SpecError`] of an option, or a missing value.
    pub fn parse_words<'a>(words: impl IntoIterator<Item = &'a str>) -> Result<Self, SpecError> {
        let mut out = Self::default();
        let mut it = words.into_iter();
        let mut n = 0usize;
        while let Some(w) = it.next() {
            n += 1;
            if n > 4 * MAX_ITEMS + 16 {
                return err("too many words");
            }
            let mut value = |name: &str| {
                it.next()
                    .ok_or_else(|| SpecError(format!("{name} needs a value")))
            };
            match w {
                "--character" => out.figure = Some(parse_figure(value(w)?)?),
                "--wear" => {
                    if out.items.len() >= MAX_ITEMS {
                        return err(format!("more than {MAX_ITEMS} items"));
                    }
                    out.items.push(parse_item(value(w)?)?);
                }
                "--clip" => out.clip = Some(parse_clip(value(w)?)?),
                "--time" => out.time = Some(parse_time(value(w)?)?),
                "--sliders" => out.sliders = Some(parse_sliders(value(w)?)?),
                "--bones" => out.bones = true,
                "--trail" => out.trail = true,
                _ => {}
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures() {
        assert_eq!(
            parse_figure("human,m"),
            Ok(FigureArg::Playable {
                species: SpeciesArg::Label("human".into()),
                sex: SEX_MALE
            })
        );
        assert_eq!(
            parse_figure(" 0x17 , Female "),
            Ok(FigureArg::Playable {
                species: SpeciesArg::Key(0x17),
                sex: SEX_FEMALE
            })
        );
        assert_eq!(
            parse_figure("RiverHobbit,0x2000"),
            Ok(FigureArg::Playable {
                species: SpeciesArg::Label("RiverHobbit".into()),
                sex: SEX_FEMALE
            })
        );
        assert_eq!(
            parse_figure("0x4700060A"),
            Ok(FigureArg::Object(0x4700_060A))
        );
        assert_eq!(parse_figure("70006986"), Ok(FigureArg::Object(0x7000_6986)));
        assert_eq!(
            parse_figure("0x0400_0553"),
            Ok(FigureArg::Setup(0x0400_0553))
        );
        for bad in [
            "",
            "human",
            "human,x",
            "hu man,m",
            ",m",
            "0x0500_0001",
            "0x47",
            "0x99999999999,m",
        ] {
            assert!(parse_figure(bad).is_err(), "{bad}");
        }
        assert!(parse_figure(&"a".repeat(MAX_ARG_LEN + 1)).is_err());
    }

    #[test]
    fn items() {
        let i = parse_item("0x790006C3").unwrap();
        assert_eq!(i, ItemArg::new(0x7900_06C3));
        let i = parse_item("7900066F@sheathed@slot=secondary@dye=0.06@dye2=1").unwrap();
        assert!(i.sheathed);
        assert_eq!(i.slot, Some(SlotArg::Secondary));
        assert_eq!(i.dye, Some(0.06));
        assert_eq!(i.dye2, Some(1.0));
        assert_eq!(
            parse_item("0x7900066F@slot=0x20000").unwrap().slot,
            Some(SlotArg::Bits(0x20000))
        );
        for bad in [
            "",
            "0x0600_0001",
            "0x7900066F@dye",
            "0x7900066F@dye=nan",
            "0x7900066F@dye=-1",
            "0x7900066F@dye=1.5",
            // 0x77 and 0x78 are no items (0x78 is the slider-definition set).
            "0x7800A9F1",
            "0x77000001",
            "0x7900066F@slot=0",
            "0x7900066F@slot=left",
            "0x7900066F@colour=1",
            "0x7900066F@a@a@a@a@a@a@a@a@a",
        ] {
            assert!(parse_item(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn clips_times_sliders() {
        assert_eq!(parse_clip("Walk"), Ok(ClipArg::State(StateArg::Walk)));
        assert_eq!(parse_clip("none"), Ok(ClipArg::None));
        assert_eq!(parse_clip("0x05008D08"), Ok(ClipArg::Did(0x0500_8D08)));
        assert!(parse_clip("0x06008D08").is_err());
        assert!(parse_clip("jump").is_err());
        assert_eq!(parse_time("0.25"), Ok(0.25));
        assert!(parse_time("-1").is_err());
        assert!(parse_time("inf").is_err());
        assert!(parse_time("1e9").is_err());
        assert_eq!(parse_sliders("80ff00"), Ok(vec![0x80, 0xFF, 0]));
        assert!(parse_sliders("8").is_err());
        assert!(parse_sliders("zz").is_err());
        assert!(parse_sliders(&"80".repeat(MAX_SLIDER_BYTES + 1)).is_err());
    }

    #[test]
    fn sheet_options() {
        assert_eq!(parse_times("0,0.5, 1.25"), Ok(vec![0.0, 0.5, 1.25]));
        assert!(parse_times("").is_err());
        assert!(parse_times("0,,1").is_err());
        assert!(parse_times(&vec!["1"; MAX_TIMES + 1].join(",")).is_err());
        assert_eq!(
            parse_angle_list("160,10;90,0/-30,45"),
            Ok(vec![[160.0, 10.0], [90.0, 0.0], [-30.0, 45.0]])
        );
        assert!(parse_angle_list("160").is_err());
        assert!(parse_angle_list("160,95").is_err());
        assert!(parse_angle_list("nan,0").is_err());
        assert_eq!(parse_tile("480x640"), Ok([480, 640]));
        assert_eq!(parse_focus("1.6,0.3"), Ok([1.6, 0.3]));
        assert!(parse_focus("1.6").is_err());
        assert!(parse_focus("1.6,0").is_err());
        assert!(parse_focus("1e9,1").is_err());
        assert!(parse_tile("10x640").is_err());
        assert!(parse_tile("480").is_err());
        assert!(parse_tile("99999x640").is_err());
    }

    #[test]
    fn words_round_trip() {
        let words = [
            "--select",
            "0x28000000",
            "--character",
            "human,m",
            "--wear",
            "0x790006C3@dye=0.5",
            "--wear",
            "0x7900066F@sheathed",
            "--clip",
            "run",
            "--time",
            "0.4",
            "--bones",
        ];
        let a = CharacterArgs::parse_words(words).unwrap();
        assert!(a.any());
        assert_eq!(a.items.len(), 2);
        let line = a.to_command_line();
        let again = CharacterArgs::parse_words(line.split(' ')).unwrap();
        assert_eq!(a, again);
        assert!(CharacterArgs::parse_words(["--clip"]).is_err());
        // Regression (fuzzing): tiny times print long and must parse back.
        let tiny = CharacterArgs {
            time: Some(1e-40),
            ..CharacterArgs::default()
        };
        let line = tiny.to_command_line();
        assert_eq!(CharacterArgs::parse_words(line.split(' ')), Ok(tiny));
        assert!(!CharacterArgs::default().any());
    }
}
