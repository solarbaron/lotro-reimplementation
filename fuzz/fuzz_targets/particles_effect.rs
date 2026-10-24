// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera_particles::effect_from_payload` (`particles.md` §6.1, §6.5, §8, §10.2;
//! `particle-runtime.md` §3): the family the DID prefix picks, the 0x39 or 0x44 payload
//! decoded, and the effect made for it (a `SystemSim` or a `ForkEffect`), then its end,
//! stop and expiry rules at input-chosen times.
//!
//! Input layout: byte 0 picks the DID and the payload form, byte 1 the fade flags, bytes
//! 2-3 the start time, then the payload part. Payload forms: 0x39 with the payload as it
//! is, 0x39 behind a synthetic system head (the fields before the emitters), 0x44 as it
//! is, 0x44 behind an empty effect image, a well-formed synthetic 0x44 record (from the
//! input's image, flag, transform, tail and bounds) and a DID of another prefix. No game
//! data: the records are invented.
//!
//! Nothing may panic or hang; a decoded effect has the family of its DID's prefix; another
//! prefix is `NotAnEffect`; a stopped fork effect reports itself stopped, and a looping fork
//! effect that is not stopped never expires.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::particles::ForkSystem;
use tessera_formats::{NoSchema, PropertySet};
use tessera_particles::glam::{Mat4, Vec3};
use tessera_particles::{Effect, ForkFade, ParticleError, effect_from_payload};

/// A synthetic system part (§4.1): time step 0.05, far fade 30-60, scale mode 1.
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

/// Little-endian reads that give 0 past the end.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let (&b, rest) = self.0.split_first().unwrap_or((&0, &[]));
        self.0 = rest;
        b
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        let n = n.min(self.0.len());
        let (a, rest) = self.0.split_at(n);
        self.0 = rest;
        a.to_vec()
    }
}

/// A well-formed fork record from the input (encoded with the crate's own encoder).
fn fork_record(b: &mut Bytes<'_>) -> Option<Vec<u8>> {
    let image_len = usize::from(b.u8() % 64);
    let flag = b.u8() & 1 == 1;
    let transform = if b.u8() & 1 == 0 {
        (0..16).flat_map(|_| b.f32().to_le_bytes()).collect()
    } else {
        b.take(usize::from(b.0.first().copied().unwrap_or(0) % 80))
    };
    let tail_value = f64::from(b.f32());
    let record = ForkSystem {
        image: b.take(image_len),
        flag,
        name: String::new(),
        name_table_hint: 0,
        name_table: Vec::new(),
        properties: PropertySet::default(),
        transform,
        resources_hint: 0,
        resources: Vec::new(),
        tail_value,
        bounds_min: [b.f32(), b.f32(), b.f32()],
        bounds_max: [b.f32(), b.f32(), b.f32()],
    };
    record.encode().ok()
}

fuzz_target!(|data: &[u8]| {
    let [form, fade_bits, t0, t1, rest @ ..] = data else {
        return;
    };
    let now = match fade_bits >> 5 {
        0 => f64::NAN,
        1 => f64::INFINITY,
        2 => -f64::from(u16::from_le_bytes([*t0, *t1])),
        _ => f64::from(u16::from_le_bytes([*t0, *t1])) * 0.01,
    };
    let fade = ForkFade {
        enabled: fade_bits & 1 == 1,
        far: [f32::from(*t0), f32::from(*t0) + f32::from(*t1)],
        near: if fade_bits & 2 == 0 {
            [0.0, f32::from(*t1)]
        } else {
            [f32::NAN, f32::INFINITY]
        },
    };
    let mut b = Bytes(rest);
    let (did, payload) = match form % 6 {
        0 => (0x3900_0001, rest.to_vec()),
        1 => {
            let mut p = system_head();
            p.extend_from_slice(rest);
            (0x3900_0002, p)
        }
        2 => (0x4400_0001, rest.to_vec()),
        3 => {
            let mut p = vec![0u8; 4];
            p.extend_from_slice(rest);
            (0x4400_0002, p)
        }
        4 => {
            let Some(p) = fork_record(&mut b) else {
                return;
            };
            (0x4400_0003, p)
        }
        _ => {
            let prefix = u32::from(b.u8());
            (prefix << 24 | 0x42, b.0.to_vec())
        }
    };
    let made = effect_from_payload(did, &payload, now, fade);
    let prefix = did >> 24;
    if prefix != 0x39 && prefix != 0x44 {
        assert!(
            matches!(made, Err(ParticleError::NotAnEffect(d)) if d == did),
            "another prefix is not an effect"
        );
        return;
    }
    let Ok(mut effect) = made else {
        return;
    };
    match (&effect, prefix) {
        (Effect::Particles(_), 0x39) | (Effect::Fork(_), 0x44) => {}
        _ => panic!("the effect has the family of its prefix"),
    }
    if form % 6 == 4 {
        // The synthetic record decodes back to itself, so the effect must exist.
        assert!(ForkSystem::decode(&payload, &NoSchema).is_ok());
    }
    let _ = effect.is_shareable_kind();
    for dt in [0.0, 0.5, 1.0e3, f64::NAN] {
        let _ = effect.has_ended(now + dt);
        let _ = effect.has_finished(now + dt);
    }
    if let Effect::Fork(f) = &effect {
        let looping = ForkSystem::decode(&payload, &NoSchema).is_ok_and(|r| r.flag);
        if looping && !f.is_stopped() {
            for dt in [0.0, 1.0e9] {
                assert!(
                    !f.has_expired(now + dt),
                    "a looping fork effect never expires"
                );
            }
        }
        let _ = f.effect_matrix(Mat4::IDENTITY, Vec3::ONE);
        let _ = f.effect_matrix(Mat4::from_scale(Vec3::splat(1.0e-30)), Vec3::ZERO);
    }
    let stop_at = if now.is_finite() { now + 0.25 } else { 1.0 };
    effect.stop(stop_at);
    if let Effect::Fork(f) = &effect {
        assert!(
            f.is_stopped(),
            "a fork effect stopped at a finite time is stopped"
        );
        let _ = f.has_expired(stop_at + 1.0e6);
    }
    let _ = effect.has_ended(stop_at + 1.0e6);
});
