// SPDX-License-Identifier: GPL-3.0-or-later
//! What the walker reads about one mount for one body (`mounts.md` §2, §4.1, §5, §14.6,
//! §14.7, §14.9–§14.11; `walker.md` §16.2): the effect a source names, its census category,
//! the rider's properties, the controller and its parameters, the induction and the camera
//! offsets. Every value comes from the install and is treated as hostile: a mount that
//! cannot be drawn is refused with its category ([`MountRefusal`]), never a panic.

use std::path::Path;
use std::sync::Arc;

use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::gameplay::combat::{ModEntry, ModOp, ModifierPids, mod_entries};
use tessera_formats::gameplay::common::{PropertyAccess, ValueExt};
use tessera_formats::gameplay::mounts::{
    MountEffect, MountLabels, TemplateMountStats, default_world_mounted_max_speed,
    forwarded_mount_speed, normal_mount_speed_multiplier,
};
use tessera_formats::{Did, EnumMapper, PropertyEntry, PropertySet, PropertyValue};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::mount::{
    GaitTable, MountCategory, MountInfo, MountPresenter, OfferSource, OfferedMount,
};
use tessera_gamedata::quest::InstallPresentationData;
use tessera_gamedata::tooltip::TooltipData;
use tessera_physics::controller::warsteed::WarSteedActor;
use tessera_props::{EnumSource, GameData};
use tessera_text::Strings;

use super::spec::{MountSource, SteedTree};

tessera_formats::property_ids! {
    /// Properties the walker reads besides `MountLabels` (`mounts.md` §2.6, §14.7, §14.9).
    pub struct WalkerMountPids {
        induction = "Skill_InductionAction",
        induction_duration = "Induction_Duration",
        induction_script = "Induction_AnimationScriptID",
        camera_z = "ForwardSource_Entity_Camera_LookAt_OffsetZ",
        camera_y = "ForwardSource_Entity_Camera_LookAt_OffsetY",
        mod_op = "Mod_Op",
        modifier_op = "Mod_ModifierOp",
        area_allowed = "Area_Allowed_Mount_Types",
        dungeon_allowed = "Dungeon_Allowed_Mount_Types",
    }
}

/// The labels of MountedCombatControl's add-mod PIDs, Walk, Trot, Cantor (spelled so),
/// Gallop (`game-controls.md` §5.3.1).
pub const TURN_ADD_MOD_PROPERTY_LABELS: [&str; 4] = [
    "Mount_Control_Turn_Rate_AddMod_Property_Walk",
    "Mount_Control_Turn_Rate_AddMod_Property_Trot",
    "Mount_Control_Turn_Rate_AddMod_Property_Cantor",
    "Mount_Control_Turn_Rate_AddMod_Property_Gallop",
];
/// The suffix of a forwarded value's target label (`mounts.md` §6.2: `Mount_Control_Turn_Rate_AddMod_<gait>`
/// is forwarded to `…_<gait>_ForwardSource`).
pub const FORWARD_SOURCE_SUFFIX: &str = "_ForwardSource";

/// Why the walker does not mount (`walker.md` §16.1, §16.2, §16.5; `mounts.md` §14.11).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MountRefusal {
    /// The install's mount data could not be read.
    #[error("mount data unavailable: {0}")]
    Unavailable(String),
    /// The avatar is a figure DID, not a playable body (`walker.md` §16.1).
    #[error("This figure cannot ride.")]
    NotPlayable,
    /// The DID is neither a mount effect nor a skill toggling one.
    #[error("{0} is neither a mount effect nor a skill toggling one")]
    NotAMount(Did),
    /// `dir:` past the end of the body's list.
    #[error("dir:{index}: this body is offered {len} mounts")]
    PastEnd {
        /// The index asked for.
        index: usize,
        /// The list's length.
        len: usize,
    },
    /// `steed:` of a tree the body's race lacks.
    #[error("steed:{}: this body's race has no war-steed of that tree", .0.keyword())]
    NoSteed(SteedTree),
    /// The effect cannot be drawn in the walker (`mounts.md` §14.11).
    #[error("{effect} cannot be drawn: {} (mounts.md §14.11)", category_text(.category))]
    NotDrawable {
        /// The effect.
        effect: Did,
        /// Its census category.
        category: MountCategory,
    },
    /// The evaluation of `Mounted_Player_Cosmetic_Mesh` finds no mount mesh (§4.4).
    #[error("{effect}: no mount mesh for this body ({})", category_text(.category))]
    NoMesh {
        /// The effect.
        effect: Did,
        /// Its census category.
        category: MountCategory,
    },
    /// The rider's setup lacks the node's holding location (`skeleton-anim.md` §8.4).
    #[error("{effect}: this body has no holding location {location:#x} ({})", category_text(.category))]
    NoLocation {
        /// The effect.
        effect: Did,
        /// The holding-location key.
        location: u32,
        /// Its census category.
        category: MountCategory,
    },
    /// The feet are in water (`mounts.md` §4.7 step 4, §14.12).
    #[error("You cannot mount in water.")]
    InWater,
    /// The area or dungeon refuses the type (`mounts.md` §4.7 steps 5–6).
    #[error("This area does not allow this mount ({0}).")]
    Area(&'static str),
    /// Not on the collision walker (`walker.md` §16.1).
    #[error("Mounts need the collision walker.")]
    NoCollision,
    /// A `--server` session (`walker.md` §16.1).
    #[error("Mounts are not available with --server.")]
    Server,
    /// Fly mode (`walker.md` §16.1).
    #[error("Mounts are not available in fly mode.")]
    Fly,
    /// Already mounted (`walker.md` §16.3).
    #[error("Already mounted. Use /ride off first.")]
    Busy,
    /// A request is still being resolved or evaluated (Tessera rule: one at a time).
    #[error("A mount request is pending.")]
    Pending,
    /// A scenario session: the feed owns the mount state (`walker.md` §16.1).
    #[error("The scenario controls mounts in this session.")]
    Scenario,
}

fn category_text(c: &MountCategory) -> &'static str {
    match c {
        MountCategory::Normal => "normal mount",
        MountCategory::Clydesdale => "Clydesdale",
        MountCategory::WarSteed => "war-steed",
        MountCategory::WarSteedStartup => "war-steed start-up record",
        MountCategory::MixedWarSteed => "horse with a war-steed bit",
        MountCategory::HobbyHorse => "hobby-horse",
        MountCategory::NoEntityNoType => "no entity and no type",
    }
}

impl MountRefusal {
    /// The census category of the effect, when the refusal concerns one.
    #[must_use]
    pub fn category(&self) -> Option<MountCategory> {
        match self {
            Self::NotDrawable { category, .. }
            | Self::NoMesh { category, .. }
            | Self::NoLocation { category, .. } => Some(*category),
            _ => None,
        }
    }
}

/// The mount data of one install: the gameplay store (records, labels) and its strings.
pub struct MountData {
    store: GameDataStore,
    strings: Arc<Strings>,
}

impl std::fmt::Debug for MountData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountData").finish_non_exhaustive()
    }
}

impl MountData {
    /// Opens the install in `dir` read-only (records and the locale's strings; strings that
    /// cannot be read leave names out).
    ///
    /// # Errors
    ///
    /// [`MountRefusal::Unavailable`] when the records cannot be opened.
    pub fn open(dir: &Path) -> Result<Self, MountRefusal> {
        let store =
            GameDataStore::open(dir).map_err(|e| MountRefusal::Unavailable(e.to_string()))?;
        let strings = Strings::open_install(dir, None).unwrap_or_else(|e| {
            tracing::warn!("mounts: strings unavailable ({e}); names left out");
            Strings::new(tessera_text::MemorySource::new())
        });
        Ok(Self {
            store,
            strings: Arc::new(strings),
        })
    }

    /// The gameplay store.
    #[must_use]
    pub fn store(&self) -> &GameDataStore {
        &self.store
    }

    /// The property facade of the records.
    #[must_use]
    pub fn game_data(&self) -> &GameData {
        self.store.object_store().game_data()
    }

    /// Runs `f` with a mount presenter over the install.
    ///
    /// # Errors
    ///
    /// [`MountRefusal::Unavailable`] when the presentation data cannot be built.
    pub fn with_presenter<R>(
        &self,
        f: impl FnOnce(&MountPresenter<'_>) -> R,
    ) -> Result<R, MountRefusal> {
        self.with_presentation(|_, p| f(p))
    }

    /// Runs `f` with the presentation data and a mount presenter over it (both built once).
    ///
    /// # Errors
    ///
    /// [`MountRefusal::Unavailable`] when the presentation data cannot be built.
    pub fn with_presentation<R>(
        &self,
        f: impl FnOnce(&dyn TooltipData, &MountPresenter<'_>) -> R,
    ) -> Result<R, MountRefusal> {
        let pd = InstallPresentationData::new(&self.store, Arc::clone(&self.strings))
            .map_err(|e| MountRefusal::Unavailable(e.to_string()))?;
        let p = MountPresenter::new(&pd);
        Ok(f(&pd, &p))
    }

    /// The object property set of `did` (`None` when absent or unreadable).
    #[must_use]
    pub fn object(&self, did: Did) -> Option<PropertySet> {
        self.store.object(did).ok().flatten().map(|r| r.properties)
    }
}

/// A playable body as a rider (`mounts.md` §14.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiderBody {
    /// `RaceTable_Race` species key.
    pub species: u32,
    /// Sex value.
    pub sex: u32,
    /// `RaceTable_IsTallRace` (the directory's Tall group).
    pub tall: bool,
    /// The player-avatar game object (its template values feed the war-steed block).
    pub player_avatar: Did,
}

/// The rider body of (`species`, `sex`) from the race table, `None` when the table lacks it.
#[must_use]
pub fn rider_body<S: tessera_props::RecordSource>(
    data: &GameData<S>,
    species: u32,
    sex: u32,
) -> Option<RiderBody> {
    let registry = data.registry().ok()?;
    let dir = data
        .lookup_path(&["WEENIECONTENT", "RaceTableDirectory"])
        .ok()??;
    let pids = RacePids::from_names(registry.dictionary()).ok()?;
    let t = RaceTable::read(dir, registry.as_ref(), &pids, |d| data.read(d)).ok()?;
    let e = t
        .entries
        .iter()
        .find(|e| e.species == species && e.sex == sex)?;
    Some(RiderBody {
        species,
        sex,
        tall: e.tall_race.unwrap_or(true),
        player_avatar: e.player_avatar,
    })
}

/// How the walker moves and draws a mount (`mounts.md` §14.11 categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountKind {
    /// Generic controller on the Mounted layer, the speed factor of §5.1.
    Normal,
    /// As normal, with the war-steed riding set and the §4.6 item 4 gait (§14.6).
    Clydesdale,
    /// The war-steed controller and the gear rule (§14.6).
    WarSteed,
    /// A held prop; the rider stays on foot (§14.11).
    HobbyHorse,
}

impl MountKind {
    /// The kind of a census category (`None`: not drawable).
    #[must_use]
    pub fn of(category: MountCategory) -> Option<Self> {
        match category {
            MountCategory::Normal => Some(Self::Normal),
            MountCategory::Clydesdale => Some(Self::Clydesdale),
            MountCategory::WarSteed
            | MountCategory::WarSteedStartup
            | MountCategory::MixedWarSteed => Some(Self::WarSteed),
            MountCategory::HobbyHorse => Some(Self::HobbyHorse),
            MountCategory::NoEntityNoType => None,
        }
    }
}

/// The war-steed's actor values (`mounts.md` §14.6: the player avatar's template values plus
/// the effect's constant `Add` entries, written to the `_Cached` forms).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarSteedSetup {
    /// The actor (`_Cached` values, multipliers); gait and gear are the walker's.
    pub actor: WarSteedActor,
    /// The actor's values of the properties MountedCombatControl's
    /// `Mount_Control_Turn_Rate_AddMod_Property_*` name, Walk, Trot, Canter, Gallop
    /// (`game-controls.md` §5.3.2 step 2; 0 when the PID is absent or 0, or the actor has no
    /// float value; [`actor_add_mods`]).
    pub turn_add_mods: [f32; 4],
    /// The world limit *W* (§5.5 Decision: the dictionary default of
    /// `World_MountedCombat_MaxSpeed`; no limit when the dictionary has none).
    pub world_limit: f32,
}

/// The skill's induction (`mounts.md` §14.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Induction {
    /// `Induction_Duration`, seconds (modifier lists ignored offline).
    pub duration: f32,
    /// `Induction_AnimationScriptID`.
    pub script: Option<u32>,
}

/// One mount resolved for one body.
#[derive(Debug, Clone, PartialEq)]
pub struct MountProfile {
    /// The mount effect.
    pub effect: Did,
    /// The skill it was reached through (or found for its induction).
    pub skill: Option<Did>,
    /// Presentation facts (name, type labels, category).
    pub info: MountInfo,
    /// `Mount_Type` (0 when absent).
    pub mount_type: u32,
    /// How it moves and is drawn.
    pub kind: MountKind,
    /// It is in the body's offered list (`mounts.md` §14.10).
    pub in_group: bool,
    /// The rider's properties (Decision of §4.1).
    pub entries: Vec<PropertyEntry>,
    /// The normal mount's speed factor *m* (Decision of §5.1).
    pub speed_factor: f32,
    /// The war-steed's values.
    pub war_steed: Option<WarSteedSetup>,
    /// The gait multipliers of MountedCombatControl.
    pub gaits: GaitTable,
    /// The induction, when a skill gives one.
    pub induction: Option<Induction>,
    /// `Mounted_StartScriptID`.
    pub start_script: Option<u32>,
    /// `Dismounted_ByPlayer_Script`.
    pub dismount_by_player: Option<u32>,
    /// Camera look-at offsets (*y* forward, *z* up), m (`mounts.md` §14.9).
    pub camera: Option<[f32; 2]>,
    /// Problems that do not stop the mount.
    pub notes: Vec<String>,
}

impl MountProfile {
    /// Whether the war-steed controller moves the rider.
    #[must_use]
    pub fn is_war_steed(&self) -> bool {
        self.kind == MountKind::WarSteed
    }

    /// Whether the rider sits on a mount (anything but the hobby-horse).
    #[must_use]
    pub fn rides(&self) -> bool {
        self.kind != MountKind::HobbyHorse
    }
}

fn mapper_of(data: &GameData, pid: Option<u32>) -> Option<Arc<EnumMapper>> {
    let registry = data.registry().ok()?;
    match registry.enum_source(pid?)? {
        EnumSource::Mapper(m) => data.enum_mapper(m).ok().flatten(),
        EnumSource::DidMap(_) => None,
    }
}

fn op_of(entry: &ModEntry, mapper: Option<&EnumMapper>) -> Option<ModOp> {
    entry.op.and_then(|k| ModOp::resolve(k, mapper))
}

/// The constant `Add` entries of `entries` on property `pid`, summed (progression-driven
/// entries are left out and counted).
fn added(entries: &[ModEntry], pid: Option<u32>, mapper: Option<&EnumMapper>) -> (f32, usize) {
    let Some(pid) = pid else { return (0.0, 0) };
    let mut sum = 0.0;
    let mut skipped = 0;
    for e in entries.iter().filter(|e| e.modified == pid) {
        if op_of(e, mapper) != Some(ModOp::Add) {
            continue;
        }
        match e.value.as_ref().and_then(ValueExt::float) {
            Some(v) if v.is_finite() => sum += v,
            _ => skipped += 1,
        }
    }
    (sum, skipped)
}

/// The first `Set` entry's float on `pid`.
fn set_value(entries: &[ModEntry], pid: Option<u32>, mapper: Option<&EnumMapper>) -> Option<f32> {
    let pid = pid?;
    entries
        .iter()
        .filter(|e| e.modified == pid && op_of(e, mapper) == Some(ModOp::Set))
        .find_map(|e| e.value.as_ref().and_then(ValueExt::float))
        .filter(|v| v.is_finite())
}

/// The resolved effect of `source` for `body`, with the skill it came through.
fn source_effect(
    p: &MountPresenter<'_>,
    body: &RiderBody,
    source: MountSource,
) -> Result<(MountInfo, bool, Vec<OfferedMount>), MountRefusal> {
    let offered = p.offered(body.species, body.tall);
    let pick = |m: &OfferedMount| m.mount.clone().ok_or(MountRefusal::NotAMount(m.skill));
    match source {
        MountSource::Did(d) => {
            let info = p.resolve(Did(d)).ok_or(MountRefusal::NotAMount(Did(d)))?;
            let in_group = offered
                .iter()
                .any(|m| m.mount.as_ref().is_some_and(|x| x.effect == info.effect));
            Ok((info, in_group, offered))
        }
        MountSource::Auto | MountSource::Dir(_) => {
            let index = match source {
                MountSource::Dir(i) => usize::from(i),
                _ => 0,
            };
            let m = offered.get(index).ok_or(MountRefusal::PastEnd {
                index,
                len: offered.len(),
            })?;
            let info = pick(m)?;
            Ok((info, true, offered))
        }
        MountSource::Steed(tree) => {
            let trees = p.war_steed_trees();
            let key = trees
                .iter()
                .find(|t| {
                    t.label.as_deref().is_some_and(|l| {
                        let l = l.to_ascii_lowercase();
                        l.ends_with(&format!("_{}", tree.label_suffix()))
                            || l == tree.label_suffix()
                    })
                })
                .map(|t| t.tree)
                .ok_or(MountRefusal::NoSteed(tree))?;
            let m = offered
                .iter()
                .find(|m| m.source == OfferSource::WarSteed(key))
                .ok_or(MountRefusal::NoSteed(tree))?;
            let info = pick(m)?;
            Ok((info, true, offered))
        }
    }
}

/// One `Add` entry of an effect on `ForwardSource_Movement_MountSpeedMultiplier`
/// (`mounts.md` §2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedEntry {
    /// The entry's inline value.
    pub value: Option<f32>,
    /// Its modifier list names `Movement_MountSpeedBonus` (with operator `Set`, §2.6).
    pub uses_bonus: bool,
}

/// The normal mount's speed factor *m* (Decision of `mounts.md` §5.1; worked example §11.5):
/// `Movement_SpeedMultiplier` × the forwarded `Movement_Effect_MountSpeedMultiplier`, which
/// is the template value (1.0 when absent) plus the entries' values, an entry's value
/// replaced by the actor's `Movement_MountSpeedBonus` when its modifier list names it and
/// the actor holds it. Without entries the template value stands alone. Values that are
/// not finite are left out.
#[must_use]
pub fn mount_speed_factor(
    entries: &[SpeedEntry],
    template: Option<f32>,
    bonus: Option<f32>,
    speed_multiplier: Option<f32>,
) -> f32 {
    let sum: f32 = entries
        .iter()
        .filter_map(|e| {
            if e.uses_bonus {
                bonus.or(e.value)
            } else {
                e.value
            }
        })
        .filter(|v| v.is_finite())
        .sum();
    let forwarded = if entries.is_empty() {
        template
    } else {
        Some(forwarded_mount_speed(template, sum, None))
    };
    normal_mount_speed_multiplier(speed_multiplier, forwarded)
}

/// Resolves `source` for `body` (`walker.md` §16.2 Resolution) and reads everything the
/// walker needs about the mount.
///
/// # Errors
///
/// [`MountRefusal`] when the source does not resolve or the effect is not drawable
/// (`mounts.md` §14.11).
pub fn resolve_mount(
    data: &MountData,
    body: &RiderBody,
    source: MountSource,
) -> Result<MountProfile, MountRefusal> {
    let (info, in_group, _offered, gaits, add_mod_pids, toggling) =
        data.with_presentation(|pd, p| {
            source_effect(p, body, source).map(|(i, g, o)| {
                // §14.7: a source without a skill takes the induction of a skill that toggles
                // the effect: the body's offered list first, then any skill of the directory
                // (both groups) or the war-steed trees.
                let toggling = if i.skill.is_some() {
                    None
                } else {
                    toggling_skill(p, &o, i.effect)
                };
                (i, g, o, p.gaits(), turn_add_mod_pids(pd), toggling)
            })
        })??;
    let category = info.category;
    let Some(kind) = MountKind::of(category) else {
        return Err(MountRefusal::NotDrawable {
            effect: info.effect,
            category,
        });
    };
    let gd = data.game_data();
    let registry = gd
        .registry()
        .map_err(|e| MountRefusal::Unavailable(e.to_string()))?;
    let dict = registry.dictionary();
    let labels = MountLabels::resolve(dict);
    let wp = WalkerMountPids::resolve(dict);
    let set = data
        .object(info.effect)
        .ok_or(MountRefusal::NotAMount(info.effect))?;
    let effect =
        MountEffect::recognise(&labels, &set).ok_or(MountRefusal::NotAMount(info.effect))?;
    let rider = effect.rider();
    let mut notes = Vec::new();
    if !in_group {
        notes.push("not this body's group".to_owned());
    }
    let mods = mod_entries(&set, &ModifierPids::resolve(dict));
    let op_mapper = mapper_of(gd, wp.mod_op.id());
    let op_mapper = op_mapper.as_deref();
    let avatar = data.object(body.player_avatar);
    let stats = avatar
        .as_ref()
        .map(|a| TemplateMountStats::read(&labels.stats, a))
        .unwrap_or_default();
    let actor_float = |pid: tessera_formats::gameplay::common::Pid| {
        avatar
            .as_ref()
            .and_then(|a| a.float(pid))
            .filter(|v| v.is_finite())
    };
    // §5.1 Decision, §2.6: the forwarded mount-speed multiplier.
    let speed_entries: Vec<&ModEntry> = mods
        .iter()
        .filter(|e| labels.stats.forward_source_speed.is(e.modified))
        .filter(|e| op_of(e, op_mapper) == Some(ModOp::Add))
        .collect();
    let entries: Vec<SpeedEntry> = speed_entries
        .iter()
        .map(|e| SpeedEntry {
            value: e.value.as_ref().and_then(ValueExt::float),
            uses_bonus: labels
                .stats
                .speed_bonus
                .id()
                .is_some_and(|b| e.modifier_list.contains(&b)),
        })
        .collect();
    let speed_factor = mount_speed_factor(
        &entries,
        stats.effect_speed,
        actor_float(labels.stats.speed_bonus),
        actor_float(labels.stats.speed_multiplier),
    );
    // §14.6: the war-steed's actor values.
    let war_steed = (kind == MountKind::WarSteed).then(|| {
        let mut skipped = 0;
        let mut sum = |base: Option<f32>, pid: tessera_formats::gameplay::common::Pid| {
            let (add, s) = added(&mods, pid.id(), op_mapper);
            skipped += s;
            base.map(|b| b + add).or((add != 0.0).then_some(add))
        };
        let max_speed = sum(stats.max_speed, labels.stats.max_speed);
        let acceleration = sum(stats.acceleration, labels.stats.acceleration);
        let turn_rate = sum(stats.turn_rate, labels.stats.turn_rate);
        let gear_change_rate = sum(stats.gear_change_rate, labels.stats.gear_change_rate);
        if skipped > 0 {
            notes.push(format!(
                "{skipped} progression-driven war-steed entries left out"
            ));
        }
        let world_limit = default_world_mounted_max_speed(&labels.stats, dict)
            .filter(|w| w.is_finite() && *w > 0.0)
            .unwrap_or(f32::INFINITY);
        WarSteedSetup {
            actor: WarSteedActor {
                max_speed_cached: max_speed,
                // §14.6 Decision: a summed acceleration not above 0 keeps the layered one.
                acceleration_cached: acceleration.filter(|a| *a > 0.0),
                turn_rate_cached: turn_rate,
                gear_change_rate_cached: gear_change_rate,
                speed_multiplier: actor_float(labels.stats.speed_multiplier),
                acceleration_multiplier: actor_float(labels.stats.acceleration_multiplier),
                ..WarSteedActor::default()
            },
            turn_add_mods: actor_add_mods(
                &add_mod_pids,
                |pid| {
                    avatar
                        .as_ref()
                        .and_then(|a| a.get(pid))
                        .and_then(ValueExt::float)
                },
                |pid| registry.name(pid).map(str::to_owned),
                |label| registry.pid(label),
                |pid| added(&mods, Some(pid), op_mapper).0,
            ),
            world_limit,
        }
    });
    // §14.7: the skill's induction; an effect without a skill takes that of a skill that
    // toggles it.
    let skill = info.skill.or_else(|| {
        let (s, from) = toggling?;
        notes.push(format!("induction of {s} ({from})"));
        Some(s)
    });
    let induction = skill.and_then(|s| {
        let skill_set = data.object(s)?;
        let ind = skill_set.did(wp.induction).filter(|d| !d.is_null())?;
        let ind_set = data.object(ind)?;
        let duration = ind_set
            .float(wp.induction_duration)
            .filter(|d| d.is_finite())?;
        Some(Induction {
            duration: duration.clamp(0.0, super::state::MAX_PHASE_SECONDS),
            script: ind_set.enum_key(wp.induction_script),
        })
    });
    if skill.is_some() && induction.is_none() {
        notes.push("the skill has no readable induction".to_owned());
    }
    let camera = match (
        set_value(&mods, wp.camera_y.id(), op_mapper),
        set_value(&mods, wp.camera_z.id(), op_mapper),
    ) {
        (None, None) => None,
        (y, z) => Some([y.unwrap_or(0.0), z.unwrap_or(0.0)]),
    };
    Ok(MountProfile {
        effect: info.effect,
        skill,
        mount_type: info.mount_type.unwrap_or(0),
        kind,
        in_group,
        entries: effect.rider_entries(),
        speed_factor,
        war_steed,
        gaits,
        induction,
        start_script: rider.start_script,
        dismount_by_player: rider.dismount_by_player,
        camera,
        notes,
        info,
    })
}

/// The first skill that toggles `effect` (`mounts.md` §14.7: a request without a skill uses
/// the induction of a skill toggling the effect): the body's `offered` list first, then the
/// directory's skills of both groups, then every war-steed tree's skills. With where it was
/// found.
fn toggling_skill(
    p: &MountPresenter<'_>,
    offered: &[OfferedMount],
    effect: Did,
) -> Option<(Did, &'static str)> {
    if let Some(m) = offered
        .iter()
        .find(|m| m.mount.as_ref().is_some_and(|x| x.effect == effect))
    {
        return Some((m.skill, "the body's offered list"));
    }
    let toggles = |s: Did| p.toggled(s).contains(&effect);
    for tall in [true, false] {
        if let Some(s) = p.directory(tall).into_iter().find(|s| toggles(*s)) {
            return Some((s, "the mount directory"));
        }
    }
    p.war_steed_trees()
        .into_iter()
        .flat_map(|t| t.skills.into_iter().map(|(_, s)| s))
        .find(|s| toggles(*s))
        .map(|s| (s, "a war-steed tree"))
}

/// The PIDs MountedCombatControl's `Mount_Control_Turn_Rate_AddMod_Property_*` name, Walk,
/// Trot, Canter, Gallop (`game-controls.md` §5.3.1; `None` when the control, the label or a
/// PID value is absent, or the PID is 0).
fn turn_add_mod_pids(pd: &dyn TooltipData) -> [Option<u32>; 4] {
    let control = pd
        .weenie(tessera_gamedata::mount::MOUNTED_COMBAT_CONTROL)
        .and_then(|d| pd.record(d).ok().flatten());
    TURN_ADD_MOD_PROPERTY_LABELS.map(|label| {
        let c = control.as_ref()?;
        let v = c.get(pd.pid(label)?)?;
        v.property_ref()
            .or_else(|| v.enum_key())
            .filter(|p| *p != 0)
    })
}

/// The actor's add-mod values of `game-controls.md` §5.3.2 step 2 for the four PIDs `pids`
/// (Walk, Trot, Canter, Gallop): each the actor's float value of the PID (`actor`), else, when
/// the PID's label (`name`) is the forwarding target `<source>_ForwardSource` of
/// `mounts.md` §6.2, the forwarded value: the actor's float of the source (`pid_of` resolves
/// it) — the Decision of `mounts.md` §14.6 for the other forwarded mount stats — plus the
/// effect's constant `Add` entries on the source (`adds`); and the effect's `Add` entries on
/// the PID itself. 0 when the PID is absent; values that are not finite count 0.
pub fn actor_add_mods(
    pids: &[Option<u32>; 4],
    actor: impl Fn(u32) -> Option<f32>,
    name: impl Fn(u32) -> Option<String>,
    pid_of: impl Fn(&str) -> Option<u32>,
    adds: impl Fn(u32) -> f32,
) -> [f32; 4] {
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    pids.map(|p| {
        let Some(p) = p else { return 0.0 };
        let own = actor(p).map(finite);
        let forwarded = || {
            let label = name(p)?;
            let source = pid_of(label.strip_suffix(FORWARD_SOURCE_SUFFIX)?)?;
            Some(finite(actor(source).unwrap_or(0.0)) + finite(adds(source)))
        };
        finite(own.or_else(forwarded).unwrap_or(0.0) + finite(adds(p)))
    })
}

/// Where the feet are, for the area check of `mounts.md` §4.7 steps 5–6 (`walker.md` §16.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaAt {
    /// The walker's lookup does not know the area (every type is allowed).
    #[default]
    Unknown,
    /// Outdoors: the block's `Area_DID` (`Area_CurrentArea`).
    Outdoors(Did),
    /// In an interior cell: its `Area_DID` and `Dungeon_DID`, each when the cell names one.
    Cell {
        /// `Area_DID` of the cell.
        area: Option<Did>,
        /// `Dungeon_DID` of the cell.
        dungeon: Option<Did>,
    },
}

/// The mask check of `mounts.md` §4.7 step 6 for an allowed mask `allowed` and the type
/// `mount_type`: refused when the type is 0 or shares no bit with the mask.
#[must_use]
pub fn mask_allows(allowed: u64, mount_type: u32) -> bool {
    mount_type != 0 && allowed & u64::from(mount_type) != 0
}

/// The area check of `mounts.md` §4.7 steps 5–6 with the walker's lookup `at` (`walker.md`
/// §16.5; §14.12 Decision): `object` reads an area or dungeon object's set. An unknown area,
/// an outdoor area or cell area object that cannot be read, and a cell without `Area_DID`
/// allow every type (Tessera: the walker's lookup is the unknown); an object that lacks its
/// allowed-types property, or a mask without the type's bits, refuses.
///
/// # Errors
///
/// [`MountRefusal::Area`] with the reason.
pub fn area_allows(
    at: AreaAt,
    mount_type: u32,
    pids: &WalkerMountPids,
    object: impl Fn(Did) -> Option<PropertySet>,
) -> Result<(), MountRefusal> {
    let mask = |did: Did, pid: tessera_formats::gameplay::common::Pid| -> Option<Option<u64>> {
        let set = object(did)?;
        Some(set.get_pid(pid).and_then(PropertyValue::as_bits))
    };
    let check = |allowed: Option<Option<u64>>, what: &'static str| -> Result<(), MountRefusal> {
        match allowed {
            None => Ok(()),
            Some(None) => Err(MountRefusal::Area(what)),
            Some(Some(a)) if mask_allows(a, mount_type) => Ok(()),
            Some(Some(_)) => Err(MountRefusal::Area(what)),
        }
    };
    match at {
        AreaAt::Unknown => Ok(()),
        AreaAt::Outdoors(area) => check(mask(area, pids.area_allowed), "area"),
        AreaAt::Cell { area, dungeon } => {
            if let Some(a) = area {
                check(mask(a, pids.area_allowed), "area")?;
            }
            match dungeon {
                Some(d) => check(mask(d, pids.dungeon_allowed), "dungeon"),
                None => Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mounts.md` §11.5: entry Add 0.62 with the bonus list; without the bonus 1.62, with
    /// a bonus of 0.75 1.75; × `Movement_SpeedMultiplier` 1 and a forward speed of 6.5
    /// m/s: 10.53 and 11.375 m/s.
    #[test]
    fn worked_example_normal_mount_speed() {
        let e = [SpeedEntry {
            value: Some(0.62),
            uses_bonus: true,
        }];
        let m = mount_speed_factor(&e, Some(1.0), None, Some(1.0));
        assert!((m - 1.62).abs() < 1e-6 && (6.5 * m - 10.53).abs() < 1e-4);
        let m = mount_speed_factor(&e, Some(1.0), Some(0.75), Some(1.0));
        assert!((m - 1.75).abs() < 1e-6 && (6.5 * m - 11.375).abs() < 1e-4);
        // Absent values: template 1.0, multiplier 1; no entries: the template alone.
        assert!((mount_speed_factor(&e, None, None, None) - 1.62).abs() < 1e-6);
        assert!((mount_speed_factor(&[], Some(1.3), None, Some(2.0)) - 2.6).abs() < 1e-6);
        // An entry without the bonus list keeps its value; hostile values drop out.
        let plain = [
            SpeedEntry {
                value: Some(0.5),
                uses_bonus: false,
            },
            SpeedEntry {
                value: Some(f32::NAN),
                uses_bonus: false,
            },
        ];
        assert!((mount_speed_factor(&plain, None, Some(9.0), None) - 1.5).abs() < 1e-6);
        assert!(mount_speed_factor(&plain, Some(f32::INFINITY), None, Some(f32::NAN)).is_finite());
    }

    #[test]
    fn kinds_of_categories() {
        assert_eq!(
            MountKind::of(MountCategory::Normal),
            Some(MountKind::Normal)
        );
        assert_eq!(
            MountKind::of(MountCategory::MixedWarSteed),
            Some(MountKind::WarSteed),
            "mounts.md §14.11: the war-steed controller with horse clips"
        );
        assert_eq!(
            MountKind::of(MountCategory::WarSteedStartup),
            Some(MountKind::WarSteed)
        );
        assert_eq!(
            MountKind::of(MountCategory::HobbyHorse),
            Some(MountKind::HobbyHorse)
        );
        assert_eq!(MountKind::of(MountCategory::NoEntityNoType), None);
        let r = MountRefusal::NotDrawable {
            effect: Did(0x7003_3562),
            category: MountCategory::NoEntityNoType,
        };
        assert_eq!(r.category(), Some(MountCategory::NoEntityNoType));
        assert!(r.to_string().contains("no entity and no type"), "{r}");
    }

    /// `game-controls.md` §11.2: the Canter add-mod names property *P*; an actor whose *P* is
    /// 0.25 adds 0.25; without *P* nothing; absent PIDs add 0. A *P* labelled as the
    /// forwarding target of a source takes the source's value plus the effect's adds.
    #[test]
    fn worked_example_turn_add_mods() {
        const P: u32 = 10;
        const SRC: u32 = 11;
        let pids = [None, None, Some(P), None];
        let with_p = actor_add_mods(
            &pids,
            |p| (p == P).then_some(0.25),
            |_| None,
            |_| None,
            |_| 0.0,
        );
        assert_eq!(with_p, [0.0, 0.0, 0.25, 0.0]);
        let without = actor_add_mods(&pids, |_| None, |_| None, |_| None, |_| 0.0);
        assert_eq!(without, [0.0; 4]);
        let forwarded = actor_add_mods(
            &pids,
            |p| (p == SRC).then_some(0.5),
            |p| (p == P).then(|| "Mount_Control_Turn_Rate_AddMod_Cantor_ForwardSource".to_owned()),
            |l| (l == "Mount_Control_Turn_Rate_AddMod_Cantor").then_some(SRC),
            |p| if p == SRC { 0.25 } else { 0.0 },
        );
        assert_eq!(forwarded, [0.0, 0.0, 0.75, 0.0]);
        let hostile = actor_add_mods(
            &pids,
            |_| Some(f32::NAN),
            |_| None,
            |_| None,
            |_| f32::INFINITY,
        );
        assert_eq!(hostile, [0.0; 4]);
    }

    /// `mounts.md` §4.7 steps 5–6 with the walker's lookup (`walker.md` §16.5).
    #[test]
    fn area_and_mask_checks() {
        use tessera_formats::PropertyValue as V;
        assert!(mask_allows(0x3C47D, 0x1));
        assert!(!mask_allows(0x4004, 0x1));
        assert!(!mask_allows(0x3C47D, 0), "type 0 refused");
        let names = |l: &str| match l {
            "Area_Allowed_Mount_Types" => Some(1),
            "Dungeon_Allowed_Mount_Types" => Some(2),
            _ => None,
        };
        let pids = WalkerMountPids::resolve(&names);
        let set = |pid: u32, v: Option<u32>| {
            let mut s = PropertySet::default();
            if let Some(v) = v {
                s.entries.push(PropertyEntry {
                    pid,
                    value: V::Bitfield32(v),
                });
            }
            s
        };
        let objects = |d: Did| match d.0 {
            0x7000_0001 => Some(set(1, Some(0x3C47D))),
            0x7000_0002 => Some(set(1, Some(0))),
            0x7000_0003 => Some(set(1, None)),
            0x7000_0004 => Some(set(2, Some(0))),
            0x7000_0005 => Some(set(2, Some(0x1))),
            _ => None,
        };
        let check = |at| area_allows(at, 0x1, &pids, objects);
        assert_eq!(check(AreaAt::Unknown), Ok(()));
        assert_eq!(check(AreaAt::Outdoors(Did(0x7000_0001))), Ok(()));
        assert_eq!(
            check(AreaAt::Outdoors(Did(0x7000_0002))),
            Err(MountRefusal::Area("area"))
        );
        assert_eq!(
            check(AreaAt::Outdoors(Did(0x7000_0003))),
            Err(MountRefusal::Area("area"))
        );
        assert_eq!(
            check(AreaAt::Outdoors(Did(0x7000_0009))),
            Ok(()),
            "unreadable: unknown"
        );
        let cell = |area, dungeon| AreaAt::Cell { area, dungeon };
        assert_eq!(check(cell(None, None)), Ok(()));
        assert_eq!(
            check(cell(Some(Did(0x7000_0001)), Some(Did(0x7000_0004)))),
            Err(MountRefusal::Area("dungeon"))
        );
        assert_eq!(
            check(cell(Some(Did(0x7000_0001)), Some(Did(0x7000_0005)))),
            Ok(())
        );
        assert!(
            MountRefusal::Area("dungeon")
                .to_string()
                .starts_with("This area does not allow this mount")
        );
    }
}
