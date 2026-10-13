// SPDX-License-Identifier: GPL-3.0-or-later
//! The mount choice as text (`walker.md` §16.2 grammar, §16.3 `/ride` arguments).
//!
//! ```text
//! MOUNT  = SOURCE *( "@" OPTION )
//! SOURCE = DID | "dir:" INDEX | "steed:" TREE | "auto"
//! DID    = "0x" 1*8HEXDIG | 1*10DIGIT          (at most 0xFFFFFFFF)
//! INDEX  = 1*4DIGIT                            (0 … 9,999)
//! TREE   = "light" | "medium" | "heavy"
//! OPTION = "summon" | "instant" | "lock=" GAIT
//! GAIT   = "walk" | "trot" | "canter" | "gallop" | "4" | "5" | "6" | "7"
//! ```
//!
//! Hostile input: at most [`MAX_SPEC_LEN`] bytes after trimming ASCII whitespace at both
//! ends; printable ASCII without spaces inside; keywords without regard to ASCII case; at
//! most [`MAX_OPTIONS`] options, each kind at most once, `summon` and `instant` not
//! together. This module depends on `std` only (the fuzz target `client_mount_spec`
//! includes it by path), never panics and allocates at most in proportion to the input.

use std::fmt;

/// Longest accepted text, bytes, after trimming.
pub const MAX_SPEC_LEN: usize = 128;
/// Most options.
pub const MAX_OPTIONS: usize = 4;
/// Highest `dir:` index.
pub const MAX_INDEX: u16 = 9_999;
/// Highest `/ride list` page.
pub const MAX_PAGE: u16 = 999;

/// A war-steed tree (`mounts.md` §14.10), by its walker keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SteedTree {
    /// `light`.
    Light,
    /// `medium`.
    Medium,
    /// `heavy`.
    Heavy,
}

impl SteedTree {
    /// The keyword.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
        }
    }

    /// The suffix of the install's tree label (`MountedCombat_Light`, …) this keyword
    /// matches, compared without regard to ASCII case.
    #[must_use]
    pub fn label_suffix(self) -> &'static str {
        self.keyword()
    }
}

/// Where the mount comes from (`walker.md` §16.2 SOURCE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountSource {
    /// A mount effect, or a skill whose `Skill_Toggle_Effect_List` names one.
    Did(u32),
    /// The INDEX-th (0-based) mount of the body's offered list.
    Dir(u16),
    /// The body's war-steed of a tree.
    Steed(SteedTree),
    /// `dir:0`.
    Auto,
}

/// How the mount starts (`walker.md` §16.2 OPTION, default timing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MountTiming {
    /// The caller's default: `instant` from the command line, `summon` from the chat
    /// command and the key.
    #[default]
    Default,
    /// Start with the skill's induction (`summon`).
    Summon,
    /// Start mounted (`instant`).
    Instant,
}

/// A parsed mount choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountSpec {
    /// The source.
    pub source: MountSource,
    /// The timing.
    pub timing: MountTiming,
    /// `lock=`: the fixed gear of a war-steed (a gait key 4–7).
    pub lock: Option<u32>,
}

impl MountSpec {
    /// A spec for `source` with default timing and no lock.
    #[must_use]
    pub fn new(source: MountSource) -> Self {
        Self {
            source,
            timing: MountTiming::Default,
            lock: None,
        }
    }
}

/// Why a text is not a mount choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// Nothing after trimming.
    Empty,
    /// Longer than [`MAX_SPEC_LEN`].
    TooLong,
    /// A character that is not printable ASCII (or a space inside).
    BadCharacter,
    /// An empty part between `@` signs.
    EmptyPart,
    /// The source is not one of the forms.
    BadSource,
    /// A DID above 0xFFFFFFFF or with too many digits.
    BadDid,
    /// An index above [`MAX_INDEX`] or with too many digits.
    BadIndex,
    /// Not a tree keyword.
    BadTree,
    /// Not an option.
    BadOption,
    /// Not a gait of `lock=`.
    BadGait,
    /// More than [`MAX_OPTIONS`] options.
    TooManyOptions,
    /// An option kind given twice.
    Repeated,
    /// `summon` and `instant` together.
    SummonAndInstant,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "empty mount choice",
            Self::TooLong => "mount choice longer than 128 bytes",
            Self::BadCharacter => "mount choice has a character that is not printable ASCII",
            Self::EmptyPart => "empty part between '@'",
            Self::BadSource => "source is not a DID, dir:INDEX, steed:TREE or auto",
            Self::BadDid => "DID above 0xFFFFFFFF or with too many digits",
            Self::BadIndex => "index above 9999",
            Self::BadTree => "tree is not light, medium or heavy",
            Self::BadOption => "option is not summon, instant or lock=GAIT",
            Self::BadGait => "gait is not walk, trot, canter, gallop or 4-7",
            Self::TooManyOptions => "more than 4 options",
            Self::Repeated => "an option is given twice",
            Self::SummonAndInstant => "summon and instant together",
        })
    }
}

impl std::error::Error for SpecError {}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| s.get(prefix.len()..))
        .flatten()
}

/// Parses a DID: `0x` and 1–8 hex digits, or 1–10 decimal digits, at most 0xFFFFFFFF.
///
/// # Errors
///
/// [`SpecError::BadDid`].
pub fn parse_did_text(s: &str) -> Result<u32, SpecError> {
    if let Some(hex) = strip_prefix_ci(s, "0x") {
        if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SpecError::BadDid);
        }
        return u32::from_str_radix(hex, 16).map_err(|_| SpecError::BadDid);
    }
    if s.is_empty() || s.len() > 10 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SpecError::BadDid);
    }
    let v: u64 = s.parse().map_err(|_| SpecError::BadDid)?;
    u32::try_from(v).map_err(|_| SpecError::BadDid)
}

/// The gait key of a `lock=` value.
///
/// # Errors
///
/// [`SpecError::BadGait`].
pub fn parse_gait(s: &str) -> Result<u32, SpecError> {
    const NAMES: [(&str, u32); 4] = [("walk", 4), ("trot", 5), ("canter", 6), ("gallop", 7)];
    if let Some((_, k)) = NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(s)) {
        return Ok(*k);
    }
    match s {
        "4" => Ok(4),
        "5" => Ok(5),
        "6" => Ok(6),
        "7" => Ok(7),
        _ => Err(SpecError::BadGait),
    }
}

fn parse_source(s: &str) -> Result<MountSource, SpecError> {
    if s.eq_ignore_ascii_case("auto") {
        return Ok(MountSource::Auto);
    }
    if let Some(i) = strip_prefix_ci(s, "dir:") {
        if i.is_empty() || i.len() > 4 || !i.bytes().all(|b| b.is_ascii_digit()) {
            return Err(SpecError::BadIndex);
        }
        let v: u16 = i.parse().map_err(|_| SpecError::BadIndex)?;
        if v > MAX_INDEX {
            return Err(SpecError::BadIndex);
        }
        return Ok(MountSource::Dir(v));
    }
    if let Some(t) = strip_prefix_ci(s, "steed:") {
        return [SteedTree::Light, SteedTree::Medium, SteedTree::Heavy]
            .into_iter()
            .find(|k| k.keyword().eq_ignore_ascii_case(t))
            .map(MountSource::Steed)
            .ok_or(SpecError::BadTree);
    }
    if s.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
        return parse_did_text(s).map(MountSource::Did);
    }
    Err(SpecError::BadSource)
}

/// Parses a mount choice (module docs).
///
/// # Errors
///
/// [`SpecError`] for text outside the grammar or the limits.
pub fn parse_mount(text: &str) -> Result<MountSpec, SpecError> {
    let t = text.trim_matches(|c: char| c.is_ascii_whitespace());
    if t.is_empty() {
        return Err(SpecError::Empty);
    }
    if t.len() > MAX_SPEC_LEN {
        return Err(SpecError::TooLong);
    }
    if !t.bytes().all(|b| (0x21..=0x7E).contains(&b)) {
        return Err(SpecError::BadCharacter);
    }
    if t.bytes().filter(|b| *b == b'@').count() > MAX_OPTIONS {
        return Err(SpecError::TooManyOptions);
    }
    let mut parts = t.split('@');
    let source = parts.next().ok_or(SpecError::Empty)?;
    if source.is_empty() {
        return Err(SpecError::EmptyPart);
    }
    let mut spec = MountSpec::new(parse_source(source)?);
    let mut count = 0usize;
    for part in parts {
        count += 1;
        if count > MAX_OPTIONS {
            return Err(SpecError::TooManyOptions);
        }
        if part.is_empty() {
            return Err(SpecError::EmptyPart);
        }
        if part.eq_ignore_ascii_case("summon") || part.eq_ignore_ascii_case("instant") {
            let want = if part.eq_ignore_ascii_case("summon") {
                MountTiming::Summon
            } else {
                MountTiming::Instant
            };
            match spec.timing {
                MountTiming::Default => spec.timing = want,
                t if t == want => return Err(SpecError::Repeated),
                _ => return Err(SpecError::SummonAndInstant),
            }
        } else if let Some(g) = strip_prefix_ci(part, "lock=") {
            if spec.lock.is_some() {
                return Err(SpecError::Repeated);
            }
            spec.lock = Some(parse_gait(g)?);
        } else {
            return Err(SpecError::BadOption);
        }
    }
    Ok(spec)
}

impl fmt::Display for MountSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source {
            MountSource::Did(d) => write!(f, "{d:#010x}")?,
            MountSource::Dir(i) => write!(f, "dir:{i}")?,
            MountSource::Steed(t) => write!(f, "steed:{}", t.keyword())?,
            MountSource::Auto => f.write_str("auto")?,
        }
        match self.timing {
            MountTiming::Default => {}
            MountTiming::Summon => f.write_str("@summon")?,
            MountTiming::Instant => f.write_str("@instant")?,
        }
        if let Some(g) = self.lock {
            write!(f, "@lock={g}")?;
        }
        Ok(())
    }
}

/// The label of gait key `gait` of the enum `MountedMovementSpeed` (`game-controls.md`
/// §5.3.2: 1 Backwards, 2 Neutral, 3 Neutral_Forwards, 4 Walk, 5 Trot, 6 Canter, 7 Gallop),
/// `None` for another key (0: unset).
#[must_use]
pub fn gait_label(gait: u32) -> Option<&'static str> {
    Some(match gait {
        1 => "Backwards",
        2 => "Neutral",
        3 => "Neutral_Forwards",
        4 => "Walk",
        5 => "Trot",
        6 => "Canter",
        7 => "Gallop",
        _ => return None,
    })
}

/// The arguments of `/ride` (`walker.md` §16.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideCommand {
    /// `/ride`: toggle.
    Toggle,
    /// `/ride SPEC`.
    Mount(MountSpec),
    /// `/ride off`.
    Off,
    /// `/ride list [PAGE]` (1-based, default 1).
    List(u16),
}

/// Parses the argument text of `/ride` (everything after the command name).
///
/// # Errors
///
/// [`SpecError`] for a SPEC that does not parse, or a page outside 1 … [`MAX_PAGE`]
/// ([`SpecError::BadIndex`]).
pub fn parse_ride(args: &str) -> Result<RideCommand, SpecError> {
    let t = args.trim_matches(|c: char| c.is_ascii_whitespace());
    if t.is_empty() {
        return Ok(RideCommand::Toggle);
    }
    if t.len() > MAX_SPEC_LEN {
        return Err(SpecError::TooLong);
    }
    if t.eq_ignore_ascii_case("off") {
        return Ok(RideCommand::Off);
    }
    let mut words = t.split_ascii_whitespace();
    if words.next().is_some_and(|w| w.eq_ignore_ascii_case("list")) {
        let page = match (words.next(), words.next()) {
            (None, _) => 1,
            (Some(p), None) => {
                if p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(SpecError::BadIndex);
                }
                let v: u16 = p.parse().map_err(|_| SpecError::BadIndex)?;
                if !(1..=MAX_PAGE).contains(&v) {
                    return Err(SpecError::BadIndex);
                }
                v
            }
            _ => return Err(SpecError::BadOption),
        };
        return Ok(RideCommand::List(page));
    }
    parse_mount(t).map(RideCommand::Mount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_of_the_grammar() {
        assert_eq!(parse_mount("auto"), Ok(MountSpec::new(MountSource::Auto)));
        assert_eq!(
            parse_mount("dir:12@summon"),
            Ok(MountSpec {
                source: MountSource::Dir(12),
                timing: MountTiming::Summon,
                lock: None
            })
        );
        assert_eq!(
            parse_mount(" steed:Medium@LOCK=canter "),
            Ok(MountSpec {
                source: MountSource::Steed(SteedTree::Medium),
                timing: MountTiming::Default,
                lock: Some(6)
            })
        );
        assert_eq!(
            parse_mount("0x7001B475"),
            Ok(MountSpec::new(MountSource::Did(0x7001_B475)))
        );
        assert_eq!(
            parse_mount("4294967295@instant@lock=7"),
            Ok(MountSpec {
                source: MountSource::Did(u32::MAX),
                timing: MountTiming::Instant,
                lock: Some(7)
            })
        );
    }

    #[test]
    fn limits_and_errors() {
        use SpecError as E;
        assert_eq!(parse_mount("   "), Err(E::Empty));
        assert_eq!(parse_mount(&"a".repeat(129)), Err(E::TooLong));
        assert_eq!(parse_mount("auto @summon"), Err(E::BadCharacter));
        assert_eq!(parse_mount("auto\u{e9}"), Err(E::BadCharacter));
        assert_eq!(parse_mount("auto@"), Err(E::EmptyPart));
        assert_eq!(parse_mount("@summon"), Err(E::EmptyPart));
        assert_eq!(parse_mount("0x100000000"), Err(E::BadDid));
        assert_eq!(parse_mount("4294967296"), Err(E::BadDid));
        assert_eq!(parse_mount("0x"), Err(E::BadDid));
        assert_eq!(parse_mount("dir:10000"), Err(E::BadIndex));
        assert_eq!(parse_mount("dir:"), Err(E::BadIndex));
        assert_eq!(parse_mount("steed:fast"), Err(E::BadTree));
        assert_eq!(parse_mount("horse"), Err(E::BadSource));
        assert_eq!(parse_mount("auto@fly"), Err(E::BadOption));
        assert_eq!(parse_mount("auto@lock=3"), Err(E::BadGait));
        assert_eq!(parse_mount("auto@summon@instant"), Err(E::SummonAndInstant));
        assert_eq!(parse_mount("auto@summon@summon"), Err(E::Repeated));
        assert_eq!(parse_mount("auto@lock=4@lock=5"), Err(E::Repeated));
        assert_eq!(
            parse_mount("auto@summon@lock=4@lock=5@lock=6@lock=7"),
            Err(E::TooManyOptions)
        );
    }

    #[test]
    fn printed_form_parses_back() {
        for t in [
            "auto",
            "dir:0@instant",
            "steed:heavy@summon@lock=5",
            "0x70000001@lock=gallop",
        ] {
            let s = parse_mount(t).unwrap();
            assert_eq!(parse_mount(&s.to_string()), Ok(s), "{t}");
        }
    }

    #[test]
    fn ride_arguments() {
        assert_eq!(parse_ride(""), Ok(RideCommand::Toggle));
        assert_eq!(parse_ride(" OFF "), Ok(RideCommand::Off));
        assert_eq!(parse_ride("list"), Ok(RideCommand::List(1)));
        assert_eq!(parse_ride("list 3"), Ok(RideCommand::List(3)));
        assert_eq!(parse_ride("list 0"), Err(SpecError::BadIndex));
        assert_eq!(parse_ride("list 1000"), Err(SpecError::BadIndex));
        assert_eq!(parse_ride("list 2 3"), Err(SpecError::BadOption));
        assert_eq!(
            parse_ride("steed:light"),
            Ok(RideCommand::Mount(MountSpec::new(MountSource::Steed(
                SteedTree::Light
            ))))
        );
    }
}
