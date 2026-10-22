// SPDX-License-Identifier: GPL-3.0-or-later
//! Content files (`test-server.md` §5) and their checks (`server-content.md` §9.3).
//!
//! A content file defines a world: spawn points, simulated wanderers, the message of the day
//! and the game clock. It holds only original content. [`check_text`] applies every rule of
//! §5.6 and reports `<line>: <RULE>: <message>` findings; [`Content::parse`] returns the
//! world only when there are none (the `SPDX` rule is checked only by `check-content`).

use std::path::Path;

use tessera_protocol::limits::{
    LANDBLOCK_SIZE, MAX_ANCHOR_OFFSET, MAX_LANDBLOCK, MAX_LOCATION_COORDINATE, MAX_LONG_TEXT_LEN,
    MAX_NAME_LEN, MAX_SOFTWARE_NAME_LEN,
};
use tessera_protocol::text::has_control;
use tessera_protocol::types::{Anchor, Appearance, Location};
use toml::de::DeValue;

use crate::appearance;
use crate::names::is_content_id;
use crate::tomlx::{self, Finding, Reader, Rule, Source, View};
use crate::words;

/// The built-in default world (`apps/server/content/default-world.toml`, §5.7).
pub const DEFAULT_WORLD: &str = include_str!("../content/default-world.toml");
/// The file name shown in findings for the built-in default world.
pub const DEFAULT_WORLD_NAME: &str = "<built-in>/default-world.toml";

/// Most spawn points (positions) per file (`server-content.md` §5.2 P4).
pub const MAX_POSITIONS: usize = 16;
/// Most opaque references per file (`server-content.md` §9.2).
pub const MAX_REFERENCES: usize = 8;
/// Most wanderer groups per file.
pub const MAX_WANDERER_GROUPS: usize = 16;
/// Most wanderers per instance over all groups.
pub const MAX_WANDERERS: u32 = 1_024;

/// A spawn point (§5.3), resolved to a region-global pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
    /// Project-made name (`a–z`, `0–9`, `-`).
    pub name: String,
    /// Region-global position.
    pub location: Location,
    /// Protocol heading (radians, 0 = east, counter-clockwise).
    pub heading: f32,
    /// The anchor that owns an interior (out-of-square) position: the given block with
    /// cell 0. `None` for outdoor positions, whose interest block follows the position.
    pub anchor: Option<Anchor>,
    /// The height is approximate: settling grace applies (§8.3).
    pub ground: bool,
    /// The offset lies outside the block's square.
    pub interior: bool,
}

/// How a wanderer group looks (§5.4).
#[derive(Debug, Clone, PartialEq)]
pub enum WandererLook {
    /// Copied from the first character who enters the instance.
    FirstPlayer,
    /// A fixed appearance from the content file.
    Fixed(Appearance),
}

/// A group of wanderers (§5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct WandererGroup {
    /// Index into [`Content::spawns`].
    pub spawn: usize,
    /// Wanderers in the group (1–256).
    pub count: u32,
    /// Circle radius, metres.
    pub radius: f64,
    /// Speed along the circle, m/s.
    pub speed: f64,
    /// Name template; `{n}` is the 1-based number within the instance.
    pub name: String,
    /// Appearance.
    pub look: WandererLook,
}

/// A validated content file.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    /// `[world] id`.
    pub id: String,
    /// `[world] title`.
    pub title: String,
    /// `[world] motd`.
    pub motd: String,
    /// Index of the default spawn in [`spawns`](Self::spawns).
    pub default_spawn: usize,
    /// Spawn points (1–16).
    pub spawns: Vec<Spawn>,
    /// Wanderer groups (0–16).
    pub wanderers: Vec<WandererGroup>,
    /// Game clock for an instance with no saved state, seconds.
    pub clock_start: f64,
    /// Game seconds per real second.
    pub clock_rate: f32,
}

impl Content {
    /// The built-in default world.
    ///
    /// # Panics
    /// Never in a correct build: the default world is checked by the crate's tests.
    #[must_use]
    pub fn default_world() -> Self {
        match Self::parse(DEFAULT_WORLD) {
            Ok(c) => c,
            Err(f) => panic!("the built-in default world is invalid: {f:?}"),
        }
    }

    /// Parses and validates a content text with the checks `run` applies (every rule of §5.6
    /// except `SPDX`).
    pub fn parse(text: &str) -> Result<Self, Vec<Finding>> {
        let (content, findings) = check_text(text, false);
        match content {
            Some(c) if findings.is_empty() => Ok(c),
            _ => Err(findings),
        }
    }

    /// The spawn with this name, compared ignoring ASCII case (§8.4).
    #[must_use]
    pub fn spawn_named(&self, name: &str) -> Option<&Spawn> {
        self.spawns
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// The default spawn (`None` only for a [`Content`] built by hand that breaks
    /// [`check_invariants`](Self::check_invariants)).
    #[must_use]
    pub fn default_spawn(&self) -> Option<&Spawn> {
        self.spawns.get(self.default_spawn)
    }

    /// Re-checks the structural rules of §5.6 that the simulation relies on, for a
    /// [`Content`] that did not come from [`Content::parse`] (its fields are public): 1–16
    /// spawns with valid positions, `default_spawn` and every group's spawn in range, group
    /// counts, radii and speeds in range, at most 1,024 wanderers, a finite clock. Returns
    /// what is wrong, in words.
    #[must_use]
    pub fn check_invariants(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.spawns.is_empty() || self.spawns.len() > MAX_POSITIONS {
            out.push(format!(
                "{} spawns; 1–{MAX_POSITIONS} needed",
                self.spawns.len()
            ));
        }
        if self.default_spawn >= self.spawns.len() {
            out.push("`default_spawn` names no spawn".to_owned());
        }
        for s in &self.spawns {
            if !s.location.is_valid() || !s.heading.is_finite() {
                out.push(format!("spawn \"{}\" has an invalid position", s.name));
            }
        }
        if self.wanderers.len() > MAX_WANDERER_GROUPS {
            out.push(format!("more than {MAX_WANDERER_GROUPS} wanderer groups"));
        }
        let mut total = 0u64;
        for g in &self.wanderers {
            total += u64::from(g.count);
            if g.spawn >= self.spawns.len() {
                out.push(format!("wanderer group \"{}\" names no spawn", g.name));
            }
            if !(1..=256).contains(&g.count)
                || !(0.5..=100.0).contains(&g.radius)
                || !(g.speed > 0.0 && g.speed <= 7.5)
            {
                out.push(format!(
                    "wanderer group \"{}\": count, radius or speed out of range",
                    g.name
                ));
            }
        }
        if total > u64::from(MAX_WANDERERS) {
            out.push(format!("more than {MAX_WANDERERS} wanderers per instance"));
        }
        if !(self.clock_start.is_finite() && self.clock_start >= 0.0)
            || !(0.0..=10_000.0).contains(&self.clock_rate)
        {
            out.push("`[clock]` out of range".to_owned());
        }
        out
    }

    /// Wanderers per instance over all groups.
    #[must_use]
    pub fn wanderer_total(&self) -> u32 {
        self.wanderers.iter().map(|g| g.count).sum()
    }
}

/// Converts a compass facing (degrees, 0 = north, clockwise) to the protocol's heading
/// (radians, 0 = east, counter-clockwise), in (−π, π] (§5.3 `facing`).
#[must_use]
pub fn heading_from_compass(degrees: f64) -> f32 {
    let d = (90.0 - degrees).rem_euclid(360.0);
    let d = if d > 180.0 { d - 360.0 } else { d };
    d.to_radians() as f32
}

/// The compass facing in whole degrees of a protocol heading, [0, 360).
#[must_use]
pub fn compass_from_heading(heading: f32) -> u32 {
    let d = (90.0 - f64::from(heading).to_degrees()).rem_euclid(360.0);
    (d.round() as u32) % 360
}

/// Whether the SPDX header rule holds (§2.6): a `# SPDX-License-Identifier: …` comment
/// before the first key, naming GPL-3.0-or-later or CC0-1.0. Returns the finding otherwise.
fn spdx_finding(text: &str) -> Option<Finding> {
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let Some(comment) = t.strip_prefix('#') else {
            break;
        };
        if let Some(lic) = comment.trim().strip_prefix("SPDX-License-Identifier:") {
            let lic = lic.trim();
            return if lic == "GPL-3.0-or-later" || lic == "CC0-1.0" {
                None
            } else {
                Some(Finding::new(
                    i + 1,
                    Rule::Spdx,
                    format!("licence \"{lic}\" is not GPL-3.0-or-later or CC0-1.0"),
                ))
            };
        }
    }
    Some(Finding::new(
        1,
        Rule::Spdx,
        "no `# SPDX-License-Identifier: …` comment before the first key",
    ))
}

fn text_class(
    r: &mut Reader<'_, '_>,
    path: &str,
    text: &str,
    min: usize,
    max: usize,
    line: usize,
) -> bool {
    let mut ok = true;
    if text.len() < min || text.len() > max || has_control(text) {
        r.push(
            line,
            Rule::Text,
            format!("`{path}` must be {min}–{max} bytes without control characters"),
        );
        ok = false;
    }
    if let Some(w) = words::find_rule_one(text) {
        r.push(
            line,
            Rule::Word,
            format!("`{path}` contains \"{w}\" (CLAUDE.md Naming rule)"),
        );
        ok = false;
    }
    ok
}

fn id_class(r: &mut Reader<'_, '_>, path: &str, text: &str, line: usize) -> bool {
    let mut ok = true;
    if !is_content_id(text) {
        r.push(
            line,
            Rule::Text,
            format!("`{path}` must be 1–32 characters of a–z, 0–9 and -"),
        );
        ok = false;
    }
    if let Some(w) = words::find_rule_one(text) {
        r.push(
            line,
            Rule::Word,
            format!("`{path}` contains \"{w}\" (CLAUDE.md Naming rule)"),
        );
        ok = false;
    }
    ok
}

fn number_array(
    r: &mut Reader<'_, '_>,
    v: View<'_, '_>,
    key: &str,
    len: usize,
) -> Option<Vec<f64>> {
    let (arr, line) = r.array(v, key, false)?;
    let mut out = Vec::new();
    for item in arr.iter() {
        match tomlx::float(item.get_ref()) {
            Some(f) if f.is_finite() => out.push(f),
            Some(_) => {
                r.push(
                    line,
                    Rule::Range,
                    format!("`{}.{key}` must hold finite numbers", v.path),
                );
                return None;
            }
            None => {
                r.push(
                    line,
                    Rule::Format,
                    format!("`{}.{key}` must hold numbers", v.path),
                );
                return None;
            }
        }
    }
    if out.len() != len {
        r.push(
            line,
            Rule::Format,
            format!("`{}.{key}` must hold exactly {len} numbers", v.path),
        );
        return None;
    }
    Some(out)
}

/// x, y, z, the block of a block + offset spawn, and whether it is an interior.
type SpawnPos = (f64, f64, f64, Option<(u8, u8)>, bool);

fn parse_spawn(r: &mut Reader<'_, '_>, v: View<'_, '_>) -> Option<Spawn> {
    r.check_keys(
        v,
        &[
            "name", "region", "block", "offset", "location", "facing", "ground",
        ],
    );
    let name = match r.string(v, "name", true) {
        Some((n, line)) => id_class(r, "spawn.name", &n, line).then_some(n),
        None => None,
    };
    let region = r.int(v, "region", true, 0, 65_535).map(|(i, _)| i as u16);
    let facing = match r.float(v, "facing", false) {
        Some((f, line)) if !(0.0..360.0).contains(&f) => {
            r.push(line, Rule::Range, "`spawn.facing` must lie in [0, 360)");
            None
        }
        Some((f, _)) => Some(f),
        None if v.table.contains_key("facing") => None,
        None => Some(0.0),
    };
    let ground = match r.bool(v, "ground", false) {
        Some((b, _)) => Some(b),
        None if v.table.contains_key("ground") => None,
        None => Some(false),
    };
    let has_block = v.table.contains_key("block") || v.table.contains_key("offset");
    let has_location = v.table.contains_key("location");
    let mut pos: Option<SpawnPos> = None;
    if has_block == has_location {
        r.push(
            v.line,
            Rule::Format,
            "a spawn needs exactly one of `block` + `offset` or `location`",
        );
    } else if has_block {
        let block = number_array(r, v, "block", 2);
        let offset = number_array(r, v, "offset", 3);
        if !v.table.contains_key("block") || !v.table.contains_key("offset") {
            r.push(v.line, Rule::Format, "`block` and `offset` go together");
        } else if let (Some(b), Some(o)) = (block, offset) {
            let line = v.table.get("block").map_or(v.line, |x| r.line(&x.span()));
            let ok_index =
                |f: f64| f.fract() == 0.0 && (0.0..=f64::from(MAX_LANDBLOCK)).contains(&f);
            let oline = v.table.get("offset").map_or(v.line, |x| r.line(&x.span()));
            if !(ok_index(b[0]) && ok_index(b[1])) {
                r.push(
                    line,
                    Rule::Range,
                    "`spawn.block` indices must be integers 0–254",
                );
            } else if o[0].abs() > MAX_ANCHOR_OFFSET
                || o[1].abs() > MAX_ANCHOR_OFFSET
                || o[2].abs() > MAX_LOCATION_COORDINATE
            {
                r.push(
                    oline,
                    Rule::Range,
                    "`spawn.offset` x and y must lie within ±1280 and z within ±10⁷",
                );
            } else {
                let (bx, by) = (b[0] as u8, b[1] as u8);
                let x = LANDBLOCK_SIZE * f64::from(bx) + o[0];
                let y = LANDBLOCK_SIZE * f64::from(by) + o[1];
                let interior = !(0.0..=LANDBLOCK_SIZE).contains(&o[0])
                    || !(0.0..=LANDBLOCK_SIZE).contains(&o[1]);
                pos = Some((x, y, o[2], Some((bx, by)), interior));
            }
        }
    } else if let Some(l) = number_array(r, v, "location", 3) {
        let line = v
            .table
            .get("location")
            .map_or(v.line, |x| r.line(&x.span()));
        if l.iter().any(|c| c.abs() > MAX_LOCATION_COORDINATE) {
            r.push(
                line,
                Rule::Range,
                "`spawn.location` coordinates must lie within ±10⁷",
            );
        } else {
            pos = Some((l[0], l[1], l[2], None, false));
        }
    }
    let (x, y, z, block, interior) = pos?;
    if interior && ground == Some(true) {
        r.push(
            v.line,
            Rule::Range,
            "`ground = true` is not allowed with an offset outside the block's square",
        );
        return None;
    }
    let location = Location::new(region?, x, y, z);
    // Only interior (out-of-square) spawns carry an anchor on the wire. An outdoor anchor
    // would pin the player's interest block to the spawn block for up to 1,280 m of walking
    // (tessera-protocol.md §8.7, §9.1); see the OPEN note in test-server.md §5.3.
    let anchor = match (interior, block) {
        (true, Some((bx, by))) => Some(Anchor::new(bx, by, 0)),
        _ => None,
    };
    Some(Spawn {
        name: name?,
        location,
        heading: heading_from_compass(facing?),
        anchor,
        ground: ground? && !interior,
        interior,
    })
}

/// Checks a content text against every rule of §5.6 (`check-content` when `spdx` is true;
/// `run` skips the SPDX rule). Returns the content when it could be built and the findings
/// sorted by line; a content with findings must not be used.
#[must_use]
pub fn check_text(text: &str, spdx: bool) -> (Option<Content>, Vec<Finding>) {
    let src = Source::new(text);
    let mut findings = Vec::new();
    if spdx && let Some(f) = spdx_finding(text) {
        findings.push(f);
    }
    let doc = match tomlx::parse(&src) {
        Ok(d) => d,
        Err(f) => {
            findings.push(f);
            findings.sort();
            return (None, findings);
        }
    };
    let mut r = Reader::new(&src, Rule::Format, Rule::Range);
    let root = View {
        table: doc.get_ref(),
        line: 1,
        path: "",
    };
    r.check_keys(root, &["format", "world", "spawn", "wanderers", "clock"]);
    match r.value(root, "format", true) {
        Some(v) if tomlx::integer(v.get_ref()) == Some(1) => {}
        Some(v) => {
            let line = r.line(&v.span());
            r.push(
                line,
                Rule::Format,
                "unknown `format` (this server reads format = 1)",
            );
        }
        None => {}
    }

    // [world]
    let mut id = None;
    let mut title = None;
    let mut motd = None;
    let mut default_name = None;
    if let Some((t, sv)) = r.table(root, "world", true) {
        let line = r.line(&sv.span());
        let wv = View {
            table: t,
            line,
            path: "world",
        };
        r.check_keys(wv, &["id", "title", "motd", "default_spawn"]);
        if let Some((s, line)) = r.string(wv, "id", true)
            && id_class(&mut r, "world.id", &s, line)
        {
            id = Some(s);
        }
        if let Some((s, line)) = r.string(wv, "title", true)
            && text_class(&mut r, "world.title", &s, 0, MAX_SOFTWARE_NAME_LEN, line)
        {
            title = Some(s);
        }
        if let Some((s, line)) = r.string(wv, "motd", true)
            && text_class(&mut r, "world.motd", &s, 0, MAX_LONG_TEXT_LEN, line)
        {
            motd = Some(s);
        }
        if let Some((s, line)) = r.string(wv, "default_spawn", true) {
            default_name = Some((s, line));
        }
    }

    // [[spawn]]
    let spawn_tables = r.tables(root, "spawn");
    if spawn_tables.is_empty() && !root.table.contains_key("spawn") {
        r.push(
            1,
            Rule::Format,
            "missing required `[[spawn]]` (at least one)",
        );
    }
    if spawn_tables.len() > MAX_POSITIONS {
        let line = spawn_tables[MAX_POSITIONS].1;
        r.push(
            line,
            Rule::Positions,
            format!("more than {MAX_POSITIONS} positions (server-content.md §5.2 P4)"),
        );
    }
    let mut spawns: Vec<Spawn> = Vec::new();
    let mut spawns_ok = true;
    for (t, line) in &spawn_tables {
        let v = View {
            table: t,
            line: *line,
            path: "spawn",
        };
        match parse_spawn(&mut r, v) {
            Some(s) => {
                if spawns.iter().any(|o| o.name == s.name) {
                    r.push(
                        *line,
                        Rule::Format,
                        format!("duplicate spawn name \"{}\"", s.name),
                    );
                    spawns_ok = false;
                } else {
                    spawns.push(s);
                }
            }
            None => spawns_ok = false,
        }
    }
    let default_spawn = match &default_name {
        Some((n, line)) => match spawns.iter().position(|s| &s.name == n) {
            Some(i) => Some(i),
            None => {
                if spawns_ok {
                    r.push(
                        *line,
                        Rule::Format,
                        format!("`world.default_spawn` names no spawn (\"{n}\")"),
                    );
                }
                None
            }
        },
        None => None,
    };

    // [[wanderers]]
    let groups = r.tables(root, "wanderers");
    if groups.len() > MAX_WANDERER_GROUPS {
        r.push(
            groups[MAX_WANDERER_GROUPS].1,
            Rule::Range,
            format!("at most {MAX_WANDERER_GROUPS} wanderer groups"),
        );
    }
    let mut wanderers = Vec::new();
    let mut references = 0usize;
    let mut references_line = 0usize;
    let mut total = 0u32;
    let mut wanderers_ok = true;
    for (t, line) in &groups {
        let v = View {
            table: t,
            line: *line,
            path: "wanderers",
        };
        r.check_keys(
            v,
            &["spawn", "count", "radius", "speed", "name", "appearance"],
        );
        let spawn = match r.string(v, "spawn", true) {
            Some((n, sline)) => match spawns.iter().position(|s| s.name == n) {
                Some(i) => Some(i),
                None => {
                    if spawns_ok {
                        r.push(
                            sline,
                            Rule::Format,
                            format!("`wanderers.spawn` names no spawn (\"{n}\")"),
                        );
                    }
                    None
                }
            },
            None => None,
        };
        let count = r.int(v, "count", true, 1, 256).map(|(i, _)| i as u32);
        if let Some(c) = count {
            total = total.saturating_add(c);
            if total > MAX_WANDERERS {
                r.push(
                    *line,
                    Rule::Range,
                    format!("more than {MAX_WANDERERS} wanderers per instance"),
                );
                wanderers_ok = false;
            }
        }
        let radius = r
            .float_in(v, "radius", true, |f| (0.5..=100.0).contains(&f), "0.5–100")
            .map(|(f, _)| f);
        let speed = r
            .float_in(v, "speed", true, |f| f > 0.0 && f <= 7.5, "(0, 7.5]")
            .map(|(f, _)| f);
        let name = match r.string(v, "name", false) {
            Some((n, nline)) => {
                // The longest result: {n} replaced by a 4-digit number.
                let longest = n.replace("{n}", "1024");
                let ok_len =
                    !longest.is_empty() && longest.len() <= MAX_NAME_LEN && !has_control(&n);
                if !ok_len {
                    r.push(
                        nline,
                        Rule::Text,
                        "`wanderers.name` must give names of 1–64 bytes without control characters",
                    );
                }
                let ok_word = match words::find_rule_one(&n.replace("{n}", " ")) {
                    Some(w) => {
                        r.push(
                            nline,
                            Rule::Word,
                            format!("`wanderers.name` contains \"{w}\" (CLAUDE.md Naming rule)"),
                        );
                        false
                    }
                    None => true,
                };
                (ok_len && ok_word).then_some(n)
            }
            None if v.table.contains_key("name") => None,
            None => Some("Wanderer {n}".to_owned()),
        };
        let look = match r.value(v, "appearance", true) {
            Some(av) => match av.get_ref() {
                DeValue::String(s) if s.as_ref() == "first-player" => {
                    Some(WandererLook::FirstPlayer)
                }
                DeValue::String(s) => {
                    let line = r.line(&av.span());
                    r.push(
                        line,
                        Rule::Format,
                        format!("`wanderers.appearance` must be \"first-player\" or a table, not \"{s}\""),
                    );
                    None
                }
                DeValue::Table(at) => {
                    let line = r.line(&av.span());
                    let view = View {
                        table: at,
                        line,
                        path: "wanderers.appearance",
                    };
                    match appearance::parse(&mut r, view) {
                        Some(a) => {
                            references += appearance::reference_count(&a);
                            if references > MAX_REFERENCES && references_line == 0 {
                                references_line = line;
                            }
                            Some(WandererLook::Fixed(a))
                        }
                        None => {
                            r.push(
                                line,
                                Rule::Range,
                                "`wanderers.appearance` breaks the protocol's appearance limits",
                            );
                            None
                        }
                    }
                }
                other => {
                    let line = r.line(&av.span());
                    r.push(
                        line,
                        Rule::Format,
                        format!(
                            "`wanderers.appearance` must be a string or a table, not {}",
                            tomlx::kind(other)
                        ),
                    );
                    None
                }
            },
            None => None,
        };
        match (spawn, count, radius, speed, name, look) {
            (Some(spawn), Some(count), Some(radius), Some(speed), Some(name), Some(look)) => {
                wanderers.push(WandererGroup {
                    spawn,
                    count,
                    radius,
                    speed,
                    name,
                    look,
                });
            }
            _ => wanderers_ok = false,
        }
    }
    if references > MAX_REFERENCES {
        r.push(
            references_line.max(1),
            Rule::References,
            format!(
                "{references} opaque references; at most {MAX_REFERENCES} (server-content.md §9.2)"
            ),
        );
    }

    // [clock]
    let mut clock_start = Some(0.0);
    let mut clock_rate = Some(1.0f32);
    if let Some((t, sv)) = r.table(root, "clock", false) {
        let line = r.line(&sv.span());
        let cv = View {
            table: t,
            line,
            path: "clock",
        };
        r.check_keys(cv, &["start", "rate"]);
        if t.contains_key("start") {
            clock_start = r
                .float_in(cv, "start", false, |f| f >= 0.0, "≥ 0")
                .map(|(f, _)| f);
        }
        if t.contains_key("rate") {
            clock_rate = r
                .float_in(
                    cv,
                    "rate",
                    false,
                    |f| (0.0..=10_000.0).contains(&f),
                    "0–10000",
                )
                .map(|(f, _)| f as f32);
        }
    }

    findings.extend(r.findings);
    findings.sort();
    let content = (|| {
        if !spawns_ok || !wanderers_ok {
            return None;
        }
        Some(Content {
            id: id?,
            title: title?,
            motd: motd?,
            default_spawn: default_spawn?,
            spawns,
            wanderers,
            clock_start: clock_start?,
            clock_rate: clock_rate?,
        })
    })();
    (content, findings)
}

/// Checks a content file (`check-content FILE`): the install guard, then [`check_text`]
/// with the SPDX rule. I/O errors become `SYNTAX` findings at line 0.
#[must_use]
pub fn check_file(path: &Path, spdx: bool) -> (Option<Content>, Vec<Finding>) {
    if crate::datadir::install_guard_hit(path) {
        return (
            None,
            vec![Finding::new(
                0,
                Rule::InstallPath,
                "the file lies in or next to a game installation; it is not read",
            )],
        );
    }
    match tomlx::read_text(path) {
        Ok(Ok(text)) => check_text(&text, spdx),
        Ok(Err(f)) => (None, vec![f]),
        Err(e) => (
            None,
            vec![Finding::new(
                0,
                Rule::Syntax,
                format!("cannot read the file: {e}"),
            )],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_world_is_valid() {
        let (c, f) = check_text(DEFAULT_WORLD, true);
        assert!(f.is_empty(), "{f:?}");
        let c = c.unwrap();
        assert_eq!(c.id, "default");
        assert_eq!(c.spawns.len(), 2);
        assert!(c.check_invariants().is_empty());
        let start = c.default_spawn().unwrap();
        assert_eq!(start.name, "start");
        assert_eq!(start.location, Location::new(1, 20_240.0, 22_420.0, 440.0));
        assert_eq!(start.anchor, None);
        assert!(start.ground);
        assert!((start.heading - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        let void = c.spawn_named("VOID").unwrap();
        assert_eq!(void.location.region, 0);
        assert_eq!(c.wanderers.len(), 1);
        assert_eq!(c.wanderers[0].look, WandererLook::FirstPlayer);
        assert_eq!(c.wanderer_total(), 3);
        let mut broken = c.clone();
        broken.spawns.clear();
        assert!(broken.default_spawn().is_none());
        assert!(!broken.check_invariants().is_empty());
    }

    fn rules(text: &str) -> Vec<(usize, Rule)> {
        check_text(text, true)
            .1
            .iter()
            .map(|f| (f.line, f.rule))
            .collect()
    }

    #[test]
    fn findings() {
        assert_eq!(
            rules("format = 1 = 2"),
            vec![(1, Rule::Syntax), (1, Rule::Spdx)]
        );
        let base = "# SPDX-License-Identifier: CC0-1.0\nformat = 1\n[world]\nid = \"t\"\ntitle = \"x\"\nmotd = \"\"\ndefault_spawn = \"a\"\n[[spawn]]\nname = \"a\"\nregion = 0\nlocation = [1.0, 2.0, 3.0]\n";
        assert!(rules(base).is_empty(), "{:?}", rules(base));
        // Unknown key, bad format.
        let t = base.replace("format = 1", "format = 2\nextra = 1");
        let r = rules(&t);
        assert!(
            r.contains(&(2, Rule::Format)) && r.contains(&(3, Rule::Format)),
            "{r:?}"
        );
        // Word check on the title, at word boundaries only.
        let t = base.replace("title = \"x\"", "title = \"Mordor test\"");
        assert!(rules(&t).contains(&(5, Rule::Word)));
        let t = base.replace("title = \"x\"", "title = \"Shirelike\"");
        assert!(rules(&t).is_empty());
        // Wrong licence and missing header.
        let t = base.replace("CC0-1.0", "MIT");
        assert_eq!(rules(&t), vec![(1, Rule::Spdx)]);
        let t = base.replacen("# SPDX-License-Identifier: CC0-1.0\n", "", 1);
        assert_eq!(rules(&t), vec![(1, Rule::Spdx)]);
        assert!(check_text(&t, false).1.is_empty());
        // Range.
        let t = base.replace("region = 0", "region = 70000");
        assert!(rules(&t).contains(&(10, Rule::Range)));
        // Too many positions.
        let mut t = base.to_owned();
        for i in 0..16 {
            t.push_str(&format!(
                "[[spawn]]\nname = \"s{i}\"\nregion = 0\nlocation = [1.0, 2.0, 3.0]\n"
            ));
        }
        assert!(rules(&t).iter().any(|(_, r)| *r == Rule::Positions));
        // Too many references.
        let mut t = base.to_owned();
        t.push_str("[[wanderers]]\nspawn = \"a\"\ncount = 1\nradius = 1.0\nspeed = 1.0\nappearance = { body = { species = 1, sex = 1 }, scale = 1.0, idle = 3, items = [");
        for i in 1..=7 {
            t.push_str(&format!("{{ did = {i}, stowed = false }}, "));
        }
        t.push_str("] }\n");
        assert!(
            rules(&t).iter().any(|(_, r)| *r == Rule::References),
            "{:?}",
            rules(&t)
        );
        // Bad spawn name and dangling default.
        let t = base.replace("name = \"a\"", "name = \"A b\"");
        let r = rules(&t);
        assert!(r.contains(&(9, Rule::Text)), "{r:?}");
        // Interior ground spawn.
        let t = base.replace(
            "location = [1.0, 2.0, 3.0]",
            "block = [1, 1]\noffset = [-10.0, 5.0, 0.0]\nground = true",
        );
        assert!(rules(&t).iter().any(|(_, r)| *r == Rule::Range));
    }

    #[test]
    fn headings() {
        assert!((heading_from_compass(0.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(heading_from_compass(90.0).abs() < 1e-6);
        assert!((heading_from_compass(180.0) + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(compass_from_heading(0.0), 90);
        assert_eq!(compass_from_heading(std::f32::consts::FRAC_PI_2), 0);
    }
}
