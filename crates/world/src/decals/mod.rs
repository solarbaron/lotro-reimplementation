// SPDX-License-Identifier: GPL-3.0-or-later
//! Placed decal volumes (`decals.md`): the 20 `DecalVolume_*` parameters of a Decal SED,
//! the projection volume and the decal mesh, the receiving rules, the per-decal colour,
//! light and opacity, the depth-bias table and the draw distance.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`DecalPids`], [`DecalParams`], [`DecalParams::collect`], [`marker_chain`] | The parameters, their initial values and where their values come from | §2.3, §3.1, §3.2 |
//! | [`builder`] ([`DecalBox`], [`build_mesh`], [`DecalMesh`]) | The box, receiving triangles, back-face test, clipping, texture coordinates, lift, mesh centre and mean normal | §4 |
//! | [`entity_accepts_decals`], [`cell_accepts_decals`] | Receiving surfaces | §5 |
//! | [`shading`] ([`shading::decal_colour`], [`shading::decal_light`], [`shading::DepthBiasTable`], [`shading::draw_range`]) | Colour, light colour, opacity, depth bias, draw distance | §6, §7 |
//!
//! Property names are looked up in the install's dictionary at run time. The numbers
//! written here are the client's initial parameter values and the algorithm constants of
//! §4–§7 (BIN, decision 0012), each documented where it is defined.
//!
//! # Hostile input
//!
//! Values of the wrong type are not assigned (the client's typed setters); non-finite
//! floats count as absent. The builder clips every triangle to the box, so its output is
//! bounded by seven triangles per input triangle; non-finite inputs are dropped.

pub mod builder;
pub mod shading;

use tessera_formats::gameplay::common::{Pid, PropertyAccess, ValueExt};
use tessera_formats::{Did, PropertySet, PropertyValue};

pub use builder::{
    BuildReport, DecalBox, DecalMesh, DecalVertex, LIFT, ReceiverKind, ReceiverTriangle,
    ReceiverVertex, TRIANGLE_WARNING, build_mesh,
};

tessera_formats::property_ids! {
    /// The 20 decal parameters (§3.1), plus the receiving properties of §5.
    pub struct DecalPids {
        material = "DecalVolume_Material",
        lod_class = "DecalVolume_LODClass",
        colour = "DecalVolume_ColorAndOpacity",
        lighting_blend = "DecalVolume_LightingBlend",
        use_backface_culling = "DecalVolume_UseBackfaceCulling",
        use_precise_backface_culling = "DecalVolume_UsePreciseBackfaceCulling",
        use_landscape_backface_culling = "DecalVolume_UseLandscapeBackfaceCulling",
        backface_culling_angle = "DecalVolume_BackfaceCullingAngle",
        use_height_based_opacity = "DecalVolume_UseHeightBasedOpacity",
        height_based_opacity_range = "DecalVolume_HeightBasedOpacityRange",
        use_depth_based_opacity = "DecalVolume_UseDepthBasedOpacity",
        depth_based_opacity_min = "DecalVolume_DepthBasedOpacityMin",
        depth_based_opacity_range = "DecalVolume_DepthBasedOpacityRange",
        stick_to_world_entities = "DecalVolume_StickToWorldEntities",
        stick_to_cells = "DecalVolume_StickToCells",
        stick_to_landscape = "DecalVolume_StickToLandscape",
        stick_to_water = "DecalVolume_StickToWater",
        render_phase = "DecalVolume_RenderPhase",
        ortho_scale = "DecalVolume_OrthoScale",
        depth_bias = "DecalVolume_DepthBias",
        allow_decals = "Render_AllowDecals",
        force_decals = "Render_ForceDecals",
    }
}

impl DecalPids {
    /// The 20 parameter labels (without the two receiving properties).
    #[must_use]
    pub fn parameters(&self) -> [Pid; 20] {
        [
            self.material,
            self.lod_class,
            self.colour,
            self.lighting_blend,
            self.use_backface_culling,
            self.use_precise_backface_culling,
            self.use_landscape_backface_culling,
            self.backface_culling_angle,
            self.use_height_based_opacity,
            self.height_based_opacity_range,
            self.use_depth_based_opacity,
            self.depth_based_opacity_min,
            self.depth_based_opacity_range,
            self.stick_to_world_entities,
            self.stick_to_cells,
            self.stick_to_landscape,
            self.stick_to_water,
            self.render_phase,
            self.ortho_scale,
            self.depth_bias,
        ]
    }

    /// Whether `pid` is one of the 20 parameters.
    #[must_use]
    pub fn is_parameter(&self, pid: u32) -> bool {
        self.parameters().iter().any(|p| p.is(pid))
    }
}

/// UNIQUEDB key of the default decal material (label `Render_DefaultDecalMaterial`, §3.1).
pub const DEFAULT_MATERIAL_UNIQUEDB_KEY: u32 = 0x29;
/// The LOD distance before the class is looked up, metres (§3.1, §7 step 1).
pub const INITIAL_LOD_DISTANCE: f32 = 30.0;
/// `LightingBlend` 0: unlit (§6.2).
pub const BLEND_NONE: u32 = 0;

/// The parameters of one decal object (§3.1). Initial values are the client's (BIN, DC6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecalParams {
    /// `DecalVolume_Material`: a surface record (0x31).
    pub material: Did,
    /// `DecalVolume_LODClass` (initial 1 `DefaultDecal`).
    pub lod_class: u32,
    /// `DecalVolume_ColorAndOpacity`, stored bytes B, G, R, A (initial white, opaque).
    pub colour: [u8; 4],
    /// `DecalVolume_LightingBlend` (initial 0 `None`).
    pub lighting_blend: u32,
    /// `DecalVolume_UseBackfaceCulling` (initial true).
    pub use_backface_culling: bool,
    /// `DecalVolume_UsePreciseBackfaceCulling` (initial false).
    pub use_precise_backface_culling: bool,
    /// `DecalVolume_UseLandscapeBackfaceCulling` (initial true).
    pub use_landscape_backface_culling: bool,
    /// `DecalVolume_BackfaceCullingAngle`, degrees (initial 85).
    pub backface_culling_angle: f32,
    /// `DecalVolume_UseHeightBasedOpacity` (initial false).
    pub use_height_based_opacity: bool,
    /// `DecalVolume_HeightBasedOpacityRange`, metres (initial 0).
    pub height_based_opacity_range: f32,
    /// `DecalVolume_UseDepthBasedOpacity` (initial false).
    pub use_depth_based_opacity: bool,
    /// `DecalVolume_DepthBasedOpacityMin`, metres (initial 1).
    pub depth_based_opacity_min: f32,
    /// `DecalVolume_DepthBasedOpacityRange`, metres (initial 1).
    pub depth_based_opacity_range: f32,
    /// `DecalVolume_StickToWorldEntities` (initial true).
    pub stick_to_world_entities: bool,
    /// `DecalVolume_StickToCells` (initial true).
    pub stick_to_cells: bool,
    /// `DecalVolume_StickToLandscape` (initial true).
    pub stick_to_landscape: bool,
    /// `DecalVolume_StickToWater` (initial false).
    pub stick_to_water: bool,
    /// `DecalVolume_RenderPhase` (initial 1 `Default`).
    pub render_phase: u32,
    /// `DecalVolume_OrthoScale`, metres along X′, Y′, Z′ (initial (1, 1, 1)).
    pub ortho_scale: [f32; 3],
    /// `DecalVolume_DepthBias`: the depth-bias entry (initial 1 `SlopeScale_NegativeOne`).
    pub depth_bias: u32,
}

impl DecalParams {
    /// The initial values (§3.1); `default_material` is the DID of the `UNIQUEDB` map at key
    /// 0x29 ([`DEFAULT_MATERIAL_UNIQUEDB_KEY`]).
    #[must_use]
    pub fn initial(default_material: Did) -> Self {
        Self {
            material: default_material,
            lod_class: 1,
            colour: [255; 4],
            lighting_blend: BLEND_NONE,
            use_backface_culling: true,
            use_precise_backface_culling: false,
            use_landscape_backface_culling: true,
            backface_culling_angle: 85.0,
            use_height_based_opacity: false,
            height_based_opacity_range: 0.0,
            use_depth_based_opacity: false,
            depth_based_opacity_min: 1.0,
            depth_based_opacity_range: 1.0,
            stick_to_world_entities: true,
            stick_to_cells: true,
            stick_to_landscape: true,
            stick_to_water: false,
            render_phase: 1,
            ortho_scale: [1.0; 3],
            depth_bias: 1,
        }
    }

    /// Assigns every parameter `set` holds with the right type (§2.3 step 3, §3.2); other
    /// properties and mistyped values are ignored. Returns the number assigned.
    pub fn assign<A: PropertyAccess + ?Sized>(&mut self, p: &DecalPids, set: &A) -> usize {
        let mut n = 0;
        let mut b = |pid: Pid, field: &mut bool| {
            if let Some(v) = set.get_pid(pid).and_then(ValueExt::boolean) {
                *field = v;
                n += 1;
            }
        };
        b(p.use_backface_culling, &mut self.use_backface_culling);
        b(
            p.use_precise_backface_culling,
            &mut self.use_precise_backface_culling,
        );
        b(
            p.use_landscape_backface_culling,
            &mut self.use_landscape_backface_culling,
        );
        b(
            p.use_height_based_opacity,
            &mut self.use_height_based_opacity,
        );
        b(p.use_depth_based_opacity, &mut self.use_depth_based_opacity);
        b(p.stick_to_world_entities, &mut self.stick_to_world_entities);
        b(p.stick_to_cells, &mut self.stick_to_cells);
        b(p.stick_to_landscape, &mut self.stick_to_landscape);
        b(p.stick_to_water, &mut self.stick_to_water);
        let mut f = |pid: Pid, field: &mut f32| {
            if let Some(v) = set
                .get_pid(pid)
                .and_then(ValueExt::float)
                .filter(|v| v.is_finite())
            {
                *field = v;
                n += 1;
            }
        };
        f(p.backface_culling_angle, &mut self.backface_culling_angle);
        f(
            p.height_based_opacity_range,
            &mut self.height_based_opacity_range,
        );
        f(p.depth_based_opacity_min, &mut self.depth_based_opacity_min);
        f(
            p.depth_based_opacity_range,
            &mut self.depth_based_opacity_range,
        );
        let mut e = |pid: Pid, field: &mut u32| {
            if let Some(v) = set.get_pid(pid).and_then(ValueExt::enum_key) {
                *field = v;
                n += 1;
            }
        };
        e(p.lod_class, &mut self.lod_class);
        e(p.lighting_blend, &mut self.lighting_blend);
        e(p.render_phase, &mut self.render_phase);
        e(p.depth_bias, &mut self.depth_bias);
        if let Some(d) = set.get_pid(p.material).and_then(ValueExt::did_ref) {
            self.material = d;
            n += 1;
        }
        if let Some(PropertyValue::Colour(c)) = set.get_pid(p.colour) {
            self.colour = *c;
            n += 1;
        }
        if let Some(v) = set
            .get_pid(p.ortho_scale)
            .and_then(ValueExt::vector3)
            .filter(|v| v.iter().all(|c| c.is_finite()))
        {
            self.ortho_scale = v;
            n += 1;
        }
        n
    }

    /// The parameters of a Decal SED (§3.2): the initial values, then the marker object's
    /// chain **root parent first** (`chain_root_first`), then the SED's own property set.
    /// The per-instance property set is not read on this path.
    #[must_use]
    pub fn collect<A: PropertyAccess + ?Sized>(
        p: &DecalPids,
        default_material: Did,
        chain_root_first: &[&A],
        sed: Option<&A>,
    ) -> Self {
        let mut out = Self::initial(default_material);
        for level in chain_root_first {
            out.assign(p, *level);
        }
        if let Some(s) = sed {
            out.assign(p, s);
        }
        out
    }

    /// Whether the decal is lit (`LightingBlend` ≠ 0).
    #[must_use]
    pub fn is_lit(&self) -> bool {
        self.lighting_blend != BLEND_NONE
    }
}

/// Longest 0x47 chain followed for a marker object.
pub const MAX_MARKER_CHAIN: usize = 16;

/// The property sets of a marker object's 0x47 chain, **root parent first** (§3.2 step 1):
/// `fetch` returns a 0x47 descriptor's own property set and its parent 0x47 (if the parent
/// is a 0x47). The walk stops at a missing record, a cycle or [`MAX_MARKER_CHAIN`] levels.
pub fn marker_chain(
    object: Did,
    mut fetch: impl FnMut(Did) -> Option<(PropertySet, Option<Did>)>,
) -> Vec<PropertySet> {
    let mut levels = Vec::new();
    let mut seen = Vec::new();
    let mut at = Some(object);
    while let Some(d) = at {
        if seen.contains(&d) || levels.len() >= MAX_MARKER_CHAIN {
            break;
        }
        seen.push(d);
        let Some((set, parent)) = fetch(d) else { break };
        levels.push(set);
        at = parent.filter(|p| p.prefix() == 0x47);
    }
    levels.reverse();
    levels
}

/// Whether an entity accepts decals (§5.1, with the Tessera Decision for the two scene-node
/// states of Q4): `Render_ForceDecals` true, or `Render_AllowDecals` not false. The values
/// come from the entity lookup (per-instance set, then the object's chain, first value
/// found); absent values are the dictionary defaults (allow true, force false). The other
/// conditions of §4.3 step 1 (no skinned part, not attached, drawn at build time) are the
/// caller's.
#[must_use]
pub fn entity_accepts_decals(allow: Option<bool>, force: Option<bool>) -> bool {
    force == Some(true) || allow != Some(false)
}

/// Whether an interior cell's render geometry accepts decals (§5.2): its own
/// `Render_AllowDecals` is not false. `Render_ForceDecals` is not consulted.
#[must_use]
pub fn cell_accepts_decals(allow: Option<bool>) -> bool {
    allow != Some(false)
}

#[cfg(test)]
mod tests;
