// SPDX-License-Identifier: GPL-3.0-or-later
//! The worked examples of `docs/specs/lighting.md` §14 on the CPU reference of the lighting
//! equation (`tessera_render::lighting::reference`), and lit pixels read back from the GPU
//! against the same reference (sun, point light with specular, shadow, fog, sRGB targets
//! and sRGB textures).
//!
//! The §14 values are printed with two or three decimals; the tests check them to that
//! precision.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::lighting::reference::{
    self, LightSample, Texel, alpha_test_keeps, ambient, attenuation, crossfade, frame_ambient,
    light_record, output_alpha, shade,
};
use tessera_render::{
    AlphaMode, Camera, ColorSpace, Filter, Fog, FrameLighting, Light, LightFields, LitSurface,
    Material, MeshData, MeshDraw, OffscreenFormat, Perspective, RenderError, Renderer, Specular,
    SunLight, SunShadow, TextureId, TextureUpload, Wrap,
};

/// `a` equals `b` to `decimals` decimals (|a − b| ≤ half a unit of the last decimal).
fn close(a: f32, b: f32, decimals: i32) -> bool {
    (a - b).abs() <= 0.5 * 10f32.powi(-decimals) + 1e-6
}

fn assert_rgb(actual: [f32; 3], expected: [f32; 3], decimals: i32, what: &str) {
    assert!(
        actual
            .iter()
            .zip(&expected)
            .all(|(a, e)| close(*a, *e, decimals)),
        "{what}: {actual:?} != {expected:?}"
    );
}

/// The sun's toward-vector at heading 0 and pitch 30° (`lighting.md` §8.1).
fn sun_30() -> Vec3 {
    let (h, e) = (0.0f32, 30f32.to_radians());
    Vec3::new(h.sin() * e.cos(), h.cos() * e.cos(), e.sin())
}

// ---- lighting.md §14 on the CPU --------------------------------------------------------

#[test]
fn example_sun_lit_texel() {
    let a = ambient(1.0, [0.10, 0.10, 0.12], [0.05, 0.05, 0.05]);
    assert_rgb(a, [0.15, 0.15, 0.17], 3, "A");
    assert!(close(Vec3::Z.dot(sun_30()), 0.5, 6), "N . L0 at pitch 30");
    let texel = Texel {
        albedo: [0.5, 0.4, 0.3],
        normal: Vec3::Z,
        up: Vec3::Z,
        a0: [0.10, 0.10, 0.12],
        h0: [0.05, 0.05, 0.05],
        lights: vec![LightSample {
            to_light: sun_30(),
            color: [0.5, 0.5, 0.45],
            factor: [1.0; 3],
        }],
        specular: [0.0; 3],
        emissive: [0.0; 3],
        fog: None,
    };
    assert_rgb(shade(&texel), [0.40, 0.32, 0.237], 3, "2 C Sigma");
    let fogged = shade(&Texel {
        fog: Some((0.25, [0.6, 0.6, 0.7])),
        ..texel
    });
    assert_rgb(fogged, [0.45, 0.39, 0.353], 3, "fogged");
}

#[test]
fn example_point_light() {
    // Step 2 alone: 1 / (1 + 3.2).
    assert!(close(attenuation(80.0, 100.0, 0.04, false), 0.238, 3));
    let s: f32 = (100.0 - 80.0) / 25.0;
    assert!(close(s, 0.8, 6));
    assert!(close(s * s * (3.0 - 2.0 * s), 0.896, 3), "smoothstep");
    assert!(close(attenuation(80.0, 100.0, 0.04, true), 0.213, 3));
    // Outside the range nothing; inside 0.75 R² no fade.
    assert_eq!(attenuation(100.0, 100.0, 0.04, true), 0.0);
    assert_eq!(
        attenuation(50.0, 100.0, 0.04, true),
        attenuation(50.0, 100.0, 0.04, false)
    );
}

#[test]
fn example_light_record() {
    let r = light_record(&LightFields {
        diffuse: [1.0, 0.6, 0.2],
        intensity: 1.0,
        range: 10.0,
        falloff: 0.5,
        ..LightFields::default()
    });
    assert!(close(r.range * r.range, 100.0, 6), "R²");
    assert!(close(r.attenuation, 0.05, 6), "k");
    assert_rgb(r.color, [1.0, 0.6, 0.2], 6, "colour");
    let r2 = r.range * r.range;
    assert!(close(attenuation(50.0, r2, r.attenuation, true), 0.286, 3));
    assert!(close(attenuation(80.0, r2, r.attenuation, false), 0.2, 6));
    assert!(close(attenuation(80.0, r2, r.attenuation, true), 0.179, 3));
    // The specular colour falls back to the diffuse colour for a white Specular without
    // KeyLight, and the default light is half strength.
    assert_rgb(r.specular, [1.0, 0.6, 0.2], 6, "specular");
    let d = light_record(&LightFields::default());
    assert_rgb(d.color, [0.5; 3], 6, "default colour");
    assert!(close(d.attenuation, 10.0 / 625.0, 6), "default k");
    let key = light_record(&LightFields {
        diffuse: [1.0, 0.0, 0.0],
        key_light: true,
        intensity: 0.25,
        ..LightFields::default()
    });
    assert_rgb(key.specular, [0.5; 3], 6, "key light keeps its specular");
}

/// Hostile light fields: the light record and the selection radius sanitise them by one rule
/// (non-finite: the field defaults of `lighting.md` §8.3; negative Falloff: 0), so a light
/// that contributes nothing is never selected and ρ never exceeds the record's range.
#[test]
fn hostile_light_fields_agree() {
    let rho = |f: &LightFields| {
        tessera_render::LightReach::from_fields(f)
            .radius
            .unwrap_or(f32::NAN)
    };
    // NaN Range: the default 25 in both (ρ = 25 × 0.85 with the default Falloff 1).
    let nan_range = LightFields {
        range: f32::NAN,
        ..LightFields::default()
    };
    let r = light_record(&nan_range);
    assert!(close(r.range, 25.0, 6), "range {}", r.range);
    assert!(close(rho(&nan_range), 21.25, 5));
    // NaN Falloff: the default 1.0 in both.
    let nan_falloff = LightFields {
        range: 10.0,
        falloff: f32::NAN,
        ..LightFields::default()
    };
    assert!(close(light_record(&nan_falloff).attenuation, 0.1, 6));
    assert!(close(rho(&nan_falloff), 8.5, 5));
    // Negative Falloff: 0 in both (no attenuation, ρ = Range).
    let negative = LightFields {
        range: 10.0,
        falloff: -2.0,
        ..LightFields::default()
    };
    assert_eq!(light_record(&negative).attenuation, 0.0);
    assert!(close(rho(&negative), 10.0, 5));
    // NaN intensity and scale: 0.5 and 1; a colour with a NaN channel: white.
    let colours = LightFields {
        intensity: f32::NAN,
        scale: f32::INFINITY,
        diffuse: [f32::NAN, 0.0, 0.0],
        ..LightFields::default()
    };
    assert_rgb(light_record(&colours).color, [0.5; 3], 6, "hostile colour");
    // Whatever the input: ρ ≤ R, both finite and non-negative.
    let values = [
        f32::NAN,
        f32::INFINITY,
        -f32::INFINITY,
        -5.0,
        0.0,
        1e-6,
        0.5,
        3.0,
        1e30,
    ];
    for range in values {
        for falloff in values {
            for global in [false, true] {
                let f = LightFields {
                    range,
                    falloff,
                    global_light: global,
                    ..LightFields::default()
                };
                let (rec, p) = (light_record(&f), rho(&f));
                assert!(
                    rec.range.is_finite() && rec.attenuation.is_finite(),
                    "{f:?}"
                );
                assert!(rec.attenuation >= 0.0, "{f:?}");
                assert!(
                    (0.0..=rec.range).contains(&p),
                    "{f:?}: ρ {p}, R {}",
                    rec.range
                );
            }
        }
    }
}

#[test]
fn example_ambient() {
    let (amb, hemi) = frame_ambient(0.2, [1.0, 0.9, 0.8], [0.4, 0.4, 0.5]);
    assert_rgb(amb, [0.14, 0.13, 0.13], 3, "frame ambient");
    assert_rgb(hemi, [0.06, 0.05, 0.03], 3, "frame hemispheric");
    assert_rgb(ambient(1.0, amb, hemi), [0.2, 0.18, 0.16], 3, "up");
    assert_rgb(ambient(-1.0, amb, hemi), [0.08, 0.08, 0.1], 3, "down");
}

#[test]
fn example_crossfade() {
    assert!(close(crossfade(2.0, 8.0), 0.146, 3));
    assert_eq!(crossfade(0.0, 8.0), 0.0);
    assert_eq!(crossfade(8.0, 8.0), 1.0);
    assert_eq!(crossfade(100.0, 8.0), 1.0);
}

#[test]
fn example_alpha_test() {
    assert!(
        !alpha_test_keeps(0.8, 0.8),
        "0.8 at threshold 0.8 is discarded"
    );
    assert!(alpha_test_keeps(0.81, 0.8));
    // Output alpha (§5.12) = α × M.alpha when the base role carries the opacity, else
    // M.alpha; × the vertex alpha where read (§5.13).
    for m in [1.0, 0.5, 0.0] {
        assert!(
            close(output_alpha(Some(0.81), m, 1.0), 0.81 * m, 6),
            "M.alpha {m}"
        );
        assert!(
            close(output_alpha(None, m, 1.0), m, 6),
            "no base opacity, M.alpha {m}"
        );
    }
    assert!(close(output_alpha(Some(0.81), 0.5, 0.5), 0.2025, 6));
    // Hostile values stay in 0..=1.
    assert_eq!(output_alpha(Some(f32::NAN), 2.0, 1.0), 0.0);
    assert_eq!(output_alpha(Some(1.0), 2.0, 1.0), 1.0);
}

/// The §14 alpha-test example on the GPU: a texel alpha just above the threshold is kept and
/// blended by 0.81 × M.alpha; one at the threshold is discarded.
#[test]
fn gpu_alpha_test_example() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    // Unlit white texels, so the blended colour over black is the output alpha itself.
    let texel_alpha = |r: &mut Renderer<'static>, a: u8| {
        r.upload_texture(&TextureUpload {
            color_space: ColorSpace::Linear,
            filter: Filter::Nearest,
            wrap: Wrap::Repeat,
            ..TextureUpload::rgba8(1, 1, &[255, 255, 255, a])
        })
    };
    let kept = texel_alpha(&mut r, 207)?; // 0.8118 > 0.8
    let at = texel_alpha(&mut r, 204)?; // 0.8 exactly: discarded
    let lighting = FrameLighting {
        sun: None,
        ..sun_lighting()
    };
    for (tex, m_alpha, alpha) in [
        (kept, 1.0, Some(207.0 / 255.0)),
        (kept, 0.5, Some(207.0 / 255.0)),
        (at, 1.0, None),
    ] {
        let material = Material {
            unlit: true,
            alpha: AlphaMode::TestBlend { cutoff: 0.8 },
            ..Material {
                base_color: [1.0, 1.0, 1.0, m_alpha],
                ..Material::textured(tex)
            }
        };
        let px = centre_pixel(
            &mut r,
            OffscreenFormat::Rgba8Unorm,
            lighting.clone(),
            40.0,
            material,
            LitSurface::default(),
            &[],
        )?;
        assert!(alpha.is_none_or(|a| alpha_test_keeps(a, 0.8)));
        let out = alpha.map_or(0.0, |a| output_alpha(Some(a), m_alpha, 1.0));
        assert_pixel(
            px,
            [out; 3],
            &format!("texel alpha {alpha:?}, M.alpha {m_alpha}"),
        );
    }
    Ok(())
}

#[test]
fn fog_factor_and_limits() {
    assert!(close(reference::fog_factor(30.0, 10.0, 90.0), 0.25, 6));
    assert_eq!(reference::fog_factor(5.0, 10.0, 90.0), 0.0);
    assert_eq!(reference::fog_factor(500.0, 10.0, 90.0), 1.0);
    // Negative near: fog starts before the eye (§10).
    assert!(close(reference::fog_factor(0.0, -100.0, 100.0), 0.5, 6));
}

// ---- GPU against the reference ----------------------------------------------------------

const SIZE: u32 = 64;
const EYE: Vec3 = Vec3::new(0.0, 0.0, 10.0);

fn camera() -> Camera {
    Camera {
        eye: EYE,
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Perspective {
            fov_y: 20f32.to_radians(),
            near: 0.5,
            far: 100.0,
        },
    }
}

/// A 1 × 1 texture holding `rgb` (stored bytes).
fn texel(
    r: &mut Renderer<'static>,
    rgb: [u8; 3],
    space: ColorSpace,
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: space,
        filter: Filter::Nearest,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(1, 1, &[rgb[0], rgb[1], rgb[2], 255])
    })
}

/// Render `draws` in one lit pass over a `plane`-sized ground at z = 0 and return the
/// centre pixel (the plane's centre).
fn centre_pixel(
    r: &mut Renderer<'static>,
    format: OffscreenFormat,
    lighting: FrameLighting,
    plane: f32,
    ground: Material,
    ground_lit: LitSurface,
    extra: &[MeshDraw],
) -> Result<[u8; 4], RenderError> {
    let mesh = r.upload_mesh(&MeshData::plane(plane, plane, 1, 1))?;
    let target = r.create_offscreen_target(SIZE, SIZE, format)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame
            .world_pass("lit", &camera())
            .clear([0.0, 0.0, 0.0, 1.0])
            .frame_lighting(lighting);
        pass.mesh(MeshDraw::new(mesh, Mat4::IDENTITY, ground).with_lit(ground_lit));
        for d in extra {
            pass.mesh(*d);
        }
    }
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    r.destroy_mesh(mesh);
    Ok(img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]))
}

fn bytes(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn assert_pixel(actual: [u8; 4], expected: [f32; 3], what: &str) {
    let e = bytes(expected);
    let ok = actual[..3].iter().zip(&e).all(|(a, b)| a.abs_diff(*b) <= 1);
    assert!(ok, "{what}: GPU {actual:?}, reference {e:?} ({expected:?})");
}

/// The albedo of the §14 sun-lit texel, as stored bytes.
const ALBEDO: [u8; 3] = [128, 102, 77];

fn albedo() -> [f32; 3] {
    ALBEDO.map(|b| f32::from(b) / 255.0)
}

fn sun_lighting() -> FrameLighting {
    FrameLighting {
        up: Vec3::Z,
        ambient: [0.10, 0.10, 0.12],
        hemispheric: [0.05, 0.05, 0.05],
        sun: Some(SunLight {
            direction: sun_30(),
            color: [0.5, 0.5, 0.45],
            specular: [0.0; 3],
        }),
        fog: None,
        shadow: None,
        lights: Vec::new(),
        time: 0.0,
    }
}

fn sun_texel() -> Texel {
    Texel {
        albedo: albedo(),
        normal: Vec3::Z,
        up: Vec3::Z,
        a0: [0.10, 0.10, 0.12],
        h0: [0.05, 0.05, 0.05],
        lights: vec![LightSample {
            to_light: sun_30(),
            color: [0.5, 0.5, 0.45],
            factor: [1.0; 3],
        }],
        specular: [0.0; 3],
        emissive: [0.0; 3],
        fog: None,
    }
}

#[test]
fn gpu_matches_the_reference() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let linear = texel(&mut r, ALBEDO, ColorSpace::Linear)?;
    let srgb = texel(&mut r, ALBEDO, ColorSpace::Srgb)?;
    let ground = |t| Material::textured(t);
    let lit = LitSurface::default();

    // §14 sun-lit texel without fog, into a Unorm and an sRGB target, from a Linear and
    // an sRGB texture: the stored result is the same.
    let expected = shade(&sun_texel());
    for (format, tex, what) in [
        (OffscreenFormat::Rgba8Unorm, linear, "sun, unorm target"),
        (OffscreenFormat::Rgba8Srgb, linear, "sun, sRGB target"),
        (OffscreenFormat::Rgba8Unorm, srgb, "sun, sRGB texture"),
    ] {
        let px = centre_pixel(&mut r, format, sun_lighting(), 40.0, ground(tex), lit, &[])?;
        assert_pixel(px, expected, what);
    }

    // Fog: the four corners of a 1 × 1 square under the eye are equally far, so the
    // per-vertex factor is the same everywhere: f = 0.25.
    let d = (EYE.z * EYE.z + 0.5f32).sqrt();
    let fog = Fog {
        near: 0.0,
        far: d / 0.25,
        color: [0.6, 0.6, 0.7],
    };
    let px = centre_pixel(
        &mut r,
        OffscreenFormat::Rgba8Unorm,
        FrameLighting {
            fog: Some(fog),
            ..sun_lighting()
        },
        1.0,
        ground(linear),
        lit,
        &[],
    )?;
    let expected = shade(&Texel {
        fog: Some((reference::fog_factor(d, fog.near, fog.far), fog.color)),
        ..sun_texel()
    });
    assert_pixel(px, expected, "sun with fog");
    // The lit base pass always contains fog (§2.1, §5.11): `LitSurface::fog` does not turn
    // it off for a lit surface.
    let fogged = FrameLighting {
        fog: Some(fog),
        ..sun_lighting()
    };
    let px = centre_pixel(
        &mut r,
        OffscreenFormat::Rgba8Unorm,
        fogged.clone(),
        1.0,
        ground(linear),
        LitSurface { fog: false, ..lit },
        &[],
    )?;
    assert_pixel(px, expected, "lit surface, fog switch off");
    // An unlit surface (§7 code 0x00): texture × M, fogged only with the switch on.
    let f = reference::fog_factor(d, fog.near, fog.far);
    let unlit = Material {
        unlit: true,
        ..ground(linear)
    };
    let c = albedo();
    for (switch, want) in [
        (
            true,
            std::array::from_fn(|i| c[i] + f * (fog.color[i] - c[i])),
        ),
        (false, c),
    ] {
        let px = centre_pixel(
            &mut r,
            OffscreenFormat::Rgba8Unorm,
            fogged.clone(),
            1.0,
            unlit,
            LitSurface { fog: switch, ..lit },
            &[],
        )?;
        assert_pixel(px, want, &format!("unlit surface, fog switch {switch}"));
    }

    // A point light 2 above the centre with a specular highlight (eye, light and normal
    // aligned, so N . H = 1): a = 1 / (1 + 0.05 × 4), clamped list specular.
    let light = Light::point(Vec3::new(0.0, 0.0, 2.0), 10.0, 0.05, [1.0, 0.6, 0.2]);
    let a = attenuation(4.0, 100.0, 0.05, true);
    let ms = [0.5, 0.5, 0.5];
    let px = centre_pixel(
        &mut r,
        OffscreenFormat::Rgba8Unorm,
        FrameLighting {
            sun: None,
            ambient: [0.0; 3],
            hemispheric: [0.0; 3],
            lights: vec![light],
            ..sun_lighting()
        },
        40.0,
        Material {
            specular: Some(Specular {
                color: ms,
                shininess: 8.0,
                texture: None,
            }),
            ..ground(linear)
        },
        lit,
        &[],
    )?;
    let half = reference::half_vector(Vec3::Z, Vec3::Z, Vec3::Z);
    // F = light specular (its diffuse colour) x Ms.
    let spec = reference::specular(Vec3::Z, half, a, 8.0, [0.5, 0.3, 0.1], true);
    let expected = shade(&Texel {
        a0: [0.0; 3],
        h0: [0.0; 3],
        lights: vec![LightSample {
            to_light: Vec3::Z,
            color: [1.0, 0.6, 0.2],
            factor: [a; 3],
        }],
        specular: spec,
        ..sun_texel()
    });
    assert_pixel(px, expected, "point light with specular");

    // Shadow: a cube between the sun (45° over +X) and the centre. Shadowed, only the
    // ambient term is left (shadow colour 0, range 1, full fade weight).
    let toward = Vec3::new(1.0, 0.0, 1.0).normalize();
    let cube = r.upload_mesh(&MeshData::cube(0.5))?;
    let blocker = MeshDraw::new(
        cube,
        Mat4::from_translation(Vec3::new(2.0, 0.0, 2.0)),
        Material::color([1.0; 4]),
    );
    let shadowed = FrameLighting {
        sun: Some(SunLight {
            direction: toward,
            color: [0.5, 0.5, 0.45],
            specular: [0.0; 3],
        }),
        shadow: Some(SunShadow {
            quality: 1,
            ..SunShadow::fit(toward, Vec3::new(0.0, 0.0, 1.0), 6.0, 100.0, 1024)
        }),
        ..sun_lighting()
    };
    let px = centre_pixel(
        &mut r,
        OffscreenFormat::Rgba8Unorm,
        shadowed.clone(),
        40.0,
        ground(linear),
        lit,
        &[blocker],
    )?;
    let expected = shade(&Texel {
        lights: vec![LightSample {
            to_light: toward,
            color: [0.5, 0.5, 0.45],
            factor: [0.0; 3],
        }],
        ..sun_texel()
    });
    assert_pixel(px, expected, "in the sun's shadow");
    // The same blocker casting no shadow leaves the centre sun-lit.
    let px = centre_pixel(
        &mut r,
        OffscreenFormat::Rgba8Unorm,
        shadowed,
        40.0,
        ground(linear),
        lit,
        &[blocker.with_lit(LitSurface {
            cast_shadow: false,
            ..lit
        })],
    )?;
    let expected = shade(&Texel {
        lights: vec![LightSample {
            to_light: toward,
            color: [0.5, 0.5, 0.45],
            factor: [1.0; 3],
        }],
        ..sun_texel()
    });
    assert_pixel(px, expected, "blocker without shadow");

    // Alpha test at threshold 0.8: opacity 203/255 is discarded (the clear colour
    // shows), 207/255 is kept (the exact boundary is checked on the CPU above).
    for (alpha, kept) in [(203u8, false), (207u8, true)] {
        let t = r.upload_texture(&TextureUpload {
            color_space: ColorSpace::Linear,
            filter: Filter::Nearest,
            ..TextureUpload::rgba8(1, 1, &[ALBEDO[0], ALBEDO[1], ALBEDO[2], alpha])
        })?;
        let px = centre_pixel(
            &mut r,
            OffscreenFormat::Rgba8Unorm,
            sun_lighting(),
            40.0,
            Material {
                alpha: AlphaMode::Mask { cutoff: 0.8 },
                ..ground(t)
            },
            lit,
            &[],
        )?;
        if kept {
            assert_pixel(px, shade(&sun_texel()), "alpha test keeps 207");
        } else {
            assert_eq!(px, [0, 0, 0, 255], "alpha test discards 203");
        }
    }
    Ok(())
}
