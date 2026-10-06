// SPDX-License-Identifier: GPL-3.0-or-later
//! The environment, lighting and draw-distance algorithms of `tessera-world`
//! (`docs/specs/environment.md` §11, `lighting.md` §8.3–§8.5, `walker.md` §11.3–§11.4,
//! `mesh.md` §8.4–§8.7) over hostile inputs: day descriptions decoded from arbitrary bytes
//! (unsorted and NaN keyframe times, wrapping and reversed sky objects), arbitrary game
//! times and clock values, arbitrary weights and day numbers, arbitrary light fields,
//! selections and distances.
//!
//! Nothing may panic. Invariants: blending and sky-object animation are bit-identical when
//! repeated; a clock reading of a usable calendar has a time of day in [0, 1) and a day of
//! the year below the year's length; a weighted pick returns a listed value; a selection
//! submits at most 8 lights; attenuation is in [0, 1]; detail levels are below the level
//! count; a fade's opacity stays in [0, 1]; no non-finite record float reaches the
//! lighting inputs or a sky object's angles (`tessera-world` environment hostile-input
//! rules).
//!
//! Run with e.g. `cargo fuzz run world_environment -- -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::Did;
use tessera_formats::environment::{GameTime, Weighted, decode_day_description};
use tessera_world::draw_distance::{DrawFade, FogClip, detail_level, global_limit};
use tessera_world::environment::{
    Calendar, SkyValues, animate_sky_objects, crossfade_factor, lerp_angle, weighted_pick,
};
use tessera_world::lighting::{
    LightParams, LightRecord, LightRef, MAX_LIGHTS, SelectionOptions, attenuation, select_lights,
};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn f32(&mut self) -> f32 {
        const PICKS: [f32; 12] = [
            0.0, 1.0, -1.0, 0.5, 0.2, 0.31, 359.0, 1e30, -1e30, 0.25, 8.0, 11_160.0,
        ];
        let tag = self.u8();
        if tag < 200 {
            PICKS[usize::from(tag % 12)]
        } else {
            f32::from_bits(self.u32())
        }
    }
    fn f64(&mut self) -> f64 {
        let tag = self.u8();
        if tag < 128 {
            f64::from(self.u32()) * 0.37
        } else {
            f64::from_bits(u64::from(self.u32()) << 32 | u64::from(self.u32()))
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let [split, rest @ ..] = input else {
        return;
    };
    let cut = usize::from(*split).min(rest.len());
    let (params, record) = rest.split_at(cut);
    let mut b = Bytes(params);

    // Clock.
    let gt = GameTime {
        day_length: b.f32(),
        days_per_year: b.u32() % 1000,
        zero_offset: b.f64(),
        zero_year: b.u32(),
        value_54: b.f32(),
        value_58: b.f32(),
        ..GameTime::default()
    };
    if let Ok(cal) = Calendar::from_record(&gt) {
        for _ in 0..4 {
            let r = cal.read(b.f64());
            assert!((0.0..1.0).contains(&r.time_of_day), "{}", r.time_of_day);
            assert!(r.day_of_year < cal.days_per_year);
            let _ = r.day_number_u32();
            let _ = cal.is_night(r.time_of_day);
        }
    }

    // Weighted pick.
    let n = usize::from(b.u8() % 6);
    let pairs: Vec<Weighted<Did>> = (0..n)
        .map(|i| Weighted {
            value: Did(u32::try_from(i).unwrap_or(0)),
            weight: b.f32(),
        })
        .collect();
    if let Some(d) = weighted_pick(&pairs, b.u32()) {
        assert!(pairs.iter().any(|p| p.value == d));
    }

    // Keyframes and sky objects of an arbitrary day description.
    let t = b.f32();
    if let Ok(day) = decode_day_description(record) {
        let a = SkyValues::at(Some(&day), t);
        let c = SkyValues::at(Some(&day), t);
        assert_eq!(
            format!("{a:?}"),
            format!("{c:?}"),
            "blend not deterministic"
        );
        let o1 = animate_sky_objects(&day, t);
        let o2 = animate_sky_objects(&day, t);
        assert_eq!(format!("{o1:?}"), format!("{o2:?}"));
        assert_eq!(o1.len(), day.objects.len());
        let f = crossfade_factor(b.f32(), b.f32());
        assert!((0.0..=1.0).contains(&f));
        for v in [&a, &a.mix(&SkyValues::defaults(), f)] {
            let l = v.lighting();
            let all = [
                l.ambient_up.as_slice(),
                &l.ambient_down,
                &l.sun_colour,
                &l.sun_specular,
                &l.sun_travel,
                &[l.fog_near, l.fog_far],
                &l.fog_colour,
            ];
            assert!(
                all.iter().all(|s| s.iter().all(|x| x.is_finite())),
                "non-finite lighting {l:?}"
            );
        }
        for o in &o1 {
            assert!(o.angle_a.is_finite() && o.angle_b.is_finite(), "{o:?}");
        }
    }
    let _ = lerp_angle(b.f32(), b.f32(), b.f32());

    // Lights.
    let p = LightParams {
        diffuse: [b.f32(), b.f32(), b.f32()],
        range: b.f32(),
        falloff: b.f32(),
        key_light: b.u8() & 1 == 1,
        ..LightParams::default()
    };
    let r = LightRecord::point(&p, [0.0; 3], p.range, b.f32(), b.f32());
    let a = r.attenuation(b.f32(), b.u8() & 1 == 1);
    assert!((0.0..=1.0).contains(&a), "{a}");
    let a = attenuation(b.f32(), b.f32(), b.f32(), true);
    assert!((0.0..=1.0).contains(&a), "{a}");
    let list: Vec<LightRef> = (0..usize::from(b.u8() % 24))
        .map(|id| {
            let f = b.u8();
            LightRef {
                id,
                directional: f & 1 != 0,
                key_light: f & 2 != 0,
            }
        })
        .collect();
    let main = (b.u8() & 1 == 1).then(|| LightRef {
        id: 99,
        directional: b.u8() & 1 == 1,
        key_light: false,
    });
    let s = select_lights(
        main,
        &list,
        SelectionOptions {
            per_pixel: b.u8() & 1 == 1,
            all_per_pixel: b.u8() & 1 == 1,
        },
    );
    assert!(s.len() <= MAX_LIGHTS);

    // Draw distances.
    let levels = usize::from(b.u8() % 8);
    let k = detail_level(levels, b.f32(), b.f32(), b.u8() & 1 == 1);
    assert!(k < levels.max(1));
    let g = global_limit(
        Some(b.f32()),
        Some(FogClip {
            enabled: true,
            far: b.f32(),
        }),
    );
    assert!(g >= 60.0 || g.is_nan());
    let mut fade = DrawFade::new();
    for _ in 0..4 {
        let o = fade.update(b.f32(), b.f32(), b.f32(), b.f32(), b.u8() & 1 == 1, true);
        assert!((0.0..=1.0).contains(&o), "{o}");
    }
});
