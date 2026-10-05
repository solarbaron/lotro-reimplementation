// SPDX-License-Identifier: GPL-3.0-or-later
//! The text form of the walker's `--avatar` option: the body and equipment of the
//! player's character (`walker.md` §10).
//!
//! ```text
//! AVATAR = BODY *( ";" ITEM )
//! BODY   = SPECIES "," SEX            playable body (race table)
//!        | DID                         a placed figure's object: 0x47 entity or 0x70–0x76 game object
//! SPECIES = label | number            a `RaceTable_Race` label (letters, digits, _ and -) or a key
//! SEX    = "m" | "male" | "f" | "female" | number
//! ITEM   = DID *( "@" OPTION )        DID prefix 0x70–0x76 or 0x79
//! OPTION = "dye=" CODE | "dye2=" CODE | "stowed" | "wielded"
//! ```
//!
//! Examples: `human,m` (the default), `elf,f;0x79006BF8@wielded`,
//! `0x17,0x1000;0x79000420@dye=0.06;0x790006C3`, `0x4700005B`.
//!
//! Held items are stowed unless `@wielded` (`appearance.md` §5.4 Decision: out of combat,
//! held items are stowed by category, and the walker's avatar is never in combat).
//!
//! This module depends on `std` only, so the fuzz target `client_avatar_spec` includes it
//! by path. Labels stay text here; the walker resolves them against the install's race
//! table at run time (no game value is named in this file). Every parser bounds its
//! input ([`MAX_AVATAR_LEN`], [`MAX_AVATAR_ITEMS`], [`MAX_ITEM_OPTIONS`]) and never panics.

use std::fmt;

/// Longest `--avatar` text accepted.
pub const MAX_AVATAR_LEN: usize = 1024;
/// Most items of one avatar (equal to `tessera_net::model::MAX_EQUIPMENT`).
pub const MAX_AVATAR_ITEMS: usize = 32;
/// Most `@` options of one item.
pub const MAX_ITEM_OPTIONS: usize = 8;
/// Longest species label.
pub const MAX_LABEL_LEN: usize = 64;
/// Largest dye code (equal to `tessera_net::model::MAX_DYE_CODE`).
pub const MAX_DYE: f32 = 1.0;
/// The male sex value of the race table (`appearance.md` §2.1).
pub const SEX_MALE: u32 = 0x1000;
/// The female sex value of the race table (`appearance.md` §2.1).
pub const SEX_FEMALE: u32 = 0x2000;
/// The avatar without `--avatar`: the species label of body P3 of `appearance.md` §2.2,
/// male.
pub const DEFAULT_AVATAR: &str = "human,m";

/// Why an `--avatar` text did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSpecError(pub String);

impl fmt::Display for AvatarSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AvatarSpecError {}

fn err<T>(msg: impl Into<String>) -> Result<T, AvatarSpecError> {
    Err(AvatarSpecError(msg.into()))
}

/// A species given as text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Species {
    /// A species key.
    Key(u32),
    /// A `RaceTable_Race` label, matched case-insensitively against the data.
    Label(String),
}

/// The body of an avatar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AvatarBody {
    /// A playable body.
    Playable {
        /// The species.
        species: Species,
        /// The race table's sex value ([`SEX_MALE`], [`SEX_FEMALE`] or another).
        sex: u32,
    },
    /// A 0x47 entity or a 0x70–0x76 game object.
    Object(u32),
}

/// One equipped item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvatarItem {
    /// The item (prefix 0x70–0x76 or 0x79).
    pub did: u32,
    /// First dye code, `0 ..= MAX_DYE`.
    pub dye: Option<f32>,
    /// Second dye code, `0 ..= MAX_DYE`.
    pub dye2: Option<f32>,
    /// Held items: stowed (the default, `appearance.md` §5.4) instead of in the hand
    /// (`@wielded`).
    pub stowed: bool,
}

/// A parsed `--avatar` value.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarSpec {
    /// The body.
    pub body: AvatarBody,
    /// Items in the order given.
    pub items: Vec<AvatarItem>,
}

impl Default for AvatarSpec {
    fn default() -> Self {
        Self {
            body: AvatarBody::Playable {
                species: Species::Label("human".into()),
                sex: SEX_MALE,
            },
            items: Vec::new(),
        }
    }
}

/// Parses a DID of exactly 8 hex digits, optionally prefixed `0x` and with `_` separators.
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

/// Parses a non-negative integer: decimal, or hexadecimal with `0x` (at most 10 decimal or
/// 8 hex significant digits after leading zeros, `_` allowed). Longer input is rejected,
/// never truncated.
fn parse_u32(s: &str) -> Option<u32> {
    let t = s.trim();
    let (digits, radix, max) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => (h, 16, 8),
        None => (t, 10, 10),
    };
    let mut v: u64 = 0;
    let mut any = false;
    let mut significant = 0usize;
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(radix)?;
        any = true;
        if significant == 0 && d == 0 {
            continue;
        }
        significant += 1;
        if significant > max {
            return None;
        }
        v = v * u64::from(radix) + u64::from(d);
    }
    if !any {
        return None;
    }
    u32::try_from(v).ok()
}

/// Whether `s` is written as a number (decimal digits, or `0x` and hex digits, with `_`).
fn looks_numeric(s: &str) -> bool {
    let (digits, radix) = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => (h, 16),
        None => (s, 10),
    };
    digits.chars().any(|c| c != '_') && digits.chars().all(|c| c == '_' || c.is_digit(radix))
}

/// Longest dye code text: covers every `f32` in `0 ..= MAX_DYE` as `Display` prints it
/// (the smallest normal value prints with 48 characters), so a printed spec parses again.
const MAX_DYE_TEXT: usize = 64;

fn parse_dye(s: &str) -> Result<f32, AvatarSpecError> {
    let t = s.trim();
    if t.is_empty() || t.len() > MAX_DYE_TEXT {
        return err(format!("not a dye code: {t:?}"));
    }
    match t.parse::<f32>() {
        Ok(v) if v.is_finite() && (0.0..=MAX_DYE).contains(&v) => Ok(v),
        _ => err(format!("dye code must be a number in 0..={MAX_DYE}: {t:?}")),
    }
}

fn parse_body(s: &str) -> Result<AvatarBody, AvatarSpecError> {
    let t = s.trim();
    if t.is_empty() {
        return err("empty body (give SPECIES,SEX or a DID)");
    }
    if let Some((species, sex)) = t.split_once(',') {
        let (species, sex) = (species.trim(), sex.trim());
        let species = match parse_u32(species) {
            Some(k) => Species::Key(k),
            None if looks_numeric(species) => {
                return err(format!(
                    "species key out of range: {species:?} (at most 0xFFFFFFFF)"
                ));
            }
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
                Species::Label(species.to_owned())
            }
        };
        let sex = match sex.to_ascii_lowercase().as_str() {
            "m" | "male" => SEX_MALE,
            "f" | "female" => SEX_FEMALE,
            other => parse_u32(other)
                .ok_or_else(|| AvatarSpecError(format!("not a sex: {sex:?} (m, f or a value)")))?,
        };
        return Ok(AvatarBody::Playable { species, sex });
    }
    let did = parse_did(t).ok_or_else(|| {
        AvatarSpecError(format!("not a body: {t:?} (SPECIES,SEX or an 8-digit DID)"))
    })?;
    match did >> 24 {
        0x47 | 0x70..=0x76 => Ok(AvatarBody::Object(did)),
        p => err(format!(
            "{did:#010x}: a body DID must be a 0x47 entity or a 0x70–0x76 game object, not prefix {p:#04x}"
        )),
    }
}

fn parse_item(s: &str) -> Result<AvatarItem, AvatarSpecError> {
    let mut parts = s.split('@');
    let did_text = parts.next().unwrap_or("");
    let did = parse_did(did_text)
        .ok_or_else(|| AvatarSpecError(format!("not an item DID: {:?}", did_text.trim())))?;
    if !matches!(did >> 24, 0x70..=0x76 | 0x79) {
        return err(format!(
            "{did:#010x}: an item must be a 0x70–0x76 game object or its 0x79 property set"
        ));
    }
    let mut item = AvatarItem {
        did,
        dye: None,
        dye2: None,
        stowed: true,
    };
    for (n, opt) in parts.enumerate() {
        if n >= MAX_ITEM_OPTIONS {
            return err(format!("more than {MAX_ITEM_OPTIONS} options on one item"));
        }
        let o = opt.trim();
        if let Some(v) = o.strip_prefix("dye2=") {
            item.dye2 = Some(parse_dye(v)?);
        } else if let Some(v) = o.strip_prefix("dye=") {
            item.dye = Some(parse_dye(v)?);
        } else if o.eq_ignore_ascii_case("stowed") || o.eq_ignore_ascii_case("sheathed") {
            item.stowed = true;
        } else if o.eq_ignore_ascii_case("wielded") {
            item.stowed = false;
        } else {
            return err(format!(
                "unknown item option {o:?} (dye=, dye2=, stowed, wielded)"
            ));
        }
    }
    Ok(item)
}

/// Parses an `--avatar` value (module grammar).
///
/// # Errors
///
/// An [`AvatarSpecError`] naming the problem.
pub fn parse_avatar(s: &str) -> Result<AvatarSpec, AvatarSpecError> {
    if s.len() > MAX_AVATAR_LEN {
        return err(format!("--avatar longer than {MAX_AVATAR_LEN} bytes"));
    }
    let mut fields = s.split(';');
    let body = parse_body(fields.next().unwrap_or(""))?;
    let mut items = Vec::new();
    for f in fields {
        if f.trim().is_empty() {
            continue;
        }
        if items.len() >= MAX_AVATAR_ITEMS {
            return err(format!("more than {MAX_AVATAR_ITEMS} items"));
        }
        items.push(parse_item(f)?);
    }
    Ok(AvatarSpec { body, items })
}

impl fmt::Display for AvatarSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.body {
            AvatarBody::Playable { species, sex } => {
                match species {
                    Species::Key(k) => write!(f, "{k:#x}")?,
                    Species::Label(l) => f.write_str(l)?,
                }
                match *sex {
                    SEX_MALE => f.write_str(",m")?,
                    SEX_FEMALE => f.write_str(",f")?,
                    other => write!(f, ",{other:#x}")?,
                }
            }
            AvatarBody::Object(d) => write!(f, "{d:#010x}")?,
        }
        for i in &self.items {
            write!(f, ";{:#010x}", i.did)?;
            if let Some(d) = i.dye {
                write!(f, "@dye={d}")?;
            }
            if let Some(d) = i.dye2 {
                write!(f, "@dye2={d}")?;
            }
            if !i.stowed {
                f.write_str("@wielded")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies() {
        assert_eq!(parse_avatar(DEFAULT_AVATAR).unwrap(), AvatarSpec::default());
        let a = parse_avatar(" Elf , female ").unwrap();
        assert_eq!(
            a.body,
            AvatarBody::Playable {
                species: Species::Label("Elf".into()),
                sex: SEX_FEMALE
            }
        );
        let a = parse_avatar("0x17,0x1000").unwrap();
        assert_eq!(
            a.body,
            AvatarBody::Playable {
                species: Species::Key(0x17),
                sex: SEX_MALE
            }
        );
        // Leading zeros do not count against the digit limit and are never truncated.
        for (text, key) in [
            ("0000000000001,m", 1),
            ("0x0000000000017,m", 0x17),
            ("00000000000017,m", 17),
            ("0x0000_0017,m", 0x17),
            ("4294967295,m", u32::MAX),
            ("0,m", 0),
            ("0x0,m", 0),
        ] {
            assert_eq!(
                parse_avatar(text).unwrap().body,
                AvatarBody::Playable {
                    species: Species::Key(key),
                    sex: SEX_MALE
                },
                "{text}"
            );
        }
        assert_eq!(
            parse_avatar("0x4700_005B").unwrap().body,
            AvatarBody::Object(0x4700_005B)
        );
        for bad in [
            "",
            ";",
            "human",
            "hu man,m",
            "human,x",
            "0x0400003E",
            "0x47",
            ",m",
            "human,0x1_0000_0000",
            "human,4294967296",
            "0x123456789,m",
            "12345678901,m",
        ] {
            assert!(parse_avatar(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn items() {
        let a =
            parse_avatar("human,m;0x79006BF8@stowed;0x70000001@dye=0.06@dye2=1@wielded;").unwrap();
        assert_eq!(a.items.len(), 2);
        assert!(!a.items[1].stowed, "@wielded");
        assert!(
            parse_avatar("human,m;0x79006BF8").unwrap().items[0].stowed,
            "default"
        );
        assert_eq!(
            a.items[0],
            AvatarItem {
                did: 0x7900_6BF8,
                dye: None,
                dye2: None,
                stowed: true
            }
        );
        assert_eq!(a.items[1].dye, Some(0.06));
        assert_eq!(a.items[1].dye2, Some(1.0));
        for bad in [
            "human,m;0x06000001",
            "human,m;zz",
            "human,m;0x79000001@dye=2",
            "human,m;0x79000001@dye=NaN",
            "human,m;0x79000001@glow",
            "human,m;0x79000001@a@b@c@d@e@f@g@h@i",
        ] {
            assert!(parse_avatar(bad).is_err(), "{bad:?}");
        }
        let many = format!("human,m{}", ";0x79000001".repeat(MAX_AVATAR_ITEMS + 1));
        assert!(parse_avatar(&many).is_err());
        assert!(parse_avatar(&"x".repeat(MAX_AVATAR_LEN + 1)).is_err());
    }

    #[test]
    fn display_round_trips() {
        for s in [
            "human,m",
            "elf,f;0x79006bf8@stowed",
            "elf,f;0x79006bf8@wielded",
            "0x41,0x3;0x70000001@dye=0.06@dye2=0.5",
            "0x4700005b",
            // Tiny codes print without an exponent (longer than the input).
            "human,m;0x79000001@dye=1e-31",
            "human,m;0x79000001@dye=1e-40@dye2=1e-45",
            "human,m;0x79000001@dye=1.1754942e-38",
        ] {
            let a = parse_avatar(s).unwrap();
            assert_eq!(parse_avatar(&a.to_string()).unwrap(), a, "{s}");
        }
        for d in [
            f32::MIN_POSITIVE,
            f32::from_bits(1),
            1.0 - f32::EPSILON,
            0.1,
            1.0,
        ] {
            let text = d.to_string();
            assert!(text.len() <= MAX_DYE_TEXT, "{text}");
            assert_eq!(parse_dye(&text), Ok(d), "{text}");
        }
    }
}
