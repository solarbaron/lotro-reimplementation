// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-material` over time (M4.5): float-curve evaluation (`docs/specs/material.md`
//! §5.3.1), texture transforms (§7.3, §8.4), per-surface lighting inputs
//! (`lighting.md` §4.1) and the water material chain (`water.md` §8.2). Nothing may
//! panic, and every value handed to a renderer is finite.
//!
//! Input layout: a control byte; a curve built from the next bytes (mode is 1 plus the
//! control byte modulo 10; eleven f32, a loop byte, up to 16 keyframe pairs) evaluated
//! at eight f64 times taken from the input; then two chunks, each a u16 little-endian
//! length and that many bytes: shader record 0x2B000001 and material 0x30000001 (each
//! chunk's DID forced), resolved through surface record 0x31000001 and evaluated at the
//! same times, with the record-wide and every permutation's texture matrices.

#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_material::{
    CurveState, CurveStates, FloatCurve, FnSource, MaterialLibrary, ObjectInputs, TextureTransform,
    TransformValue, WaterMaterial, program_time,
};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        let n = N.min(self.0.len());
        out[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        out
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take::<4>())
    }
    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.take::<8>())
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&ctl, rest)) = data.split_first() else {
        return;
    };
    let mut b = Bytes(rest);

    // A curve of any mode with arbitrary parameters and keys.
    let mode = u32::from(ctl % 10) + 1;
    let mut params = [0.0f32; 11];
    for p in &mut params {
        *p = b.f32();
    }
    let loop_flag = b.take::<1>()[0] & 1;
    let nkeys = usize::from(b.take::<1>()[0] % 17);
    let keys: Vec<(f32, f32)> = (0..nkeys).map(|_| (b.f32(), b.f32())).collect();
    let curve = FloatCurve {
        mode,
        params,
        param_count: match mode {
            1 => 1,
            10 => 11,
            _ => 10,
        },
        loop_flag,
        keys: if mode == 10 { keys } else { Vec::new() },
    };
    let times: Vec<f64> = (0..8).map(|_| b.f64()).collect();
    let mut st = CurveState::with_seed(u32::from(ctl));
    let mut states = CurveStates::with_seed(u32::from(ctl));
    for &t in &times {
        let _ = curve.evaluate_at(t, &mut st);
        assert!(curve.evaluate_finite(t, &mut st).is_finite());
        assert!(states.evaluate(1000, &curve, t).is_finite());
        assert!(program_time(t).is_finite());
    }

    // A texture transform from the same parameters.
    let mut tt = TextureTransform::RESET;
    for (value, &x) in TransformValue::ALL.into_iter().zip(&params) {
        tt.set(value, x);
    }
    let m = tt.matrix();
    assert!(m.rows.iter().flatten().all(|x| x.is_finite()));
    let mm = m.then(&m);
    assert!(mm.rows.iter().flatten().all(|x| x.is_finite()));
    let _ = mm.apply(params[0], params[1]);

    // The chain: shader record and material from the input.
    let mut store: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut rest = b.0;
    for did in [0x2B00_0001u32, 0x3000_0001] {
        let Some(len) = rest.get(..2) else { break };
        let len = usize::from(u16::from_le_bytes([len[0], len[1]]));
        let body = rest.get(2..).unwrap_or(&[]);
        let chunk = &body[..len.min(body.len())];
        rest = &body[chunk.len()..];
        let mut payload = did.to_le_bytes().to_vec();
        payload.extend_from_slice(chunk.get(4..).unwrap_or(&[]));
        store.insert(did, payload);
    }
    let mut surface = Vec::new();
    for w in [0x3100_0001u32, 0x2B00_0001, 1, 1, 0x3000_0001] {
        surface.extend_from_slice(&w.to_le_bytes());
    }
    surface.extend_from_slice(&1u16.to_le_bytes());
    store.insert(0x3100_0001, surface);
    let lib = MaterialLibrary::new(FnSource(move |d: Did| {
        Ok::<_, std::io::Error>(store.get(&d.0).cloned())
    }));
    let Ok(r) = lib.resolve_surface(Did(0x3100_0001)) else {
        return;
    };
    let object = ObjectInputs {
        tint: [params[0], params[1], params[2], params[3]],
        specular_power_scale: params[4],
    };
    let mut states = CurveStates::new();
    for &t in &times {
        if let Ok(i) = lib.surface_inputs(&r, &object, t, &mut states) {
            assert!(i.all_finite());
        }
        if let Some(w) = WaterMaterial::evaluate(&r, t, &mut states) {
            assert!(w.all_finite());
        }
        if let Ok(layout) = lib.texture_matrices(r.shader.did) {
            for (p, _) in layout.permutations() {
                for m in layout.evaluate_for(p, &r.params, t, &mut states) {
                    assert!(m.map.rows.iter().flatten().all(|x| x.is_finite()));
                }
            }
        }
    }
});
