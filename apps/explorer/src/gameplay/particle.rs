// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle inspectors (`particles.md`): particle-system descriptors (0x39, §4) with their
//! emitters and keyframes, and the outer layer of fork-system descriptors (0x44, §6).

use tessera_formats::gameplay::particles::{Curve, Emitter, ForkSystem, ParticleSystem};

use super::card::{Section, did_text, opt_text, yes};
use super::{Cx, Family, GameplayProduct, product};
use crate::error::ExplorerError;

/// Most emitters given their own keyframe table.
const MAX_EMITTER_DETAILS: usize = 32;

fn curve(c: &Curve) -> String {
    match c.constant_value() {
        Some(v) => v.to_string(),
        None => format!("mode {} base {}", c.mode, c.base()),
    }
}

/// The card of a particle (0x39) or fork (0x44) system.
pub(super) fn system_product(cx: &Cx<'_>, did: u32) -> Result<GameplayProduct, ExplorerError> {
    let raw = crate::decoders::read_raw(cx.ctx, did, None)?;
    if did >> 24 == 0x39 {
        let ps = ParticleSystem::decode(&raw.bytes)?;
        Ok(particle_system(did, &ps))
    } else {
        let fs = ForkSystem::decode(&raw.bytes, cx.s.registry.dictionary())?;
        Ok(fork_system(cx, did, &fs))
    }
}

fn particle_system(did: u32, ps: &ParticleSystem) -> GameplayProduct {
    let mut p = product(did, Family::Particle, "particle system");
    let mut s = Section::fields("Particle system (§4.1)");
    s.field("time step", ps.time_step.to_string());
    s.field("fast forward", ps.fast_forward.to_string());
    s.field("physics duration", ps.physics_duration.to_string());
    s.field("world space", yes(ps.world_space));
    s.field("force draw", yes(ps.force_draw));
    s.field("uses video post", yes(ps.uses_video_post));
    s.field("draw reflection", yes(ps.draw_reflection));
    s.field("use lighting", yes(ps.use_lighting));
    s.field("inherit opacity", yes(ps.inherit_opacity));
    s.field(
        "far fade",
        format!("{} … {}", ps.far_fade_start, ps.far_fade_end),
    );
    s.field(
        "near fade",
        format!("{} … {}", ps.near_fade_start, ps.near_fade_end),
    );
    s.field(
        "scale mode",
        format!("{} ({:?})", ps.scale_mode, ps.scale_mode_kind()),
    );
    if ps.has_bounds {
        s.field(
            "bounds",
            format!("{:?} … {:?}", ps.bounds_min, ps.bounds_max),
        );
    }
    s.field("sorting priority", ps.sorting_priority.to_string());
    s.field("shareable", yes(ps.shareable));
    s.did("surface", Some(ps.surface));
    s.did("extra surface", Some(ps.extra_surface));
    s.field("emitters", ps.emitters.len().to_string());
    s.field("frame rate", ps.frame_rate().to_string());
    p.sections.push(s);

    let mut refs = Section::new("Referenced records", &["DID"]);
    for d in ps.references() {
        refs.push(vec![did_text(d)], vec![d.0]);
    }
    if !refs.rows.is_empty() {
        p.sections.push(refs);
    }

    let mut em = Section::new(
        "Emitters (§4.2)",
        &[
            "#",
            "name",
            "shape",
            "primitive",
            "interval",
            "lifespan",
            "speed",
            "max particles",
            "cap",
            "keyframes",
            "active",
        ],
    );
    for (i, e) in ps.emitters.iter().enumerate() {
        em.push(
            vec![
                i.to_string(),
                e.name.clone(),
                format!("{} ({:?})", e.shape, e.spawn_shape()),
                format!("{} ({:?})", e.primitive, e.primitive_kind()),
                e.emit_interval.to_string(),
                curve(&e.lifespan),
                curve(&e.speed),
                e.max_particles.to_string(),
                e.emission_cap().to_string(),
                e.keyframes.len().to_string(),
                yes(e.active).into(),
            ],
            Vec::new(),
        );
    }
    p.sections.push(em);
    for (i, e) in ps.emitters.iter().enumerate().take(MAX_EMITTER_DETAILS) {
        p.sections.push(emitter_details(i, e));
    }
    if ps.emitters.len() > MAX_EMITTER_DETAILS {
        p.notes.push(format!(
            "keyframe tables shown for the first {MAX_EMITTER_DETAILS} of {} emitters",
            ps.emitters.len()
        ));
    }
    p
}

fn emitter_details(i: usize, e: &Emitter) -> Section {
    let mut s = Section::new(
        format!("Emitter {i} {}: timing and keyframes (§4.2, §4.3)", e.name),
        &[
            "time",
            "flags",
            "scale",
            "colour (ARGB)",
            "mass",
            "controller",
            "points",
        ],
    );
    s.push(
        vec![
            format!("start {}", e.start_time),
            format!("limit {}", e.time_limit),
            format!("burst {}", e.burst_count),
            format!("fade {} / {}", e.fade_in, e.fade_out),
            format!("atlas {}×{} @ {}", e.frames_u, e.frames_v, e.frame_rate),
            format!("bone {} hold {}", e.bone, e.holding_location),
            opt_text(e.timed_interval()),
        ],
        Vec::new(),
    );
    for k in &e.keyframes {
        s.push(
            vec![
                k.time.to_string(),
                format!("{:#x}", k.flags),
                format!("{} × {}", k.scale_x, k.scale_y),
                format!("{:08X}", k.colour.0),
                k.mass.to_string(),
                format!("{} ({:?})", k.controller, k.motion()),
                k.points.len().to_string(),
            ],
            Vec::new(),
        );
    }
    s
}

fn fork_system(cx: &Cx<'_>, did: u32, fs: &ForkSystem) -> GameplayProduct {
    let l = &cx.labels;
    let mut p = product(did, Family::Particle, "fork system");
    let mut s = Section::fields("Fork system (§6.1)");
    s.field(
        "effect image",
        format!("{} bytes (opaque, §6.2)", fs.image.len()),
    );
    if let Some(h) = fs.image_header() {
        s.field("image header", format!("{h:?}"));
    }
    s.field("flag", yes(fs.flag));
    s.field("name", fs.name.clone());
    s.field("name-table hint", fs.name_table_hint.to_string());
    s.field("transform", format!("{} bytes", fs.transform.len()));
    if let Some(m) = fs.transform_matrix() {
        s.field("transform matrix", format!("{m:?}"));
    }
    s.field("resources hint", fs.resources_hint.to_string());
    p.sections.push(s);

    let mut names = Section::new("Name table (§6.1)", &["key", "path"]);
    for n in &fs.name_table {
        names.push(vec![n.key.clone(), n.path.clone()], Vec::new());
    }
    if !names.rows.is_empty() {
        p.sections.push(names);
    }
    let mut res = Section::new("Resources (§6.4)", &["DID", "name", "kind", "path"]);
    for r in &fs.resources {
        res.push(
            vec![
                did_text(r.did),
                r.name.clone(),
                format!("{:?}", r.kind()),
                fs.path_of(r).unwrap_or("").to_owned(),
            ],
            vec![r.did.0],
        );
    }
    if !res.rows.is_empty() {
        p.sections.push(res);
    }
    let mut props = Section::new("Properties", super::card::PROPERTY_COLUMNS);
    for e in fs.properties.iter() {
        let (text, label, links) = l.value(e.pid, &e.value);
        props.push(vec![l.pid_name(e.pid), text, label], links);
    }
    if !props.rows.is_empty() {
        p.sections.push(props);
    }
    p
}
