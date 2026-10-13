// SPDX-License-Identifier: GPL-3.0-or-later
//! The mount viewer's data side (`mounts.md` §10), built on the asset workers with the
//! character ([`crate::character::build_character`]).
//!
//! For a mount effect (class 2459, recognised by its fields, §2.1) and a rider (the
//! character view's figure: a playable body or an NPC template's figure):
//!
//! 1. The rider takes the properties of the Decision of §4.1 ([`MountEffect::rider_entries`]:
//!    the rider fields, `Mount_CurrentMountType` = the effect's `Mount_Type`,
//!    `Mount_CurrentMountStatus` 0) and the gait the view selects as
//!    `MountMovement_CurrentSpeed` ([`RiderMount`]).
//! 2. The rider's clip selection runs with `Mounted_Channeling_Loop` running and started
//!    (the rider's mounted idle: the WhenRunning branch of `script-table.md` §8.4 step 2)
//!    and `Mounted_Player_Cosmetic_Mesh` started; the MeshFX node that run starts, chosen
//!    by the rider's mount type, species and sex, gives the mount entity, the holding
//!    location, the bone offset and the scale ([`MountMesh`], §4.4, §11.4).
//! 3. The mount entity is assembled like any figure (`build_mount`), at the rider's
//!    holding location (`Mount_Connection` in the data) by the Decision of §4.4; its clip
//!    selection starts the node's `Behavior` (`Horsepants_Startup`) with the same gait.
//!
//! This is the static evaluation of the script tables (the "without the script machinery"
//! branch of §10): the clips the two runs start are played, not a live script graph.
//! Script keys, gaits and labels are looked up by name in the install.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_anim::glam::Mat4;
use tessera_anim::{BoundClip, Skeleton};
use tessera_assets::JobContext;
use tessera_formats::gameplay::mounts::{MountEffect, MountLabels, MountMesh};
use tessera_formats::script_table::resolve::{
    ClipResolver, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::{Did, PropertyEntry, PropertySetRecord, PropertyValue, TSize};
use tessera_mesh::assembly::{
    AssemblyVocabulary, CharacterAssembler, GameDataSource, PartOrigin, RigKind,
};
use tessera_physics::controller::warsteed::{GAIT_NEUTRAL, GAIT_NEUTRAL_FORWARDS};

use crate::character::{Catalog, enum_name, property_key, property_mapper};
use crate::data::DataContext;
use crate::scene::{Collector, GeometryBudget, ScenePart, SceneRequest, build_part};

/// Most parts of a mount figure.
pub const MAX_MOUNT_PARTS: usize = 32;
/// Most gait labels listed.
pub const MAX_GAITS: usize = 64;

/// A mount request of the character view: the mount effect (a 0x70 object) and the gait
/// set as `MountMovement_CurrentSpeed` on rider and mount (`None`: unset, Neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountArg {
    /// The mount effect.
    pub effect: u32,
    /// The gait key.
    pub gait: Option<u32>,
}

/// Whether gait key `g` moves the mount (neither absent nor one of the two neutral gaits,
/// `game-controls.md` §5.3.2).
#[must_use]
pub fn is_moving_gait(g: Option<u32>) -> bool {
    g.is_some_and(|g| g != 0 && g != GAIT_NEUTRAL && g != GAIT_NEUTRAL_FORWARDS)
}

/// The rider's side of a mount request (step 1 of the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct RiderMount {
    /// The mount effect.
    pub effect: Did,
    /// The properties the rider takes (Decision of §4.1, and the gait).
    pub entries: Vec<PropertyEntry>,
    /// `Mounted_Channeling_Loop`.
    pub channelling: Option<u32>,
    /// `Mounted_Player_Cosmetic_Mesh`.
    pub mesh_script: Option<u32>,
    /// `Horsepants_Startup` (used when the node's `Behavior` is 0).
    pub startup: Option<u32>,
    /// `MountMovement_CurrentSpeed`.
    pub gait_pid: Option<u32>,
    /// The gait.
    pub gait: Option<u32>,
    /// The effect's `Mount_Type`.
    pub mount_type: Option<u32>,
    /// Facts for the inspector.
    pub facts: Vec<(String, String)>,
}

impl RiderMount {
    /// Applies the rider's mount state to its scripts before clip selection: the entries
    /// as property values, the channelling script running (and started), the mesh script
    /// started.
    pub fn apply(&self, scripts: &mut tessera_formats::script_table::resolve::EntityScripts) {
        let set = tessera_formats::PropertySet {
            size: TSize {
                hint: 0,
                count: u32::try_from(self.entries.len()).unwrap_or(u32::MAX),
            },
            entries: self.entries.clone(),
        };
        scripts.properties.merge_scalars(&set);
        if let Some(k) = self.channelling
            && !scripts.running.contains(&k)
        {
            scripts.running.push(k);
        }
        for k in [self.channelling, self.mesh_script].into_iter().flatten() {
            if !scripts.startup_scripts.contains(&k) {
                scripts.startup_scripts.push(k);
            }
        }
    }
}

/// Reads mount effect `arg.effect` and builds the rider's side (step 1).
///
/// # Errors
///
/// When the effect's property set cannot be read or it is not a mount effect.
pub fn rider_mount(ctx: &DataContext, arg: MountArg) -> Result<RiderMount, String> {
    let data = &ctx.game;
    let registry = data.registry().map_err(|e| e.to_string())?;
    let dict = registry.dictionary();
    let set_did = tessera_formats::object_property_set_did(Did(arg.effect))
        .ok_or_else(|| format!("{:#010x} is no game object", arg.effect))?;
    let bytes = data
        .read(set_did)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{set_did}: missing"))?;
    let set = PropertySetRecord::parse_expecting(&bytes, set_did, registry.as_ref())
        .map_err(|e| e.to_string())?
        .set;
    let labels = MountLabels::resolve(dict);
    let effect = MountEffect::recognise(&labels, &set).ok_or_else(|| {
        format!(
            "{:#010x} is not a mount effect (mounts.md §2.1)",
            arg.effect
        )
    })?;
    let mut entries = effect.rider_entries();
    let gait_pid = labels.state.current_speed.id();
    if let (Some(p), Some(g)) = (gait_pid, arg.gait) {
        entries.retain(|e| e.pid != p);
        entries.push(PropertyEntry {
            pid: p,
            value: PropertyValue::Enum(g),
        });
    }
    let key = |label: &str| property_key(data, "ScriptTable_ScriptID", label);
    let mount_type = effect.mount_type().map(|t| t.0);
    let rider = effect.rider();
    let mut facts = vec![
        ("mount effect".into(), Did(arg.effect).to_string()),
        (
            "mount type (effect)".into(),
            mount_type.map_or_else(|| "—".into(), |t| format!("{t:#x}")),
        ),
        (
            "mount entity (effect)".into(),
            rider.entity.map_or_else(|| "—".into(), |d| d.to_string()),
        ),
        (
            "slot scale (effect)".into(),
            rider.scale.map_or_else(|| "—".into(), |s| format!("{s:?}")),
        ),
    ];
    if let Some(name) = effect.name() {
        facts.push(("effect name".into(), format!("{name:?}")));
    }
    Ok(RiderMount {
        effect: Did(arg.effect),
        entries,
        channelling: key("Mounted_Channeling_Loop"),
        mesh_script: key("Mounted_Player_Cosmetic_Mesh"),
        startup: key("Horsepants_Startup"),
        gait_pid,
        gait: arg.gait,
        mount_type,
        facts,
    })
}

/// The mount of a mounted figure, ready to draw and animate.
#[derive(Debug, Clone)]
pub struct MountProduct {
    /// The mount effect.
    pub effect: Did,
    /// The mount entity (0x47).
    pub entity: Did,
    /// The MeshFX choice (§4.4).
    pub mesh: MountMesh,
    /// Its label of the holding location.
    pub location_label: String,
    /// Whether the rider's setup has that holding location (else the mount is not drawn,
    /// as a held item at a missing location, `skeleton-anim.md` §8.4).
    pub rider_has_location: bool,
    /// The mount's transform in the location's frame (Decision of §4.4).
    pub local: Mat4,
    /// The mount's skeleton (`None`: drawn static).
    pub skeleton: Option<Arc<Skeleton>>,
    /// The clip the mount's scripts start (bound to its skeleton).
    pub clip: Option<Arc<BoundClip>>,
    /// The clip's DID.
    pub clip_did: Option<Did>,
    /// Its playback rate.
    pub rate: f32,
    /// The gait keys and labels of `MountMovement_CurrentSpeed`.
    pub gaits: Vec<(u32, String)>,
    /// Facts for the inspector.
    pub facts: Vec<(String, String)>,
    /// Problems that do not stop the view.
    pub notes: Vec<String>,
}

/// The gait labels (`MountMovement_CurrentSpeed`'s mapper and its bases).
#[must_use]
pub fn gait_labels(ctx: &DataContext) -> Vec<(u32, String)> {
    let data = &ctx.game;
    let mut out: Vec<(u32, String)> = Vec::new();
    let mut next = property_mapper(data, "MountMovement_CurrentSpeed");
    let mut hops = 0;
    while let Some(m) = next.filter(|d| !d.is_null()) {
        hops += 1;
        if hops > 16 {
            break;
        }
        let Ok(Some(mapper)) = data.enum_mapper(m) else {
            break;
        };
        for (k, n) in mapper.names() {
            if out.len() < MAX_GAITS && !out.iter().any(|(x, _)| x == k) {
                out.push((*k, n.clone()));
            }
        }
        next = Some(mapper.base);
    }
    out.sort_by_key(|(k, _)| *k);
    out
}

/// What `build_mount` adds to the character scene.
#[derive(Debug)]
pub struct MountBuilt {
    /// The mount.
    pub product: MountProduct,
    /// Its parts (every one with [`ScenePart::figure`] 1).
    pub parts: Vec<ScenePart>,
}

/// Builds the mount figure (step 3 of the module documentation).
///
/// # Errors
///
/// When the mount entity cannot be assembled.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn build_mount(
    ctx: &DataContext,
    catalog: &Catalog,
    rider: &RiderMount,
    mesh: &MountMesh,
    rider_model_scale: [f32; 3],
    rider_has_location: bool,
    mats: &mut Collector<'_>,
    budget: &mut GeometryBudget,
    request: SceneRequest,
    job: &JobContext<'_>,
) -> Result<MountBuilt, String> {
    let data = &ctx.game;
    let registry = data.registry().map_err(|e| e.to_string())?;
    let dict = registry.dictionary();
    let vocab = AssemblyVocabulary::from_game_data(data).map_err(|e| e.to_string())?;
    let mut asm =
        CharacterAssembler::new(GameDataSource::new(data).map_err(|e| e.to_string())?, vocab)
            .with_options(ctx.assembly_options());
    let c = asm
        .assemble_object(mesh.entity, None)
        .map_err(|e| format!("mount entity {}: {e}", mesh.entity))?;
    let mut notes = Vec::new();
    let mut facts = rider.facts.clone();
    facts.push(("mount entity".into(), mesh.entity.to_string()));
    facts.push(("mount setup".into(), c.skeleton.setup.to_string()));
    facts.push((
        "MeshFX node".into(),
        format!("#{} in {}", mesh.node_id, mesh.table),
    ));
    facts.push((
        "scale".into(),
        format!(
            "{:?} ({})",
            mesh.scale,
            if mesh.scale_driven {
                "driven by the rider's slot scale"
            } else {
                "the node's stored scale"
            }
        ),
    ));
    facts.push(("bone offset".into(), format!("{:?}", mesh.bone_offset)));
    facts.push(("use parent scale".into(), mesh.use_parent_scale.to_string()));
    let location_label =
        enum_name_of_property(data, "Inventory_ParentingLocation", mesh.holding_location);
    facts.push(("holding location".into(), location_label.clone()));
    if !rider_has_location {
        notes.push(format!(
            "the rider's setup has no holding location {location_label}: the mount is not \
             drawn (skeleton-anim.md §8.4)"
        ));
    }
    let skeleton = match tessera_havok::Skeleton::parse(&c.skeleton.tagfile)
        .map_err(|e| e.to_string())
        .and_then(|h| Skeleton::from_havok(&h).map_err(|e| e.to_string()))
    {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            notes.push(format!("mount skeleton: {e}; drawn static"));
            None
        }
    };
    let bones = skeleton.as_ref().map_or(0, |s| s.bone_count());
    let mut parts = Vec::new();
    for p in c.parts.iter().filter(|p| !p.hidden).take(MAX_MOUNT_PARTS) {
        if job.is_cancelled() {
            return Err("cancelled".into());
        }
        let mut part = build_part(
            ctx,
            p.mesh,
            request,
            budget,
            (p.skinned && bones > 0).then_some(bones),
            None,
        );
        let layers: Vec<(Option<Did>, Did, u32)> = p
            .materials
            .iter()
            .map(|l| (l.shader, l.material, l.binding_key))
            .collect();
        for s in &mut part.submeshes {
            s.material = s
                .surface
                .map(|d| mats.surface_material_layered(Did(d), &layers));
        }
        let origin = match p.origin {
            PartOrigin::Setup { index } => format!("setup part {index}"),
            PartOrigin::Appearance { record, key, .. } => format!("{record} key {key:#x}"),
        };
        part.caption = format!("mount · {origin} · {}", p.mesh);
        part.figure = 1;
        parts.push(part);
    }
    if c.parts.len() > MAX_MOUNT_PARTS {
        notes.push(format!(
            "more than {MAX_MOUNT_PARTS} mount parts; the rest are left out"
        ));
    }
    facts.push((
        "mount rig".into(),
        match c.rig {
            RigKind::Humanoid => "humanoid",
            RigKind::Creature => "creature",
            RigKind::Static => "static",
        }
        .into(),
    ));

    // The mount's clip: its scripts with the node's Behavior started and the gait set.
    let mut clip_did = None;
    let mut rate = 1.0;
    let mut clip = None;
    let vocab = ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    })
    .map_err(|e| format!("script vocabulary: {e}"))?;
    match collect_entity_scripts(c.clip_inputs.start, dict, &vocab, |d| data.read(d)) {
        Ok(chain) => {
            let mut scripts = chain.scripts;
            if let Some(inst) = &c.clip_inputs.instance_properties {
                scripts.add_property_set(inst, &vocab);
            }
            let behavior = Some(mesh.behavior).filter(|b| *b != 0).or(rider.startup);
            if let Some(b) = behavior
                && !scripts.startup_scripts.contains(&b)
            {
                scripts.startup_scripts.push(b);
            }
            let mut state = scripts.properties.clone();
            MotionState::Idle.inputs().apply(&vocab, &mut state);
            if let (Some(p), Some(g)) = (rider.gait_pid, rider.gait) {
                state.set(p, PropertyValue::Enum(g));
            }
            let mut resolver = ClipResolver::new(dict, vocab, |d: Did| data.read(d))
                .with_dictionary_defaults(dict);
            match resolver.resolve_with_state(&scripts, state) {
                Ok(r) => {
                    let moving = is_moving_gait(rider.gait);
                    let chosen = moving
                        .then(|| catalog.movement_layer.and_then(|l| r.primary(l)))
                        .flatten()
                        .or_else(|| catalog.idle_layer.and_then(|l| r.primary(l)))
                        .or_else(|| {
                            r.candidates.iter().filter(|c| !c.is_held()).max_by(|a, b| {
                                (a.blend_weight * a.chance)
                                    .partial_cmp(&(b.blend_weight * b.chance))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                        });
                    facts.push(("mount clips started".into(), r.clips().len().to_string()));
                    if let Some(ch) = chosen {
                        clip_did = Some(ch.clip);
                        rate = Some(ch.playback_rate())
                            .filter(|r| r.is_finite() && *r > 0.0)
                            .unwrap_or(1.0);
                        facts.push(("mount clip".into(), ch.clip.to_string()));
                    }
                }
                Err(e) => notes.push(format!("mount clip selection: {e}")),
            }
        }
        Err(e) => notes.push(format!("mount scripts: {e}")),
    }
    if let (Some(d), Some(sk)) = (clip_did, &skeleton) {
        match crate::character::bind_clip(ctx, d, sk) {
            Ok(b) => clip = Some(b),
            Err(e) => notes.push(e),
        }
    }
    let local = Mat4::from_cols_array(&mesh.local_transform(rider_model_scale));
    Ok(MountBuilt {
        product: MountProduct {
            effect: rider.effect,
            entity: mesh.entity,
            mesh: mesh.clone(),
            location_label,
            rider_has_location,
            local,
            skeleton,
            clip,
            clip_did,
            rate,
            gaits: gait_labels(ctx),
            facts,
            notes,
        },
        parts,
    })
}

fn enum_name_of_property<S: tessera_props::RecordSource>(
    data: &tessera_props::GameData<S>,
    property: &str,
    key: u32,
) -> String {
    property_mapper(data, property)
        .and_then(|m| enum_name(data, m, key))
        .map_or_else(|| format!("{key:#x}"), |n| format!("{key:#x} ({n})"))
}

/// The labels of `keys` in the gait list, for the UI (`None` → "unset").
#[must_use]
pub fn gait_text(gaits: &[(u32, String)], g: Option<u32>) -> String {
    match g {
        None => "unset (Neutral)".into(),
        Some(k) => {
            let map: HashMap<u32, &str> = gaits.iter().map(|(k, n)| (*k, n.as_str())).collect();
            map.get(&k)
                .map_or_else(|| format!("{k}"), |n| format!("{k} ({n})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_gaits() {
        assert!(!is_moving_gait(None));
        assert!(!is_moving_gait(Some(GAIT_NEUTRAL)));
        assert!(!is_moving_gait(Some(GAIT_NEUTRAL_FORWARDS)));
        assert!(is_moving_gait(Some(GAIT_NEUTRAL_FORWARDS + 1)));
        let g = vec![(2, "Neutral".to_owned()), (7, "Gallop".to_owned())];
        assert_eq!(gait_text(&g, Some(7)), "7 (Gallop)");
        assert_eq!(gait_text(&g, None), "unset (Neutral)");
        assert_eq!(gait_text(&g, Some(9)), "9");
    }
}
