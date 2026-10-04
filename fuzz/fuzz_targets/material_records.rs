// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-material::records` (`docs/specs/material.md` §4–§7): the surface record
//! (0x31), material (0x30) and shader record (0x2B) decoders, and the shader interface
//! analysis and resolution on top of a decoded shader record. Nothing may panic or
//! allocate beyond the reader budget; a decoded record re-checks its own invariants.
//!
//! Input layout: one selector byte, then the payload. The payload's DID prefix is forced
//! to the selected record type so the decoder gets past the prefix check.

#![no_main]

use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use tessera_formats::Record;
use tessera_material::records::GROUP_TEXTURE;
use tessera_material::{
    MaterialRecord, ResolvedMaterial, ShaderInterface, ShaderRecord, SurfaceRecord,
};

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else {
        return;
    };
    let mut payload = rest.to_vec();
    let prefix = [0x31u8, 0x30, 0x2B][usize::from(sel % 3)];
    if payload.len() >= 4 {
        payload[3] = prefix;
    }
    match prefix {
        0x31 => {
            if let Ok(s) = SurfaceRecord::parse(&payload) {
                assert_eq!(
                    payload.len(),
                    SurfaceRecord::FIXED_SIZE + 4 * s.materials.len()
                );
            }
        }
        0x30 => {
            if let Ok(m) = MaterialRecord::parse(&payload) {
                let _ = m.has_unusual_fields();
                for e in &m.entries {
                    let _ = m.get(e.id);
                }
            }
        }
        _ => {
            if let Ok(s) = ShaderRecord::parse(&payload) {
                // MT4 holds for every accepted record.
                for d in &s.params {
                    for b in &d.bindings {
                        assert!((b.permutation as usize) < s.permutations.len());
                        if tessera_material::records::group_of(d.id) == GROUP_TEXTURE {
                            assert!(s.sampler(b.permutation, b.slot).is_some());
                        }
                    }
                }
                let iface = Arc::new(ShaderInterface::from_record(&s));
                // Resolve over the shader with a material decoded from the same bytes
                // (prefix forced), when that happens to decode.
                let mut mat = payload.clone();
                mat[3] = 0x30;
                let mats: Vec<MaterialRecord> = MaterialRecord::parse(&mat).into_iter().collect();
                let refs: Vec<&MaterialRecord> = mats.iter().collect();
                let r = ResolvedMaterial::resolve(Arc::clone(&iface), &refs);
                assert_eq!(r.slots.len(), iface.slots.len());
                if let Some(at) = r.features.alpha_test {
                    assert!(at.threshold.is_finite());
                }
            }
        }
    }
});
