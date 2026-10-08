// SPDX-License-Identifier: GPL-3.0-or-later
//! Lit-pipeline features read back from the GPU against `docs/specs/lighting.md` and the
//! CPU reference (`tessera_render::lighting::reference`): the output alpha of each opacity
//! source (§5.12), the normal-map green convention (§5.2.4) with mirrored transforms, dye
//! from a mask texture and from the base alpha (never an opacity, `material.md` §11.2, also
//! in the shadow pass), directional table lights and explicit light selections, the
//! eight-lights-in-all cap (§8.5), blending with depth write, and the M2 fallback of the
//! lit-only alpha modes.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::lighting::reference::{LightSample, Texel, shade};
use tessera_render::{
    AlphaMode, BlendFactor, BlendOp, Camera, ColorSpace, Dye, DyeMask, Filter, FixedBlend,
    FrameLighting, Light, LightKind, LightSelection, LitSurface, Material, MeshData, MeshDraw,
    MeshId, OffscreenFormat, Opacity, Perspective, RenderError, Renderer, RgbaImage, SunLight,
    SunShadow, TextureId, TextureUpload, Wrap,
};

const SIZE: u32 = 64;

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Perspective {
            fov_y: 20f32.to_radians(),
            near: 0.5,
            far: 100.0,
        },
    }
}

/// A 1 × 1 linear texture holding `rgba` (stored bytes).
fn texel(r: &mut Renderer<'static>, rgba: [u8; 4]) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        wrap: Wrap::Repeat,
        ..TextureUpload::rgba8(1, 1, &rgba)
    })
}

/// No sun, no lights, ambient `a` (an up-facing white texel then shades to 2 × `a`).
fn ambient_only(a: f32) -> FrameLighting {
    FrameLighting {
        ambient: [a; 3],
        hemispheric: [0.0; 3],
        sun: None,
        ..FrameLighting::default()
    }
}

/// Render each pass's draws (one lit pass per entry, `None` = an M2 pass) into a Unorm
/// target over black; later passes keep the depth of earlier ones.
fn render(
    r: &mut Renderer<'static>,
    passes: &[(Option<FrameLighting>, Vec<MeshDraw>)],
) -> Result<RgbaImage, RenderError> {
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    for (i, (lighting, draws)) in passes.iter().enumerate() {
        let pass = frame.world_pass(format!("pass-{i}"), &camera());
        if i == 0 {
            pass.clear([0.0, 0.0, 0.0, 1.0]);
        }
        if let Some(l) = lighting {
            pass.frame_lighting(l.clone());
        }
        for d in draws {
            pass.mesh(*d);
        }
    }
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

fn centre(
    r: &mut Renderer<'static>,
    lighting: &FrameLighting,
    draws: Vec<MeshDraw>,
) -> Result<[u8; 4], RenderError> {
    let img = render(r, &[(Some(lighting.clone()), draws)])?;
    Ok(img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]))
}

fn bytes(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn assert_pixel(actual: [u8; 4], expected: [f32; 3], what: &str) {
    let e = bytes(expected);
    let ok = actual[..3].iter().zip(&e).all(|(a, b)| a.abs_diff(*b) <= 1);
    assert!(ok, "{what}: GPU {actual:?}, expected {e:?} ({expected:?})");
}

fn ground(r: &mut Renderer<'static>) -> Result<MeshId, RenderError> {
    r.upload_mesh(&MeshData::plane(40.0, 40.0, 1, 1))
}

fn draw(mesh: MeshId, material: Material, lit: LitSurface) -> MeshDraw {
    MeshDraw::new(mesh, Mat4::IDENTITY, material).with_lit(lit)
}

#[test]
fn output_alpha_follows_the_opacity_source() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let base_06 = texel(&mut r, [255, 255, 255, 153])?;
    let alpha_06 = texel(&mut r, [0, 0, 0, 153])?;
    let alpha_04 = texel(&mut r, [0, 0, 0, 102])?;
    // White texel under ambient 0.5: lit colour 1.0.
    let l = ambient_only(0.5);
    let test_blend = |t| Material {
        alpha: AlphaMode::TestBlend { cutoff: 0.5 },
        ..Material::textured(t)
    };
    let lit = |opacity, emissive| LitSurface {
        opacity,
        emissive,
        ..LitSurface::default()
    };
    // lighting.md §5.12: the opacity of slot 3020 or 3018 drives only the alpha test; the
    // kept texels are opaque (output alpha = M.alpha = 1).
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            test_blend(white),
            lit(Opacity::Texture(alpha_06), None),
        )],
    )?;
    assert_pixel(px, [1.0; 3], "opacity texture, kept");
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            test_blend(white),
            lit(Opacity::EmissiveAlpha, Some(alpha_06)),
        )],
    )?;
    assert_pixel(px, [1.0; 3], "emissive alpha, kept");
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            test_blend(white),
            lit(Opacity::Texture(alpha_04), None),
        )],
    )?;
    assert_eq!(px, [0, 0, 0, 255], "opacity texture 0.4 at threshold 0.5");
    // The base opacity blends by its alpha: 0.6 over black.
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            test_blend(base_06),
            lit(Opacity::BaseAlpha, None),
        )],
    )?;
    assert_pixel(px, [0.6; 3], "base opacity blends");
    // M.alpha still blends with any source.
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            Material {
                alpha: AlphaMode::Blend,
                base_color: [1.0, 1.0, 1.0, 0.5],
                ..Material::textured(white)
            },
            lit(Opacity::Texture(alpha_06), None),
        )],
    )?;
    assert_pixel(px, [0.5; 3], "M.alpha with an opacity texture");
    Ok(())
}

/// The shading normal of a tangent-space texel on `MeshData::plane` (T = +X, B = +V = −Y,
/// N₀ = +Z) by `lighting.md` §5.2.4: N = n.x T − n.y B + n.z N₀.
fn plane_normal(t: [u8; 3]) -> Vec3 {
    let n = Vec3::from(t.map(|c| f32::from(c) / 255.0 * 2.0 - 1.0)).normalize();
    (n.x * Vec3::X - n.y * Vec3::NEG_Y + n.z * Vec3::Z).normalize()
}

#[test]
fn normal_maps_follow_the_lighting_spec() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let sun = |toward: Vec3| FrameLighting {
        sun: Some(SunLight {
            direction: toward,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..ambient_only(0.0)
    };
    let expected = |n: Vec3, toward: Vec3| {
        shade(&Texel {
            albedo: [1.0; 3],
            normal: n,
            up: Vec3::Z,
            a0: [0.0; 3],
            h0: [0.0; 3],
            lights: vec![LightSample {
                to_light: toward.normalize(),
                color: [0.5; 3],
                factor: [1.0; 3],
            }],
            specular: [0.0; 3],
            emissive: [0.0; 3],
            fog: None,
        })
    };
    let mirror = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    for (rgb, toward) in [
        // Positive green tilts towards the image top (+Y here): lit by a light at +Y.
        ([128u8, 255, 128], Vec3::new(0.0, 1.0, 0.1)),
        ([128, 0, 128], Vec3::new(0.0, -1.0, 0.1)),
        // Positive red along the tangent (+u = +X).
        ([255, 128, 128], Vec3::new(1.0, 0.0, 0.1)),
    ] {
        let normal = texel(&mut r, [rgb[0], rgb[1], rgb[2], 255])?;
        let n = plane_normal(rgb);
        let want = expected(n, toward);
        assert!(want[0] > 0.9, "the light faces the perturbed normal");
        for green_up in [false, true] {
            let m = Material {
                normal_texture: Some(normal),
                normal_green_up: green_up,
                ..Material::textured(white)
            };
            let px = centre(
                &mut r,
                &sun(toward),
                vec![draw(plane, m, LitSurface::default())],
            )?;
            assert_pixel(px, want, &format!("texel {rgb:?}, green_up {green_up}"));
            // Mirrored along X: the texture is mirrored with the surface, so the tangent
            // flips and the red axis points to −X; green is unchanged.
            let mirrored_toward = Vec3::new(-toward.x, toward.y, toward.z);
            let px = centre(
                &mut r,
                &sun(mirrored_toward),
                vec![MeshDraw::new(plane, mirror, m).with_lit(LitSurface::default())],
            )?;
            assert_pixel(px, want, &format!("mirrored texel {rgb:?}"));
        }
    }
    Ok(())
}

#[test]
fn dye_from_a_mask_and_from_the_base_alpha() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let base = [200u8, 200, 200];
    let c = f32::from(base[0]) / 255.0;
    let d = [1.0, 0.5, 0.25];
    let dyed = d.map(|x| c * x);
    let l = ambient_only(0.5);
    let mask = texel(&mut r, [255, 0, 0, 255])?;
    let opaque_base = texel(&mut r, [base[0], base[1], base[2], 255])?;
    let px = centre(
        &mut r,
        &l,
        vec![draw(
            plane,
            Material {
                dye: Some(Dye {
                    color: d,
                    mask: DyeMask::Texture(mask),
                }),
                ..Material::textured(opaque_base)
            },
            LitSurface::default(),
        )],
    )?;
    assert_pixel(px, dyed, "dye mask texture");
    // Base alpha 1 = fully dyed, 0 = undyed; with an alpha test the undyed texel must not
    // be discarded (the base alpha is a dye mask, not an opacity).
    let undyed_base = texel(&mut r, [base[0], base[1], base[2], 0])?;
    for (tex, expected, what) in [
        (opaque_base, dyed, "base alpha 1 dyes"),
        (undyed_base, [c; 3], "base alpha 0 stays undyed and is kept"),
    ] {
        for alpha in [
            AlphaMode::Opaque,
            AlphaMode::Mask { cutoff: 0.5 },
            AlphaMode::TestBlend { cutoff: 0.5 },
        ] {
            let px = centre(
                &mut r,
                &l,
                vec![draw(
                    plane,
                    Material {
                        alpha,
                        dye: Some(Dye {
                            color: d,
                            mask: DyeMask::BaseAlpha,
                        }),
                        ..Material::textured(tex)
                    },
                    LitSurface::default(),
                )],
            )?;
            assert_pixel(px, expected, &format!("{what} ({alpha:?})"));
        }
    }
    Ok(())
}

#[test]
fn base_alpha_dye_casts_a_full_shadow() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let card = r.upload_mesh(&MeshData::plane(1.0, 1.0, 1, 1))?;
    let white = texel(&mut r, [255; 4])?;
    let undyed = texel(&mut r, [255, 255, 255, 0])?;
    // The sun at 45° over +X; a card at (2, 0, 2) shadows the centre (outside the view).
    let toward = Vec3::new(1.0, 0.0, 1.0).normalize();
    let l = FrameLighting {
        sun: Some(SunLight {
            direction: toward,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        shadow: Some(SunShadow {
            quality: 1,
            ..SunShadow::fit(toward, Vec3::new(0.0, 0.0, 1.0), 6.0, 100.0, 1024)
        }),
        ..ambient_only(0.25)
    };
    let ground_draw = draw(plane, Material::textured(white), LitSurface::default());
    let card_at = |dye: Option<Dye>| {
        MeshDraw::new(
            card,
            Mat4::from_translation(Vec3::new(2.0, 0.0, 2.0)),
            Material {
                alpha: AlphaMode::Mask { cutoff: 0.5 },
                dye,
                ..Material::textured(undyed)
            },
        )
    };
    let shadowed = 2.0 * 0.25;
    let sunlit = 2.0 * (0.25 + 0.5 * toward.z);
    // Without a dye the base alpha 0 is the opacity: the card is cut out, no shadow.
    let px = centre(&mut r, &l, vec![ground_draw, card_at(None)])?;
    assert_pixel(px, [sunlit; 3], "cut-out card casts nothing");
    // With the base alpha as the dye mask the card is solid and casts a shadow.
    let dye = Dye {
        color: [1.0, 0.0, 0.0],
        mask: DyeMask::BaseAlpha,
    };
    let px = centre(&mut r, &l, vec![ground_draw, card_at(Some(dye))])?;
    assert_pixel(px, [shadowed; 3], "dyed card casts a shadow");
    Ok(())
}

/// lighting.md §5.2.4: N = n.x T − n.y B + n.z N₀ with the stored tangent as given (only
/// normalised), not made orthogonal to the normal first.
#[test]
fn tangents_are_used_as_given() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let mut mesh = MeshData::plane(40.0, 40.0, 1, 1);
    let t = Vec3::new(1.0, 0.0, 1.0).normalize();
    let Some(tangents) = mesh.tangents.as_mut() else {
        panic!("the plane has tangents");
    };
    for tg in tangents.iter_mut() {
        *tg = [t.x, t.y, t.z, tg[3]];
    }
    let plane = r.upload_mesh(&mesh)?;
    let white = texel(&mut r, [255; 4])?;
    let rgb = [204u8, 128, 230];
    let normal = texel(&mut r, [rgb[0], rgb[1], rgb[2], 255])?;
    let lighting = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::X,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        ..ambient_only(0.0)
    };
    let m = Material {
        normal_texture: Some(normal),
        ..Material::textured(white)
    };
    let px = centre(
        &mut r,
        &lighting,
        vec![draw(plane, m, LitSurface::default())],
    )?;
    let n = Vec3::from(rgb.map(|c| f32::from(c) / 255.0 * 2.0 - 1.0)).normalize();
    // B = −Y on the plane (w × cross(N₀, T) normalised); colour = 2 × 0.5 × N · L.
    let shading = (n.x * t - n.y * Vec3::NEG_Y + n.z * Vec3::Z).normalize();
    assert_pixel(px, [shading.x; 3], "non-orthogonal tangent");
    Ok(())
}

#[test]
fn directional_table_lights_and_explicit_selections() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let toward = Vec3::new(0.0, 0.6, 0.8);
    let l = FrameLighting {
        lights: vec![
            Light {
                kind: LightKind::Directional { direction: toward },
                color: [0.5, 0.25, 0.125],
                specular: [0.0; 3],
            },
            // Out of reach of the centre for Auto, but listed explicitly below.
            Light::point(Vec3::new(0.0, 0.0, 1.0), 2.0, 0.0, [0.25; 3]),
        ],
        ..ambient_only(0.0)
    };
    let m = Material::textured(white);
    let with = |lights| LitSurface {
        lights,
        ..LitSurface::default()
    };
    let dir = [0.5 * 0.8 * 2.0, 0.25 * 0.8 * 2.0, 0.125 * 0.8 * 2.0];
    // Auto: the point light touches the ground's bounding sphere too (it is near the
    // centre), so both contribute: 2 × (0.25 × a) with a = 1 at d = 1.
    let px = centre(&mut r, &l, vec![draw(plane, m, with(LightSelection::Auto))])?;
    let both = [dir[0] + 0.5, dir[1] + 0.5, dir[2] + 0.5];
    assert_pixel(px, both, "auto: directional and point");
    let px = centre(&mut r, &l, vec![draw(plane, m, with(LightSelection::None))])?;
    assert_eq!(px, [0, 0, 0, 255], "no table light");
    let list = |indices: &[u8]| {
        let mut a = [0u8; 8];
        a[..indices.len()].copy_from_slice(indices);
        LightSelection::List {
            indices: a,
            count: indices.len() as u8,
        }
    };
    let px = centre(&mut r, &l, vec![draw(plane, m, with(list(&[0])))])?;
    assert_pixel(px, dir, "list: the directional light");
    let px = centre(&mut r, &l, vec![draw(plane, m, with(list(&[1, 9])))])?;
    assert_pixel(px, [0.5; 3], "list: the point light (9 is past the table)");
    // One extra directional slot per draw (§5.6, §8.2): with two directional table lights,
    // Auto takes the first only.
    let mut two = l.clone();
    two.lights.push(Light {
        kind: LightKind::Directional { direction: toward },
        color: [0.25; 3],
        specular: [0.0; 3],
    });
    let px = centre(
        &mut r,
        &two,
        vec![draw(plane, m, with(LightSelection::Auto))],
    )?;
    assert_pixel(px, both, "auto: one directional light");
    // A list is the caller's: both directional lights.
    let px = centre(&mut r, &two, vec![draw(plane, m, with(list(&[0, 2])))])?;
    let sum = [dir[0] + 0.4, dir[1] + 0.4, dir[2] + 0.4];
    assert_pixel(px, sum, "list: two directional lights");
    Ok(())
}

#[test]
fn eight_lights_in_all_with_the_sun() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let lights = vec![Light::point(Vec3::new(0.0, 0.0, 2.0), 10.0, 0.0, [0.05; 3]); 8];
    let black_sun = FrameLighting {
        sun: Some(SunLight {
            direction: Vec3::Z,
            color: [0.0; 3],
            specular: [0.0; 3],
        }),
        lights: lights.clone(),
        ..ambient_only(0.0)
    };
    let m = Material::textured(white);
    // lighting.md §8.5: at most 8 in all, the main light included: 7 table lights.
    let px = centre(
        &mut r,
        &black_sun,
        vec![draw(plane, m, LitSurface::default())],
    )?;
    assert_pixel(px, [2.0 * 7.0 * 0.05; 3], "sun + 7");
    // Without a main light the default directional light holds the light-0 slot and counts
    // toward the 8 (lighting.md §8.5 Decision): still 7 table lights.
    let mut no_sun = FrameLighting {
        sun: None,
        ..black_sun
    };
    let px = centre(&mut r, &no_sun, vec![draw(plane, m, LitSurface::default())])?;
    assert_pixel(px, [2.0 * 7.0 * 0.05; 3], "default light + 7 without a sun");
    // A key directional table light takes the light-0 slot instead: 7 point lights and it.
    no_sun.lights.push(Light {
        kind: LightKind::Directional { direction: Vec3::Z },
        color: [0.1; 3],
        specular: [0.0; 3],
    });
    let mut reach = vec![tessera_render::LightReach::default(); 9];
    let target = r.create_offscreen_target(SIZE, SIZE, OffscreenFormat::Rgba8Unorm)?;
    let shade_with = |r: &mut Renderer<'static>, reach: Vec<tessera_render::LightReach>| {
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame
            .world_pass("key", &camera())
            .clear([0.0, 0.0, 0.0, 1.0])
            .frame_lighting(no_sun.clone())
            .light_reach(reach)
            .mesh(draw(plane, m, LitSurface::default()));
        r.end_frame(frame)?;
        let img = r.read_pixels(&target)?;
        Ok::<_, RenderError>(img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]))
    };
    // Not a key light: the extra directional slot is not reached (7 point lights).
    let px = shade_with(&mut r, reach.clone())?;
    assert_pixel(
        px,
        [2.0 * 7.0 * 0.05; 3],
        "non-key directional light left out",
    );
    reach[8].key_light = true;
    let px = shade_with(&mut r, reach)?;
    assert_pixel(
        px,
        [2.0 * (7.0 * 0.05 + 0.1); 3],
        "key directional light as light 0",
    );
    Ok(())
}

#[test]
fn blend_with_depth_write_occludes_later_passes() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let l = ambient_only(0.5);
    let pane = |alpha| {
        MeshDraw::new(
            plane,
            Mat4::from_translation(Vec3::Z),
            Material {
                alpha,
                base_color: [1.0, 1.0, 1.0, 0.5],
                ..Material::textured(white)
            },
        )
        .with_lit(LitSurface {
            opacity: Opacity::One,
            ..LitSurface::default()
        })
    };
    // Behind the pane, drawn by a later pass that keeps the depth.
    let red = draw(
        plane,
        Material::color([1.0, 0.0, 0.0, 1.0]),
        LitSurface::default(),
    );
    let at_centre = |img: RgbaImage| img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]);
    let img = render(
        &mut r,
        &[
            (Some(l.clone()), vec![pane(AlphaMode::BLEND_DEPTH_WRITE)]),
            (Some(l.clone()), vec![red]),
        ],
    )?;
    assert_pixel(at_centre(img), [0.5; 3], "pane writes depth");
    let img = render(
        &mut r,
        &[
            (Some(l.clone()), vec![pane(AlphaMode::Blend)]),
            (Some(l), vec![red]),
        ],
    )?;
    assert_pixel(at_centre(img), [1.0, 0.0, 0.0], "Blend writes no depth");
    Ok(())
}

/// material.md §11.3 (unlit records): the most common (src, dst, op) of the render state as
/// a fixed blend, depth write from the same block.
#[test]
fn fixed_blends_follow_the_render_state() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let white = texel(&mut r, [255; 4])?;
    let l = ambient_only(0.5);
    let fixed = |src, dst, depth_write| {
        AlphaMode::Fixed(FixedBlend {
            src,
            dst,
            op: BlendOp::Add,
            depth_write,
        })
    };
    // An additive pane (SRCALPHA / ONE, alpha 0.5, lit colour 1) in front of the ground.
    let pane = |alpha| {
        MeshDraw::new(
            plane,
            Mat4::from_translation(Vec3::Z),
            Material {
                alpha,
                base_color: [1.0, 1.0, 1.0, 0.5],
                ..Material::textured(white)
            },
        )
        .with_lit(LitSurface {
            opacity: Opacity::One,
            ..LitSurface::default()
        })
    };
    let red = draw(
        plane,
        Material::color([1.0, 0.0, 0.0, 1.0]),
        LitSurface::default(),
    );
    let at_centre = |img: RgbaImage| img.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]);
    for (depth_write, want) in [(true, [0.5; 3]), (false, [1.0, 0.0, 0.0])] {
        let img = render(
            &mut r,
            &[
                (
                    Some(l.clone()),
                    vec![pane(fixed(
                        BlendFactor::SrcAlpha,
                        BlendFactor::One,
                        depth_write,
                    ))],
                ),
                (Some(l.clone()), vec![red]),
            ],
        )?;
        assert_pixel(at_centre(img), want, &format!("depth write {depth_write}"));
    }
    // Additive keeps its meaning: SRCALPHA / ONE without depth write.
    let img = render(
        &mut r,
        &[
            (Some(l.clone()), vec![pane(AlphaMode::Additive)]),
            (Some(l.clone()), vec![red]),
        ],
    )?;
    assert_pixel(at_centre(img), [1.0, 0.0, 0.0], "Additive writes no depth");
    // ZERO / SRCCOLOR multiplies the destination by the source colour (an unlit
    // (0.5, 0.25, 1) over the white-lit ground).
    let modulate = MeshDraw::new(
        plane,
        Mat4::from_translation(Vec3::Z),
        Material {
            alpha: fixed(BlendFactor::Zero, BlendFactor::SrcColor, false),
            base_color: [0.5, 0.25, 1.0, 1.0],
            unlit: true,
            ..Material::textured(white)
        },
    );
    let ground_draw = draw(plane, Material::textured(white), LitSurface::default());
    let img = render(&mut r, &[(Some(l.clone()), vec![ground_draw, modulate])])?;
    assert_pixel(at_centre(img), [0.5, 0.25, 1.0], "ZERO / SRCCOLOR");
    // DESTCOLOR / ONE: dst × src + dst.
    let dst_one = MeshDraw::new(
        plane,
        Mat4::from_translation(Vec3::Z),
        Material {
            alpha: fixed(BlendFactor::DstColor, BlendFactor::One, false),
            base_color: [0.5, 0.25, 0.0, 1.0],
            unlit: true,
            ..Material::textured(white)
        },
    );
    let half = draw(
        plane,
        Material::color([0.5, 0.5, 0.5, 1.0]),
        LitSurface::default(),
    );
    let img = render(&mut r, &[(Some(l), vec![half, dst_one])])?;
    assert_pixel(at_centre(img), [0.75, 0.625, 0.5], "DESTCOLOR / ONE");
    Ok(())
}

#[test]
fn lit_only_alpha_modes_fall_back_outside_lit_passes() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    for a in [102u8, 153] {
        let t = texel(&mut r, [200, 150, 100, a])?;
        let m = |alpha| Material {
            alpha,
            ..Material::textured(t)
        };
        let mask = render(
            &mut r,
            &[(
                None,
                vec![MeshDraw::new(
                    plane,
                    Mat4::IDENTITY,
                    m(AlphaMode::Mask { cutoff: 0.5 }),
                )],
            )],
        )?;
        let test_blend = render(
            &mut r,
            &[(
                None,
                vec![MeshDraw::new(
                    plane,
                    Mat4::IDENTITY,
                    m(AlphaMode::TestBlend { cutoff: 0.5 }),
                )],
            )],
        )?;
        assert_eq!(mask, test_blend, "TestBlend draws as Mask (alpha {a})");
    }
    let t = texel(&mut r, [200, 150, 100, 255])?;
    let m = |alpha| Material {
        alpha,
        base_color: [1.0, 1.0, 1.0, 0.5],
        ..Material::textured(t)
    };
    let blend = render(
        &mut r,
        &[(
            None,
            vec![MeshDraw::new(plane, Mat4::IDENTITY, m(AlphaMode::Blend))],
        )],
    )?;
    let additive = render(
        &mut r,
        &[(
            None,
            vec![MeshDraw::new(plane, Mat4::IDENTITY, m(AlphaMode::Additive))],
        )],
    )?;
    assert_eq!(blend, additive, "Additive draws as Blend");
    let fixed = render(
        &mut r,
        &[(
            None,
            vec![MeshDraw::new(
                plane,
                Mat4::IDENTITY,
                m(AlphaMode::Fixed(FixedBlend {
                    src: BlendFactor::Zero,
                    dst: BlendFactor::SrcColor,
                    op: BlendOp::Add,
                    depth_write: true,
                })),
            )],
        )],
    )?;
    assert_eq!(blend, fixed, "Fixed draws as Blend");
    let centre = blend.pixel(SIZE / 2, SIZE / 2).unwrap_or([0; 4]);
    assert!(centre[0] > 0, "the blended plane is visible: {centre:?}");
    Ok(())
}

#[test]
fn casters_beyond_the_fitted_sphere_still_cast() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let plane = ground(&mut r)?;
    let card = r.upload_mesh(&MeshData::plane(1.0, 1.0, 1, 1))?;
    let white = texel(&mut r, [255; 4])?;
    let toward = Vec3::new(1.0, 0.0, 1.0).normalize();
    // A small fitted sphere (radius 3 around (0, 0, 1)): the cards at height 5 and 12 lie
    // 6.4 and 16.3 toward the light, beyond 2 × radius. All of them stay out of view.
    let l = FrameLighting {
        sun: Some(SunLight {
            direction: toward,
            color: [0.5; 3],
            specular: [0.0; 3],
        }),
        shadow: Some(SunShadow {
            quality: 1,
            ..SunShadow::fit(toward, Vec3::new(0.0, 0.0, 1.0), 3.0, 100.0, 1024)
        }),
        ..ambient_only(0.25)
    };
    for h in [2.0f32, 5.0, 12.0] {
        let blocker = MeshDraw::new(
            card,
            Mat4::from_translation(Vec3::new(h, 0.0, h)),
            Material::textured(white),
        );
        let px = centre(
            &mut r,
            &l,
            vec![
                draw(plane, Material::textured(white), LitSurface::default()),
                blocker,
            ],
        )?;
        assert_pixel(px, [0.5; 3], &format!("caster at height {h}"));
    }
    Ok(())
}
