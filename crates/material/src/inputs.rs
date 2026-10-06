// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-surface lighting inputs (`lighting.md` §4.1; `material.md` §8.3, §8.4, §11).
//!
//! [`SurfaceInputs::evaluate`] turns a [`ResolvedMaterial`] and its shader record's
//! [`TextureMatrixLayout`] into the engine-neutral values the programs receive for one
//! drawn surface at one time: material colour *M* after the per-object tint, specular
//! colour *Ms* and power *p* (black below power 0.5), glow and dye colours, cull mode,
//! depth test and write, the alpha-test threshold *τ*, the texture matrices, every
//! parameter bound to a program constant by name (flag byte 1, `material.md` §8.6) and
//! the program time (`lighting.md` §4.2 E20). A renderer maps these to GPU state; nothing
//! here depends on one.
//!
//! Float parameters are curves evaluated at the clock of `material.md` §5.3.1 with the
//! drawn instance's [`CurveStates`]; every float handed out is finite. A surface without
//! animated parameters ([`SurfaceInputs::is_static`]) can be evaluated once.

use tessera_formats::Did;

use crate::records::{CullMode, CurveStates, ParamValue, curve_finite, program_time};
use crate::resolve::ResolvedMaterial;
use crate::transform::{EvaluatedMatrix, TextureMatrixLayout};
use crate::vocab::ids;

/// Below this effective specular power the client uploads a black specular colour, so
/// the surface has no highlight (`material.md` §8.3, `lighting.md` §5.8).
pub const SPECULAR_POWER_CUTOFF: f32 = 0.5;

/// What the drawn object contributes (`material.md` §8.3, `lighting.md` §4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectInputs {
    /// Per-object tint multiplied into the material colour 2000 and the specular colour
    /// 2001 (RGBA). White for ordinary objects (Tessera Decision, `lighting.md` §4.2).
    pub tint: [f32; 4],
    /// Global specular-power scale (an engine setting, Inferred to be 1, §8.3).
    pub specular_power_scale: f32,
}

impl Default for ObjectInputs {
    fn default() -> Self {
        Self {
            tint: [1.0; 4],
            specular_power_scale: 1.0,
        }
    }
}

/// Specular inputs (`lighting.md` §4.1 semantics 1 and 2, §5.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecularInputs {
    /// *Ms*: the effective 2001 RGB × the per-object tint RGB, or black when
    /// [`SpecularInputs::power`] is below [`SPECULAR_POWER_CUTOFF`]. White when the shader
    /// declares 1000 but not 2001 (never observed: they are always declared together).
    pub color: [f32; 3],
    /// *p*: the effective 1000 at the clock × the global specular-power scale.
    pub power: f32,
}

/// The value of a parameter bound to a program constant by name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NamedValue {
    /// A float curve, evaluated.
    Float(f32),
    /// An RGBA colour.
    Color([f32; 4]),
    /// A texture list (`None` for 0).
    Texture(Option<Did>),
    /// A flag word, non-zero = true.
    Flag(bool),
}

/// A parameter whose flag byte is 1 (`material.md` §8.6): it feeds the program constant
/// whose name equals its label in enum mapper 0x230001A6, with no binding entries. The
/// labels are data; this crate keys on the id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamedInput {
    /// Parameter id.
    pub id: u32,
    /// Its effective value.
    pub value: NamedValue,
}

/// The per-surface inputs of `lighting.md` §4.1 at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceInputs {
    /// *M* (semantic 0): the effective 2000 (white when undeclared) × the per-object
    /// tint, RGBA. Its alpha is the output-alpha factor (`lighting.md` §5.12).
    pub material_color: [f32; 4],
    /// *Ms* and *p* (semantics 1, 2), when the shader declares 1000 (§11.5 `specular`).
    pub specular: Option<SpecularInputs>,
    /// *G* (semantic 3): the effective 2014, when declared.
    pub glow_color: Option<[f32; 4]>,
    /// *D* (semantic 4): the effective 2003, when declared.
    pub dye_color: Option<[f32; 4]>,
    /// *D2*: the effective 2013, when declared.
    pub dye_color_2: Option<[f32; 4]>,
    /// Cull mode (semantic 5, `material.md` §11.4): [`CullMode::None`] for a two-sided
    /// surface, else [`CullMode::Back`]. Front faces are counter-clockwise.
    pub cull: CullMode,
    /// Depth test (semantic 6): off only when the effective 4001 is 0.
    pub depth_test: bool,
    /// Depth write (semantic 7): the blend state's choice, off when the effective 4002
    /// is 0 (`material.md` §8.5, §11.3).
    pub depth_write: bool,
    /// Alpha test (semantics 8, 9): the threshold *τ* when the test is on
    /// (`material.md` §11.2); a fragment is kept only when its opacity is above it. An
    /// animated 1043 is evaluated at the clock; a constant one is the feature flags'
    /// threshold (with its fallbacks).
    pub alpha_test: Option<f32>,
    /// The bound texture matrices (semantics 38–44, `material.md` §8.4) of the record-wide
    /// table ([`TextureMatrixLayout::matrices`], the colour passes in observed data), by
    /// ascending index. A matrix index not listed is the identity. A pass whose
    /// permutation binds other parameters (a glow pass in a few records) takes its own
    /// from [`TextureMatrixLayout::evaluate_for`] or
    /// [`crate::MaterialLibrary::permutation_texture_matrices`].
    pub texture_matrices: Vec<EvaluatedMatrix>,
    /// Parameters bound by name (flag byte 1), in declaration order.
    pub named: Vec<NamedInput>,
    /// The program time (E20, [`program_time`]).
    pub time: f32,
}

fn finite4(c: [f32; 4]) -> [f32; 4] {
    c.map(curve_finite)
}

fn mul4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    finite4([a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]])
}

impl SurfaceInputs {
    /// Whether the inputs of `material` never change over time: no effective float
    /// curve is animated (so one evaluation serves every frame, apart from
    /// [`SurfaceInputs::time`]).
    #[must_use]
    pub fn is_static(material: &ResolvedMaterial) -> bool {
        !material.params.any_animated()
    }

    /// The inputs of `material` at time `t` seconds (the clock of `material.md` §5.3.1),
    /// with `layout` the texture-matrix bindings of its shader record
    /// ([`TextureMatrixLayout::from_record`] or
    /// [`crate::MaterialLibrary::texture_matrices`]), the drawn object's contribution and
    /// the instance's curve states. Every float in the result is finite.
    pub fn evaluate(
        material: &ResolvedMaterial,
        layout: &TextureMatrixLayout,
        object: &ObjectInputs,
        t: f64,
        states: &mut CurveStates,
    ) -> Self {
        let params = &material.params;
        let tint = finite4(object.tint);
        let material_color = mul4(material.tint(), tint);

        let specular = params.float_at(ids::SPECULAR_POWER, t, states).map(|p| {
            let power = curve_finite(p * curve_finite(object.specular_power_scale));
            let color = if power < SPECULAR_POWER_CUTOFF {
                [0.0; 3]
            } else {
                let c = mul4(params.color(ids::SPECULAR_COLOR).unwrap_or([1.0; 4]), tint);
                [c[0], c[1], c[2]]
            };
            SpecularInputs { color, power }
        });

        let alpha_test =
            material
                .features
                .alpha_test
                .map(|at| match params.curve(ids::ALPHA_THRESHOLD) {
                    // One fallback rule for §11.2: a non-finite value of an animated
                    // 1043 falls back like a non-finite constant one (the feature
                    // flags' threshold), not to 0.
                    Some(c) if c.is_animated() => {
                        let v = c.evaluate_at(t, states.state(ids::ALPHA_THRESHOLD));
                        if v.is_finite() {
                            v
                        } else {
                            curve_finite(at.threshold)
                        }
                    }
                    _ => curve_finite(at.threshold),
                });

        let named = material
            .shader
            .declarations
            .iter()
            .filter(|d| d.flag == 1)
            .filter_map(|d| {
                let value = match &params.get(d.id)?.value {
                    ParamValue::Float(c) => NamedValue::Float(states.evaluate(d.id, c, t)),
                    ParamValue::Color(c) => NamedValue::Color(finite4(*c)),
                    v @ ParamValue::Texture(_) => NamedValue::Texture(v.as_texture()),
                    ParamValue::Flag(w) => NamedValue::Flag(*w != 0),
                };
                Some(NamedInput { id: d.id, value })
            })
            .collect();

        Self {
            material_color,
            specular,
            glow_color: params.color(ids::GLOW_COLOR).map(finite4),
            dye_color: params.color(ids::DYE_COLOR).map(finite4),
            dye_color_2: params.color(ids::DYE_COLOR_2).map(finite4),
            cull: if material.features.two_sided {
                CullMode::None
            } else {
                CullMode::Back
            },
            depth_test: material.depth_test(),
            depth_write: material.features.blend.depth_write,
            alpha_test,
            texture_matrices: layout.evaluate(params, t, states),
            named,
            time: program_time(t),
        }
    }

    /// The evaluated texture matrix `index`, if the shader record binds it.
    #[must_use]
    pub fn texture_matrix(&self, index: u32) -> Option<&EvaluatedMatrix> {
        self.texture_matrices.iter().find(|m| m.matrix == index)
    }

    /// The named input `id`, if the shader declares it with flag byte 1.
    #[must_use]
    pub fn named(&self, id: u32) -> Option<NamedValue> {
        self.named.iter().find(|n| n.id == id).map(|n| n.value)
    }

    /// Whether every float of the inputs is finite (always true; for tests and
    /// assertions).
    #[must_use]
    pub fn all_finite(&self) -> bool {
        let mut floats: Vec<f32> = Vec::new();
        floats.extend(self.material_color);
        if let Some(s) = self.specular {
            floats.extend(s.color);
            floats.push(s.power);
        }
        for c in [self.glow_color, self.dye_color, self.dye_color_2]
            .into_iter()
            .flatten()
        {
            floats.extend(c);
        }
        floats.extend(self.alpha_test);
        for m in &self.texture_matrices {
            floats.extend(m.map.rows.iter().flatten());
            floats.extend(m.transform.early);
            floats.extend(m.transform.scale);
            floats.extend(m.transform.translate);
            floats.push(m.transform.rotation_degrees);
        }
        for n in &self.named {
            match n.value {
                NamedValue::Float(f) => floats.push(f),
                NamedValue::Color(c) => floats.extend(c),
                NamedValue::Texture(_) | NamedValue::Flag(_) => {}
            }
        }
        floats.push(self.time);
        floats.iter().all(|f| f.is_finite())
    }
}
