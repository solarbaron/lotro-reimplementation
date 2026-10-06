// SPDX-License-Identifier: GPL-3.0-or-later
//! Record tables of the M4.5 world records for the record view: terrain type tables
//! (0x19, `terrain-texturing.md` §3), terrain-scene and scene-choice tables (0x12, 0x11,
//! `scenery.md` §4.2–§4.3), scene records (0x02, §5), water and fog-zone tables (0x16,
//! 0x17, `water.md` §4–§5), the sky description (0x15), day descriptions (0x1A), game time
//! (0x35), the LOD-class table (0x32) and camera states (0x08) (`environment.md` §5, §6,
//! §9.1, `walker.md` §11.5), collision meshes (0x66) and cell geometry (0x01)
//! (`collision.md` §4–§5), and region descriptors (0x0F, `environment.md` §3).
//!
//! Everything here formats decoded records; nothing reads the archive. Tables are bounded
//! by [`crate::decoders::MAX_ROWS`].

use tessera_formats::environment::{CameraBody, FloatCurve, ListRef};
use tessera_formats::scenery::{PlacementNode, SceneChild, SceneSite};
use tessera_formats::terrain_type::{Argb, TextureProjection};
use tessera_formats::{AnyRecord, Did, RegionTable};

use crate::decoders::{MAX_ROWS, RecordProduct, RecordRow};

/// A DID as text: `0x…` or `—` for 0.
fn did(d: Did) -> String {
    if d.is_null() {
        "—".into()
    } else {
        format!("{:#010x}", d.0)
    }
}

/// A non-null DID as a link target.
fn link(d: Did) -> Option<u32> {
    (!d.is_null()).then_some(d.0)
}

/// A colour as `#RRGGBB` plus alpha when it is not opaque.
pub(crate) fn colour(c: Argb) -> String {
    if c.alpha() == 255 {
        format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue())
    } else {
        format!(
            "#{:02X}{:02X}{:02X} a{}",
            c.red(),
            c.green(),
            c.blue(),
            c.alpha()
        )
    }
}

/// A float curve in short form (`material.md` §5.3).
fn curve(c: &FloatCurve) -> String {
    match c {
        FloatCurve::Constant(v) => format!("{v}"),
        FloatCurve::Waveform { mode, w } => format!("mode {} {:?}", mode.get(), &w[..4]),
        FloatCurve::Keyframed {
            period,
            looping,
            keys,
            ..
        } => format!(
            "{} keys / {period} s{}",
            keys.len(),
            if *looping { " loop" } else { "" }
        ),
    }
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// Short flags text: the names of the set flags, joined.
fn flags(pairs: &[(bool, &str)]) -> String {
    let set: Vec<&str> = pairs.iter().filter(|(b, _)| *b).map(|(_, n)| *n).collect();
    if set.is_empty() {
        "—".into()
    } else {
        set.join(" ")
    }
}

fn range(r: tessera_formats::scenery::FloatRange) -> String {
    format!("{}…{}", r.min, r.max)
}

/// Adds the record's summary and table to `p` when it is one of the M4.5 world records;
/// returns whether it was. Sets [`RecordProduct::link_column`] for tables whose rows link
/// to a record.
pub(crate) fn world_record(p: &mut RecordProduct, rec: &AnyRecord<'_>) -> bool {
    match rec {
        AnyRecord::TerrainTypeTable(t) => terrain_types(p, t),
        AnyRecord::TerrainSceneTable(t) => terrain_scenes(p, t),
        AnyRecord::SceneChoiceTable(t) => scene_choices(p, t),
        AnyRecord::Scene(s) => scene(p, s),
        AnyRecord::WaterTable(t) => water_table(p, t),
        AnyRecord::FogZoneTable(t) => fog_zones(p, t),
        AnyRecord::SkyDescription(s) => sky(p, s),
        AnyRecord::DayDescription(d) => day(p, d),
        AnyRecord::GameTime(g) => game_time(p, g),
        AnyRecord::LodClassTable(t) => {
            p.summary.push(("classes".into(), t.len().to_string()));
            p.columns = vec![
                "class", "max", "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7",
            ];
            for c in t.entries.iter().take(MAX_ROWS) {
                let mut cells = vec![c.class.to_string(), format!("{}", c.max_distance())];
                cells.extend(c.values.iter().map(|v| format!("{v}")));
                p.rows.push(RecordRow { cells, link: None });
            }
        }
        AnyRecord::CameraState { did: d, state } => {
            p.summary.push(("DID".into(), did(*d)));
            p.summary.push((
                "controller".into(),
                match &state.body {
                    CameraBody::ThirdPerson(_) => "third person".into(),
                    CameraBody::FirstPerson(_) => "first person".into(),
                    CameraBody::Cinematic(_) => "cinematic".into(),
                    CameraBody::Flight(_) => "flight".into(),
                },
            ));
            p.summary
                .push(("fov".into(), format!("{}", state.body.fov())));
            p.summary.push((
                "clip".into(),
                format!("{} … {}", state.near_clip, state.far_clip),
            ));
            p.summary
                .push(("physics".into(), yes(state.uses_physics).into()));
            if let Some(l) = link(state.entity) {
                p.links.push(("entity".into(), l));
            }
        }
        AnyRecord::CollisionMesh { did: d, mesh } => {
            p.summary.push(("DID".into(), did(*d)));
            p.summary
                .push(("vertices".into(), mesh.vertices.len().to_string()));
            p.summary
                .push(("triangles".into(), mesh.triangles.len().to_string()));
            p.summary.push((
                "tree".into(),
                format!(
                    "{} code bytes, scale {}, build byte {:?}",
                    mesh.tree.code.len(),
                    mesh.tree.scale,
                    mesh.tree.build_byte
                ),
            ));
            if let Some((lo, hi)) = mesh.bounds() {
                p.summary
                    .push(("bounds".into(), format!("{lo:?} … {hi:?}")));
            }
            p.summary
                .push(("note".into(), "see the 3D view for the mesh".into()));
            p.columns = vec!["triangle", "a", "b", "c"];
            for (i, t) in mesh.triangles.iter().enumerate().take(MAX_ROWS) {
                p.rows.push(RecordRow {
                    cells: vec![
                        i.to_string(),
                        t[0].to_string(),
                        t[1].to_string(),
                        t[2].to_string(),
                    ],
                    link: None,
                });
            }
        }
        AnyRecord::CellGeometry(c) => cell(p, c),
        AnyRecord::RegionDescriptor(r) => {
            p.summary.push(("region".into(), r.region.to_string()));
            p.summary.push(("map image".into(), did(r.map_image)));
            p.summary.push((
                "map origin".into(),
                format!("{} {}", r.map_origin[0], r.map_origin[1]),
            ));
            p.summary
                .push(("map scale".into(), format!("{}", r.map_scale)));
            p.columns = vec!["slot", "table", "did"];
            p.link_column = Some(2);
            for t in RegionTable::ALL {
                let d = r.table(t).unwrap_or(Did(0));
                p.rows.push(RecordRow {
                    cells: vec![t.slot().to_string(), t.name().into(), did(d)],
                    link: link(d),
                });
            }
        }
        _ => return false,
    }
    true
}

fn terrain_types(p: &mut RecordProduct, t: &tessera_formats::TerrainTypeTable) {
    p.summary.push(("entries".into(), t.len().to_string()));
    p.summary.push((
        "procedural".into(),
        t.entries()
            .iter()
            .filter(|e| e.procedural)
            .count()
            .to_string(),
    ));
    p.columns = vec![
        "key",
        "label",
        "order",
        "diffuse",
        "normal",
        "u×v scale",
        "rot",
        "offset u / v",
        "axis",
        "switches",
        "alpha test / max",
        "physics",
        "colours",
        "spec power",
    ];
    p.link_column = Some(3);
    for e in t.iter_by_key().take(MAX_ROWS) {
        let switches = flags(&[
            (e.use_opacity_mask, "opacity"),
            (e.use_specular, "spec"),
            (e.use_gloss_specular, "gloss"),
            (e.specular_from_normal, "spec-from-normal"),
            (e.slope_orientation, "slope"),
            (e.procedural, "procedural"),
            (!e.emissive.is_null(), "emissive"),
            (!e.glow.is_null(), "glow"),
        ]);
        p.rows.push(RecordRow {
            cells: vec![
                e.key.to_string(),
                e.label.clone(),
                e.draw_order.to_string(),
                did(e.diffuse),
                did(e.normal),
                format!("{}×{}", e.u_scale, e.v_scale),
                format!("{}", e.rotation),
                format!("{} / {}", curve(&e.u_offset), curve(&e.v_offset)),
                match e.projection {
                    TextureProjection::Xz => "XZ".into(),
                    TextureProjection::Yz => "YZ".into(),
                    TextureProjection::Xy => "XY".into(),
                },
                switches,
                format!("{} / {}", e.alpha_test_threshold, e.alpha_blend_max),
                did(e.physics_material),
                format!(
                    "×{} glow {} spec {}",
                    colour(e.diffuse_scale),
                    colour(e.glow_colour),
                    colour(e.specular_colour)
                ),
                format!("{}", e.specular_power),
            ],
            link: link(e.diffuse),
        });
    }
}

fn terrain_scenes(p: &mut RecordProduct, t: &tessera_formats::TerrainSceneTable) {
    p.summary
        .push(("entries".into(), t.entries().len().to_string()));
    if let Some(d) = t.default_entry() {
        p.summary.push(("default".into(), d.name.clone()));
    }
    p.columns = vec!["id", "name", "kind", "unknown_f", "default pair", "pairs"];
    for e in t.entries().iter().take(MAX_ROWS) {
        let pairs: Vec<String> = e
            .pairs
            .iter()
            .take(32)
            .map(|q| format!("{}→{:#x}", q.slot, q.scene_key))
            .collect();
        let more = e.pairs.len().saturating_sub(32);
        p.rows.push(RecordRow {
            cells: vec![
                e.id.to_string(),
                e.name.clone(),
                e.kind.to_string(),
                format!("{}", e.unknown_f),
                e.default_pair
                    .map_or_else(|| "—".into(), |q| format!("{}→{:#x}", q.slot, q.scene_key)),
                if more > 0 {
                    format!("{} … (+{more})", pairs.join(" "))
                } else {
                    pairs.join(" ")
                },
            ],
            link: None,
        });
    }
}

fn scene_choices(p: &mut RecordProduct, t: &tessera_formats::SceneChoiceTable) {
    p.summary
        .push(("keys".into(), t.entries().len().to_string()));
    let choices: usize = t.entries().iter().map(|e| e.choices.len()).sum();
    p.summary.push(("choices".into(), choices.to_string()));
    p.columns = vec!["key", "choice", "scene", "weight", "share", "flag"];
    p.link_column = Some(2);
    'outer: for e in t.entries() {
        let total = e.total_weight();
        for (i, c) in e.choices.iter().enumerate() {
            if p.rows.len() >= MAX_ROWS {
                break 'outer;
            }
            let share = if total > 0.0 && total.is_finite() {
                format!("{:.1} %", 100.0 * c.weight / total)
            } else {
                "—".into()
            };
            p.rows.push(RecordRow {
                cells: vec![
                    format!("{:#x}", e.key),
                    i.to_string(),
                    did(c.scene),
                    format!("{}", c.weight),
                    share,
                    c.flag.to_string(),
                ],
                link: link(c.scene),
            });
        }
    }
}

/// One-line description of a placement node.
fn node_text(n: &PlacementNode) -> String {
    format!(
        "freq {} · disp {:?}…{:?} · scale {:?}…{:?} · heading {} · slope cos {}…{} · \
         height mode {} · spacing {} · clearance {} · max {}{}",
        n.frequency,
        n.disp_min,
        n.disp_max,
        n.scale_min,
        n.scale_max,
        range(n.heading),
        n.slope_min_cos,
        n.slope_max_cos,
        n.height_mode,
        n.spacing,
        n.clearance,
        n.max_objects,
        {
            let f = flags(&[
                (n.align_to_slope, "align-to-slope"),
                (n.face_direction, "face-direction"),
                (n.no_checks, "no-checks"),
                (n.no_spacing, "no-spacing"),
                (n.spacing_3d, "spacing-3d"),
                (n.disp_relative, "disp-relative"),
                (n.orient_relative, "orient-relative"),
                (n.inherit_scale, "inherit-scale"),
            ]);
            if f == "—" {
                String::new()
            } else {
                format!(" · {f}")
            }
        }
    )
}

fn scene(p: &mut RecordProduct, s: &tessera_formats::Scene) {
    p.summary
        .push(("groups".into(), s.groups.len().to_string()));
    p.summary
        .push(("frill layer".into(), yes(s.frill_layer.is_some()).into()));
    p.summary
        .push(("objects".into(), s.objects().count().to_string()));
    p.summary.push((
        "scene references".into(),
        s.scene_references().count().to_string(),
    ));
    p.summary.push(("flag_a".into(), yes(s.flag_a).into()));
    p.columns = vec!["site", "depth", "child", "target", "placement"];
    p.link_column = Some(3);
    if let Some(root) = &s.frill_layer {
        p.rows.push(RecordRow {
            cells: vec![
                "frill layer".into(),
                "1".into(),
                "node (root)".into(),
                "—".into(),
                node_text(root),
            ],
            link: None,
        });
    }
    for c in s.descendants().take(MAX_ROWS) {
        let site = match c.site {
            SceneSite::FrillLayer => "frill layer".to_owned(),
            SceneSite::Group(g) => format!("group {g}"),
        };
        let indent = "  ".repeat(c.depth.saturating_sub(1).min(16));
        let (kind, target, text, l) = match c.child {
            SceneChild::Object(d) => ("object", did(*d), String::new(), link(*d)),
            SceneChild::Scene(d) => ("scene", did(*d), String::new(), link(*d)),
            SceneChild::Node(n) => ("node", "—".into(), node_text(n), None),
        };
        p.rows.push(RecordRow {
            cells: vec![
                site,
                c.depth.to_string(),
                format!("{indent}{kind}"),
                target,
                text,
            ],
            link: l,
        });
    }
}

fn water_table(p: &mut RecordProduct, t: &tessera_formats::WaterTable) {
    p.summary
        .push(("entries".into(), t.entries().len().to_string()));
    p.summary
        .push(("level key".into(), format!("{:#x}", t.level_key)));
    p.summary
        .push(("type key".into(), format!("{:#x}", t.type_key)));
    p.summary.push((
        "default".into(),
        t.default.as_ref().map_or_else(
            || "none".into(),
            |d| {
                if d.is_dry() {
                    format!("key {} (dry)", d.key)
                } else {
                    format!("key {}", d.key)
                }
            },
        ),
    ));
    p.columns = vec!["key", "kind", "surface", "volume entity", "extra", "value"];
    p.link_column = Some(2);
    let rows = t
        .default
        .iter()
        .map(|d| (true, d))
        .chain(t.entries().iter().map(|e| (false, e)));
    for (default, e) in rows.take(MAX_ROWS) {
        let key = if default {
            format!("{} (default)", e.key)
        } else {
            e.key.to_string()
        };
        match &e.wet {
            None => p.rows.push(RecordRow {
                cells: vec![
                    key,
                    "dry".into(),
                    "—".into(),
                    "—".into(),
                    "—".into(),
                    "—".into(),
                ],
                link: None,
            }),
            Some(w) => p.rows.push(RecordRow {
                cells: vec![
                    key,
                    "wet".into(),
                    did(w.surface),
                    did(w.volume_entity),
                    w.extra
                        .iter()
                        .map(|d| did(*d))
                        .collect::<Vec<_>>()
                        .join(" "),
                    format!("{}", w.value),
                ],
                link: link(w.surface),
            }),
        }
    }
}

fn fog_zones(p: &mut RecordProduct, t: &tessera_formats::FogZoneTable) {
    p.summary
        .push(("entries".into(), t.entries().len().to_string()));
    p.summary
        .push(("zone key".into(), format!("{:#x}", t.zone_key)));
    p.summary
        .push(("any active".into(), yes(t.any_active()).into()));
    p.columns = vec![
        "key",
        "active",
        "near",
        "far",
        "amount",
        "colour",
        "light a",
        "light b",
        "overrides",
    ];
    for (default, z) in t
        .default
        .iter()
        .map(|d| (true, d))
        .chain(t.entries().iter().map(|e| (false, e)))
        .take(MAX_ROWS)
    {
        let overrides: Vec<String> = z
            .overrides
            .iter()
            .filter(|o| o.mode != 0)
            .map(|o| format!("{}:{:#x}/{:#x}", o.mode, o.a, o.b))
            .collect();
        p.rows.push(RecordRow {
            cells: vec![
                if default {
                    format!("{} (default)", z.key)
                } else {
                    z.key.to_string()
                },
                yes(z.is_active()).into(),
                format!("{}", z.near),
                format!("{}", z.far),
                format!("{}", z.amount),
                colour(z.colour),
                format!("{} {}", z.light_a, colour(z.colour_a1)),
                format!("{} {}", z.light_b, colour(z.colour_b)),
                if overrides.is_empty() {
                    "—".into()
                } else {
                    overrides.join(" ")
                },
            ],
            link: None,
        });
    }
}

fn list_text<T: Copy>(r: &ListRef<T>, fmt: impl Fn(T) -> String) -> String {
    match r {
        ListRef::Named(k) => format!("named list {k:#x}"),
        ListRef::Inline(l) => l
            .pairs
            .iter()
            .take(8)
            .map(|w| format!("{} ×{}", fmt(w.value), w.weight))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn sky(p: &mut RecordProduct, s: &tessera_formats::SkyDescription) {
    p.summary
        .push(("day groups".into(), s.groups.len().to_string()));
    p.summary
        .push(("local defaults".into(), s.local_defaults.len().to_string()));
    p.summary.push((
        "intervals".into(),
        format!(
            "light {}…{} s, sky {}…{} s",
            s.light_interval_min, s.light_interval_max, s.sky_interval_min, s.sky_interval_max
        ),
    ));
    if let Some(l) = link(s.game_time) {
        p.links.push(("game time".into(), l));
    }
    p.summary.push((
        "note".into(),
        "the Sky view shows the chosen day descriptions and their timeline".into(),
    ));
    p.columns = vec![
        "day type",
        "chance",
        "sky key",
        "first day description",
        "list",
    ];
    p.link_column = Some(3);
    let first = |r: &ListRef<Did>| -> Option<Did> {
        s.resolve_sky_list(r)
            .and_then(|l| l.pairs.first().map(|w| w.value))
    };
    for g in &s.groups {
        for e in &g.entries {
            if p.rows.len() >= MAX_ROWS {
                return;
            }
            let f = first(&e.list).unwrap_or(Did(0));
            p.rows.push(RecordRow {
                cells: vec![
                    g.day_type.to_string(),
                    format!("{}", g.chance),
                    format!("{:#x}", e.sky_key),
                    did(f),
                    list_text(&e.list, did),
                ],
                link: link(f),
            });
        }
    }
    for e in &s.local_defaults {
        if p.rows.len() >= MAX_ROWS {
            return;
        }
        let f = first(&e.list).unwrap_or(Did(0));
        p.rows.push(RecordRow {
            cells: vec![
                "local default".into(),
                "—".into(),
                format!("{:#x}", e.sky_key),
                did(f),
                list_text(&e.list, did),
            ],
            link: link(f),
        });
    }
}

fn day(p: &mut RecordProduct, d: &tessera_formats::DayDescription) {
    p.summary
        .push(("keyframes".into(), d.keyframes.len().to_string()));
    p.summary
        .push(("sky objects".into(), d.objects.len().to_string()));
    p.summary
        .push(("transition".into(), format!("{} s", d.transition_seconds)));
    p.summary.push((
        "note".into(),
        "the Sky view draws the blended timeline".into(),
    ));
    p.columns = vec![
        "time",
        "fog",
        "fog colour",
        "sun",
        "heading / pitch",
        "sun colour",
        "ambient",
        "ambient colours",
        "effect",
    ];
    p.link_column = Some(8);
    for k in d.keyframes.iter().take(MAX_ROWS) {
        p.rows.push(RecordRow {
            cells: vec![
                format!("{:.4}", k.time),
                format!("{} … {}", k.fog_near, k.fog_far),
                colour(k.fog_colour),
                format!("{}", k.sun_brightness),
                format!("{} / {}", k.sun_heading, k.sun_pitch),
                colour(k.sun_colour),
                format!("{}", k.ambient_brightness),
                format!(
                    "{} {}",
                    colour(k.ambient_colour_1),
                    colour(k.ambient_colour_2)
                ),
                k.effect.map_or_else(|| "—".into(), |e| did(e.object)),
            ],
            link: k.effect.and_then(|e| link(e.object)),
        });
    }
}

fn game_time(p: &mut RecordProduct, g: &tessera_formats::GameTime) {
    p.summary
        .push(("day length".into(), format!("{} s", g.day_length)));
    p.summary
        .push(("days per year".into(), g.days_per_year.to_string()));
    p.summary
        .push(("zero offset".into(), format!("{}", g.zero_offset)));
    p.summary
        .push(("zero year".into(), g.zero_year.to_string()));
    p.summary.push((
        "calendar ordered".into(),
        yes(g.calendar_is_ordered()).into(),
    ));
    p.columns = vec!["kind", "index", "start", "name token"];
    let periods = g.periods.iter().enumerate().map(|(i, d)| {
        vec![
            "day period".into(),
            i.to_string(),
            format!("{}", d.start),
            format!("{:#x}", d.name),
        ]
    });
    let months = g.months.iter().enumerate().map(|(i, m)| {
        vec![
            "month".into(),
            i.to_string(),
            format!("day {}", m.start_day),
            format!("{:#x}", m.name),
        ]
    });
    let weekdays = g.weekdays.iter().enumerate().map(|(i, w)| {
        vec![
            "weekday".into(),
            i.to_string(),
            "—".into(),
            format!("{w:#x}"),
        ]
    });
    // Rows are built only up to the bound: a hostile record may hold millions.
    for cells in periods.chain(months).chain(weekdays).take(MAX_ROWS) {
        p.rows.push(RecordRow { cells, link: None });
    }
}

fn cell(p: &mut RecordProduct, c: &tessera_formats::CellGeometry) {
    p.summary.push((
        "physics mesh".into(),
        c.physics_mesh.as_ref().map_or_else(
            || "none".into(),
            |m| {
                format!(
                    "{} vertices, {} triangles",
                    m.vertices.len(),
                    m.triangles.len()
                )
            },
        ),
    ));
    p.summary
        .push(("planes".into(), c.planes.len().to_string()));
    p.summary
        .push(("portals".into(), c.portals.len().to_string()));
    p.summary
        .push(("wall polygons".into(), c.walls.len().to_string()));
    p.summary
        .push(("hull vertices".into(), c.hull.len().to_string()));
    p.summary.push((
        "render mesh".into(),
        format!("{} bytes", c.render_mesh.len()),
    ));
    p.summary.push((
        "note".into(),
        "the 3D view draws the cell with its collision overlays".into(),
    ));
    p.columns = vec!["part", "index", "normal / corners", "distance / lead"];
    let planes = c.planes.iter().enumerate().map(|(i, pl)| {
        vec![
            "plane".into(),
            i.to_string(),
            format!("{:?}", pl.normal),
            format!("{}", pl.distance),
        ]
    });
    let portals = c.portals.iter().enumerate().map(|(i, pt)| {
        vec![
            "portal".into(),
            i.to_string(),
            corners(&pt.corners),
            pt.lead.to_string(),
        ]
    });
    let walls = c.walls.iter().enumerate().map(|(i, w)| {
        vec![
            "wall".into(),
            i.to_string(),
            corners(&w.corners),
            "—".into(),
        ]
    });
    // Rows are built only up to the bound: a hostile record may hold millions of walls.
    for cells in planes.chain(portals).chain(walls).take(MAX_ROWS) {
        p.rows.push(RecordRow { cells, link: None });
    }
}

/// Corners shown per polygon; the rest are counted.
pub(crate) const MAX_CORNERS_SHOWN: usize = 8;

/// A polygon's corners as text: the first [`MAX_CORNERS_SHOWN`], then `… (+N)`.
pub(crate) fn corners(c: &[[f32; 3]]) -> String {
    let shown: Vec<String> = c
        .iter()
        .take(MAX_CORNERS_SHOWN)
        .map(|p| format!("{p:?}"))
        .collect();
    let mut s = format!("[{}", shown.join(", "));
    if c.len() > MAX_CORNERS_SHOWN {
        s.push_str(&format!(", … (+{})", c.len() - MAX_CORNERS_SHOWN));
    }
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::LodClassTable;
    use tessera_formats::environment::LodClass;

    fn product() -> RecordProduct {
        RecordProduct {
            did: 0x3200_0000,
            ..RecordProduct::default()
        }
    }

    #[test]
    fn colours_and_dids() {
        assert_eq!(colour(Argb(0xFF80_4020)), "#804020");
        assert_eq!(colour(Argb(0x1080_4020)), "#804020 a16");
        assert_eq!(did(Did(0)), "—");
        assert_eq!(link(Did(0)), None);
        assert_eq!(link(Did(0x4000_0001)), Some(0x4000_0001));
        assert_eq!(flags(&[(false, "a"), (true, "b"), (true, "c")]), "b c");
        assert_eq!(flags(&[(false, "a")]), "—");
    }

    #[test]
    fn lod_classes_are_tabled() {
        let t = LodClassTable {
            did: Did(0x3200_0000),
            entries: vec![LodClass {
                class: 3,
                values: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            }],
        };
        let mut p = product();
        assert!(world_record(&mut p, &AnyRecord::LodClassTable(t)));
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].cells.len(), p.columns.len());
        assert_eq!(p.rows[0].cells[0], "3");
    }

    #[test]
    fn corner_text_is_bounded() {
        assert_eq!(corners(&[]), "[]");
        assert_eq!(corners(&[[1.0, 2.0, 3.0]]), "[[1.0, 2.0, 3.0]]");
        let many = vec![[0.0f32; 3]; 1_000_000];
        let t = corners(&many);
        assert!(t.len() < 200, "{}", t.len());
        assert!(t.ends_with(", … (+999992)]"));
    }

    #[test]
    fn huge_cells_and_calendars_stop_at_the_row_bound() {
        // Regression: every wall and weekday became a row before the table was cut to
        // MAX_ROWS, about 250 bytes per 4 stored bytes.
        use tessera_formats::GameTime;
        let g = GameTime {
            weekdays: vec![7; MAX_ROWS + 10],
            ..GameTime::default()
        };
        let mut p = product();
        assert!(world_record(&mut p, &AnyRecord::GameTime(g)));
        assert_eq!(p.rows.len(), MAX_ROWS);
        assert_eq!(p.rows[0].cells[0], "weekday");
        assert!(p.rows.capacity() < MAX_ROWS * 2);
    }
}
