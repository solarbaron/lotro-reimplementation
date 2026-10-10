// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-pass lighting of the lit pipelines (M4.5): the sun or moon with its shadow, the
//! ambient and hemispheric terms, distance fog and the pass's light table.
//!
//! A world pass becomes **lit** when it is given a [`FrameLighting`]
//! ([`crate::Pass::frame_lighting`]). Its mesh draws then go through the lit pipeline,
//! which evaluates the equation of `docs/specs/lighting.md` §5 (one base pass that loops
//! over all lights, which §2.1 allows), and the renderer uploads one frame uniform
//! (`docs/specs/render.md` §4.2) for the pass. Every other pipeline that draws into a lit pass (terrain layers, water, sky,
//! instanced scenery) binds the same uniform and shadow map as its group 0 and prepends
//! [`FRAME_WGSL`] and [`FRAME_SHADING_WGSL`] to its shader (`docs/specs/render.md` §4).
//!
//! Colours are in the original's **stored-value space**: no back end of the original
//! decodes sRGB (`lighting.md` §5.1), so light, fog and material colours are given as the
//! data stores them and the lit shaders compute on stored values.
//!
//! [`reference`](mod@reference) is a CPU twin of the shader equations, used by the tests to reproduce the
//! worked examples of `lighting.md` §14 and to check rendered pixels.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Light-table entries per lit pass (the size of the frame uniform's light table).
pub const MAX_FRAME_LIGHTS: usize = 256;

/// Lights a single draw receives **in all, the main light included** (`lighting.md` §8.5:
/// at most 8). A draw takes at most 7 table lights: the eighth slot is light 0 of the sun or,
/// in a pass without one, of the default directional light (Tessera Decision of §8.5), unless
/// a key directional table light takes it.
pub const MAX_DRAW_LIGHTS: usize = 8;

/// Shared WGSL declarations of the frame uniform (`Frame`, `FrameLight`) and its binding
/// (`@group(0) @binding(0) var<uniform> frame`). Prepend to every shader that draws into
/// a lit pass.
pub const FRAME_WGSL: &str = include_str!("shaders/frame.wgsl");

/// Shared WGSL lighting functions (`frame_ambient`, `frame_sun`, `frame_light`,
/// `frame_fog_factor`, `frame_shadow_coords`, `frame_shadow_factor`, `frame_output`, …)
/// and the shadow-map bindings (`@group(0) @binding(1)` depth texture,
/// `@binding(2)` comparison sampler). Append after [`FRAME_WGSL`].
pub const FRAME_SHADING_WGSL: &str = include_str!("shaders/frame_shading.wgsl");

/// Fog limits of the original (`lighting.md` §10, L26). The cap of the keyframe's fog end
/// ([`FOG_FAR_CAP`]) comes before an interior override; the other limits after it.
const FOG_FAR_MAX: f32 = FOG_FAR_CAP;
/// The cap of a keyframe's fog end (`lighting.md` §10: applied **before** an interior
/// override, so an override's end is not capped). Callers apply it with
/// [`Fog::keyframe_capped`] (or [`Fog::limited`] when there is no override); the renderer
/// does not apply it ([`Fog::lower_limits`]).
pub const FOG_FAR_CAP: f32 = 10_000.0;
const FOG_FAR_MIN: f32 = 20.0;
const FOG_NEAR_MIN: f32 = -10_000.0;
const FOG_NEAR_GAP: f32 = 20.0;

/// Smallest light range (`lighting.md` §8.3, light record).
pub const MIN_LIGHT_RANGE: f32 = 0.0002;

/// Share of its Range that a light with Falloff ≥ 1 reaches when lights are selected
/// (`lighting.md` §8.5 step 1, a client constant quoted by the spec, decision 0012).
pub const SELECTION_FRACTION: f32 = 0.85;

/// The main directional light, light 0 (`lighting.md` §5.4, §8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunLight {
    /// Vector **towards** the light (the negated travel direction), world space;
    /// normalised by the renderer. A zero or non-finite vector disables the light.
    pub direction: Vec3,
    /// Light colour *E₀* before the material colour (Tessera Decision of §8.1:
    /// *Cd* × *bd*).
    pub color: [f32; 3],
    /// Specular colour before the material specular colour (§8.1: *Cs*).
    pub specular: [f32; 3],
}

/// Linear distance fog (`lighting.md` §10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    /// Distance from the eye where fog starts (may be negative).
    pub near: f32,
    /// Distance where fog is complete.
    pub far: f32,
    /// Fog colour.
    pub color: [f32; 3],
}

impl Fog {
    /// The distances after all the original's limits (`lighting.md` §10, L26), for a fog
    /// **without** an interior override: [`Self::keyframe_capped`] then
    /// [`Self::lower_limits`] (far in `20..=10,000`, near in `-10,000..=far - 20`).
    /// Non-finite values become the limits. With an override, cap the keyframe's fog first,
    /// apply the override, and leave the rest to the renderer.
    #[must_use]
    pub fn limited(self) -> Self {
        self.keyframe_capped().lower_limits()
    }

    /// A keyframe's fog with its end capped at [`FOG_FAR_CAP`] (`lighting.md` §10: the
    /// first limit, applied before an interior override replaces the end). A non-finite
    /// end becomes the cap; the start is unchanged.
    #[must_use]
    pub fn keyframe_capped(self) -> Self {
        let far = if self.far.is_finite() {
            self.far.min(FOG_FAR_CAP)
        } else {
            FOG_FAR_CAP
        };
        Self { far, ..self }
    }

    /// The limits the original applies after the interior override (`lighting.md` §10,
    /// L26): far at least 20, near in `-10,000..=far - 20`; the end is **not** capped. A
    /// non-finite far becomes [`FOG_FAR_CAP`] and a non-finite near −10,000. The renderer
    /// applies this to every fog it uploads.
    #[must_use]
    pub fn lower_limits(self) -> Self {
        let far = if self.far.is_finite() {
            self.far.max(FOG_FAR_MIN)
        } else {
            FOG_FAR_MAX
        };
        let near = if self.near.is_finite() {
            self.near.clamp(FOG_NEAR_MIN, far - FOG_NEAR_GAP)
        } else {
            FOG_NEAR_MIN
        };
        Self {
            near,
            far,
            color: self.color,
        }
    }
}

/// What a light-table entry is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    /// A point light (`lighting.md` §8.3).
    Point {
        /// World position.
        position: Vec3,
        /// Range *R* (the light contributes nothing at *d* ≥ *R*).
        range: f32,
        /// Attenuation factor *k* in 1 / (1 + *k* · *d²*).
        attenuation: f32,
    },
    /// An extra directional light (`lighting.md` §8.2).
    Directional {
        /// Vector towards the light (normalised by the renderer).
        direction: Vec3,
    },
}

/// One entry of a pass's light table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    /// Point or directional.
    pub kind: LightKind,
    /// Diffuse colour before the material colour.
    pub color: [f32; 3],
    /// Specular colour before the material specular colour.
    pub specular: [f32; 3],
}

impl Light {
    /// A point light whose specular colour is its diffuse colour.
    #[must_use]
    pub fn point(position: Vec3, range: f32, attenuation: f32, color: [f32; 3]) -> Self {
        Self {
            kind: LightKind::Point {
                position,
                range,
                attenuation,
            },
            color,
            specular: color,
        }
    }

    /// The light record of a placed light (`lighting.md` §8.3, L19) at `position`.
    #[must_use]
    pub fn from_fields(position: Vec3, fields: &LightFields) -> Self {
        let r = reference::light_record(fields);
        Self {
            kind: LightKind::Point {
                position,
                range: r.range,
                attenuation: r.attenuation,
            },
            color: r.color,
            specular: r.specular,
        }
    }
}

/// The fields of a placed light that its light record is built from (`lighting.md` §8.3;
/// the `Light…` properties of a Light entity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightFields {
    /// `LightDiffuse` (default white).
    pub diffuse: [f32; 3],
    /// `LightDiffuseIntensity` evaluated at the current time (default 0.5).
    pub intensity: f32,
    /// `LightSpecular` (default white).
    pub specular: [f32; 3],
    /// `LightRange` (default 25).
    pub range: f32,
    /// `LightFalloff` (default 1.0).
    pub falloff: f32,
    /// `KeyLight` (default off).
    pub key_light: bool,
    /// The run-time scale *σ* (Tessera Decision: 1).
    pub scale: f32,
    /// `Light_GlobalLight` (default off): the selection radius is the full Range
    /// (`lighting.md` §8.5 step 1).
    pub global_light: bool,
    /// `Light_BakedLandscapeLight` (default off): the light never reaches terrain or
    /// ceiling chunks (`lighting.md` §8.3, §8.5 step 3, L30).
    pub baked_landscape: bool,
    /// `Light_OnlyInfluenceParent` (default off; §8.5 step 3).
    pub only_influence_parent: bool,
    /// `Light_NeverInfluenceParent` (default off; §8.5 step 3).
    pub never_influence_parent: bool,
}

impl Default for LightFields {
    /// The defaults of a new light (`lighting.md` §8.3).
    fn default() -> Self {
        Self {
            diffuse: [1.0; 3],
            intensity: 0.5,
            specular: [1.0; 3],
            range: 25.0,
            falloff: 1.0,
            key_light: false,
            scale: 1.0,
            global_light: false,
            baked_landscape: false,
            only_influence_parent: false,
            never_influence_parent: false,
        }
    }
}

/// Which objects a light-table entry reaches when the renderer builds a draw's light list
/// ([`crate::LightSelection::Auto`], `lighting.md` §8.5). Entry *i* of
/// [`crate::Pass::light_reach`] describes entry *i* of [`FrameLighting::lights`]; a light
/// without an entry uses [`LightReach::default`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LightReach {
    /// Selection radius ρ (§8.5 step 1, [`reference::selection_radius`]). `None`, negative or
    /// non-finite: the light's range *R*.
    pub radius: Option<f32>,
    /// `KeyLight`: put at the front of the list (§8.5 step 4).
    pub key_light: bool,
    /// `Light_BakedLandscapeLight`: skips terrain and ceiling chunks ([`crate::TerrainDraw`];
    /// §8.3, L30).
    pub baked_landscape: bool,
    /// The object the light is attached to (its parent), as the caller's object handle
    /// ([`crate::MeshDraw::light_object`]). `None`: not attached, so no object is its
    /// parent (`lighting.md` §8.5 step 3, row L34).
    pub parent: Option<u64>,
    /// `Light_OnlyInfluenceParent`: reaches no object other than its parent. Applied to
    /// every other object whether or not the light has a parent, so an unattached light
    /// with this switch reaches no object (`lighting.md` §8.5 step 3, row L34).
    pub only_influence_parent: bool,
    /// `Light_NeverInfluenceParent`: does not reach its parent.
    pub never_influence_parent: bool,
    /// The world position (in the pass's frame) of a **directional** table light, for the
    /// light-list walk (`lighting.md` §8.5, L39: the walk tests a directional light that is
    /// among the walked lights with its own position and ρ like a point light, and orders
    /// it by step 4). Used only together with a usable `radius`; otherwise a directional
    /// light reaches every draw (`render.md` §4.3 Decision). Point lights use their table
    /// position and ignore this field.
    pub position: Option<Vec3>,
}

impl LightReach {
    /// The reach of a placed light with `fields` (`lighting.md` §8.5 steps 1–4): ρ from
    /// Range, Falloff and `Light_GlobalLight`, and the key, baked and parent switches. Not
    /// attached to an object ([`Self::with_parent`]).
    #[must_use]
    pub fn from_fields(fields: &LightFields) -> Self {
        Self {
            radius: Some(reference::selection_radius(
                fields.range,
                fields.falloff,
                fields.global_light,
            )),
            key_light: fields.key_light,
            baked_landscape: fields.baked_landscape,
            parent: None,
            only_influence_parent: fields.only_influence_parent,
            never_influence_parent: fields.never_influence_parent,
            position: None,
        }
    }

    /// The same reach, attached to the object with handle `parent`.
    #[must_use]
    pub fn with_parent(mut self, parent: u64) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// The light record built from [`LightFields`] (see [`reference::light_record`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightRecord {
    /// Diffuse colour.
    pub color: [f32; 3],
    /// Specular colour.
    pub specular: [f32; 3],
    /// Range *R*.
    pub range: f32,
    /// Attenuation factor *k*.
    pub attenuation: f32,
}

/// The sun's shadow map (`lighting.md` §9; Tessera Decision: one cascade).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunShadow {
    /// World → shadow clip space: orthographic, depth `0..1` with 0 nearest the light
    /// (standard, not reversed, depth). [`SunShadow::fit`] builds one.
    pub view_proj: Mat4,
    /// Shadow-map size in texels (clamped to `64..=4096` and the device limit).
    pub map_size: u32,
    /// Quality: 1 = one comparison tap, 2 or more = five taps (§9 step 4).
    pub quality: u32,
    /// Landscape shadow colour (§9 step 5; Tessera Decision 0).
    pub shadow_color: [f32; 3],
    /// Shadow colour range (Tessera Decision 1).
    pub color_range: [f32; 3],
    /// Fade start (view depth, §9 step 6).
    pub fade_start: f32,
    /// Fade scale (§9 step 6): the shadow is gone at view depth
    /// 0.1 + `fade_start` + 1 / `fade_scale`.
    pub fade_scale: f32,
    /// Constant subtracted from the comparison depth (shadow-acne bias), in shadow
    /// depth units.
    pub depth_bias: f32,
}

impl SunShadow {
    /// How far beyond the fitted sphere [`SunShadow::fit`] still catches casters toward
    /// the light, in world units (Tessera Decision: tall terrain, buildings and trees
    /// outside the view sphere still shadow it, also at low sun angles).
    pub const DEFAULT_CASTER_DISTANCE: f32 = 1_000.0;

    /// A shadow map covering the sphere (`center`, `radius`) as seen from a directional
    /// light shining **from** `toward_light`, with casters up to
    /// [`Self::DEFAULT_CASTER_DISTANCE`] toward the light beyond the sphere
    /// ([`Self::fit_with_casters`]).
    #[must_use]
    pub fn fit(
        toward_light: Vec3,
        center: Vec3,
        radius: f32,
        distance: f32,
        map_size: u32,
    ) -> Self {
        Self::fit_with_casters(
            toward_light,
            center,
            radius,
            Self::DEFAULT_CASTER_DISTANCE,
            distance,
            map_size,
        )
    }

    /// A shadow map covering the sphere (`center`, `radius`) as seen from a directional
    /// light shining **from** `toward_light`. Receivers are the points of the sphere;
    /// casters may also lie up to `caster_distance` beyond it toward the light (the
    /// light-space depth range starts there, at least 2 × `radius` from the centre). The
    /// light's view is snapped to whole shadow-map texels so the shadow does not shimmer as
    /// `center` moves. The shadow fades linearly to nothing over the last fifth of
    /// `distance` (view depth; Tessera Decision of §9). Five-tap quality, colour 0,
    /// range 1, and a depth bias of 0.006 × `radius` world units.
    #[must_use]
    pub fn fit_with_casters(
        toward_light: Vec3,
        center: Vec3,
        radius: f32,
        caster_distance: f32,
        distance: f32,
        map_size: u32,
    ) -> Self {
        let dir = toward_light.try_normalize().unwrap_or(Vec3::Z);
        let radius = if radius.is_finite() {
            radius.max(1e-3)
        } else {
            1.0
        };
        let caster_distance = if caster_distance.is_finite() {
            caster_distance.max(0.0)
        } else {
            Self::DEFAULT_CASTER_DISTANCE
        };
        let up = if dir.z.abs() > 0.99 { Vec3::Y } else { Vec3::Z };
        let size = map_size.clamp(64, 4096);
        // Light view looking along -dir; snap the centre to texels in light space.
        let view = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, -dir, up);
        let texel = 2.0 * radius / size as f32;
        let c = view.transform_point3(if center.is_finite() {
            center
        } else {
            Vec3::ZERO
        });
        let snapped = Vec3::new(
            (c.x / texel).floor() * texel,
            (c.y / texel).floor() * texel,
            c.z,
        );
        // Depth range: from the casters' limit toward the light to 2 r behind the centre.
        let toward = (radius + caster_distance).max(radius * 2.0);
        let away = radius * 2.0;
        let proj = glam::camera::rh::proj::directx::orthographic(
            snapped.x - radius,
            snapped.x + radius,
            snapped.y - radius,
            snapped.y + radius,
            -snapped.z - toward,
            -snapped.z + away,
        );
        let distance = if distance.is_finite() && distance > 0.0 {
            distance
        } else {
            radius
        };
        Self {
            view_proj: proj * view,
            map_size: size,
            quality: 2,
            shadow_color: [0.0; 3],
            color_range: [1.0; 3],
            fade_start: distance * 0.8 - 0.1,
            fade_scale: 1.0 / (distance * 0.2),
            // 0.0015 of a 4 r depth range: the same world-space bias whatever the caster
            // distance.
            depth_bias: 0.006 * radius / (toward + away),
        }
    }
}

/// Lighting of one world pass (`lighting.md` §4.2, §8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct FrameLighting {
    /// World up *U* (`coordinates.md`: +Z). Normalised by the renderer.
    pub up: Vec3,
    /// Frame ambient ½ (*X* + *Y*) (§8.1; [`reference::frame_ambient`]).
    pub ambient: [f32; 3],
    /// Frame hemispheric ½ (*X* − *Y*).
    pub hemispheric: [f32; 3],
    /// The main directional light (sun or moon), if any.
    pub sun: Option<SunLight>,
    /// Distance fog, or `None` for no fog. The renderer applies [`Fog::lower_limits`]; the
    /// cap of a keyframe's end ([`Fog::keyframe_capped`]) is the caller's, before any
    /// interior override (`lighting.md` §10).
    pub fog: Option<Fog>,
    /// The sun's shadow, or `None` for no shadow. Ignored without a sun.
    pub shadow: Option<SunShadow>,
    /// The pass's light table; draws pick at most [`MAX_DRAW_LIGHTS`] of it
    /// ([`crate::LightSelection`]). Entries past [`MAX_FRAME_LIGHTS`] are ignored.
    pub lights: Vec<Light>,
    /// The time input in seconds (§4.2 E20), for animated effects: the global clock. The
    /// renderer reduces it modulo 3,600 once it exceeds 3,600, as E20 does, so callers may
    /// pass the raw clock.
    pub time: f32,
}

impl Default for FrameLighting {
    /// The client's lighting without a day description (`lighting.md` §8.1): ambient
    /// brightness 0.7 with white ambient colours, a white sun at brightness 0.1 travelling
    /// along (1, 0, −0.4); no fog, no shadow, no lights. By the Tessera Decision of §8.1
    /// (*E₀* = *Cd* × *bd*, specular = *Cs*) the sun colour is 0.1 and its specular colour
    /// white (*Cs* = white).
    fn default() -> Self {
        let (ambient, hemispheric) = reference::frame_ambient(0.7, [1.0; 3], [1.0; 3]);
        Self {
            up: Vec3::Z,
            ambient,
            hemispheric,
            sun: Some(SunLight {
                direction: -Vec3::new(1.0, 0.0, -0.4),
                color: [0.1; 3],
                specular: [1.0; 3],
            }),
            fog: None,
            shadow: None,
            lights: Vec::new(),
            time: 0.0,
        }
    }
}

/// One light-table entry as the shaders read it (must match `FrameLight` in
/// `frame.wgsl`). Crate-internal: the layout is documented in `render.md` §4.2 for WGSL
/// authors, not part of the Rust API.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct GpuLight {
    /// Point: xyz = world position, w = *k*. Directional: xyz = towards the light, w = 0.
    pub position_k: [f32; 4],
    /// rgb = colour, w = *R²*.
    pub color_r2: [f32; 4],
    /// rgb = specular colour, w = 0 point / 1 directional.
    pub specular_kind: [f32; 4],
}

/// Flag of [`FrameUniform::params`]`[1]`: the pass renders into an sRGB target.
pub(crate) const FRAME_SRGB_TARGET: u32 = 1;

/// The time input wraps at this many seconds (`lighting.md` §4.2 E20).
const TIME_WRAP: f32 = 3_600.0;

/// The per-pass uniform shared by all lit pipelines (must match `Frame` in `frame.wgsl`;
/// layout table in `docs/specs/render.md` §4.2). 12,624 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct FrameUniform {
    /// World → clip (reversed depth).
    pub view_proj: [[f32; 4]; 4],
    /// xyz = eye, w = time.
    pub eye: [f32; 4],
    /// xyz = up.
    pub up: [f32; 4],
    /// xyz = towards the sun, w = 1 when present.
    pub sun_dir: [f32; 4],
    /// rgb = *E₀*.
    pub sun_color: [f32; 4],
    /// rgb = sun specular.
    pub sun_specular: [f32; 4],
    /// rgb = frame ambient.
    pub ambient: [f32; 4],
    /// rgb = frame hemispheric.
    pub hemispheric: [f32; 4],
    /// rgb = fog colour, w = 1 when fog is on.
    pub fog_color: [f32; 4],
    /// near, 1 / (far − near), far, far − near.
    pub fog: [f32; 4],
    /// World → shadow clip.
    pub shadow_view_proj: [[f32; 4]; 4],
    /// quality, fade start, fade scale, texel size.
    pub shadow: [f32; 4],
    /// rgb = landscape shadow colour, w = depth bias.
    pub shadow_color: [f32; 4],
    /// rgb = shadow colour range.
    pub shadow_range: [f32; 4],
    /// light count, flags, reserved, reserved.
    pub params: [u32; 4],
    /// The light table.
    pub lights: [GpuLight; MAX_FRAME_LIGHTS],
}

fn finite3(c: [f32; 3], fallback: f32) -> [f32; 3] {
    c.map(|v| if v.is_finite() { v } else { fallback })
}

fn rgb4(c: [f32; 3], w: f32) -> [f32; 4] {
    let [r, g, b] = finite3(c, 0.0);
    [r, g, b, w]
}

fn unit(v: Vec3) -> Option<Vec3> {
    let m = v.abs().max_element();
    if v.is_finite() && m.is_normal() {
        (v / m).try_normalize()
    } else {
        None
    }
}

impl FrameUniform {
    /// Pack `lighting` for a pass with `view_proj` and camera position `eye`.
    /// `srgb_target` says whether the pass's colour target encodes sRGB on write.
    /// Non-finite inputs are replaced by neutral values; lights past
    /// [`MAX_FRAME_LIGHTS`] are dropped.
    #[must_use]
    pub fn new(lighting: &FrameLighting, view_proj: Mat4, eye: Vec3, srgb_target: bool) -> Self {
        let mut u = Self::zeroed();
        u.view_proj = view_proj.to_cols_array_2d();
        // E20: the global clock, reduced modulo 3,600 once it exceeds 3,600.
        let time = match lighting.time {
            t if !t.is_finite() => 0.0,
            t if t > TIME_WRAP => t.rem_euclid(TIME_WRAP),
            t => t,
        };
        u.eye = eye.extend(time).to_array();
        u.up = unit(lighting.up).unwrap_or(Vec3::Z).extend(0.0).to_array();
        let sun = lighting.sun.and_then(|s| unit(s.direction).map(|d| (s, d)));
        if let Some((s, d)) = sun {
            u.sun_dir = d.extend(1.0).to_array();
            u.sun_color = rgb4(s.color, 0.0);
            u.sun_specular = rgb4(s.specular, 0.0);
        }
        u.ambient = rgb4(lighting.ambient, 0.0);
        u.hemispheric = rgb4(lighting.hemispheric, 0.0);
        if let Some(f) = lighting.fog {
            // The keyframe cap is the caller's (it precedes an interior override, §10).
            let f = f.lower_limits();
            let range = f.far - f.near;
            u.fog_color = rgb4(f.color, 1.0);
            u.fog = [f.near, 1.0 / range, f.far, range];
        }
        u.shadow_view_proj = Mat4::IDENTITY.to_cols_array_2d();
        if let (Some(sh), Some(_)) = (lighting.shadow, sun)
            && sh.view_proj.is_finite()
        {
            let size = sh.map_size.clamp(64, 4096);
            let f = |v: f32, d: f32| if v.is_finite() { v } else { d };
            u.shadow_view_proj = sh.view_proj.to_cols_array_2d();
            u.shadow = [
                sh.quality.min(2) as f32,
                f(sh.fade_start, 0.0),
                f(sh.fade_scale, 0.0).max(0.0),
                1.0 / size as f32,
            ];
            u.shadow_color = rgb4(sh.shadow_color, f(sh.depth_bias, 0.0));
            u.shadow_range = rgb4(sh.color_range, 0.0);
        }
        let mut count = 0u32;
        for (slot, light) in u.lights.iter_mut().zip(&lighting.lights) {
            *slot = gpu_light(light);
            count += 1;
        }
        u.params = [count, if srgb_target { FRAME_SRGB_TARGET } else { 0 }, 0, 0];
        u
    }

    /// Whether the uniform asks for a shadow (quality > 0).
    #[must_use]
    pub fn has_shadow(&self) -> bool {
        self.shadow[0] > 0.0
    }
}

fn gpu_light(light: &Light) -> GpuLight {
    match light.kind {
        LightKind::Point {
            position,
            range,
            attenuation,
        } => {
            let (position, r2, k) = if position.is_finite() && range.is_finite() {
                let r = range.max(MIN_LIGHT_RANGE);
                let k = if attenuation.is_finite() {
                    attenuation.max(0.0)
                } else {
                    0.0
                };
                (position, r * r, k)
            } else {
                // Unusable: a light of zero range contributes nothing.
                (Vec3::ZERO, 0.0, 0.0)
            };
            GpuLight {
                position_k: position.extend(k).to_array(),
                color_r2: rgb4(light.color, r2),
                specular_kind: rgb4(light.specular, 0.0),
            }
        }
        LightKind::Directional { direction } => match unit(direction) {
            Some(d) => GpuLight {
                position_k: d.extend(0.0).to_array(),
                color_r2: rgb4(light.color, 0.0),
                specular_kind: rgb4(light.specular, 1.0),
            },
            None => GpuLight::zeroed(),
        },
    }
}

/// Bind group layout entries of a lit pass's group 0: the frame uniform (dynamic offset),
/// the shadow map and its comparison sampler.
pub(crate) fn frame_layout_entries() -> [wgpu::BindGroupLayoutEntry; 3] {
    [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(size_of::<FrameUniform>() as u64),
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        },
    ]
}

/// CPU reference of the lit shaders' equations (`lighting.md` §5, §8, §9, §10), used by the
/// tests and by callers that need the same numbers on the CPU.
pub mod reference {
    use glam::Vec3;

    use super::{LightFields, LightRecord, MIN_LIGHT_RANGE, SELECTION_FRACTION};

    /// Clamp to `[0, 1]` ("sat").
    #[must_use]
    pub fn sat(x: f32) -> f32 {
        x.clamp(0.0, 1.0)
    }

    fn sat3(v: Vec3) -> Vec3 {
        v.clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// Frame ambient and hemispheric inputs from the ambient brightness *ba* and the two
    /// ambient colours (`lighting.md` §8.1, L16): *X* = *Ca1* × *ba*, *Y* = *Ca2* × *ba*,
    /// ambient = ½ (*X* + *Y*), hemispheric = ½ (*X* − *Y*).
    #[must_use]
    pub fn frame_ambient(brightness: f32, ca1: [f32; 3], ca2: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let x = Vec3::from(ca1) * brightness;
        let y = Vec3::from(ca2) * brightness;
        (((x + y) * 0.5).to_array(), ((x - y) * 0.5).to_array())
    }

    /// Ambient *A* = sat(*A₀* + (*N* · *U*) × *H₀*) (`lighting.md` §5.3).
    #[must_use]
    pub fn ambient(n_dot_up: f32, a0: [f32; 3], h0: [f32; 3]) -> [f32; 3] {
        sat3(Vec3::from(a0) + n_dot_up * Vec3::from(h0)).to_array()
    }

    /// Point-light attenuation at squared distance `d2` (`lighting.md` §8.3): 0 at or
    /// beyond `r2`, else 1 / (1 + *k* · *d²*), multiplied by the smooth fade over the last
    /// quarter of `r2` when `fade` (newer back end, light 0 and per-pixel lists).
    #[must_use]
    pub fn attenuation(d2: f32, r2: f32, k: f32, fade: bool) -> f32 {
        if d2 >= r2 {
            return 0.0;
        }
        let mut a = 1.0 / (1.0 + k * d2);
        if fade && d2 > 0.75 * r2 {
            let s = (r2 - d2) / (0.25 * r2);
            a *= s * s * (3.0 - 2.0 * s);
        }
        a
    }

    /// A non-finite number takes its field default (`lighting.md` §8.3: the values a new
    /// light starts with).
    fn field(v: f32, default: f32) -> f32 {
        if v.is_finite() { v } else { default }
    }

    /// A colour with a non-finite channel takes its field default as a whole.
    fn field_rgb(c: [f32; 3], default: [f32; 3]) -> Vec3 {
        Vec3::from(if c.iter().all(|v| v.is_finite()) {
            c
        } else {
            default
        })
    }

    /// The Range and Falloff a light record and its selection radius are built from:
    /// non-finite values take the field defaults (25, 1.0) and a negative Falloff counts
    /// as 0 (Tessera's choice for hostile input, `render.md` §4.3 implementer OPEN; the
    /// install's Falloffs lie in [0, 1]). Both functions use this one rule, so a light
    /// whose record contributes nothing is never selected and a selected light is the one
    /// the record draws.
    fn range_falloff(range: f32, falloff: f32) -> (f32, f32) {
        let d = LightFields::default();
        (field(range, d.range), field(falloff, d.falloff).max(0.0))
    }

    /// The light record of a placed light (`lighting.md` §8.3, L19): colour = Diffuse ×
    /// clamp(intensity × *σ*, 0, 1); specular = Specular × clamp(2 × *σ* × intensity, 0, 1),
    /// with Diffuse in place of Specular when KeyLight is off and Specular is white
    /// (within 0.0002 per channel); *R* = max(Range, 0.0002); *k* = 10 × Falloff / *R²*.
    /// Non-finite fields take the defaults of a new light (Range 25, Falloff 1.0,
    /// intensity 0.5, *σ* 1, Diffuse and Specular white); a negative Falloff counts as 0
    /// (as in [`selection_radius`]).
    #[must_use]
    pub fn light_record(f: &LightFields) -> LightRecord {
        let d = LightFields::default();
        let intensity = field(f.intensity, d.intensity);
        let scale = field(f.scale, d.scale);
        let diffuse = field_rgb(f.diffuse, d.diffuse);
        let specular = field_rgb(f.specular, d.specular);
        let white = (specular - Vec3::ONE).abs().max_element() <= 0.0002;
        let spec_base = if !f.key_light && white {
            diffuse
        } else {
            specular
        };
        let (range, falloff) = range_falloff(f.range, f.falloff);
        let range = range.max(MIN_LIGHT_RANGE);
        LightRecord {
            color: (diffuse * sat(intensity * scale)).to_array(),
            specular: (spec_base * sat(2.0 * scale * intensity)).to_array(),
            range,
            attenuation: 10.0 * falloff / (range * range),
        }
    }

    /// Selection radius ρ of a placed light (`lighting.md` §8.5 step 1, L31):
    /// Range × (1 − (1 − 0.85) × min(Falloff, 1)), or Range for a `Light_GlobalLight`
    /// light. Range and Falloff are sanitised as in [`light_record`] (non-finite: the
    /// defaults 25 and 1.0; a negative Falloff counts as 0); the result is at least 0.
    #[must_use]
    pub fn selection_radius(range: f32, falloff: f32, global: bool) -> f32 {
        let (range, falloff) = range_falloff(range, falloff);
        let rho = if global {
            range
        } else {
            range * (1.0 - (1.0 - SELECTION_FRACTION) * falloff.min(1.0))
        };
        if rho.is_finite() { rho.max(0.0) } else { 0.0 }
    }

    /// Squared distance from `p` to the axis-aligned box (`min`, `max`); 0 inside it
    /// (`lighting.md` §8.5 step 2).
    #[must_use]
    pub fn box_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
        let d = (min - p).max(p - max).max(Vec3::ZERO);
        d.length_squared()
    }

    /// Whether a light at `p` with selection radius `rho` reaches the box (`min`, `max`):
    /// squared box distance strictly below ρ² (`lighting.md` §8.5 step 2).
    #[must_use]
    pub fn reaches_box(p: Vec3, rho: f32, min: Vec3, max: Vec3) -> bool {
        box_distance2(p, min, max) < rho * rho
    }

    /// Environment cross-fade factor after `t` seconds of a `duration`-second transition
    /// (`lighting.md` §8.1, L18): ½ + ½ sin(π (min(*t* / *T*, 1) − ½)), snapped to 0 below
    /// 0.0002 and to 1 above 0.9998.
    #[must_use]
    pub fn crossfade(t: f32, duration: f32) -> f32 {
        let x = if duration > 0.0 {
            (t / duration).min(1.0)
        } else {
            1.0
        };
        let f = 0.5 + 0.5 * (std::f32::consts::PI * (x - 0.5)).sin();
        if f < 0.0002 {
            0.0
        } else if f > 0.9998 {
            1.0
        } else {
            f
        }
    }

    /// Fog factor *f* = clamp((distance − near) / (far − near), 0, 1) (`lighting.md` §10).
    #[must_use]
    pub fn fog_factor(distance: f32, near: f32, far: f32) -> f32 {
        sat((distance - near) / (far - near))
    }

    /// Whether the alpha test keeps a texel: kept only when opacity > threshold
    /// (`lighting.md` §5.12).
    #[must_use]
    pub fn alpha_test_keeps(opacity: f32, threshold: f32) -> bool {
        opacity > threshold
    }

    /// Output alpha of a lit record (`lighting.md` §5.12, §5.13, row L41; `material.md`
    /// §11.3, MA61): the tested opacity `base_opacity` (`material.md` §11.2 table, whatever
    /// its source; `None` without an opacity-bearing role, *α* = 1) × *M*.alpha, times the
    /// vertex alpha where the record reads vertex colours (1 otherwise); the same in the base
    /// passes and the additive light passes. Clamped to 0–1 as written to the target; a NaN
    /// gives 0.
    #[must_use]
    pub fn output_alpha(base_opacity: Option<f32>, material_alpha: f32, vertex_alpha: f32) -> f32 {
        let a = base_opacity.unwrap_or(1.0) * material_alpha * vertex_alpha;
        if a.is_nan() { 0.0 } else { a.clamp(0.0, 1.0) }
    }

    /// One light evaluated at a surface point, already multiplied by the material colour
    /// where the original pre-multiplies it (`lighting.md` §4.2).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct LightSample {
        /// Unit vector from the surface towards the light.
        pub to_light: Vec3,
        /// Light colour × *M*.
        pub color: [f32; 3],
        /// Attenuation *a* (1 for directional lights) times the shadow factor for the sun.
        pub factor: [f32; 3],
    }

    /// Inputs of the lit base pass at one texel (`lighting.md` §5).
    #[derive(Debug, Clone, PartialEq)]
    pub struct Texel {
        /// Albedo *C* after dye.
        pub albedo: [f32; 3],
        /// Shading normal *N* (unit).
        pub normal: Vec3,
        /// World up *U*.
        pub up: Vec3,
        /// *A₀* = ambient × *M*.
        pub a0: [f32; 3],
        /// *H₀* = hemispheric × *M*.
        pub h0: [f32; 3],
        /// The main light and the listed lights.
        pub lights: Vec<LightSample>,
        /// Specular terms already evaluated (added after the factor 2, §5.8).
        pub specular: [f32; 3],
        /// Emissive term *Em* × *G* (§5.9).
        pub emissive: [f32; 3],
        /// Fog factor and colour (§5.11).
        pub fog: Option<(f32, [f32; 3])>,
    }

    /// The lit colour of `t` (`lighting.md` §5.3–§5.11):
    /// lerp(2 × *C* × (*A* + Σ sat(*N* · *L*) × factor × colour) + specular + emissive,
    /// fog colour, *f*).
    #[must_use]
    pub fn shade(t: &Texel) -> [f32; 3] {
        let a = Vec3::from(ambient(t.normal.dot(t.up), t.a0, t.h0));
        let sum = t.lights.iter().fold(a, |acc, l| {
            acc + sat(t.normal.dot(l.to_light)) * Vec3::from(l.factor) * Vec3::from(l.color)
        });
        let lit =
            2.0 * Vec3::from(t.albedo) * sum + Vec3::from(t.specular) + Vec3::from(t.emissive);
        match t.fog {
            Some((f, c)) => lit.lerp(Vec3::from(c), f).to_array(),
            None => lit.to_array(),
        }
    }

    /// The unit half-vector *H* = normalise(*V* + *L*) (`fallback` when *V* = −*L*).
    #[must_use]
    pub fn half_vector(to_eye: Vec3, to_light: Vec3, fallback: Vec3) -> Vec3 {
        (to_eye + to_light).normalize_or(fallback)
    }

    /// Specular term of one light (`lighting.md` §5.8): `scale` × sat(*N* · *H*)^*p* ×
    /// `color`, where `scale` is *g* × *a* for a point light and *g* for the sun (whose
    /// shadow factor goes into `color`); `clamp` clamps the scalar part to `[0, 1]`
    /// (per-pixel light lists). The power is applied as exp2(*p* × log2(x)).
    #[must_use]
    pub fn specular(
        normal: Vec3,
        half: Vec3,
        scale: f32,
        power: f32,
        color: [f32; 3],
        clamp: bool,
    ) -> [f32; 3] {
        let x = sat(normal.dot(half));
        let p = if x > 0.0 {
            (power * x.log2()).exp2()
        } else {
            0.0
        };
        let mut s = scale * p;
        if clamp {
            s = sat(s);
        }
        (s * Vec3::from(color)).to_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_layout_matches_the_wgsl() {
        assert_eq!(size_of::<GpuLight>(), 48);
        assert_eq!(size_of::<FrameUniform>(), 336 + 48 * MAX_FRAME_LIGHTS);
        assert_eq!(std::mem::offset_of!(FrameUniform, shadow_view_proj), 208);
        assert_eq!(std::mem::offset_of!(FrameUniform, params), 320);
        assert_eq!(std::mem::offset_of!(FrameUniform, lights), 336);
        // Fits the smallest uniform binding every adapter offers.
        assert!(size_of::<FrameUniform>() <= 16 << 10);
    }

    #[test]
    fn frame_wgsl_validates() {
        let src = format!("{FRAME_WGSL}\n{FRAME_SHADING_WGSL}");
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        );
        if let Err(e) = v.validate(&module) {
            panic!("{}", e.emit_to_string(&src));
        }
        // The struct size naga computes equals the Rust one.
        let frame = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Frame"))
            .map(|(_, t)| t.inner.size(module.to_ctx()));
        assert_eq!(frame, Some(size_of::<FrameUniform>() as u32));
    }

    #[test]
    fn fog_limits() {
        let f = Fog {
            near: -50_000.0,
            far: 90_000.0,
            color: [0.0; 3],
        }
        .limited();
        assert_eq!((f.near, f.far), (-10_000.0, 10_000.0));
        let f = Fog {
            near: 15.0,
            far: 5.0,
            color: [0.0; 3],
        }
        .limited();
        assert_eq!((f.near, f.far), (0.0, 20.0));
        let f = Fog {
            near: f32::NAN,
            far: f32::INFINITY,
            color: [0.0; 3],
        }
        .limited();
        assert_eq!((f.near, f.far), (-10_000.0, 10_000.0));
        // An interior override after the keyframe cap is not capped (§10, R-F1): the
        // renderer keeps only the lower limits.
        let f = Fog {
            near: -50_000.0,
            far: 90_000.0,
            color: [0.0; 3],
        };
        let k = f.keyframe_capped();
        assert_eq!((k.near, k.far), (-50_000.0, 10_000.0));
        let l = f.lower_limits();
        assert_eq!((l.near, l.far), (-10_000.0, 90_000.0));
        let l = Fog {
            near: 30.0,
            far: 10.0,
            color: [0.0; 3],
        }
        .lower_limits();
        assert_eq!((l.near, l.far), (0.0, 20.0));
        let l = Fog {
            near: f32::NAN,
            far: f32::NEG_INFINITY,
            color: [0.0; 3],
        }
        .lower_limits();
        assert_eq!((l.near, l.far), (-10_000.0, 10_000.0));
        // The uploaded fog: an override end of 15,000 stays 15,000.
        let u = FrameUniform::new(
            &FrameLighting {
                fog: Some(Fog {
                    near: 100.0,
                    far: 15_000.0,
                    color: [0.5; 3],
                }),
                ..FrameLighting::default()
            },
            Mat4::IDENTITY,
            Vec3::ZERO,
            false,
        );
        assert_eq!(u.fog, [100.0, 1.0 / 14_900.0, 15_000.0, 14_900.0]);
    }

    #[test]
    fn uniform_sanitises_hostile_values() {
        let lighting = FrameLighting {
            up: Vec3::NAN,
            sun: Some(SunLight {
                direction: Vec3::ZERO,
                color: [f32::NAN; 3],
                specular: [1.0; 3],
            }),
            shadow: Some(SunShadow::fit(Vec3::Z, Vec3::ZERO, 10.0, 50.0, 1 << 20)),
            lights: vec![
                Light::point(Vec3::NAN, 5.0, 0.1, [1.0; 3]),
                Light {
                    kind: LightKind::Directional {
                        direction: Vec3::splat(f32::MAX),
                    },
                    color: [1.0; 3],
                    specular: [1.0; 3],
                },
            ],
            ..FrameLighting::default()
        };
        let u = FrameUniform::new(&lighting, Mat4::IDENTITY, Vec3::ZERO, true);
        assert_eq!(u.up, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(u.sun_dir[3], 0.0, "a zero direction disables the sun");
        assert!(!u.has_shadow(), "no shadow without a sun");
        assert_eq!(u.params, [2, FRAME_SRGB_TARGET, 0, 0]);
        assert_eq!(
            u.lights[0].color_r2[3], 0.0,
            "unusable point light has no range"
        );
        let d = Vec3::from_slice(&u.lights[1].position_k[..3]);
        assert!((d.length() - 1.0).abs() < 1e-6);
        let many = FrameLighting {
            lights: vec![Light::point(Vec3::ZERO, 1.0, 0.0, [1.0; 3]); MAX_FRAME_LIGHTS + 9],
            ..FrameLighting::default()
        };
        let u = FrameUniform::new(&many, Mat4::IDENTITY, Vec3::ZERO, false);
        assert_eq!(u.params[0] as usize, MAX_FRAME_LIGHTS);
    }

    #[test]
    fn time_input_wraps_like_e20() {
        let at = |time: f32| {
            let l = FrameLighting {
                time,
                ..FrameLighting::default()
            };
            FrameUniform::new(&l, Mat4::IDENTITY, Vec3::ZERO, false).eye[3]
        };
        assert_eq!(at(12.5), 12.5);
        assert_eq!(at(3_600.0), 3_600.0, "reduced only once it exceeds 3,600");
        assert_eq!(at(3_612.5), 12.5);
        assert!((at(86_400.0 * 30.0 + 7.25) - 7.25).abs() < 1e-2);
        assert_eq!(at(f32::NAN), 0.0);
        assert_eq!(at(f32::INFINITY), 0.0);
        assert!((0.0..TIME_WRAP).contains(&at(f32::MAX)));
    }

    #[test]
    fn default_sun_follows_the_decision() {
        let sun = FrameLighting::default().sun.expect("default sun");
        assert_eq!(sun.color, [0.1; 3], "E0 = Cd x bd = white x 0.1");
        assert_eq!(sun.specular, [1.0; 3], "specular = Cs = white");
    }

    #[test]
    fn shadow_fit_covers_the_sphere() {
        let center = Vec3::new(100.0, -40.0, 12.0);
        let s = SunShadow::fit(Vec3::new(0.3, 0.2, 0.9), center, 30.0, 200.0, 2048);
        for p in [
            center,
            center + Vec3::X * 29.0,
            center - Vec3::Y * 29.0,
            center + Vec3::Z * 29.0,
            center - Vec3::Z * 29.0,
        ] {
            let c = s.view_proj.project_point3(p);
            assert!(c.x.abs() <= 1.0 && c.y.abs() <= 1.0, "{p} -> {c}");
            assert!((0.0..=1.0).contains(&c.z), "{p} -> {c}");
        }
        // A point nearer the light has a smaller depth.
        let near = s
            .view_proj
            .project_point3(center + Vec3::new(0.3, 0.2, 0.9) * 10.0);
        let far = s
            .view_proj
            .project_point3(center - Vec3::new(0.3, 0.2, 0.9) * 10.0);
        assert!(near.z < far.z);
        // The shadow is gone at the requested distance.
        let gone = 0.1 + s.fade_start + 1.0 / s.fade_scale;
        assert!((gone - 200.0).abs() < 1e-3);
        // Casters far toward the light are inside the depth range, up to the caster
        // distance; points behind the sphere beyond 2 r are not.
        let toward = Vec3::new(0.3, 0.2, 0.9).normalize();
        for (d, inside) in [(500.0, true), (1_020.0, true), (1_040.0, false)] {
            let z = s.view_proj.project_point3(center + toward * d).z;
            assert_eq!(
                (0.0..=1.0).contains(&z),
                inside,
                "{d} toward the light: {z}"
            );
        }
        let z = s.view_proj.project_point3(center - toward * 61.0).z;
        assert!(z > 1.0, "behind the sphere: {z}");
        // The bias is the same in world units as without extra caster range.
        let tight = SunShadow::fit_with_casters(toward, center, 30.0, 0.0, 200.0, 2048);
        let world = |s: &SunShadow, range: f32| s.depth_bias * range;
        assert!((world(&s, 30.0 + 1_000.0 + 60.0) - world(&tight, 120.0)).abs() < 1e-4);
        // Hostile caster distances fall back to sane ranges.
        for d in [f32::NAN, f32::INFINITY, -5.0] {
            let h = SunShadow::fit_with_casters(toward, center, 30.0, d, 200.0, 2048);
            assert!(h.view_proj.is_finite() && h.depth_bias.is_finite());
        }
    }
}
