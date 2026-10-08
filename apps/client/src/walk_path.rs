// SPDX-License-Identifier: GPL-3.0-or-later
//! Scripted walks (`tessera-client --walk-path FILE`): the walk format of
//! `docs/checklists/m4-5-visual.md` §2.1–§2.2, its two forms (one `key = value` file per
//! walk, or the walk table of the checklist's §2.3), and the per-step input script a walk
//! describes.
//!
//! | Item | Checklist |
//! |---|---|
//! | [`parse_walk_file`], [`Walk::to_file_text`] | §2.1 file form |
//! | [`parse_walk_table`], [`WALK_TABLE_ANCHOR`], [`WALK_TABLE_HEADER`] | §2.1 table form |
//! | [`Walk`], [`Category`], [`Gait`], [`Inputs`], [`TimedInput`], [`Action`] | §2.1 fields |
//! | [`Expect`] | §2.2 expected outcomes |
//! | [`WalkScript`], [`StepCommand`], [`SETTLE_SECONDS`], [`WAYPOINT_SWITCH`], [`WAYPOINT_STOP`] | §2.1 `inputs`, `start`, `duration` |
//!
//! This module depends on `std` only, so the fuzz target `client_walk_path` includes it by
//! path. Input is hostile: the text and every list are bounded ([`MAX_WALK_FILE_LEN`],
//! [`MAX_WALK_TABLE_LEN`], [`MAX_WALKS`], [`MAX_INPUT_TOKENS`], [`MAX_WAYPOINTS`],
//! [`MAX_RISERS`]), numbers must be finite and within [`MAX_COORDINATE`] or
//! [`MAX_DURATION`], and nothing panics.

use std::fmt;
use std::fmt::Write as _;

/// Longest walk file accepted (bytes).
pub const MAX_WALK_FILE_LEN: usize = 64 * 1024;
/// Longest Markdown text searched for the walk table (bytes).
pub const MAX_WALK_TABLE_LEN: usize = 8 << 20;
/// Most walks one table may hold.
pub const MAX_WALKS: usize = 4096;
/// Most tokens of one `inputs` field (timed tokens and waypoints together).
pub const MAX_INPUT_TOKENS: usize = 256;
/// Most waypoints of one walk.
pub const MAX_WAYPOINTS: usize = 256;
/// Most risers of one `reach` expectation.
pub const MAX_RISERS: usize = 64;
/// Longest field value (bytes).
pub const MAX_FIELD_LEN: usize = 4096;
/// Longest walk id (characters).
pub const MAX_ID_LEN: usize = 64;
/// Largest magnitude of a coordinate, a heading or a distance (block-local metres or
/// degrees). Interior coordinates may lie outside 0 … 160 (checklist §2.1), but never this
/// far.
pub const MAX_COORDINATE: f64 = 1.0e6;
/// Longest walk (simulated seconds after the settle period) and latest input time.
pub const MAX_DURATION: f64 = 3_600.0;
/// Simulated settle time between placing the capsule at `start` and the first input
/// (checklist §2.1 `start`).
pub const SETTLE_SECONDS: f64 = 0.5;
/// Tolerance of the placement at `start` (checklist §2.1 `start`).
pub const PLACE_TOLERANCE: f64 = 0.02;
/// Waypoint steering: switch to the next waypoint within this horizontal distance
/// (checklist §2.1 `inputs`).
pub const WAYPOINT_SWITCH: f64 = 0.25;
/// Waypoint steering: release forward within this distance of the last waypoint
/// (checklist §2.1 `inputs`).
pub const WAYPOINT_STOP: f64 = 0.05;

/// The anchor the walk table follows (checklist §2.1 table form).
pub const WALK_TABLE_ANCHOR: &str = "id=\"s2-3\"";
/// The exact header row of the walk table (checklist §2.1 table form).
pub const WALK_TABLE_HEADER: &str = "| id | category | region | landblock | cell | start | loc | heading | mode | inputs | duration | expect |";

/// The twelve field names, in table order.
pub const FIELD_NAMES: [&str; 12] = [
    "id",
    "category",
    "region",
    "landblock",
    "cell",
    "start",
    "loc",
    "heading",
    "mode",
    "inputs",
    "duration",
    "expect",
];

/// Why a walk did not parse: the 1-based line (file form) or table row (table form) when
/// known, and a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkPathError {
    /// 1-based line of the file, or of the Markdown text for a table row.
    pub line: Option<usize>,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for WalkPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "line {l}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for WalkPathError {}

fn err<T>(message: impl Into<String>) -> Result<T, WalkPathError> {
    Err(WalkPathError {
        line: None,
        message: message.into(),
    })
}

fn at_line(line: usize) -> impl Fn(WalkPathError) -> WalkPathError {
    move |mut e| {
        if e.line.is_none() {
            e.line = Some(line);
        }
        e
    }
}

/// The criterion-4 category a walk counts for (checklist §2.1 `category`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    /// A wall of a placed building.
    Wall,
    /// An undrawn blocker.
    Undrawn,
    /// A generated tree or rock.
    Generated,
    /// A step no higher than the step-up height.
    Step,
    /// A slope steeper than walkable.
    Steep,
    /// An interior entered through its doorway.
    Interior,
    /// A cave floor under a ceiling.
    Cave,
    /// A water entry.
    Water,
    /// An area covered only by frills.
    Frill,
}

impl Category {
    /// Every category, in the checklist's order.
    pub const ALL: [Self; 9] = [
        Self::Wall,
        Self::Undrawn,
        Self::Generated,
        Self::Step,
        Self::Steep,
        Self::Interior,
        Self::Cave,
        Self::Water,
        Self::Frill,
    ];

    /// The field text.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Undrawn => "undrawn",
            Self::Generated => "generated",
            Self::Step => "step",
            Self::Steep => "steep",
            Self::Interior => "interior",
            Self::Cave => "cave",
            Self::Water => "water",
            Self::Frill => "frill",
        }
    }

    fn parse(s: &str) -> Result<Self, WalkPathError> {
        Self::ALL
            .into_iter()
            .find(|c| c.name() == s)
            .map_or_else(|| err(format!("unknown category {:?}", clip(s))), Ok)
    }
}

/// How the walker moves (checklist §2.1 `mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gait {
    /// The Walking layer (`Movement_MovementStyle` 1).
    Walk,
    /// The default (BasePlayer) layer.
    Run,
    /// `Run` with a jump at every `jump` token of the inputs.
    Jump,
    /// Reserved for walks that start in deep water; moves like `Run`.
    Swim,
}

impl Gait {
    /// The field text.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Jump => "jump",
            Self::Swim => "swim",
        }
    }

    /// Whether the Walking layer applies.
    #[must_use]
    pub fn walking(self) -> bool {
        self == Self::Walk
    }

    fn parse(s: &str) -> Result<Self, WalkPathError> {
        [Self::Walk, Self::Run, Self::Jump, Self::Swim]
            .into_iter()
            .find(|g| g.name() == s)
            .map_or_else(|| err(format!("unknown mode {:?}", clip(s))), Ok)
    }
}

/// What a timed input token does (checklist §2.1 `inputs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// `fwd`: hold forward from then on.
    Forward,
    /// `stop`: release forward.
    Stop,
    /// `jump`: press jump.
    Jump,
    /// `turn=H`: set the compass heading to H degrees.
    Turn(f64),
}

/// One timed token `T:action`; `t` counts seconds from the end of the settle period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedInput {
    /// Seconds after the settle period.
    pub t: f64,
    /// What happens.
    pub action: Action,
}

/// The `inputs` field: timed tokens (in the order given, stably sorted by time) and the
/// waypoint list of a `wp=` token (block-local x, y).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inputs {
    /// Timed tokens, ascending in time (ties keep their written order).
    pub timed: Vec<TimedInput>,
    /// Waypoints, in order.
    pub waypoints: Vec<[f64; 2]>,
}

/// The pass condition of a walk (checklist §2.2). Positions are feet positions in the
/// frame of the walk's landblock.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// Stopped at `p` within `d`, never further along the start heading than `p` + `d`.
    Blocked {
        /// Stop point.
        p: [f64; 3],
        /// Horizontal tolerance.
        d: f64,
    },
    /// Ends within `d` of `e`, climbing `risers` without a jump.
    Reach {
        /// End point.
        e: [f64; 3],
        /// Horizontal tolerance.
        d: f64,
        /// Vertical steps of the support profile on the way, in order (metres).
        risers: Vec<f64>,
    },
    /// Never climbs above `zmax` nor further than `rmax` along the path; slides at least once.
    NoClimb {
        /// Foot of the steep band on the path.
        b: [f64; 3],
        /// Highest feet height allowed.
        zmax: f64,
        /// Furthest horizontal distance from the start along the path.
        rmax: f64,
    },
    /// Cell `from` at the start, `to` at the end; ends within `d` of `e`.
    Cells {
        /// Cell at the start.
        from: u16,
        /// Cell at the end.
        to: u16,
        /// End point.
        e: [f64; 3],
        /// Horizontal tolerance.
        d: f64,
    },
    /// Follows the terrain within `dz`, never blocked, ends within `d` of `e`.
    Follow {
        /// End point.
        e: [f64; 3],
        /// Horizontal tolerance.
        d: f64,
        /// Largest distance of the feet from the terrain height.
        dz: f64,
        /// Smallest ceiling-minus-floor distance on the path (information only).
        headroom: f64,
    },
    /// Enters the water near `entry`, swims from near `swim`, then floats at `h` − `fh`.
    WadeSwim {
        /// Where the terrain under the path first goes below `h` (x, y).
        entry: [f64; 2],
        /// Where it first goes below `h` − `fh` (x, y).
        swim: [f64; 2],
        /// Water height.
        h: f64,
        /// Float height of the walker.
        fh: f64,
        /// Horizontal tolerance.
        d: f64,
    },
    /// Ends within `d` of `e`; no object contact; feet on the terrain.
    Pass {
        /// End point.
        e: [f64; 3],
        /// Horizontal tolerance.
        d: f64,
    },
}

impl Expect {
    /// The kind's field text.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Blocked { .. } => "blocked",
            Self::Reach { .. } => "reach",
            Self::NoClimb { .. } => "noclimb",
            Self::Cells { .. } => "cells",
            Self::Follow { .. } => "follow",
            Self::WadeSwim { .. } => "wade_swim",
            Self::Pass { .. } => "pass",
        }
    }
}

/// One walk (checklist §2.1): a row of the walk table or one walk file.
#[derive(Debug, Clone, PartialEq)]
pub struct Walk {
    /// Unique walk name (letters and digits).
    pub id: String,
    /// The criterion-4 category.
    pub category: Category,
    /// Region *R*.
    pub region: u8,
    /// The landblock (X, Y) whose frame every coordinate uses.
    pub landblock: (u8, u8),
    /// Interior cell of the start; 0 = outdoors.
    pub cell: u16,
    /// Feet position at the start, block-local metres.
    pub start: [f64; 3],
    /// The same start as the original's `/loc` prints it (kept as text).
    pub loc: String,
    /// Compass heading at the start, degrees (0 = north, 90 = east).
    pub heading: f64,
    /// How the walker moves.
    pub mode: Gait,
    /// The inputs.
    pub inputs: Inputs,
    /// Simulated seconds after the settle period.
    pub duration: f64,
    /// The pass condition.
    pub expect: Expect,
}

/// At most 40 characters of `s`, for messages.
fn clip(s: &str) -> String {
    s.chars().take(40).collect()
}

fn number(s: &str, what: &str) -> Result<f64, WalkPathError> {
    let t = s.trim();
    if t.is_empty() || t.len() > 64 {
        return err(format!("{what}: expected a number"));
    }
    // Only plain decimal numbers: no "inf", "NaN" or hex.
    if !t
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+' | b'e' | b'E'))
    {
        return err(format!("{what}: {:?} is not a number", clip(t)));
    }
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() && v.abs() <= MAX_COORDINATE => Ok(v),
        Ok(_) => err(format!("{what}: {:?} is out of range", clip(t))),
        Err(_) => err(format!("{what}: {:?} is not a number", clip(t))),
    }
}

fn non_negative(s: &str, what: &str) -> Result<f64, WalkPathError> {
    let v = number(s, what)?;
    if v < 0.0 {
        return err(format!("{what}: must not be negative"));
    }
    Ok(v)
}

fn time(s: &str, what: &str) -> Result<f64, WalkPathError> {
    let v = non_negative(s, what)?;
    if v > MAX_DURATION {
        return err(format!("{what}: at most {MAX_DURATION} s"));
    }
    Ok(v)
}

fn unsigned<T: TryFrom<u64>>(s: &str, what: &str) -> Result<T, WalkPathError> {
    let t = s.trim();
    if t.is_empty() || t.len() > 20 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return err(format!("{what}: expected a decimal number"));
    }
    t.parse::<u64>()
        .ok()
        .and_then(|v| T::try_from(v).ok())
        .map_or_else(|| err(format!("{what}: {t} is out of range")), Ok)
}

fn vector<const N: usize>(s: &str, what: &str) -> Result<[f64; N], WalkPathError> {
    let mut out = [0.0; N];
    let mut parts = s.split(',');
    for v in &mut out {
        let Some(p) = parts.next() else {
            return err(format!("{what}: expected {N} comma-separated numbers"));
        };
        *v = number(p, what)?;
    }
    if parts.next().is_some() {
        return err(format!("{what}: expected {N} comma-separated numbers"));
    }
    Ok(out)
}

fn landblock(s: &str) -> Result<(u8, u8), WalkPathError> {
    let t = s.trim();
    let hex = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .ok_or_else(|| WalkPathError {
            line: None,
            message: format!("landblock: expected 0xXXYY, got {:?}", clip(t)),
        })?;
    if hex.is_empty() || hex.len() > 4 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return err(format!("landblock: expected 0xXXYY, got {:?}", clip(t)));
    }
    let v = u16::from_str_radix(hex, 16).map_err(|e| WalkPathError {
        line: None,
        message: format!("landblock: {e}"),
    })?;
    let [x, y] = v.to_be_bytes();
    Ok((x, y))
}

fn walk_id(s: &str) -> Result<String, WalkPathError> {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > MAX_ID_LEN || !t.chars().all(char::is_alphanumeric) {
        return err(format!(
            "id: expected 1 to {MAX_ID_LEN} letters and digits, got {:?}",
            clip(t)
        ));
    }
    Ok(t.to_owned())
}

/// Parses the `inputs` field (checklist §2.1).
///
/// # Errors
///
/// A malformed token, a bare waypoint before any `wp=`, more than [`MAX_INPUT_TOKENS`]
/// tokens / [`MAX_WAYPOINTS`] waypoints, or a `stop` or `turn` token together with
/// waypoints (they would have no effect: [`WalkScript`]).
pub fn parse_inputs(s: &str) -> Result<Inputs, WalkPathError> {
    let mut out = Inputs::default();
    let mut in_waypoints = false;
    for (n, token) in s
        .split(';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .enumerate()
    {
        if n >= MAX_INPUT_TOKENS {
            return err(format!("inputs: more than {MAX_INPUT_TOKENS} tokens"));
        }
        let point = if let Some(rest) = token.strip_prefix("wp=") {
            in_waypoints = true;
            Some(rest)
        } else if !token.contains(':') && token.contains(',') {
            if !in_waypoints {
                return err(format!(
                    "inputs: waypoint {:?} without a preceding wp=",
                    clip(token)
                ));
            }
            Some(token)
        } else {
            None
        };
        if let Some(p) = point {
            if out.waypoints.len() >= MAX_WAYPOINTS {
                return err(format!("inputs: more than {MAX_WAYPOINTS} waypoints"));
            }
            out.waypoints.push(vector::<2>(p, "inputs waypoint")?);
            continue;
        }
        let Some((t, action)) = token.split_once(':') else {
            return err(format!("inputs: unknown token {:?}", clip(token)));
        };
        let t = time(t, "inputs time")?;
        let action = match action.trim() {
            "fwd" => Action::Forward,
            "stop" => Action::Stop,
            "jump" => Action::Jump,
            a => match a.strip_prefix("turn=") {
                Some(h) => Action::Turn(number(h, "inputs turn")?),
                None => return err(format!("inputs: unknown action {:?}", clip(a))),
            },
        };
        out.timed.push(TimedInput { t, action });
    }
    // Waypoints steer forward and heading every step, so `stop` and `turn` would have no
    // effect (Tessera reading, OPEN in checklist §2.1): refused rather than ignored.
    if !out.waypoints.is_empty()
        && out
            .timed
            .iter()
            .any(|t| matches!(t.action, Action::Stop | Action::Turn(_)))
    {
        return err(
            "inputs: stop and turn cannot be combined with wp= (waypoints steer every step)",
        );
    }
    // Stable: tokens at the same time keep their order.
    out.timed.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(out)
}

/// Parses the `expect` field (checklist §2.2).
///
/// # Errors
///
/// An unknown kind, a missing, duplicate or unknown key, or a malformed value.
pub fn parse_expect(s: &str) -> Result<Expect, WalkPathError> {
    let mut words = s.split_whitespace();
    let Some(kind) = words.next() else {
        return err("expect: empty");
    };
    let mut cells: Option<(u16, u16)> = None;
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for w in words {
        if pairs.len() >= 16 {
            return err("expect: too many values");
        }
        match w.split_once('=') {
            Some((k, v)) => {
                if pairs.iter().any(|(pk, _)| *pk == k) {
                    return err(format!("expect: duplicate key {:?}", clip(k)));
                }
                pairs.push((k, v));
            }
            None if kind == "cells" && cells.is_none() => {
                let (a, b) = w.split_once('>').ok_or_else(|| WalkPathError {
                    line: None,
                    message: format!("expect: expected A>B, got {:?}", clip(w)),
                })?;
                cells = Some((unsigned(a, "expect cell")?, unsigned(b, "expect cell")?));
            }
            None => return err(format!("expect: unexpected {:?}", clip(w))),
        }
    }
    let keys: &[&str] = match kind {
        "blocked" => &["P", "d"],
        "reach" => &["E", "d", "risers"],
        "noclimb" => &["B", "zmax", "rmax"],
        "cells" => &["E", "d"],
        "follow" => &["E", "d", "dz", "headroom"],
        "wade_swim" => &["in", "swim", "h", "fh", "d"],
        "pass" => &["E", "d"],
        _ => return err(format!("expect: unknown kind {:?}", clip(kind))),
    };
    if let Some((k, _)) = pairs.iter().find(|(k, _)| !keys.contains(k)) {
        return err(format!("expect {kind}: unknown key {:?}", clip(k)));
    }
    let get = |k: &str| -> Result<&str, WalkPathError> {
        pairs
            .iter()
            .find(|(pk, _)| *pk == k)
            .map(|(_, v)| *v)
            .ok_or_else(|| WalkPathError {
                line: None,
                message: format!("expect {kind}: missing {k}"),
            })
    };
    let dist = |k: &str| -> Result<f64, WalkPathError> { non_negative(get(k)?, k) };
    Ok(match kind {
        "blocked" => Expect::Blocked {
            p: vector(get("P")?, "P")?,
            d: dist("d")?,
        },
        "reach" => {
            let mut risers = Vec::new();
            for r in get("risers")?.split('/') {
                if risers.len() >= MAX_RISERS {
                    return err(format!("expect reach: more than {MAX_RISERS} risers"));
                }
                risers.push(non_negative(r, "risers")?);
            }
            Expect::Reach {
                e: vector(get("E")?, "E")?,
                d: dist("d")?,
                risers,
            }
        }
        "noclimb" => Expect::NoClimb {
            b: vector(get("B")?, "B")?,
            zmax: number(get("zmax")?, "zmax")?,
            rmax: dist("rmax")?,
        },
        "cells" => {
            let Some((from, to)) = cells else {
                return err("expect cells: missing A>B");
            };
            Expect::Cells {
                from,
                to,
                e: vector(get("E")?, "E")?,
                d: dist("d")?,
            }
        }
        "follow" => Expect::Follow {
            e: vector(get("E")?, "E")?,
            d: dist("d")?,
            dz: dist("dz")?,
            headroom: dist("headroom")?,
        },
        "wade_swim" => Expect::WadeSwim {
            entry: vector(get("in")?, "in")?,
            swim: vector(get("swim")?, "swim")?,
            h: number(get("h")?, "h")?,
            fh: dist("fh")?,
            d: dist("d")?,
        },
        _ => Expect::Pass {
            e: vector(get("E")?, "E")?,
            d: dist("d")?,
        },
    })
}

/// Builds a walk from its twelve field values (`fields[i]` for [`FIELD_NAMES`]`[i]`); an
/// error in field `i` carries `lines[i]`.
///
/// Besides the raw value, the value as [`Walk::to_file_text`] writes it back (`inputs`,
/// `expect`) must fit in [`MAX_FIELD_LEN`], so that every accepted walk's file form parses
/// back (a number such as `.5` is written `0.5`). The other fields write back no longer
/// than a few hundred bytes.
fn walk_from_fields(
    fields: &[&str; 12],
    lines: &[Option<usize>; 12],
) -> Result<Walk, WalkPathError> {
    let at = |i: usize| {
        let line = lines[i];
        move |mut e: WalkPathError| {
            if e.line.is_none() {
                e.line = line;
            }
            e
        }
    };
    let field = |i: usize| -> Result<&str, WalkPathError> {
        let v = fields[i];
        if v.len() > MAX_FIELD_LEN {
            return err(format!(
                "{}: longer than {MAX_FIELD_LEN} bytes",
                FIELD_NAMES[i]
            ))
            .map_err(at(i));
        }
        Ok(v)
    };
    let region: u8 = unsigned(field(2)?, "region").map_err(at(2))?;
    if region == 0 {
        return err("region: must be positive").map_err(at(2));
    }
    let loc = field(6)?.trim();
    if loc.is_empty() || loc.chars().any(|c| c == '#' || c == '|' || c.is_control()) {
        return err("loc: empty, or holds '#', '|' or a control character").map_err(at(6));
    }
    let duration = time(field(10)?, "duration").map_err(at(10))?;
    let heading = number(field(7)?, "heading").map_err(at(7))?;
    if heading.abs() > 3_600.0 {
        return err("heading: out of range").map_err(at(7));
    }
    let inputs = parse_inputs(field(9)?).map_err(at(9))?;
    if inputs.to_text().len() > MAX_FIELD_LEN {
        return err(format!(
            "inputs: longer than {MAX_FIELD_LEN} bytes when written back"
        ))
        .map_err(at(9));
    }
    let expect = parse_expect(field(11)?).map_err(at(11))?;
    if expect.to_text().len() > MAX_FIELD_LEN {
        return err(format!(
            "expect: longer than {MAX_FIELD_LEN} bytes when written back"
        ))
        .map_err(at(11));
    }
    Ok(Walk {
        id: walk_id(field(0)?).map_err(at(0))?,
        category: Category::parse(field(1)?.trim()).map_err(at(1))?,
        region,
        landblock: landblock(field(3)?).map_err(at(3))?,
        cell: unsigned(field(4)?, "cell").map_err(at(4))?,
        start: vector(field(5)?, "start").map_err(at(5))?,
        loc: loc.to_owned(),
        heading,
        mode: Gait::parse(field(8)?.trim()).map_err(at(8))?,
        inputs,
        duration,
        expect,
    })
}

/// Parses the file form (checklist §2.1): UTF-8 text, one `key = value` per line, keys as
/// the column names, `#` starts a comment, blank lines ignored; every key exactly once. A
/// leading byte-order mark (U+FEFF) is skipped.
///
/// # Errors
///
/// A text longer than [`MAX_WALK_FILE_LEN`], an unknown, duplicate or missing key, a line
/// without `=`, or a malformed value (with its line).
pub fn parse_walk_file(text: &str) -> Result<Walk, WalkPathError> {
    if text.len() > MAX_WALK_FILE_LEN {
        return err(format!("walk file longer than {MAX_WALK_FILE_LEN} bytes"));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut values: [Option<(&str, usize)>; 12] = [None; 12];
    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        let line = raw.split_once('#').map_or(raw, |(a, _)| a).trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return err(format!("expected key = value, got {:?}", clip(line)))
                .map_err(at_line(line_no));
        };
        let k = k.trim();
        let Some(i) = FIELD_NAMES.iter().position(|f| *f == k) else {
            return err(format!("unknown key {:?}", clip(k))).map_err(at_line(line_no));
        };
        if values[i].is_some() {
            return err(format!("duplicate key {k:?}")).map_err(at_line(line_no));
        }
        values[i] = Some((v.trim(), line_no));
    }
    let mut fields = [""; 12];
    let mut lines = [None; 12];
    for (i, v) in values.iter().enumerate() {
        match v {
            Some((s, l)) => {
                fields[i] = s;
                lines[i] = Some(*l);
            }
            None => return err(format!("missing key {:?}", FIELD_NAMES[i])),
        }
    }
    walk_from_fields(&fields, &lines)
}

/// Removes surrounding backticks from a trimmed table cell.
fn unquote(cell: &str) -> &str {
    let t = cell.trim();
    t.strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(t)
}

/// Parses the table form (checklist §2.1): the first Markdown pipe table after
/// [`WALK_TABLE_ANCHOR`] whose header row is exactly [`WALK_TABLE_HEADER`]; every following
/// row up to the first line that does not start with `|` is a walk (the delimiter row is
/// skipped). Walk ids must be unique.
///
/// # Errors
///
/// A text longer than [`MAX_WALK_TABLE_LEN`], no anchor or header, a row without twelve
/// cells, more than [`MAX_WALKS`] rows, a duplicate id, or a malformed value (with its
/// line).
pub fn parse_walk_table(markdown: &str) -> Result<Vec<Walk>, WalkPathError> {
    if markdown.len() > MAX_WALK_TABLE_LEN {
        return err(format!("text longer than {MAX_WALK_TABLE_LEN} bytes"));
    }
    let mut lines = markdown.lines().enumerate();
    if !lines.by_ref().any(|(_, l)| l.contains(WALK_TABLE_ANCHOR)) {
        return err(format!("no anchor {WALK_TABLE_ANCHOR}"));
    }
    if !lines.by_ref().any(|(_, l)| l.trim() == WALK_TABLE_HEADER) {
        return err("no walk table header after the anchor");
    }
    let mut out: Vec<Walk> = Vec::new();
    for (n, line) in lines {
        let line_no = n + 1;
        let t = line.trim();
        if !t.starts_with('|') {
            break;
        }
        let inner = t.strip_prefix('|').unwrap_or(t);
        let inner = inner.strip_suffix('|').unwrap_or(inner);
        let is_delimiter = |c: &str| !c.is_empty() && c.bytes().all(|b| matches!(b, b'-' | b':'));
        // At most 13 cells are kept: a row of thousands of pipes allocates nothing more.
        let mut split = inner.split('|').map(unquote);
        let cells: Vec<&str> = split.by_ref().take(13).collect();
        if cells.iter().all(|c| is_delimiter(c)) && split.all(is_delimiter) {
            continue; // delimiter row
        }
        let Ok(fields) = <[&str; 12]>::try_from(cells.as_slice()) else {
            let got = if cells.len() > 12 {
                "more".to_owned()
            } else {
                cells.len().to_string()
            };
            return err(format!("expected 12 cells, got {got}")).map_err(at_line(line_no));
        };
        if out.len() >= MAX_WALKS {
            return err(format!("more than {MAX_WALKS} walks")).map_err(at_line(line_no));
        }
        let walk = walk_from_fields(&fields, &[Some(line_no); 12])?;
        if out.iter().any(|w| w.id == walk.id) {
            return err(format!("duplicate walk id {:?}", walk.id)).map_err(at_line(line_no));
        }
        out.push(walk);
    }
    if out.is_empty() {
        return err("the walk table has no rows");
    }
    Ok(out)
}

/// Formats a number for the walk format: the shorter of the plain and the exponent form
/// (both the shortest digits that parse back to the same value; the plain one on a tie).
/// It is never longer than the number text it was parsed from, except that a leading `.`
/// gains a `0`.
fn num(v: f64) -> String {
    let plain = format!("{v}");
    let exp = format!("{v:e}");
    if exp.len() < plain.len() { exp } else { plain }
}

fn vec_text(v: &[f64]) -> String {
    v.iter().map(|x| num(*x)).collect::<Vec<_>>().join(",")
}

impl Inputs {
    /// The field text: timed tokens in time order, then the waypoint list.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut parts: Vec<String> = self
            .timed
            .iter()
            .map(|t| {
                let a = match t.action {
                    Action::Forward => "fwd".to_owned(),
                    Action::Stop => "stop".to_owned(),
                    Action::Jump => "jump".to_owned(),
                    Action::Turn(h) => format!("turn={}", num(h)),
                };
                format!("{}:{a}", num(t.t))
            })
            .collect();
        for (i, w) in self.waypoints.iter().enumerate() {
            let p = vec_text(w);
            parts.push(if i == 0 { format!("wp={p}") } else { p });
        }
        parts.join(";")
    }
}

impl Expect {
    /// The field text.
    #[must_use]
    pub fn to_text(&self) -> String {
        match self {
            Self::Blocked { p, d } => format!("blocked P={} d={}", vec_text(p), num(*d)),
            Self::Reach { e, d, risers } => format!(
                "reach E={} d={} risers={}",
                vec_text(e),
                num(*d),
                risers.iter().map(|r| num(*r)).collect::<Vec<_>>().join("/")
            ),
            Self::NoClimb { b, zmax, rmax } => format!(
                "noclimb B={} zmax={} rmax={}",
                vec_text(b),
                num(*zmax),
                num(*rmax)
            ),
            Self::Cells { from, to, e, d } => {
                format!("cells {from}>{to} E={} d={}", vec_text(e), num(*d))
            }
            Self::Follow { e, d, dz, headroom } => format!(
                "follow E={} d={} dz={} headroom={}",
                vec_text(e),
                num(*d),
                num(*dz),
                num(*headroom)
            ),
            Self::WadeSwim {
                entry,
                swim,
                h,
                fh,
                d,
            } => format!(
                "wade_swim in={} swim={} h={} fh={} d={}",
                vec_text(entry),
                vec_text(swim),
                num(*h),
                num(*fh),
                num(*d)
            ),
            Self::Pass { e, d } => format!("pass E={} d={}", vec_text(e), num(*d)),
        }
    }
}

impl Walk {
    /// The walk in the file form; [`parse_walk_file`] gives it back unchanged (an empty
    /// risers list is the one value the format cannot express).
    #[must_use]
    pub fn to_file_text(&self) -> String {
        let (x, y) = self.landblock;
        let mut s = String::new();
        let _ = writeln!(s, "id = {}", self.id);
        let _ = writeln!(s, "category = {}", self.category.name());
        let _ = writeln!(s, "region = {}", self.region);
        let _ = writeln!(s, "landblock = 0x{x:02X}{y:02X}");
        let _ = writeln!(s, "cell = {}", self.cell);
        let _ = writeln!(s, "start = {}", vec_text(&self.start));
        let _ = writeln!(s, "loc = {}", self.loc);
        let _ = writeln!(s, "heading = {}", num(self.heading));
        let _ = writeln!(s, "mode = {}", self.mode.name());
        let _ = writeln!(s, "inputs = {}", self.inputs.to_text());
        let _ = writeln!(s, "duration = {}", num(self.duration));
        let _ = writeln!(s, "expect = {}", self.expect.to_text());
        s
    }

    /// Number of fixed steps of `step_seconds` the walk's inputs run for (its duration),
    /// rounded to the nearest step; at most `MAX_DURATION / step_seconds`.
    #[must_use]
    pub fn input_steps(&self, step_seconds: f64) -> u64 {
        steps_for(self.duration, step_seconds)
    }
}

/// Whole steps of `step_seconds` in `seconds` (rounded to the nearest; 0 for unusable input).
#[must_use]
pub fn steps_for(seconds: f64, step_seconds: f64) -> u64 {
    if !(seconds.is_finite() && step_seconds.is_finite() && step_seconds > 0.0 && seconds > 0.0) {
        return 0;
    }
    let n = (seconds.min(MAX_DURATION) / step_seconds).round();
    // At most MAX_DURATION / step_seconds, far below u64::MAX for any sane step.
    if (0.0..1.0e12).contains(&n) {
        n as u64
    } else {
        0
    }
}

/// What the walker does in one fixed step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepCommand {
    /// Forward (+) / backward (−), −1 ..= 1.
    pub forward: f32,
    /// Strafe right (+) / left (−), −1 ..= 1.
    pub strafe: f32,
    /// The compass heading (degrees) to face from this step on, `None` to keep the facing.
    pub heading: Option<f64>,
    /// Press jump in this step.
    pub jump: bool,
    /// Use the Walking layer.
    pub walking: bool,
}

/// The per-step input of a walk (checklist §2.1 `inputs`): timed tokens fire at the first
/// step whose time is at or past theirs; waypoints steer every step (heading toward the
/// current waypoint, forward held, the next waypoint taken within [`WAYPOINT_SWITCH`],
/// forward released while within [`WAYPOINT_STOP`] of the last one).
///
/// With waypoints, `fwd` changes nothing (forward is held until the last waypoint) and
/// `jump` still presses jump; `stop` and `turn` are refused by [`parse_inputs`].
///
/// Tessera reading (recorded as OPEN in `m4-5-visual.md` §2.1): the release is decided
/// every step, so a walker that coasts past the last waypoint (about 0.4 m at walking
/// speed under the controller's deceleration) is steered back to it rather than left
/// where it stopped.
#[derive(Debug, Clone)]
pub struct WalkScript {
    timed: Vec<TimedInput>,
    next: usize,
    waypoints: Vec<[f64; 2]>,
    target: usize,
    forward: bool,
    heading: f64,
    walking: bool,
    arrived: bool,
}

/// The compass heading (degrees, [0, 360)) from `from` toward `to` (x east, y north).
#[must_use]
pub fn compass_toward(from: [f64; 2], to: [f64; 2]) -> f64 {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let h = dx.atan2(dy).to_degrees().rem_euclid(360.0);
    if h.is_finite() && h < 360.0 { h } else { 0.0 }
}

impl WalkScript {
    /// The script of `walk`.
    #[must_use]
    pub fn new(walk: &Walk) -> Self {
        Self {
            timed: walk.inputs.timed.clone(),
            next: 0,
            waypoints: walk.inputs.waypoints.clone(),
            target: 0,
            forward: false,
            heading: walk.heading,
            walking: walk.mode.walking(),
            arrived: false,
        }
    }

    /// Whether the feet have been within [`WAYPOINT_STOP`] of the last waypoint (always
    /// true without waypoints).
    #[must_use]
    pub fn waypoints_done(&self) -> bool {
        self.waypoints.is_empty() || self.arrived
    }

    /// The command for the step that starts `t` seconds after the settle period, with the
    /// feet at block-local `feet` (x, y).
    pub fn command(&mut self, t: f64, feet: [f64; 2]) -> StepCommand {
        let mut jump = false;
        let mut turned = false;
        while let Some(ti) = self.timed.get(self.next) {
            if ti.t > t + 1e-9 {
                break;
            }
            self.next += 1;
            match ti.action {
                Action::Forward => self.forward = true,
                Action::Stop => self.forward = false,
                // The `jump` mode is `run` with these presses; a `jump` token in another
                // mode presses jump too (the token is explicit).
                Action::Jump => jump = true,
                Action::Turn(h) => {
                    self.heading = h.rem_euclid(360.0);
                    turned = true;
                }
            }
        }
        let mut forward = self.forward;
        let mut heading = turned.then_some(self.heading);
        if !self.waypoints.is_empty() {
            // Advance past waypoints already within the switch distance (not the last).
            while self.target + 1 < self.waypoints.len()
                && dist2(feet, self.waypoints[self.target]) <= WAYPOINT_SWITCH * WAYPOINT_SWITCH
            {
                self.target += 1;
            }
            if let Some(&wp) = self.waypoints.get(self.target) {
                let last = self.target + 1 == self.waypoints.len();
                let d2 = dist2(feet, wp);
                if last && d2 <= WAYPOINT_STOP * WAYPOINT_STOP {
                    self.arrived = true;
                    forward = false;
                } else {
                    // Every step (§2.1): a walker that coasts past the last waypoint
                    // after the release is steered back to it.
                    self.heading = compass_toward(feet, wp);
                    heading = Some(self.heading);
                    forward = true;
                }
            }
        }
        StepCommand {
            forward: if forward { 1.0 } else { 0.0 },
            strafe: 0.0,
            heading,
            jump,
            walking: self.walking,
        }
    }
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    const WL1: &str = "id = WL1\ncategory = wall\nregion = 1\nlandblock = 0xA197\ncell = 0\n\
        start = 127.81,89.59,389.26\nloc = r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26\n\
        heading = 0.0\nmode = run\ninputs = 0:fwd\nduration = 3\n\
        expect = blocked P=127.81,94.77,389.26 d=0.10\n";

    #[test]
    fn file_form_example() {
        let w = parse_walk_file(WL1).expect("parses");
        assert_eq!(w.id, "WL1");
        assert_eq!(w.category, Category::Wall);
        assert_eq!((w.region, w.landblock, w.cell), (1, (0xA1, 0x97), 0));
        assert_eq!(w.start, [127.81, 89.59, 389.26]);
        assert_eq!(w.mode, Gait::Run);
        assert_eq!(
            w.inputs.timed,
            vec![TimedInput {
                t: 0.0,
                action: Action::Forward
            }]
        );
        assert_eq!(w.duration, 3.0);
        assert_eq!(
            w.expect,
            Expect::Blocked {
                p: [127.81, 94.77, 389.26],
                d: 0.10
            }
        );
        assert_eq!(parse_walk_file(&w.to_file_text()), Ok(w));
    }

    #[test]
    fn comments_blank_lines_and_errors() {
        let text = format!(
            "# a walk\n\n{}",
            WL1.replace("cell = 0", "cell = 0 # outdoors")
        );
        assert!(parse_walk_file(&text).is_ok());
        let e = parse_walk_file(&format!("{WL1}colour = red\n")).unwrap_err();
        assert_eq!(e.line, Some(13));
        assert!(e.message.contains("unknown key"));
        assert!(
            parse_walk_file(&format!("{WL1}id = X\n"))
                .unwrap_err()
                .message
                .contains("duplicate")
        );
        assert!(
            parse_walk_file(&WL1.replace("mode = run\n", ""))
                .unwrap_err()
                .message
                .contains("missing key \"mode\"")
        );
        let e = parse_walk_file(&WL1.replace("heading = 0.0", "heading = inf")).unwrap_err();
        assert_eq!(e.line, Some(8), "{e}");
        assert!(parse_walk_file(&WL1.replace("0xA197", "A197")).is_err());
        assert!(parse_walk_file(&WL1.replace("0xA197", "0x1A197")).is_err());
        assert!(parse_walk_file(&WL1.replace("= WL1", "= W-1")).is_err());
        assert!(parse_walk_file(&WL1.replace("duration = 3", "duration = 1e9")).is_err());
        assert!(parse_walk_file(&WL1.replace("region = 1", "region = 0")).is_err());
        assert!(parse_walk_file("no equals sign").is_err());
        assert!(parse_walk_file(&"x".repeat(MAX_WALK_FILE_LEN + 1)).is_err());
    }

    #[test]
    fn inputs_grammar() {
        let i = parse_inputs("1.5:jump; 0:fwd;2:turn=90;3:stop").unwrap();
        let actions: Vec<Action> = i.timed.iter().map(|t| t.action).collect();
        assert_eq!(
            actions,
            [
                Action::Forward,
                Action::Jump,
                Action::Turn(90.0),
                Action::Stop
            ]
        );
        let i = parse_inputs("wp=1,2;3,4;5.5,-6").unwrap();
        assert_eq!(i.waypoints, vec![[1.0, 2.0], [3.0, 4.0], [5.5, -6.0]]);
        assert!(i.timed.is_empty());
        assert!(parse_inputs("3,4").is_err(), "waypoint without wp=");
        assert!(parse_inputs("0:dance").is_err());
        assert!(parse_inputs("-1:fwd").is_err());
        assert!(parse_inputs("x:fwd").is_err());
        assert!(parse_inputs("wp=1,2,3").is_err());
        assert_eq!(parse_inputs("").unwrap(), Inputs::default());
        let many = vec!["0:fwd"; MAX_INPUT_TOKENS + 1].join(";");
        assert!(parse_inputs(&many).is_err());
    }

    #[test]
    fn expectations() {
        for text in [
            "blocked P=1,2,3 d=0.1",
            "reach E=47.23,49.67,375.64 d=0.15 risers=0.26/0.13",
            "noclimb B=71.65,69.96,556.72 zmax=557.32 rmax=6.00",
            "cells 9>8 E=46.86,-108.12,-212.90 d=0.15",
            "follow E=115.11,140.20,208.03 d=0.15 dz=0.05 headroom=3.37",
            "wade_swim in=72.58,127.40 swim=70.50,125.31 h=337.255 fh=1.42 d=0.50",
            "pass E=115.96,27.56,335.92 d=0.10",
        ] {
            let e = parse_expect(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(parse_expect(&e.to_text()).as_ref(), Ok(&e), "{text}");
        }
        assert!(parse_expect("blocked P=1,2,3").is_err(), "missing d");
        assert!(parse_expect("blocked P=1,2,3 d=1 d=2").is_err());
        assert!(parse_expect("blocked P=1,2,3 d=1 E=1,2,3").is_err());
        assert!(parse_expect("cells E=1,2,3 d=1").is_err());
        assert!(parse_expect("cells 1>70000 E=1,2,3 d=1").is_err());
        assert!(parse_expect("fly E=1,2,3").is_err());
        assert!(parse_expect("").is_err());
        assert!(parse_expect("pass E=1,2,3 d=-1").is_err());
    }

    fn table(rows: &str) -> String {
        format!(
            "# title\n\n<a id=\"s2-3\"></a>\n### 2.3 The walks\n\ntext\n\n{WALK_TABLE_HEADER}\n|---|---|---|---|---|---|---|---|---|---|---|---|\n{rows}\nafter\n| not | a | row |\n"
        )
    }

    #[test]
    fn table_form() {
        let rows = "| WL1 | wall | 1 | 0xA197 | 0 | 127.81,89.59,389.26 | `r1 lx1294 ly1212 ox127.81 oy89.59 oz389.26` | 0.0 | run | `0:fwd` | 3 | `blocked P=127.81,94.77,389.26 d=0.10` |\n\
            | ST1 | step | 1 | 0x5B6D | 0 | 47.23,59.12,374.46 | `r1 lx730 ly874` | 180.0 | walk | `wp=47.23,49.67` | 6 | `reach E=47.23,49.67,375.64 d=0.15 risers=0.26/0.13` |";
        let walks = parse_walk_table(&table(rows)).expect("table");
        assert_eq!(walks.len(), 2);
        assert_eq!(walks[0], parse_walk_file(WL1).unwrap());
        assert_eq!(walks[1].inputs.waypoints, vec![[47.23, 49.67]]);
        // A table before the anchor is ignored; the anchor and header are required.
        let early = format!("{WALK_TABLE_HEADER}\n| x |\n\n{}", table(rows));
        assert_eq!(parse_walk_table(&early).unwrap().len(), 2);
        assert!(parse_walk_table(WALK_TABLE_HEADER).is_err());
        assert!(parse_walk_table("<a id=\"s2-3\"></a>\n| id |\n").is_err());
        let dup = format!(
            "{rows}\n{}",
            rows.lines().next().unwrap_or_default().trim_start()
        );
        let e = parse_walk_table(&table(&dup)).unwrap_err();
        assert!(e.message.contains("duplicate"), "{e}");
        let short = "| WL1 | wall | 1 |";
        let e = parse_walk_table(&table(short)).unwrap_err();
        assert_eq!(e.line, Some(10), "{e}");
    }

    #[test]
    fn checklist_table_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/checklists/m4-5-visual.md"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let walks = parse_walk_table(&text).expect("checklist walk table");
        assert!(walks.len() >= 24, "{}", walks.len());
        for w in &walks {
            assert_eq!(
                parse_walk_file(&w.to_file_text()).as_ref(),
                Ok(w),
                "{}",
                w.id
            );
        }
    }

    #[test]
    fn script_timed_tokens() {
        let mut w = parse_walk_file(WL1).unwrap();
        w.mode = Gait::Jump;
        w.inputs = parse_inputs("0:fwd;0.5:jump;1:turn=90;2:stop").unwrap();
        let mut s = WalkScript::new(&w);
        let c = s.command(0.0, [0.0, 0.0]);
        assert_eq!((c.forward, c.jump, c.heading), (1.0, false, None));
        let c = s.command(0.5, [0.0, 0.0]);
        assert!(c.jump);
        assert!(!s.command(0.51, [0.0, 0.0]).jump, "a jump fires once");
        assert_eq!(s.command(1.0, [0.0, 0.0]).heading, Some(90.0));
        assert_eq!(s.command(1.1, [0.0, 0.0]).heading, None);
        assert_eq!(s.command(2.0, [0.0, 0.0]).forward, 0.0);
    }

    #[test]
    fn script_waypoints() {
        let mut w = parse_walk_file(WL1).unwrap();
        w.inputs = parse_inputs("wp=0,10;10,10").unwrap();
        w.mode = Gait::Walk;
        let mut s = WalkScript::new(&w);
        let c = s.command(0.0, [0.0, 0.0]);
        assert_eq!((c.forward, c.heading, c.walking), (1.0, Some(0.0), true));
        // Within the switch distance of the first: steer to the second (east).
        let c = s.command(1.0, [0.0, 9.8]);
        assert!((c.heading.unwrap() - compass_toward([0.0, 9.8], [10.0, 10.0])).abs() < 1e-9);
        // Near the last: release.
        assert_eq!(s.command(2.0, [9.97, 10.0]).forward, 0.0);
        assert!(s.waypoints_done());
        let c = s.command(3.0, [10.5, 10.0]);
        assert_eq!(c.forward, 1.0, "steered back after coasting past");
        assert!((c.heading.unwrap() - 270.0).abs() < 1e-9);
        assert!((compass_toward([0.0, 0.0], [-1.0, 0.0]) - 270.0).abs() < 1e-9);
        assert!((compass_toward([0.0, 0.0], [0.0, -1.0]) - 180.0).abs() < 1e-9);
    }

    #[test]
    fn step_counts() {
        assert_eq!(steps_for(3.0, 1.0 / 60.0), 180);
        assert_eq!(steps_for(f64::NAN, 1.0 / 60.0), 0);
        assert_eq!(steps_for(1.0, 0.0), 0);
        assert_eq!(steps_for(1.0e9, 1.0), 3_600);
    }
}
