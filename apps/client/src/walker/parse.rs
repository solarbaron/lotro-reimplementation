// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure text handling of the world walker: the `--fly-path` value, place-name matching and
//! HUD text sanitising.
//!
//! Everything here is plain data in, plain data out (no I/O, no GPU), so the fuzz target
//! `client_walker_text` includes this file by path. Every input is treated as hostile:
//! lengths are capped before any work and nothing panics.

/// Side of a landblock in world units (`coordinates.md` §4). A copy of
/// `tessera_world::LANDBLOCK_SIZE` (a unit test keeps them equal), because the fuzz target
/// includes this file without the world crate; code elsewhere uses the world crate's.
pub const BLOCK: f64 = 160.0;
/// Largest landblock index (`coordinates.md` §4; equals `tessera_world::MAX_BLOCK`).
pub const MAX_BLOCK_INDEX: u8 = 254;
/// Largest region number the landblock DID layout can carry (`landblock.md` §2.1; equals
/// `tessera_world::MAX_REGION`). Which regions exist is data, checked when the walker
/// starts.
pub const MAX_REGION: u8 = 15;
/// Longest `--fly-path` value accepted.
pub const MAX_PATH_TEXT: usize = 256;
/// Longest place query accepted.
pub const MAX_QUERY_TEXT: usize = 256;
/// Longest HUD line kept (in characters).
pub const MAX_HUD_LINE: usize = 160;

/// One end of a `--fly-path`: region (if given) and a region-global position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPoint {
    /// Region, when the text named one.
    pub region: Option<u8>,
    /// Region-global (G_x, G_y).
    pub g: [f64; 2],
}

/// A straight flight between two points of one region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyPath {
    /// Region of both ends; `None` when neither end named one (the caller supplies it).
    pub region: Option<u8>,
    /// Start (G_x, G_y).
    pub from: [f64; 2],
    /// End (G_x, G_y).
    pub to: [f64; 2],
}

impl FlyPath {
    /// Length in world units.
    #[must_use]
    pub fn length(&self) -> f64 {
        (self.to[0] - self.from[0]).hypot(self.to[1] - self.from[1])
    }

    /// The point at distance `d` along the path (clamped to its ends).
    #[must_use]
    pub fn at(&self, d: f64) -> [f64; 2] {
        let len = self.length();
        if len.is_nan() || len <= 0.0 || !d.is_finite() {
            return self.from;
        }
        let t = (d / len).clamp(0.0, 1.0);
        [
            self.from[0] + (self.to[0] - self.from[0]) * t,
            self.from[1] + (self.to[1] - self.from[1]) * t,
        ]
    }

    /// Heading in radians (0 = +X east, counter-clockwise), 0 for a zero-length path.
    #[must_use]
    pub fn heading(&self) -> f64 {
        let (dx, dy) = (self.to[0] - self.from[0], self.to[1] - self.from[1]);
        if dx == 0.0 && dy == 0.0 {
            0.0
        } else {
            dy.atan2(dx)
        }
    }
}

fn parse_index(s: &str) -> Result<u8, String> {
    s.trim()
        .parse::<u8>()
        .ok()
        .filter(|v| *v <= MAX_BLOCK_INDEX)
        .ok_or_else(|| format!("{:?} is not a landblock index 0..=254", cut(s, 16)))
}

fn parse_local(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && (0.0..=BLOCK).contains(v))
        .ok_or_else(|| format!("{:?} is not a block-local offset 0..=160", cut(s, 16)))
}

fn parse_region(s: &str) -> Result<u8, String> {
    s.trim()
        .parse::<u8>()
        .ok()
        .filter(|v| (1..=MAX_REGION).contains(v))
        .ok_or_else(|| format!("{:?} is not a region 1..={MAX_REGION}", cut(s, 16)))
}

/// Parses one path end: `X:Y`, `R:X:Y`, `X:Y:x:y` or `R:X:Y:x:y` (landblock indices
/// 0–254, block-local offsets 0–160; the block centre when no offset is given).
///
/// # Errors
///
/// A message naming the bad field.
pub fn parse_path_point(text: &str) -> Result<PathPoint, String> {
    if text.len() > MAX_PATH_TEXT {
        return Err("path point too long".into());
    }
    let f: Vec<&str> = text.split(':').collect();
    let (region, bx, by, lx, ly) = match f.len() {
        2 => (None, f[0], f[1], None, None),
        3 => (Some(f[0]), f[1], f[2], None, None),
        4 => (None, f[0], f[1], Some(f[2]), Some(f[3])),
        5 => (Some(f[0]), f[1], f[2], Some(f[3]), Some(f[4])),
        _ => {
            return Err(format!(
                "{:?}: expected X:Y, R:X:Y, X:Y:x:y or R:X:Y:x:y",
                cut(text, 32)
            ));
        }
    };
    let region = region.map(parse_region).transpose()?;
    let (bx, by) = (parse_index(bx)?, parse_index(by)?);
    let lx = lx.map(parse_local).transpose()?.unwrap_or(BLOCK / 2.0);
    let ly = ly.map(parse_local).transpose()?.unwrap_or(BLOCK / 2.0);
    Ok(PathPoint {
        region,
        g: [f64::from(bx) * BLOCK + lx, f64::from(by) * BLOCK + ly],
    })
}

/// Parses `FROM,TO` (each as in [`parse_path_point`]). When both ends name a region they
/// must agree.
///
/// # Errors
///
/// A message naming the bad part.
pub fn parse_fly_path(text: &str) -> Result<FlyPath, String> {
    if text.len() > MAX_PATH_TEXT {
        return Err("fly path too long".into());
    }
    let (a, b) = text
        .split_once(',')
        .ok_or_else(|| "expected FROM,TO (for example 1:20:100,1:230:100)".to_owned())?;
    let a = parse_path_point(a)?;
    let b = parse_path_point(b)?;
    let region = match (a.region, b.region) {
        (Some(x), Some(y)) if x != y => {
            return Err(format!("the ends are in different regions ({x} and {y})"));
        }
        (Some(x), Some(_)) => Some(x),
        (r, None) | (None, r) => r,
    };
    Ok(FlyPath {
        region,
        from: a.g,
        to: b.g,
    })
}

/// The first `max` characters of `s` (for error messages).
#[must_use]
pub fn cut(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Normalises a place name or query for matching: lower case, letters and digits kept
/// (accented letters folded to their base letter where it is Latin-1), every run of other
/// characters becomes one space, trimmed. Input is capped at [`MAX_QUERY_TEXT`] characters.
#[must_use]
pub fn normalize_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(MAX_QUERY_TEXT));
    let mut space = false;
    for c in s.chars().take(MAX_QUERY_TEXT) {
        let c = fold_latin1(c);
        if c.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            // Lower-casing can add combining marks (İ → i + dot): keep letters only.
            out.extend(c.to_lowercase().filter(|l| l.is_alphanumeric()));
        } else {
            space = true;
        }
    }
    out
}

/// Folds Latin-1 accented letters to their base letter (å → a, ú → u, …).
fn fold_latin1(c: char) -> char {
    match c {
        'À'..='Å' | 'à'..='å' => 'a',
        'Ç' | 'ç' => 'c',
        'È'..='Ë' | 'è'..='ë' => 'e',
        'Ì'..='Ï' | 'ì'..='ï' => 'i',
        'Ñ' | 'ñ' => 'n',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' => 'o',
        'Ù'..='Ü' | 'ù'..='ü' => 'u',
        'Ý' | 'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// How well a (normalised) name matches a (normalised) query; higher is better, `None`
/// for no match: exact (3), the name starts with the query at a word boundary (2), the
/// query is a whole word sequence inside the name (1).
#[must_use]
pub fn match_score(name: &str, query: &str) -> Option<u8> {
    if query.is_empty() || name.is_empty() {
        return None;
    }
    if name == query {
        return Some(3);
    }
    let bounded = |idx: usize| {
        let before_ok = idx == 0 || name.as_bytes().get(idx - 1) == Some(&b' ');
        let end = idx + query.len();
        let after_ok = end == name.len() || name.as_bytes().get(end) == Some(&b' ');
        before_ok && after_ok
    };
    if name.starts_with(query) && bounded(0) {
        return Some(2);
    }
    name.match_indices(query)
        .any(|(i, _)| bounded(i))
        .then_some(1)
}

/// A HUD line: control characters become spaces, at most [`MAX_HUD_LINE`] characters.
#[must_use]
pub fn sanitize_line(s: &str) -> String {
    s.chars()
        .take(MAX_HUD_LINE)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_points() {
        let p = parse_path_point("10:20").unwrap();
        assert_eq!(p.region, None);
        assert_eq!(p.g, [1680.0, 3280.0]);
        let p = parse_path_point("2:10:20:0:160").unwrap();
        assert_eq!(p.region, Some(2));
        assert_eq!(p.g, [1600.0, 3360.0]);
        assert!(parse_path_point("255:1").is_err());
        assert!(parse_path_point("1:1:161:0").is_err());
        assert!(parse_path_point("0:1:1").is_err());
        assert!(parse_path_point("1").is_err());
        assert!(parse_path_point("1:2:3:4:5:6").is_err());
        assert!(parse_path_point("a:b").is_err());
        assert!(parse_path_point("1:1:nan:0").is_err());
    }

    #[test]
    fn fly_paths() {
        let p = parse_fly_path("1:0:100,1:254:100").unwrap();
        assert_eq!(p.region, Some(1));
        assert_eq!(p.from, [80.0, 16_080.0]);
        assert_eq!(p.to, [254.0 * 160.0 + 80.0, 16_080.0]);
        assert!((p.length() - 254.0 * 160.0).abs() < 1e-9);
        assert_eq!(p.heading(), 0.0);
        assert_eq!(p.at(-5.0), p.from);
        assert_eq!(p.at(1e12), p.to);
        assert_eq!(p.at(f64::NAN), p.from);
        assert_eq!(parse_fly_path("3:4,5:6").unwrap().region, None);
        assert_eq!(parse_fly_path("3:4,2:5:6").unwrap().region, Some(2));
        assert!(parse_fly_path("1:3:4,2:5:6").is_err());
        assert!(parse_fly_path("3:4").is_err());
        assert!(parse_fly_path(&"1:".repeat(300)).is_err());
        let zero = parse_fly_path("3:4,3:4").unwrap();
        assert_eq!(zero.at(10.0), zero.from);
        assert_eq!(zero.heading(), 0.0);
    }

    #[test]
    fn names_normalise_and_match() {
        assert_eq!(normalize_name("  The  Old-Forest! "), "the old forest");
        assert_eq!(normalize_name("Állúvale"), "alluvale");
        assert_eq!(normalize_name("\u{0}\u{7}"), "");
        assert_eq!(normalize_name("İx"), "ix");
        assert_eq!(match_score("old forest", "old forest"), Some(3));
        assert_eq!(match_score("old forest road", "old forest"), Some(2));
        assert_eq!(match_score("the old forest", "old forest"), Some(1));
        assert_eq!(match_score("the older forest", "old"), None);
        assert_eq!(match_score("abc", ""), None);
        assert_eq!(match_score("", "a"), None);
    }

    #[test]
    fn constants_match_the_world_crate() {
        assert_eq!(BLOCK, tessera_world::LANDBLOCK_SIZE);
        assert_eq!(MAX_BLOCK_INDEX, tessera_world::coords::MAX_BLOCK);
        assert_eq!(MAX_REGION, tessera_world::coords::MAX_REGION);
    }

    #[test]
    fn hud_lines_are_clean() {
        assert_eq!(sanitize_line("a\nb\tc"), "a b c");
        assert_eq!(sanitize_line(&"x".repeat(1000)).len(), MAX_HUD_LINE);
    }
}
