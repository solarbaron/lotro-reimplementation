// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-render` UI composition entries (`docs/specs/render.md` §6.5, §6.6, §6.8):
//! `UiEntry::geometry` and `UiPass::entry` on arbitrary entries (canvas and box sizes of 0 or
//! beyond the limits, coordinates near `i32` bounds, NaN, infinite and huge opacities,
//! thresholds, node scales, angles and rotation centres, up to 12 rotations and 10 clip items
//! with any space index). Nothing may panic; an accepted entry has finite corners, at most 8
//! clip items, an opacity and threshold in 0–1, and with no rotation its whole clip chain is
//! one scissor.
//!
//! Input layout: a header byte selecting the counts and modes, then the bytes every field
//! reads its value from (cycled when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_render::{
    MAX_UI_CLIPS, TextureId, UiAddressing, UiClip, UiEntry, UiFilter, UiPass, UiRect, UiRotation,
    UiSource,
};

/// Reads values from the input, cycling, zero when it is empty.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    /// Small sizes most of the time, raw bits sometimes.
    fn size(&mut self) -> u32 {
        match self.byte() % 4 {
            0 => self.u32(),
            _ => u32::from(self.byte()) * 4 + u32::from(self.byte() % 4),
        }
    }
    fn coord(&mut self) -> i32 {
        match self.byte() % 4 {
            0 => self.i32(),
            _ => i32::from(self.byte()) * 2 - 128,
        }
    }
    /// Plausible floats most of the time, raw bits sometimes.
    fn f32(&mut self) -> f32 {
        match self.byte() % 4 {
            0 => f32::from_bits(self.u32()),
            1 => f32::from(self.byte()) / 255.0,
            _ => f32::from(self.byte() as i8) * 3.0,
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let header = b.byte();
    let rotations: Vec<UiRotation> = (0..usize::from(b.byte() % 13))
        .map(|_| UiRotation {
            centre: [b.f32() * 50.0, b.f32() * 50.0],
            angles: [b.f32(), b.f32(), b.f32()],
        })
        .collect();
    let clips: Vec<UiClip> = (0..usize::from(b.byte() % 11))
        .map(|_| UiClip {
            rect: [b.coord(), b.coord(), b.coord(), b.coord()],
            space: u32::from(b.byte() % 14),
        })
        .collect();
    let addressing = if header & 1 == 0 {
        UiAddressing::Fit
    } else {
        UiAddressing::Tile {
            node_scale: [b.f32(), b.f32()],
        }
    };
    let entry = UiEntry {
        texture: TextureId::placeholder(),
        source: if header & 2 == 0 {
            UiSource::Straight
        } else {
            UiSource::Premultiplied
        },
        canvas: [b.size(), b.size()],
        tex_offset: [b.coord(), b.coord()],
        dest: UiRect::new(b.coord(), b.coord(), b.size(), b.size()),
        addressing,
        filter: if header & 4 == 0 {
            UiFilter::Linear
        } else {
            UiFilter::Nearest
        },
        opacity: b.f32(),
        alpha_threshold: b.f32(),
        rotations: &rotations,
        clips: &clips,
    };
    if let Ok(g) = entry.geometry() {
        assert!(g.corners.iter().flatten().all(|v| v.is_finite()));
        assert!(g.clips.len() <= MAX_UI_CLIPS);
        assert!((0.0..=1.0).contains(&g.opacity));
        assert!((0.0..=1.0).contains(&g.alpha_threshold));
        assert!(
            g.clips
                .iter()
                .all(|c| c.map.iter().flatten().all(|v| v.is_finite()))
        );
        if !g.rotated {
            assert!(g.clips.is_empty(), "an unrotated clip chain is one scissor");
        }
    }
    let mut pass = UiPass::default();
    for _ in 0..usize::from(header >> 5) {
        pass.entry(&entry);
    }
    assert_eq!(
        pass.len() + pass.refused() as usize,
        usize::from(header >> 5)
    );
});
