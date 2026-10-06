// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure inputs of the walker's M4.5 systems (`walker.md` §14): the profiling switches
//! ([`Systems`], `--disable`), the looping camera path of the performance walk spots
//! ([`CameraLoop`], `--camera-loop`, `m4-5-visual.md` §3) and the game-clock arithmetic of
//! `--day` / `--time-of-day` ([`clock_seconds`], `environment.md` §11.1).
//!
//! Plain data in, plain data out (no I/O, no GPU), so the fuzz target `client_world_fx`
//! includes this file by path. Every input is hostile: lengths are capped before any work,
//! numbers are checked for range and finiteness, and nothing panics.

/// Longest `--disable` or `--camera-loop` value accepted.
pub const MAX_FX_TEXT: usize = 1024;
/// Most waypoints of a camera loop.
pub const MAX_LOOP_POINTS: usize = 64;
/// Side of a landblock in world units (`coordinates.md` §4).
const BLOCK: f64 = 160.0;
/// Largest region number (`landblock.md` §2.1).
const MAX_REGION: u8 = 15;
/// Largest landblock index.
const MAX_BLOCK_INDEX: u8 = 254;

/// One M4.5 system of the walker, switchable for profiling (`walker.md` §14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum System {
    /// Terrain texture layers (`terrain-texturing.md`); off: the M3 height-shaded meshes.
    TerrainLayers,
    /// The lit pipeline (`lighting.md` §5); off: the M2/M3 studio lighting, which also
    /// turns off every system that needs a lit pass (layers, water, sky, instancing, frills).
    Lit,
    /// The sun's shadow map (`lighting.md` §9).
    Shadows,
    /// Point lights of Light entities (`lighting.md` §8.3).
    Lights,
    /// Landblock water (`water.md` §8).
    Water,
    /// The sky objects and the environment's sun, ambient and fog (`environment.md` §11–§12);
    /// off: the defaults of `lighting.md` §8.1 and a fixed clear colour.
    Sky,
    /// Distance fog (`lighting.md` §10) and the fog clip of the global limit.
    Fog,
    /// Generated scenery (`scenery.md` §7).
    Scenery,
    /// Instanced drawing of static objects and generated scenery (`docs/specs/render.md`
    /// §5.5).
    Instancing,
    /// Frills (`scenery.md` §8).
    Frills,
    /// Per-class draw distances, fades and detail levels (`walker.md` §11.4); off: every
    /// object to the distance cap at its finest level.
    Lod,
}

impl System {
    /// Every system, in the order of [`System::name`]s on the HUD.
    pub const ALL: [Self; 11] = [
        Self::TerrainLayers,
        Self::Lit,
        Self::Shadows,
        Self::Lights,
        Self::Water,
        Self::Sky,
        Self::Fog,
        Self::Scenery,
        Self::Instancing,
        Self::Frills,
        Self::Lod,
    ];

    /// The name used by `--disable` and the HUD.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TerrainLayers => "layers",
            Self::Lit => "lit",
            Self::Shadows => "shadows",
            Self::Lights => "lights",
            Self::Water => "water",
            Self::Sky => "sky",
            Self::Fog => "fog",
            Self::Scenery => "scenery",
            Self::Instancing => "instancing",
            Self::Frills => "frills",
            Self::Lod => "lod",
        }
    }

    /// The system named `name` (case-insensitive; `terrain-layers` and `terrain` also
    /// name [`System::TerrainLayers`]).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.eq_ignore_ascii_case("terrain") || n.eq_ignore_ascii_case("terrain-layers") {
            return Some(Self::TerrainLayers);
        }
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(n))
    }
}

/// Which M4.5 systems are on (all by default: the default-settings profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Systems {
    bits: u16,
}

impl Default for Systems {
    fn default() -> Self {
        Self::all()
    }
}

impl Systems {
    /// Every system on.
    #[must_use]
    pub fn all() -> Self {
        Self {
            bits: (1 << System::ALL.len()) - 1,
        }
    }

    /// Every system off (the M3/M4 walker).
    #[must_use]
    pub fn none() -> Self {
        Self { bits: 0 }
    }

    fn bit(s: System) -> u16 {
        let i = System::ALL.iter().position(|x| *x == s).unwrap_or(0);
        1 << i
    }

    /// Whether `s` is switched on (whatever the systems it needs).
    #[must_use]
    pub fn switched(&self, s: System) -> bool {
        self.bits & Self::bit(s) != 0
    }

    /// Whether `s` is in effect: switched on, and the systems it needs are in effect (the
    /// lit pass for layers, shadows, lights, water, sky, instancing and frills).
    #[must_use]
    pub fn on(&self, s: System) -> bool {
        let lit = self.switched(System::Lit);
        match s {
            System::Lit | System::Scenery | System::Lod | System::Fog => self.switched(s),
            _ => lit && self.switched(s),
        }
    }

    /// Switches `s` on or off.
    pub fn set(&mut self, s: System, on: bool) {
        if on {
            self.bits |= Self::bit(s);
        } else {
            self.bits &= !Self::bit(s);
        }
    }

    /// Flips `s`; returns its new switch state.
    pub fn toggle(&mut self, s: System) -> bool {
        let on = !self.switched(s);
        self.set(s, on);
        on
    }

    /// The names of the systems switched off, comma-separated ("none" when all are on).
    #[must_use]
    pub fn disabled_text(&self) -> String {
        let off: Vec<&str> = System::ALL
            .into_iter()
            .filter(|s| !self.switched(*s))
            .map(System::name)
            .collect();
        if off.is_empty() {
            "none".to_owned()
        } else {
            off.join(",")
        }
    }
}

/// A parsed `--disable` list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemList(pub Vec<System>);

/// Parses a `--disable` list: system names separated by commas (`all` switches every
/// system off). Empty items are ignored.
///
/// # Errors
///
/// A message naming the first unknown name, or a text over [`MAX_FX_TEXT`].
pub fn parse_systems(text: &str) -> Result<Vec<System>, String> {
    if text.len() > MAX_FX_TEXT {
        return Err("system list too long".into());
    }
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if item.eq_ignore_ascii_case("all") {
            return Ok(System::ALL.to_vec());
        }
        match System::from_name(item) {
            Some(s) if !out.contains(&s) => out.push(s),
            Some(_) => {}
            None => {
                let names: Vec<&str> = System::ALL.iter().map(|s| s.name()).collect();
                return Err(format!(
                    "unknown system {:?} (known: {}, all)",
                    item.chars().take(24).collect::<String>(),
                    names.join(", ")
                ));
            }
        }
    }
    Ok(out)
}

/// A closed camera path through block-local waypoints of one landblock, flown at a
/// constant speed (`m4-5-visual.md` §3).
#[derive(Debug, Clone, PartialEq)]
pub struct CameraLoop {
    /// Region.
    pub region: u8,
    /// Landblock X.
    pub x: u8,
    /// Landblock Y.
    pub y: u8,
    /// Waypoints, block-local (they may lie outside 0 … 160), at least two.
    pub points: Vec<[f64; 2]>,
}

impl CameraLoop {
    /// The region-global position of a block-local point.
    #[must_use]
    pub fn global(&self, p: [f64; 2]) -> [f64; 2] {
        [
            f64::from(self.x) * BLOCK + p[0],
            f64::from(self.y) * BLOCK + p[1],
        ]
    }

    /// Length of the closed loop in world units.
    #[must_use]
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        (0..n)
            .map(|i| {
                let a = self.points[i];
                let b = self.points[(i + 1) % n];
                (b[0] - a[0]).hypot(b[1] - a[1])
            })
            .sum()
    }

    /// The block-local point at distance `d` along the loop (wrapped) and the heading of
    /// its segment in radians (0 = east, counter-clockwise). The first waypoint for a
    /// loop of zero length or a non-finite `d`.
    #[must_use]
    pub fn at(&self, d: f64) -> ([f64; 2], f64) {
        let len = self.length();
        let first = self.points.first().copied().unwrap_or([0.0; 2]);
        if !(len.is_finite() && len > 0.0 && d.is_finite()) {
            let h = self
                .points
                .get(1)
                .map_or(0.0, |b| (b[1] - first[1]).atan2(b[0] - first[0]));
            return (first, h);
        }
        let mut rest = d.rem_euclid(len);
        let n = self.points.len();
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + 1) % n];
            let seg = (b[0] - a[0]).hypot(b[1] - a[1]);
            if rest <= seg && seg > 0.0 {
                let t = rest / seg;
                return (
                    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t],
                    (b[1] - a[1]).atan2(b[0] - a[0]),
                );
            }
            rest -= seg;
        }
        (first, 0.0)
    }
}

fn parse_u8_in(s: &str, lo: u8, hi: u8, what: &str) -> Result<u8, String> {
    s.trim()
        .parse::<u8>()
        .ok()
        .filter(|v| (lo..=hi).contains(v))
        .ok_or_else(|| format!("{what} must be {lo}..={hi}"))
}

/// Parses `--camera-loop`: `R:X:Y:x,y;x,y[;x,y…]` (region, landblock and at least two
/// block-local waypoints within ±1,000 units of the block; at most [`MAX_LOOP_POINTS`]).
///
/// # Errors
///
/// A message naming the bad field.
pub fn parse_camera_loop(text: &str) -> Result<CameraLoop, String> {
    if text.len() > MAX_FX_TEXT {
        return Err("camera loop too long".into());
    }
    let mut head = text.splitn(4, ':');
    let (Some(r), Some(x), Some(y), Some(rest)) =
        (head.next(), head.next(), head.next(), head.next())
    else {
        return Err("expected R:X:Y:x,y;x,y[;...]".into());
    };
    let region = parse_u8_in(r, 1, MAX_REGION, "region")?;
    let x = parse_u8_in(x, 0, MAX_BLOCK_INDEX, "landblock X")?;
    let y = parse_u8_in(y, 0, MAX_BLOCK_INDEX, "landblock Y")?;
    let mut points = Vec::new();
    for item in rest.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if points.len() >= MAX_LOOP_POINTS {
            return Err(format!("at most {MAX_LOOP_POINTS} waypoints"));
        }
        let (a, b) = item
            .split_once(',')
            .ok_or_else(|| "a waypoint is x,y".to_owned())?;
        let v = |s: &str| {
            s.trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && v.abs() <= 1_000.0)
                .ok_or_else(|| "waypoint coordinates must be numbers within ±1000".to_owned())
        };
        points.push([v(a)?, v(b)?]);
    }
    if points.len() < 2 {
        return Err("a camera loop needs at least two waypoints".into());
    }
    Ok(CameraLoop {
        region,
        x,
        y,
        points,
    })
}

/// The clock value *G* (game seconds) at which the clock of `environment.md` §11.1 reads
/// game day number `day` and time of day `t` (wrapped into [0, 1)): with *Y* =
/// `days_per_year` × `day_length`, year = ⌊`day` / `days_per_year`⌋, *d* = `day` mod
/// `days_per_year`, *G* = (year − `zero_year`) × *Y* + (*d* + *t*) × `day_length` −
/// `zero_offset`. `None` for a calendar that cannot drive a clock (non-positive or
/// non-finite day length, zero days per year) or non-finite inputs.
#[must_use]
pub fn clock_seconds(
    days_per_year: u32,
    day_length: f64,
    zero_offset: f64,
    zero_year: u32,
    day: i64,
    t: f64,
) -> Option<f64> {
    if days_per_year == 0
        || !(day_length.is_finite() && day_length > 0.0)
        || !zero_offset.is_finite()
        || !t.is_finite()
    {
        return None;
    }
    let dpy = i64::from(days_per_year);
    let year = day.div_euclid(dpy);
    let d = day.rem_euclid(dpy);
    let year_len = f64::from(days_per_year) * day_length;
    // In f64: `year − zero_year` overflows i64 for a day near i64::MIN with one day a year.
    let g = (year as f64 - f64::from(zero_year)) * year_len
        + (d as f64 + t.rem_euclid(1.0)) * day_length
        - zero_offset;
    g.is_finite().then_some(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_names_round_trip() {
        for s in System::ALL {
            assert_eq!(System::from_name(s.name()), Some(s));
            assert_eq!(System::from_name(&s.name().to_uppercase()), Some(s));
        }
        assert_eq!(System::from_name("terrain"), Some(System::TerrainLayers));
        assert_eq!(System::from_name("x"), None);
    }

    #[test]
    fn disable_lists() {
        assert_eq!(parse_systems(""), Ok(vec![]));
        assert_eq!(
            parse_systems("water, frills,water"),
            Ok(vec![System::Water, System::Frills])
        );
        assert_eq!(parse_systems("all").map(|v| v.len()), Ok(System::ALL.len()));
        assert!(parse_systems("water,bogus").is_err());
        assert!(parse_systems(&"a,".repeat(2000)).is_err());
    }

    #[test]
    fn dependent_systems_follow_the_lit_pass() {
        let mut s = Systems::all();
        assert!(System::ALL.iter().all(|x| s.on(*x)));
        s.set(System::Lit, false);
        assert!(!s.on(System::Water) && !s.on(System::TerrainLayers) && !s.on(System::Frills));
        assert!(s.on(System::Lod) && s.on(System::Scenery) && s.on(System::Fog));
        assert!(s.switched(System::Water));
        s.set(System::Lit, true);
        s.set(System::Scenery, false);
        assert!(s.on(System::Instancing) && !s.on(System::Scenery));
        assert!(s.toggle(System::Scenery));
        assert_eq!(Systems::all().disabled_text(), "none");
        let mut d = Systems::all();
        d.set(System::Water, false);
        d.set(System::Sky, false);
        assert_eq!(d.disabled_text(), "water,sky");
        assert_eq!(
            Systems::none().disabled_text().split(',').count(),
            System::ALL.len()
        );
    }

    #[test]
    fn camera_loops() {
        let l = parse_camera_loop("2:119:68:30,30;130,30;130,125;30,125").unwrap();
        assert_eq!((l.region, l.x, l.y, l.points.len()), (2, 119, 68, 4));
        assert!((l.length() - 390.0).abs() < 1e-9);
        let (p, h) = l.at(50.0);
        assert_eq!(p, [80.0, 30.0]);
        assert!(h.abs() < 1e-12);
        let (p, h) = l.at(100.0 + 95.0 + 50.0);
        assert_eq!(p, [80.0, 125.0]);
        assert!((h - std::f64::consts::PI).abs() < 1e-12);
        // Wraps around.
        assert_eq!(l.at(390.0 + 50.0).0, [80.0, 30.0]);
        assert_eq!(l.at(-340.0).0, [80.0, 30.0]);
        assert_eq!(l.at(f64::NAN).0, [30.0, 30.0]);
        assert_eq!(
            l.global([1.0, 2.0]),
            [119.0 * 160.0 + 1.0, 68.0 * 160.0 + 2.0]
        );
        for bad in [
            "",
            "1:2:3",
            "0:2:3:1,1;2,2",
            "1:255:3:1,1;2,2",
            "1:2:3:1,1",
            "1:2:3:1,1;nan,2",
            "1:2:3:1,1;5000,2",
            "1:2:3:1;2,2",
        ] {
            assert!(parse_camera_loop(bad).is_err(), "{bad}");
        }
        let many = format!("1:2:3:{}", "1,1;".repeat(65));
        assert!(parse_camera_loop(&many).is_err());
    }

    #[test]
    fn clock_seconds_reads_back() {
        // environment.md §14: G = 0 reads day 12,240 at t = 3,600 / 11,160.
        let (dpy, len, off, zy) = (360, 11_160.0, 3_600.0, 34);
        let t0 = 3_600.0 / 11_160.0;
        let g = clock_seconds(dpy, len, off, zy, 12_240, t0).unwrap();
        assert!(g.abs() < 1e-6, "{g}");
        let g = clock_seconds(dpy, len, off, zy, 12_240, 0.5).unwrap();
        assert!((g - (0.5 * 11_160.0 - 3_600.0)).abs() < 1e-6);
        let g2 = clock_seconds(dpy, len, off, zy, 12_241, 0.5).unwrap();
        assert!((g2 - g - 11_160.0).abs() < 1e-6);
        assert!(clock_seconds(0, len, off, zy, 1, 0.5).is_none());
        assert!(clock_seconds(dpy, f64::NAN, off, zy, 1, 0.5).is_none());
        assert!(clock_seconds(dpy, len, off, zy, 1, f64::INFINITY).is_none());
        assert!(clock_seconds(dpy, len, off, zy, i64::MIN, 0.5).is_some());
        // One day a year: the year is the day itself, and year − zero_year must not
        // overflow (review finding; the fuzz target reaches it).
        assert!(clock_seconds(1, 1.0, 0.0, 1, i64::MIN, 0.0).is_some());
        assert!(clock_seconds(1, 1.0, 0.0, u32::MAX, i64::MIN, 0.0).is_some());
        assert!(clock_seconds(1, 1.0, 0.0, 0, i64::MAX, 0.0).is_some());
    }
}
