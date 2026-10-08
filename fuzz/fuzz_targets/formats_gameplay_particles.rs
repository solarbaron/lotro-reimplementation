// SPDX-License-Identifier: GPL-3.0-or-later
//! Particle-system descriptors (prefix 0x39) and the outer layer of fork-system descriptors
//! (prefix 0x44) on arbitrary bytes (`docs/specs/particles.md` §4, §6.1).
//!
//! Byte 0 selects the input: the rest as a 0x39 payload; the rest as a 0x44 payload; a
//! fixed 0x39 system part with one emitter followed by the rest (so short inputs reach the
//! emitter and keyframe readers); or a 0x44 payload with an empty image followed by the rest.
//!
//! Nothing may panic. An accepted record re-encodes canonically (a fixpoint, never longer
//! than the input), and the runtime views (§8.2, §8.6.1, §8.9, §8.10, §8.11, §6.2) run on
//! every decoded emitter and fork record.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_gameplay_particles -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::NoSchema;
use tessera_formats::gameplay::particles::{ForkSystem, ParticleSystem};

/// A synthetic system part (§4.1): time step 0.05, far fade 30–60, scale mode 1.
fn system_head() -> Vec<u8> {
    let mut out = Vec::with_capacity(39);
    for v in [0.05f32, 0.0, 0.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
    for v in [30.0f32, 60.0, 0.0, 0.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&1u32.to_le_bytes());
    out.push(1);
    out
}

fn f32_from(bytes: &[u8], at: usize) -> f32 {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map_or(0.5, f32::from_le_bytes)
}

fn particle(data: &[u8]) {
    let Ok(sys) = ParticleSystem::decode(data) else {
        return;
    };
    let bytes = sys.encode().expect("decoded system encodes");
    assert!(bytes.len() <= data.len());
    let again = ParticleSystem::decode(&bytes).expect("re-encoded system decodes");
    assert_eq!(again.encode().expect("encodes"), bytes);

    let t = f32_from(data, 0);
    let _ = sys.update_interval();
    let _ = sys.has_ended(f64::from(t));
    let _ = sys.draw_order().count();
    let _ = sys.references().count();
    for e in &sys.emitters {
        let c = e.channel_state(t);
        if let Some(k) = c.key {
            assert!(k < e.keyframes.len());
        }
        if let Some(k) = c.controller_key {
            assert!(k < e.keyframes.len());
        }
        let _ = e.atlas_uv(1.0, 0.0, f64::from(t), f32_from(data, 4));
        let a = e.fade_alpha(sys.fast_forward, f64::from(t));
        assert!(a.is_nan() || (0.0..=1.0).contains(&a));
        let _ = e.has_finished(f64::from(t));
        assert!(e.emission_cap() <= 100_000);
        let _ = e.timed_interval();
        assert!(e.primitive_vertices() <= 4);
        let _ = e.curves().count();
    }
}

fn fork(data: &[u8]) {
    let Ok(f) = ForkSystem::decode(data, &NoSchema) else {
        return;
    };
    let _ = f.image_header();
    let _ = f.image_name_block();
    let _ = f.transform_matrix();
    for r in &f.resources {
        let _ = f.path_of(r);
        let _ = r.kind();
    }
    let Ok(bytes) = f.encode() else {
        return;
    };
    assert!(bytes.len() <= data.len());
    let again = ForkSystem::decode(&bytes, &NoSchema).expect("re-encoded fork decodes");
    assert_eq!(again.encode().expect("encodes"), bytes);
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    match mode % 4 {
        0 => particle(rest),
        1 => fork(rest),
        2 => {
            let mut data = system_head();
            data.extend_from_slice(rest);
            particle(&data);
        }
        _ => {
            let mut data = vec![0u8; 4];
            data.extend_from_slice(rest);
            fork(&data);
        }
    }
});
