// SPDX-License-Identifier: GPL-3.0-or-later
//! Combat visuals on the walker's figures (`combat-visuals.md`; ROADMAP P2 exit criterion 3)
//! and the walker's side of selection (`combat-ui.md` §3.1–§3.2, §3.5 TR6, §6.4).
//!
//! The neutral per-entity API of [`CombatLayer`] takes what the session reports, by network
//! ID: casts (start, delay, end), hits ([`CombatLayer::strike`]), effects applied and
//! removed, combat state, defeat and revive, vitals, the selection; or a whole
//! [`tessera_net::domain::CombatEvent`] ([`CombatLayer::handle`]). Behind it:
//!
//! | Part | Where | Spec |
//! |---|---|---|
//! | Stories, strikes and hooks, casts, inductions and channels, effects, stance, defeat and revive | [`state::CombatState`] | §3, §4, §5, §6, §7, §9, §11 |
//! | The figures of session entities as script owners; their clips, hooks, Execute answers and beams | [`super::effects::session`] | §3.3, §4.6, §4.7; `particle-runtime.md` §11.1 |
//! | Clips the scripts start on a figure (upper body with track opacities, full body, death holds) | [`super::characters::combat_anim`] | `script-table.md` §6.10, `skeleton-anim.md` §6.6 |
//! | Particle nodes attached to bones and holding locations, looping while an effect lasts, stop and linger | the effects layer and `tessera-particles` | `particle-runtime.md` §5–§7 |
//! | Picking a figure under a screen point (TR1), anchors above figures for floating text | [`pick`], [`CombatLayer::pick`], [`CombatLayer::anchor_box`] | `combat-ui.md` §3.1, §3.2, §6.4 |
//!
//! # Per frame ([`CombatLayer::update`])
//!
//! 1. The clips the scripts started or stopped reach the character layer.
//! 2. Every session entity whose figure is on the GPU is spawned in the effects runtime (its
//!    persistent combat state is then shown on it, [`state::CombatState::entity_ready`],
//!    and the selection indicator starts on it when it is selected); figures the runtime
//!    lost (a region change) are spawned again the same way; gone ones are removed, and the
//!    state of entities the session no longer lists is dropped after [`ABSENT_GRACE`].
//!    Figures refused for a lasting reason (cap, region, no tables) are retried after a
//!    change or every [`REFUSAL_RETRY`] and are not waited for by [`CombatLayer::settled`]. `ScriptSystem_CurrentlyMoving` follows the figure's locomotion
//!    (`combat-visuals.md` §3.3: it switches the upper-body playback).
//! 3. The queued session events are handled in order (§9.6), then the story hooks the
//!    runtime reported, then stories whose behaviour ended or timed out.
//!
//! Data come from `tessera-gamedata`'s combat visuals over the install (opened on a worker
//! at start-up); without an install nothing plays and events are dropped.
//!
//! # No record reads on the frame thread (ARCHITECTURE §21)
//!
//! The skill, induction, channel and effect records a cue names, the worn items of the
//! stance (§11 rule 6) and the body records of picking are answered on the asset workers
//! ([`records`]). Before a cue is handled, its records must be answered and the clips its
//! scripts start on the figures decoded ([`super::characters::CharacterLayer::script_clips_ready`]);
//! until then the cue, and every cue behind it (§9.6 order), waits (each cue at most
//! [`MAX_CUE_WAIT`] from when it was queued, or from when the layer became ready, after
//! which it is handled with what is there; a run of such cues is handled together). A session figure is
//! spawned in the runtime only once its worn items are answered and the clips of its
//! start-up scripts are decoded. The frame thread's lookups
//! read only answers ([`records::RecordCache::frame_thread_misses`] counts those that found
//! none; [`CombatLayerStats::record_misses`]).

pub mod pick;
pub mod records;
pub mod state;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_formats::gameplay::items::equipment_category_keys;
use tessera_formats::{Did, PropertyValue};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::combat::{
    ChannelVisuals, CombatData, DIE_SCRIPT_LABEL, EffectVisuals, InductionVisuals, LIFE_ALIVE,
    LIFE_DEAD, REVIVE_SCRIPT_LABEL, ScriptKey, SkillVisuals, StrikeResult, stance_for,
};
use tessera_gamedata::tooltip::{InstallTooltipData, TooltipData};
use tessera_net::domain::{CastKind, CastOutcome, CombatEvent, HitResult, Ref, Vital};
use tessera_net::{Body, EntityId as NetId, EntityKind};
use tessera_render::glam::Vec3;
use tessera_scriptgraph::{EntityId, RunId, StoryHookEvent};

use self::records::{ItemFacts, Need, RecordCache, RecordReader};
use self::state::{CombatState, CombatStats, CombatVocab, ScriptHost, UpcomingView, VisualData};
use super::characters::CharacterLayer;
use super::characters::prefetch::ClipPrefetch;
use super::effects::EffectLayer;
use super::effects::session::ExecuteVocab;

/// Most session events queued before the layer is ready (oldest dropped beyond).
pub const MAX_QUEUED_EVENTS: usize = 4096;
/// Most events handled per frame (the rest wait, in order).
pub const MAX_EVENTS_PER_FRAME: usize = 1024;
/// Seconds after which a session figure refused for a lasting reason is tried again.
pub const REFUSAL_RETRY: f64 = 1.0;
/// Seconds an entity with combat state may be missing from the session before its state
/// is dropped.
pub const ABSENT_GRACE: f64 = 5.0;
/// Longest (wall-clock) wait of a cue for its records and clips (module docs).
pub const MAX_CUE_WAIT: Duration = Duration::from_secs(10);

/// A queued cue and when it was queued.
#[derive(Debug, Clone)]
struct Queued {
    cue: CombatCue,
    at: Instant,
}

/// How many cues at the head of the queue are handled now, and how many of them without
/// all their records or clips: `cues` gives each cue's readiness and the time its wait
/// started, in queue order (evaluated lazily: the first cue that is neither ready nor
/// [`MAX_CUE_WAIT`] old stops the count, and the ones behind it are not asked). Every cue
/// waits at most [`MAX_CUE_WAIT`] from its own start, so a run of cues that never become
/// ready stalls the queue for one wait, not one wait each.
fn handled_cues(cues: impl Iterator<Item = (bool, Instant)>, now: Instant) -> (usize, u64) {
    let mut handled = 0usize;
    let mut unready = 0u64;
    for (ready, since) in cues {
        if ready {
            handled += 1;
        } else if now.saturating_duration_since(since) >= MAX_CUE_WAIT {
            handled += 1;
            unready += 1;
        } else {
            break;
        }
    }
    (handled, unready)
}

/// One session report, by network ID (the neutral API's input).
#[derive(Debug, Clone, PartialEq)]
pub enum CombatCue {
    /// A cast, induction, channel or craft started (§9.1).
    CastStarted {
        /// The caster.
        entity: NetId,
        /// Its kind.
        cast: CastKind,
        /// The skill (or item) record, when the session names one.
        what: Option<Did>,
        /// Its target.
        target: Option<NetId>,
    },
    /// A cast ended (§9.1).
    CastEnded {
        /// The caster.
        entity: NetId,
        /// How.
        outcome: CastOutcome,
    },
    /// A hit, miss or heal (§9.3).
    Strike {
        /// Who caused it.
        source: Option<NetId>,
        /// Who shows it.
        target: NetId,
        /// The vital.
        vital: Vital,
        /// The amount.
        amount: f32,
        /// The result.
        result: HitResult,
        /// The skill or effect record.
        what: Option<Did>,
        /// The damage type key.
        damage_type: Option<u32>,
    },
    /// A vital's maximum (rule 5's intensity).
    VitalMax {
        /// The entity.
        entity: NetId,
        /// The vital.
        vital: Vital,
        /// Its maximum.
        max: f32,
    },
    /// An effect instance was applied (§9.4).
    EffectApplied {
        /// The entity.
        entity: NetId,
        /// The instance.
        instance: u64,
        /// The effect record.
        what: Option<Did>,
    },
    /// An effect instance ended.
    EffectRemoved {
        /// The entity.
        entity: NetId,
        /// The instance.
        instance: u64,
    },
    /// The entity entered or left combat (§9.5).
    CombatState {
        /// The entity.
        entity: NetId,
        /// In combat.
        in_combat: bool,
    },
    /// The entity was defeated.
    Defeated {
        /// The entity.
        entity: NetId,
    },
    /// The entity was revived.
    Revived {
        /// The entity.
        entity: NetId,
    },
    /// The local player's selection changed (`combat-ui.md` TR6).
    Selection {
        /// The new selection.
        target: Option<NetId>,
    },
}

/// The record a [`Ref`] names (a literal names none).
#[must_use]
pub fn record_of(r: &Ref) -> Option<Did> {
    match r {
        Ref::Record(d) => Some(Did(d.0)),
        _ => None,
    }
}

impl CombatCue {
    /// The cues of a session event (`None` for events without a visual, `combat-visuals.md`
    /// §9; `TargetChanged` of the player is the selection).
    #[must_use]
    pub fn from_event(e: &CombatEvent, player: Option<NetId>) -> Option<Self> {
        Some(match e {
            CombatEvent::CastStarted {
                entity,
                cast,
                what,
                target,
                ..
            } => Self::CastStarted {
                entity: *entity,
                cast: *cast,
                what: record_of(what),
                target: *target,
            },
            CombatEvent::CastEnded { entity, outcome } => Self::CastEnded {
                entity: *entity,
                outcome: *outcome,
            },
            CombatEvent::VitalDelta {
                source,
                target,
                vital,
                // A heal is a strike like any other (§9.3 step 4).
                kind: _,
                amount,
                result,
                what,
                damage_type,
            } => Self::Strike {
                source: *source,
                target: *target,
                vital: vital.clone(),
                amount: *amount,
                result: *result,
                what: what.as_ref().and_then(record_of),
                damage_type: *damage_type,
            },
            CombatEvent::VitalChanged {
                entity, vital, max, ..
            } => Self::VitalMax {
                entity: *entity,
                vital: vital.clone(),
                max: *max,
            },
            CombatEvent::EffectApplied {
                entity,
                effect,
                what,
                ..
            } => Self::EffectApplied {
                entity: *entity,
                instance: u64::from(effect.0),
                what: record_of(what),
            },
            CombatEvent::EffectRemoved { entity, effect } => Self::EffectRemoved {
                entity: *entity,
                instance: u64::from(effect.0),
            },
            CombatEvent::CombatState { entity, in_combat } => Self::CombatState {
                entity: *entity,
                in_combat: *in_combat,
            },
            CombatEvent::Defeated { entity, .. } => Self::Defeated { entity: *entity },
            CombatEvent::Revived { entity } => Self::Revived { entity: *entity },
            CombatEvent::TargetChanged { entity, target } if Some(*entity) == player => {
                Self::Selection { target: *target }
            }
            _ => return None,
        })
    }
}

/// A shared inbox of cues (`Send`, cheap to clone): producers that do not hold the walker
/// (a session pump on another thread, a test observer) queue cues here; the layer takes
/// them, in order, at its next update. Bounded by [`MAX_QUEUED_EVENTS`] (oldest dropped).
#[derive(Debug, Clone, Default)]
pub struct CombatInbox(Arc<Mutex<VecDeque<CombatCue>>>);

impl CombatInbox {
    /// Queues a cue.
    pub fn push(&self, cue: CombatCue) {
        let mut q = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if q.len() >= MAX_QUEUED_EVENTS {
            q.pop_front();
        }
        q.push_back(cue);
    }

    /// Queues the cue of a session event (`player`: the local player, for the selection).
    pub fn handle(&self, event: &CombatEvent, player: Option<NetId>) {
        if let Some(c) = CombatCue::from_event(event, player) {
            self.push(c);
        }
    }

    fn take(&self) -> Vec<CombatCue> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }
}

/// Everything the layer reads from the install, opened once.
struct Boot {
    data: CombatData<InstallTooltipData<'static>>,
    vocab: CombatVocab,
    exec: ExecuteVocab,
    /// `ScriptSystem_CurrentlyMoving`.
    moving: Option<u32>,
    /// `Selection_Script` and the key of `ui_selected_fx` (`combat-ui.md` TR6).
    selection_pid: Option<u32>,
    selected_fx: Option<ScriptKey>,
    /// `SharedUISelection_ScriptTable`.
    selection_table: Option<Did>,
    /// `Item_EquipmentCategory`, and its keys by label (§11 rule 6).
    category_pid: Option<u32>,
    categories: HashMap<u32, String>,
    /// `Inventory_DefaultSlot` and `Inventory_CompatibleSlot`.
    slot_pids: [Option<u32>; 2],
    /// The slot keys of `Weapon_Primary` and `Weapon_Secondary`.
    hand_keys: Option<[u32; 2]>,
    /// `Entity_Mode` keys by stance label.
    stances: HashMap<String, u32>,
    /// `IsSelectable`, `ClientIsSelectableOverride` (`combat-ui.md` §3.1).
    selectable: [Option<u32>; 2],
}

/// The install's answers; read only on the asset workers ([`records::RecordCache`]).
impl RecordReader for Boot {
    fn skill(&self, did: Did) -> Option<Arc<SkillVisuals>> {
        self.data.skill_visuals(did)
    }
    fn induction(&self, did: Did) -> Option<Arc<InductionVisuals>> {
        self.data.induction_visuals(did)
    }
    fn channel(&self, did: Did) -> Option<Arc<ChannelVisuals>> {
        self.data.channel_visuals(did)
    }
    fn effect(&self, did: Did) -> Option<Arc<EffectVisuals>> {
        self.data.effect_visuals(did)
    }
    fn induced_state_script(&self, did: Did) -> Option<ScriptKey> {
        self.data.induced_state_script(did)
    }
    fn item(&self, did: Did) -> Option<ItemFacts> {
        let record = self.data.data().record(did).ok().flatten()?;
        let bits = |pid: Option<u32>| pid.and_then(|p| record.get(p)?.as_bits()).unwrap_or(0);
        // `Item_EquipmentCategory` is a bit field: key *k* is bit *k* − 1.
        let category = equipment_category_keys(bits(self.category_pid))
            .find_map(|k| self.categories.get(&k).cloned())
            .unwrap_or_default();
        Some(ItemFacts {
            category,
            default_bits: bits(self.slot_pids[0]),
            compatible_bits: bits(self.slot_pids[1]),
        })
    }
    fn body_selectable(&self, did: Did) -> Option<bool> {
        body_selectable(self, did)
    }
}

/// The record stores opened per install directory, kept for the process (the tooltip data
/// borrows its store; opening an install is expensive).
fn store_for(dir: &Path) -> Result<&'static GameDataStore, String> {
    static STORES: OnceLock<Mutex<HashMap<PathBuf, &'static GameDataStore>>> = OnceLock::new();
    let m = STORES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut m = m.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(s) = m.get(dir) {
        return Ok(s);
    }
    let store = GameDataStore::open(dir).map_err(|e| e.to_string())?;
    let leaked: &'static GameDataStore = Box::leak(Box::new(store));
    m.insert(dir.to_path_buf(), leaked);
    Ok(leaked)
}

/// The key of `label` in the DID map that holds it, searched over the maps the root DID map
/// names (the callback map of `combat-visuals.md` §4.6 is found by its labels).
fn callback_keys(store: &GameDataStore, labels: &[&str]) -> Vec<Option<u32>> {
    let gd = store.object_store().game_data();
    let mut out = vec![None; labels.len()];
    let Ok(root) = gd.root_did_map() else {
        return out;
    };
    for e in root.entries() {
        let Ok(Some(map)) = gd.did_map(e.value) else {
            continue;
        };
        for entry in map.entries() {
            if let Some(i) = labels.iter().position(|l| *l == &*entry.label) {
                out[i].get_or_insert(entry.key);
            }
        }
        if out.iter().all(Option::is_some) {
            break;
        }
    }
    out
}

const RULE_6_CATEGORIES: [&str; 20] = [
    "1HSword",
    "1HAxe",
    "1HClub",
    "1HMace",
    "1HHammer",
    "Dagger",
    "2HSword",
    "2HClub",
    "2HMace",
    "2HHammer",
    "Halberd",
    "2HAxe",
    "Spear",
    "2HStaff",
    "Bow",
    "Crossbow",
    "Javelin",
    "Thrown",
    "BattleGauntlets",
    "Shield",
];
const RULE_6_EXTRA: [&str; 2] = ["HeavyShield", "MediumShield"];
const STANCES: [&str; 17] = [
    "combat_monster",
    "combat_1hshield",
    "combat_1h",
    "combat_2h",
    "combat_staff",
    "combat_magic",
    "combat_spear",
    "combat_dualwield",
    "combat_throw",
    "combat_staffsword",
    "combat_ua",
    "combat_2haxe",
    "combat_bow",
    "combat_spearshield",
    "combat_gauntlets",
    "combat_xbow",
    "combat_rune",
];

fn open_boot(dir: &Path) -> Result<Boot, String> {
    let store = store_for(dir)?;
    let strings = Arc::new(tessera_text::Strings::new(tessera_text::MemorySource::new()));
    let data = InstallTooltipData::new(store, strings).map_err(|e| e.to_string())?;
    let p = |n: &str| data.pid(n);
    let key_of = |pid: Option<u32>, label: &str| pid.and_then(|p| data.enum_key(p, label));
    let script_id = p("ScriptTable_ScriptID");
    let mut types = HashMap::new();
    let mut vocab = CombatVocab {
        story_id: p("StorySystem_StoryID"),
        speed_multiplier: p("Combat_AnimationSpeedMultiplier"),
        skill_mode: p("Combat_SkillMode"),
        mounted: p("Mount_Is_Mounted"),
        entity_mode: p("Entity_Mode"),
        combat_violent: p("Combat_Violent"),
        life_state: p("LifeState"),
        dead_script: p("Death_DeadScript"),
        died_knocked_down: p("Death_DiedWhileKnockedDown"),
        attacker_id: p("CombatStrikeData_AttackerID"),
        attack_result: p("CombatStrikeData_AttackResult"),
        damage_type: p("CombatStrikeData_DamageType"),
        vital: p("CombatStrikeData_Vital"),
        angle: p("CombatStrikeData_TargetToAttackerAngle"),
        intensity: p("CombatStrikeData_Intensity"),
        striking_implement: p("ScriptSystem_Snd_StrikingImplementType"),
        struck_surface: p("ScriptSystem_Snd_StruckSurfaceType"),
        implement_by_mode: [
            p("Combat_DefaultStrikingImplementType_Melee"),
            p("Combat_DefaultStrikingImplementType_Ranged"),
            p("Combat_DefaultStrikingImplementType_Thrown"),
        ],
        materials: [
            p("Material_WeaponMaterial"),
            p("Material_OffHandMaterial"),
            p("Material_TorsoMaterial"),
            p("Content_IAmMadeOf"),
        ],
        die_key: key_of(script_id, DIE_SCRIPT_LABEL),
        revive_key: key_of(script_id, REVIVE_SCRIPT_LABEL),
        alive: LIFE_ALIVE,
        dead: LIFE_DEAD,
        ..CombatVocab::default()
    };
    for r in [
        StrikeResult::Normal,
        StrikeResult::Critical,
        StrikeResult::Miss,
        StrikeResult::Evade,
        StrikeResult::Parry,
        StrikeResult::Block,
        StrikeResult::Resist,
        StrikeResult::Immune,
    ] {
        if let Some(k) = key_of(vocab.attack_result, r.label()) {
            vocab.results.insert(r.label().to_owned(), k);
        }
    }
    let pids = [
        vocab.story_id,
        vocab.speed_multiplier,
        vocab.skill_mode,
        vocab.entity_mode,
        vocab.combat_violent,
        vocab.life_state,
        vocab.dead_script,
        vocab.died_knocked_down,
        vocab.attacker_id,
        vocab.attack_result,
        vocab.damage_type,
        vocab.vital,
        vocab.angle,
        vocab.intensity,
        vocab.striking_implement,
        vocab.struck_surface,
    ];
    for pid in pids.into_iter().flatten() {
        if let Some(t) = data.property_type(pid) {
            types.insert(pid, t);
        }
    }
    vocab.types = types;
    let keys = callback_keys(store, &["GetUpcomingTargets", "GetUpcomingHitResult"]);
    let exec = ExecuteVocab {
        upcoming_targets: keys[0],
        upcoming_hit_result: keys[1],
        filter: p("SCP_GetUpcomingTargets_CombatResultType_Filter"),
        targets_out: p("SCP_Output_GetUpcomingTargets_TargetArray"),
        targets_result: p("SCP_GetUpcomingTargets_Result"),
        hit_result_out: p("SCP_GetUpcomingHitResult_Result"),
    };
    let category_pid = p("Item_EquipmentCategory");
    let categories = RULE_6_CATEGORIES
        .iter()
        .chain(&RULE_6_EXTRA)
        .filter_map(|l| Some((key_of(category_pid, l)?, (*l).to_owned())))
        .collect();
    let stances = STANCES
        .iter()
        .filter_map(|l| Some(((*l).to_owned(), key_of(vocab.entity_mode, l)?)))
        .collect();
    tracing::info!(
        die = ?vocab.die_key,
        revive = ?vocab.revive_key,
        upcoming = ?exec.upcoming_targets,
        "combat visuals ready"
    );
    let default_slot = p("Inventory_DefaultSlot");
    let hand_keys = key_of(default_slot, "Weapon_Primary")
        .zip(key_of(default_slot, "Weapon_Secondary"))
        .map(|(a, b)| [a, b]);
    Ok(Boot {
        slot_pids: [default_slot, p("Inventory_CompatibleSlot")],
        hand_keys,
        moving: p("ScriptSystem_CurrentlyMoving"),
        selection_pid: p("Selection_Script"),
        selected_fx: key_of(script_id, "ui_selected_fx"),
        selection_table: data.weenie("SharedUISelection_ScriptTable"),
        selectable: [p("IsSelectable"), p("ClientIsSelectableOverride")],
        category_pid,
        categories,
        stances,
        exec,
        vocab,
        data: CombatData::new(data),
    })
}

/// The script host over the effects layer (state IDs are the network IDs).
struct Host<'a> {
    fx: &'a mut EffectLayer,
    chars: &'a CharacterLayer,
    boot: &'a Boot,
    records: &'a RecordCache<Boot>,
}

impl Host<'_> {
    fn rid(&self, e: EntityId) -> Option<EntityId> {
        self.fx.session_id(NetId(e.0))
    }
}

/// Run parameters with their instance IDs (state IDs are network IDs, for example
/// `CombatStrikeData_AttackerID`) turned into the runtime's entity IDs, so that Property
/// targets over them name the figure; an entity that does not run in the runtime becomes 0
/// (no entity).
fn runtime_params(
    params: &[(u32, PropertyValue)],
    rid: impl Fn(EntityId) -> Option<EntityId>,
) -> Vec<(u32, PropertyValue)> {
    params
        .iter()
        .map(|(p, v)| match v {
            PropertyValue::InstanceId(x) => (
                *p,
                PropertyValue::InstanceId(rid(EntityId(*x)).map_or(0, |e| e.0)),
            ),
            other => (*p, other.clone()),
        })
        .collect()
}

/// One worn item that is not stowed, for the stance (§11 rule 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct HeldItem {
    /// The feed's equipment slot key, when given (decision 0025 D3).
    slot: Option<u32>,
    /// `Inventory_DefaultSlot` bits.
    default_bits: u64,
    /// `Inventory_CompatibleSlot` bits.
    compatible_bits: u64,
    /// Its rule-6 category label; `""` for an item of another category.
    category: String,
}

/// The `ContainerSlot` bit of slot key `key` (key *k* is bit *k* − 1).
fn slot_bit(key: u32) -> u64 {
    key.checked_sub(1)
        .filter(|b| *b < 64)
        .map_or(0, |b| 1u64 << b)
}

/// The main- and off-hand categories of a figure's held items (§11 rule 6): an item with a
/// slot is in that slot's hand; without one, the hand rule of `character-panels.md` §7 (its
/// `Inventory_DefaultSlot` hand, the secondary hand when an earlier item took the primary
/// one and the item fits `Weapon_Secondary`). Items in neither hand (ranged slot, armour)
/// do not count. `hands` holds the slot keys of `Weapon_Primary` and `Weapon_Secondary`;
/// without them, the order rule of the rule-6 OPEN applies.
fn hand_categories(
    items: &[HeldItem],
    hands: Option<[u32; 2]>,
) -> (Option<String>, Option<String>) {
    let mut main: Option<String> = None;
    let mut off: Option<String> = None;
    let Some([primary, secondary]) = hands else {
        for it in items.iter().filter(|i| !i.category.is_empty()) {
            let shield = it.category.ends_with("Shield");
            if !shield && main.is_none() {
                main = Some(it.category.clone());
            } else if off.is_none() {
                off = Some(it.category.clone());
            }
        }
        return (main, off);
    };
    let (pb, sb) = (slot_bit(primary), slot_bit(secondary));
    for it in items {
        let hand = match it.slot {
            Some(k) if k == primary => Some(0),
            Some(k) if k == secondary => Some(1),
            Some(_) => None,
            None if it.default_bits & pb != 0 => {
                if main.is_some() && sb != 0 && it.compatible_bits & sb != 0 {
                    Some(1)
                } else {
                    Some(0)
                }
            }
            None if it.default_bits & sb != 0 => Some(1),
            None => None,
        };
        match hand {
            Some(0) if main.is_none() => main = Some(it.category.clone()),
            Some(1) if off.is_none() => off = Some(it.category.clone()),
            _ => {}
        }
    }
    (main, off)
}

/// The stance of a figure by rule 6 (`combat-visuals.md` §11): players by the categories of
/// their main- and off-hand items ([`hand_categories`]), other figures `combat_monster`.
fn stance_of(
    boot: &Boot,
    records: &RecordCache<Boot>,
    chars: &CharacterLayer,
    e: NetId,
) -> Option<u32> {
    let player = is_player_figure(chars, e);
    let mut items = Vec::new();
    if player && let Some(f) = chars.figure_of(e) {
        for eq in f
            .request
            .equipment
            .iter()
            .filter(|q| !q.stowed)
            .take(MAX_STANCE_ITEMS)
        {
            // Answered on the workers before the cue or the figure's spawn (module docs); a
            // record that does not read counts as an item of no category in no slot.
            let facts = records.item(Did(eq.item.0)).flatten().unwrap_or_default();
            items.push(HeldItem {
                slot: eq.slot.map(u32::from),
                default_bits: facts.default_bits,
                compatible_bits: facts.compatible_bits,
                category: facts.category,
            });
        }
    }
    let (main, off) = hand_categories(&items, boot.hand_keys);
    let label = stance_for(main.as_deref(), off.as_deref(), player);
    boot.stances.get(label).copied()
}

/// Most worn items the stance rule reads per figure.
const MAX_STANCE_ITEMS: usize = 64;

/// Whether `e`'s figure is a player's (§11 rule 6).
fn is_player_figure(chars: &CharacterLayer, e: NetId) -> bool {
    matches!(chars.entity_kind(e), Some(EntityKind::Player))
        || matches!(
            chars.figure_of(e).map(|f| f.request.body),
            Some(Body::Character { .. })
        )
}

/// The worn-item records the stance of `e` reads (§11 rule 6).
fn item_needs(chars: &CharacterLayer, e: NetId) -> Vec<Need> {
    if !is_player_figure(chars, e) {
        return Vec::new();
    }
    chars.figure_of(e).map_or_else(Vec::new, |f| {
        f.request
            .equipment
            .iter()
            .filter(|q| !q.stowed)
            .take(MAX_STANCE_ITEMS)
            .map(|q| Need::Item(Did(q.item.0)))
            .collect()
    })
}

/// The records cue `cue` reads (module docs).
fn cue_needs(cue: &CombatCue, chars: &CharacterLayer) -> Vec<Need> {
    match cue {
        CombatCue::CastStarted { what, .. }
        | CombatCue::Strike { what, .. }
        | CombatCue::EffectApplied { what, .. } => what.map(Need::What).into_iter().collect(),
        CombatCue::CombatState { entity, .. } => item_needs(chars, *entity),
        _ => Vec::new(),
    }
}

/// Pushes the non-zero keys of `keys` into `out` without duplicates.
fn push_keys(out: &mut Vec<u32>, keys: impl IntoIterator<Item = ScriptKey>) {
    for k in keys {
        if k != 0 && !out.contains(&k) {
            out.push(k);
        }
    }
}

/// The scripts a skill, induction, channel or effect record may run (`combat-visuals.md`
/// §3, §5, §9).
fn record_keys(data: &dyn VisualData, what: Did) -> Vec<u32> {
    let mut keys = Vec::new();
    if let Some(s) = data.skill(what) {
        push_keys(&mut keys, s.self_list.iter().copied());
        push_keys(&mut keys, s.other_list.iter().copied());
        push_keys(&mut keys, s.mounted_self_list.iter().copied());
        push_keys(&mut keys, s.mounted_other_list.iter().copied());
        push_keys(&mut keys, [s.execution_failure, s.conjunction_windup]);
        for h in &s.hooks {
            push_keys(
                &mut keys,
                [
                    h.behaviour_target_hit,
                    h.fx_target_hit,
                    h.fx_attacker_hit,
                    h.fx_weapon_hit,
                    h.behaviour_weapon_hit,
                ],
            );
        }
        if let Some(i) = s.induction.and_then(|d| data.induction(d)) {
            push_keys(
                &mut keys,
                [
                    i.animation,
                    i.mounted_animation,
                    i.fx,
                    i.failure_fx,
                    i.target_animation,
                    i.target_fx,
                    i.target_failure_fx,
                ],
            );
        }
        if let Some(c) = s.channelling.and_then(|d| data.channel(d)) {
            for m in [&c.start, &c.running, &c.success, &c.failure, &c.abandoned] {
                push_keys(&mut keys, [m.caster, m.caster_fx, m.target, m.target_fx]);
            }
        }
    }
    if let Some(e) = data.effect(what) {
        push_keys(&mut keys, [e.fx, e.interval_fx, e.threat_interval_fx]);
    }
    push_keys(&mut keys, data.induced_state_script(what));
    keys
}

/// The clips cue `cue` needs decoded, by figure (module docs): the scripts its records name
/// on the entities it involves; the stance's start-up scripts for a combat-state change;
/// the death and revive scripts.
fn cue_clips(
    cue: &CombatCue,
    boot: &Boot,
    records: &RecordCache<Boot>,
    chars: &CharacterLayer,
) -> Vec<(NetId, ClipPrefetch)> {
    let on = |entities: &[Option<NetId>], keys: Vec<u32>| -> Vec<(NetId, ClipPrefetch)> {
        if keys.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<(NetId, ClipPrefetch)> = Vec::new();
        for e in entities.iter().flatten() {
            if !out.iter().any(|(o, _)| o == e) {
                out.push((
                    *e,
                    ClipPrefetch {
                        keys: keys.clone(),
                        overrides: Vec::new(),
                    },
                ));
            }
        }
        out
    };
    match cue {
        CombatCue::CastStarted {
            entity,
            what: Some(w),
            target,
            ..
        } => on(&[Some(*entity), *target], record_keys(records, *w)),
        CombatCue::Strike {
            source,
            target,
            what: Some(w),
            ..
        } => on(&[Some(*target), *source], record_keys(records, *w)),
        CombatCue::EffectApplied {
            entity,
            what: Some(w),
            ..
        } => on(&[Some(*entity)], record_keys(records, *w)),
        CombatCue::CombatState {
            entity,
            in_combat: true,
        } => {
            let mut overrides = Vec::new();
            if let Some(p) = boot.vocab.combat_violent {
                overrides.push((p, PropertyValue::Bool(true)));
            }
            if let (Some(p), Some(k)) = (
                boot.vocab.entity_mode,
                stance_of(boot, records, chars, *entity),
            ) {
                overrides.push((p, boot.vocab.value(p, f64::from(k))));
            }
            vec![(
                *entity,
                ClipPrefetch {
                    keys: Vec::new(),
                    overrides,
                },
            )]
        }
        CombatCue::Defeated { entity } => {
            on(&[Some(*entity)], boot.vocab.die_key.into_iter().collect())
        }
        CombatCue::Revived { entity } => on(
            &[Some(*entity)],
            boot.vocab.revive_key.into_iter().collect(),
        ),
        _ => Vec::new(),
    }
}

impl ScriptHost for Host<'_> {
    fn available(&mut self, e: EntityId) -> bool {
        self.rid(e).is_some()
    }
    fn run(
        &mut self,
        e: EntityId,
        key: ScriptKey,
        params: &[(u32, PropertyValue)],
    ) -> Option<RunId> {
        let id = self.rid(e)?;
        let params = runtime_params(params, |n| self.rid(n));
        self.fx.run_script(id, key, &params)
    }
    fn stop(&mut self, run: RunId) {
        self.fx.stop_run(run);
    }
    fn live(&mut self, run: RunId) -> bool {
        self.fx.run_live(run)
    }
    fn set_property(&mut self, e: EntityId, pid: u32, value: PropertyValue) {
        if let Some(id) = self.rid(e) {
            self.fx.set_entity_property(id, pid, value);
        }
    }
    fn clear_property(&mut self, e: EntityId, pid: u32) {
        if let Some(id) = self.rid(e) {
            self.fx.clear_entity_property(id, pid);
        }
    }
    fn property(&mut self, e: EntityId, pid: u32) -> Option<PropertyValue> {
        let id = self.rid(e)?;
        self.fx.entity_property(id, pid)
    }
    fn switch_cases(&mut self, e: EntityId, key: ScriptKey, pid: u32) -> Vec<PropertyValue> {
        self.rid(e)
            .map(|id| self.fx.switch_cases(id, key, pid))
            .unwrap_or_default()
    }
    fn pose(&mut self, e: EntityId) -> Option<([f64; 3], [f32; 2])> {
        let (pos, heading) = self.chars.entity_pose(NetId(e.0))?;
        // Figures face their model +Y turned by heading − 90°: (cos h, sin h).
        Some(([pos.x, pos.y, pos.z], [heading.cos(), heading.sin()]))
    }
    fn stance(&mut self, e: EntityId) -> Option<u32> {
        stance_of(self.boot, self.records, self.chars, NetId(e.0))
    }
    fn set_upcoming(&mut self, view: UpcomingView) {
        // Runtime IDs for the Execute callbacks' arrays.
        let mut out = UpcomingView {
            hit: view.hit,
            ..UpcomingView::default()
        };
        for (id, (strikes, target, received)) in view.stories {
            let strikes = strikes
                .into_iter()
                .filter_map(|(t, r)| Some((self.rid(t)?, r)))
                .collect();
            out.stories
                .insert(id, (strikes, target.and_then(|t| self.rid(t)), received));
        }
        self.fx.set_upcoming(out);
    }
    fn take_hooks(&mut self) -> Vec<StoryHookEvent> {
        self.fx
            .take_story_hooks()
            .into_iter()
            .filter_map(|mut h| {
                h.entity = EntityId(self.fx.session_net(h.entity)?.0);
                Some(h)
            })
            .collect()
    }
}

/// What the layer needs each frame.
#[derive(Debug)]
pub struct CombatInput<'a> {
    /// The material clock, seconds.
    pub now: f64,
    /// The local player.
    pub player: Option<NetId>,
    /// The character layer (figures, clips).
    pub characters: &'a mut CharacterLayer,
    /// The effects layer (the runtime the scripts run on).
    pub effects: &'a mut EffectLayer,
}

/// Counters of the layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CombatLayerStats {
    /// Start-up finished.
    pub ready: bool,
    /// Session events handled.
    pub events: u64,
    /// Session events dropped (queue full, no data).
    pub dropped: u64,
    /// Session figures spawned in the runtime.
    pub figures: usize,
    /// The state machine's counters.
    pub state: CombatStats,
    /// Frame-thread record lookups that found no answer (no record is read there; module
    /// docs).
    pub record_misses: u64,
    /// Records read on the asset workers.
    pub record_reads: u64,
    /// Cues handled after [`MAX_CUE_WAIT`] without all their records or clips.
    pub cues_unready: u64,
}

/// The combat layer (module docs).
pub struct CombatLayer {
    boot_rx: Option<mpsc::Receiver<Result<Boot, String>>>,
    boot: Option<Arc<Boot>>,
    /// The record answers (module docs); made with the boot.
    records: Option<Arc<RecordCache<Boot>>>,
    /// The workers the records are read on.
    pool: WorkerPool,
    /// Since when the layer's data and the runtime are ready (a cue queued earlier waits
    /// from then on, [`MAX_CUE_WAIT`]).
    ready_since: Option<Instant>,
    state: Option<CombatState>,
    seed: u32,
    queue: VecDeque<Queued>,
    inbox: CombatInbox,
    ready: HashSet<u64>,
    moving: HashMap<u64, bool>,
    selection: Option<(NetId, Option<RunId>)>,
    /// Session figures on the GPU with scripts that the runtime has not taken yet.
    waiting: usize,
    /// Session figures the runtime refused for a lasting reason, with the time of the
    /// refusal.
    refused: HashMap<u64, f64>,
    /// Entities with combat state that the session does not list, since when.
    absent: HashMap<u64, f64>,
    problem: Option<String>,
    stats: CombatLayerStats,
}

impl std::fmt::Debug for CombatLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatLayer")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl CombatLayer {
    /// A layer reading the install at `dir` on `pool` (`None`: no install, nothing plays);
    /// `seed` seeds its random draws (§11 rule 2).
    #[must_use]
    pub fn new(dir: Option<PathBuf>, pool: &WorkerPool, seed: u32) -> Self {
        let mut problem = None;
        let boot_rx = dir.and_then(|dir| {
            let (tx, rx) = mpsc::sync_channel(1);
            let job = move |_: &tessera_assets::JobContext<'_>| {
                let _ = tx.send(open_boot(&dir));
            };
            match pool.spawn(Priority::Prefetch, CancelToken::new(), job) {
                Ok(_) => Some(rx),
                Err(e) => {
                    problem = Some(format!("start-up job: {e}"));
                    None
                }
            }
        });
        Self {
            boot_rx,
            boot: None,
            records: None,
            pool: pool.clone(),
            ready_since: None,
            state: None,
            seed,
            queue: VecDeque::new(),
            inbox: CombatInbox::default(),
            ready: HashSet::new(),
            moving: HashMap::new(),
            selection: None,
            waiting: 0,
            refused: HashMap::new(),
            absent: HashMap::new(),
            problem,
            stats: CombatLayerStats::default(),
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> CombatLayerStats {
        let mut s = self.stats;
        if let Some(st) = &self.state {
            s.state = st.stats();
        }
        if let Some(r) = &self.records {
            s.record_misses = r.frame_thread_misses();
            s.record_reads = r.worker_reads();
        }
        s.figures = self.ready.len();
        s
    }

    /// Whether nothing is pending: start-up finished (with or without data), no session
    /// event waits, and every session figure on the GPU runs in the runtime.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.boot_rx.is_none()
            && (self.boot.is_none() || (self.queue.is_empty() && self.waiting == 0))
    }

    /// One HUD line.
    #[must_use]
    pub fn hud_line(&self) -> String {
        if let Some(p) = &self.problem {
            return format!("Combat: {p}");
        }
        let s = self.stats();
        format!(
            "Combat: {} figures, {} stories ({} open), {} strikes shown, {} waiting, {} effect runs",
            s.figures,
            s.state.stories,
            s.state.open_stories,
            s.state.strikes_shown,
            s.state.strikes_waiting,
            s.state.effect_runs
        )
    }

    // ----- the neutral per-entity API -----

    /// Queues a cue (handled at the next [`CombatLayer::update`], in order).
    pub fn push(&mut self, cue: CombatCue) {
        if self.queue.len() >= MAX_QUEUED_EVENTS {
            self.queue.pop_front();
            self.stats.dropped += 1;
        }
        self.queue.push_back(Queued {
            cue,
            at: Instant::now(),
        });
    }

    /// The layer's shared inbox ([`CombatInbox`]).
    #[must_use]
    pub fn inbox(&self) -> CombatInbox {
        self.inbox.clone()
    }

    /// Queues the cue of a session event (`player`: the local player, for the selection).
    pub fn handle(&mut self, event: &CombatEvent, player: Option<NetId>) {
        if let Some(c) = CombatCue::from_event(event, player) {
            self.push(c);
        }
    }

    /// A cast started (§9.1).
    pub fn cast_started(
        &mut self,
        entity: NetId,
        cast: CastKind,
        what: Option<Did>,
        target: Option<NetId>,
    ) {
        self.push(CombatCue::CastStarted {
            entity,
            cast,
            what,
            target,
        });
    }

    /// A cast was delayed: no visual changes (the bar is `combat-ui.md`'s).
    pub fn cast_delayed(&mut self, _entity: NetId, _added_ms: u32) {}

    /// A cast ended (§9.1).
    pub fn cast_ended(&mut self, entity: NetId, outcome: CastOutcome) {
        self.push(CombatCue::CastEnded { entity, outcome });
    }

    /// A hit and its result (§9.3).
    #[allow(clippy::too_many_arguments)]
    pub fn strike(
        &mut self,
        source: Option<NetId>,
        target: NetId,
        vital: Vital,
        amount: f32,
        result: HitResult,
        what: Option<Did>,
        damage_type: Option<u32>,
    ) {
        self.push(CombatCue::Strike {
            source,
            target,
            vital,
            amount,
            result,
            what,
            damage_type,
        });
    }

    /// An effect instance was applied (§9.4).
    pub fn effect_applied(&mut self, entity: NetId, instance: u64, what: Option<Did>) {
        self.push(CombatCue::EffectApplied {
            entity,
            instance,
            what,
        });
    }

    /// An effect instance was removed (§9.4).
    pub fn effect_removed(&mut self, entity: NetId, instance: u64) {
        self.push(CombatCue::EffectRemoved { entity, instance });
    }

    /// The entity entered or left combat (§9.5).
    pub fn combat_state(&mut self, entity: NetId, in_combat: bool) {
        self.push(CombatCue::CombatState { entity, in_combat });
    }

    /// The entity was defeated (§9.5).
    pub fn defeated(&mut self, entity: NetId) {
        self.push(CombatCue::Defeated { entity });
    }

    /// The entity was revived (§9.5).
    pub fn revived(&mut self, entity: NetId) {
        self.push(CombatCue::Revived { entity });
    }

    /// The local player's selection changed (`combat-ui.md` TR6: the world indicator).
    pub fn set_selection(&mut self, target: Option<NetId>) {
        self.push(CombatCue::Selection { target });
    }

    /// Whether the entity is defeated (TR4's foe set, TR39).
    #[must_use]
    pub fn is_defeated(&self, entity: NetId) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.is_defeated(EntityId(entity.0)))
    }

    // ----- picking and anchors -----

    /// Whether `entity` may be selected (`combat-ui.md` §3.1, TR1).
    #[must_use]
    pub fn selectable(&self, characters: &CharacterLayer, entity: NetId) -> bool {
        let kind = characters.entity_kind(entity);
        let body_selectable = match (kind, characters.figure_of(entity).map(|f| f.request.body)) {
            // Answered on the workers (module docs); while the answer is coming the object
            // is not offered (the lookup queues it).
            (Some(EntityKind::Object), Some(Body::Object(did))) => {
                self.records
                    .as_ref()
                    .and_then(|r| match r.body(Did(did.0)) {
                        Some(answer) => answer,
                        None => Some(false),
                    })
            }
            _ => None,
        };
        pick::eligible(pick::Eligibility {
            kind,
            body_selectable,
        })
    }

    /// The selectable session entity whose figure `ray` meets first (`combat-ui.md` §3.2
    /// TR3); boxes are converted to the ray's frame by `to_frame` (region-global → render).
    #[must_use]
    pub fn pick(
        &self,
        characters: &CharacterLayer,
        ray: &pick::Ray,
        to_frame: &dyn Fn(u8, [f64; 3]) -> Option<Vec3>,
    ) -> Option<NetId> {
        let candidates = characters.session_entities().into_iter().filter_map(|e| {
            let b = characters.entity_box(e)?;
            if !b.drawn {
                return None;
            }
            let lo = to_frame(b.region, b.min)?;
            let hi = to_frame(b.region, b.max)?;
            Some((e.0, lo, hi, self.selectable(characters, e)))
        });
        pick::pick(ray, candidates).map(NetId)
    }

    /// The box of `entity`'s figure in the frame of `to_frame`, for its anchor
    /// ([`pick::anchor_point`]).
    #[must_use]
    pub fn anchor_box(
        characters: &CharacterLayer,
        entity: NetId,
        to_frame: &dyn Fn(u8, [f64; 3]) -> Option<Vec3>,
    ) -> Option<(Vec3, Vec3)> {
        let b = characters.entity_box(entity)?;
        Some((to_frame(b.region, b.min)?, to_frame(b.region, b.max)?))
    }

    // ----- per frame -----

    fn poll_boot(&mut self) {
        let Some(rx) = &self.boot_rx else { return };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("start-up job failed".into()),
        };
        self.boot_rx = None;
        match r {
            Ok(b) => {
                self.state = Some(CombatState::new(b.vocab.clone(), self.seed));
                let b = Arc::new(b);
                let records = Arc::new(RecordCache::new(Arc::clone(&b)));
                // The layer is updated on the walker's frame thread.
                records.mark_frame_thread();
                self.records = Some(records);
                self.boot = Some(b);
                self.stats.ready = true;
            }
            Err(e) => {
                tracing::warn!("combat visuals unavailable: {e}");
                self.problem = Some(e);
            }
        }
    }

    /// One frame (module docs).
    pub fn update(&mut self, input: CombatInput<'_>) {
        self.poll_boot();
        for c in self.inbox.take() {
            self.push(c);
        }
        let CombatInput {
            now,
            player,
            characters,
            effects,
        } = input;
        characters.apply_clip_commands(effects.take_clip_commands());
        let (Some(boot), Some(records), Some(state)) =
            (self.boot.clone(), self.records.clone(), self.state.as_mut())
        else {
            if self.problem.is_some() || (self.boot_rx.is_none() && self.boot.is_none()) {
                // No data: nothing will ever play.
                self.stats.dropped += self.queue.len() as u64;
                self.queue.clear();
            }
            return;
        };
        if !effects.runtime_ready() {
            return;
        }
        let ready_since = *self.ready_since.get_or_insert_with(Instant::now);
        effects.set_execute_vocab(boot.exec);
        let extra: Vec<Did> = boot.selection_table.into_iter().collect();
        // 2. Session figures.
        let present: HashSet<u64> = characters.session_entities().iter().map(|e| e.0).collect();
        // Figures the runtime lost while they stay in the session (a region change clears
        // the runtime): they are spawned again below and get their persistent state again.
        let lost: Vec<u64> = self
            .ready
            .iter()
            .copied()
            .filter(|n| present.contains(n) && effects.session_id(NetId(*n)).is_none())
            .collect();
        let gone: Vec<u64> = self
            .ready
            .iter()
            .copied()
            .filter(|n| !present.contains(n))
            .collect();
        if !lost.is_empty() || !gone.is_empty() {
            // Room or a region may have come free: refused figures are tried again.
            self.refused.clear();
        }
        for n in lost {
            self.ready.remove(&n);
            self.moving.remove(&n);
            state.entity_lost(EntityId(n));
            if let Some((t, run)) = &mut self.selection
                && t.0 == n
            {
                *run = None;
            }
        }
        for n in gone {
            self.ready.remove(&n);
            self.moving.remove(&n);
            let mut host = Host {
                fx: effects,
                chars: characters,
                boot: &boot,
                records: &records,
            };
            state.entity_removed(&mut host, EntityId(n));
            effects.remove_session_entity(NetId(n));
            if let Some((t, run)) = &mut self.selection
                && t.0 == n
            {
                *run = None;
            }
        }
        // The state of entities the session no longer has and whose figure never ran in the
        // runtime (no tables, another region, despawned while loading) is dropped after a
        // grace time (events may name an entity just before the session lists it).
        self.absent.retain(|n, _| !present.contains(n));
        let mut expired = Vec::new();
        for e in state.known_entities() {
            if present.contains(&e.0) || self.ready.contains(&e.0) {
                continue;
            }
            let since = *self.absent.entry(e.0).or_insert(now);
            if now - since >= ABSENT_GRACE {
                expired.push(e);
            }
        }
        if !expired.is_empty() {
            let mut host = Host {
                fx: effects,
                chars: characters,
                boot: &boot,
                records: &records,
            };
            for e in expired {
                self.absent.remove(&e.0);
                state.entity_removed(&mut host, e);
            }
        }
        self.refused.retain(|n, _| present.contains(n));
        let mut fresh = Vec::new();
        self.waiting = 0;
        for &n in &present {
            if self.ready.contains(&n) {
                continue;
            }
            // Lasting refusals (cap, region, no tables, spawn error) are tried again only
            // after a change above or once per [`REFUSAL_RETRY`], and are not waited for.
            if self
                .refused
                .get(&n)
                .is_some_and(|at| now - *at < REFUSAL_RETRY && now >= *at)
            {
                continue;
            }
            // The stance of a figure that is in combat reads its worn items as soon as it
            // runs (`entity_ready`): they are answered on the workers first.
            if !records.ensure(&self.pool, &item_needs(characters, NetId(n))) {
                self.waiting += 1;
                continue;
            }
            // Its start-up scripts start clips as soon as it runs: decoded on the workers
            // first (the frame thread reads no clip, module docs).
            if !characters.script_clips_ready(NetId(n), &ClipPrefetch::default()) {
                self.waiting += 1;
                continue;
            }
            match effects.session_entity(NetId(n), characters, &extra) {
                Ok(_) => {
                    tracing::debug!(entity = n, "combat: session figure runs scripts");
                    self.ready.insert(n);
                    self.refused.remove(&n);
                    fresh.push(n);
                }
                Err(r) if r.transient() => {
                    if characters
                        .entity_effect_view(NetId(n))
                        .is_some_and(|v| v.scripts.is_some())
                    {
                        self.waiting += 1;
                    }
                }
                Err(r) => {
                    tracing::debug!(entity = n, ?r, "combat: session figure not spawned");
                    self.refused.insert(n, now);
                }
            }
        }
        // Picking's body records asked for on this thread.
        records.flush(&self.pool);
        // 3a. How many cues at the head of the queue have their records answered and their
        // clips decoded (module docs), or have waited [`MAX_CUE_WAIT`] since they were
        // queued (or since the layer became ready); the first other one stops the count.
        let wall = Instant::now();
        let mut overdue = Vec::new();
        let (ready_cues, unready) = handled_cues(
            self.queue.iter().take(MAX_EVENTS_PER_FRAME).map(|q| {
                let answered = records.ensure(&self.pool, &cue_needs(&q.cue, characters));
                let decoded = answered
                    && cue_clips(&q.cue, &boot, &records, characters)
                        .into_iter()
                        .fold(true, |all, (e, r)| {
                            characters.script_clips_ready(e, &r) && all
                        });
                let since = q.at.max(ready_since);
                if !decoded
                    && wall.saturating_duration_since(since) >= MAX_CUE_WAIT
                    && overdue.len() < 8
                {
                    overdue.push((q.cue.clone(), answered));
                }
                (decoded, since)
            }),
            wall,
        );
        for (cue, answered) in overdue {
            tracing::warn!(
                ?cue,
                answered,
                "combat cue handled without all its records or clips"
            );
        }
        self.stats.cues_unready += unready;
        let runtime_player = player.and_then(|p| effects.session_id(p));
        effects.set_runtime_player(runtime_player);
        state.set_player(player.map(|p| EntityId(p.0)));
        state.set_time(now);
        // `ScriptSystem_CurrentlyMoving` (§3.3).
        if let Some(pid) = boot.moving {
            for &n in &self.ready {
                let Some(m) = characters.entity_moving(NetId(n)) else {
                    continue;
                };
                if self.moving.get(&n) != Some(&m)
                    && let Some(id) = effects.session_id(NetId(n))
                {
                    effects.set_entity_property(id, pid, PropertyValue::Bool(m));
                    self.moving.insert(n, m);
                }
            }
        }
        let mut host = Host {
            fx: effects,
            chars: characters,
            boot: &boot,
            records: &records,
        };
        for &n in &fresh {
            state.entity_ready(&mut host, EntityId(n));
        }
        // TR6: the selection's indicator starts once its figure runs in the runtime (it may
        // have been selected before its figure was built, or the figure was spawned again).
        if let Some((t, run)) = &mut self.selection
            && fresh.contains(&t.0)
            && run.is_none_or(|r| !host.live(r))
        {
            *run = start_selection(&mut host, &boot, *t);
        }
        // 3. Events, hooks, story ends.
        for _ in 0..ready_cues {
            let Some(Queued { cue, .. }) = self.queue.pop_front() else {
                break;
            };
            self.stats.events += 1;
            tracing::debug!(?cue, "combat: cue");
            Self::apply(state, &mut host, &boot, &records, &mut self.selection, cue);
        }
        state.update(&mut host, now);
        characters.apply_clip_commands(effects.take_clip_commands());
    }

    fn apply(
        state: &mut CombatState,
        host: &mut Host<'_>,
        boot: &Boot,
        data: &RecordCache<Boot>,
        selection: &mut Option<(NetId, Option<RunId>)>,
        cue: CombatCue,
    ) {
        let id = |n: NetId| EntityId(n.0);
        match cue {
            CombatCue::CastStarted {
                entity,
                cast,
                what,
                target,
            } => state.cast_started(host, data, id(entity), cast, what, target.map(id)),
            CombatCue::CastEnded { entity, outcome } => state.cast_ended(host, id(entity), outcome),
            CombatCue::Strike {
                source,
                target,
                vital,
                amount,
                result,
                what,
                damage_type,
            } => state.vital_delta(
                host,
                data,
                source.map(id),
                id(target),
                &vital,
                amount,
                result,
                what,
                damage_type,
            ),
            CombatCue::VitalMax { entity, vital, max } => {
                state.vital_changed(id(entity), &vital, max)
            }
            CombatCue::EffectApplied {
                entity,
                instance,
                what,
            } => state.effect_applied(host, data, id(entity), instance, what),
            CombatCue::EffectRemoved { entity, instance } => {
                state.effect_removed(host, id(entity), instance);
            }
            CombatCue::CombatState { entity, in_combat } => {
                state.combat_state(host, id(entity), in_combat);
            }
            CombatCue::Defeated { entity } => state.defeated(host, id(entity)),
            CombatCue::Revived { entity } => state.revived(host, id(entity)),
            CombatCue::Selection { target } => {
                // TR6: stop the previous selection's script, start the new one's.
                if let Some((_, Some(run))) = selection.take() {
                    host.stop(run);
                }
                if let Some(t) = target {
                    let run = start_selection(host, boot, t);
                    *selection = Some((t, run));
                }
            }
        }
    }
}

/// Starts the selection script on `t` (`combat-ui.md` TR6): its `Selection_Script`, else
/// `ui_selected_fx`; `None` when its figure does not run in the runtime yet.
fn start_selection(host: &mut Host<'_>, boot: &Boot, t: NetId) -> Option<RunId> {
    let e = EntityId(t.0);
    if !host.available(e) {
        return None;
    }
    let key = boot
        .selection_pid
        .and_then(|p| host.property(e, p))
        .and_then(|v| v.as_bits())
        .and_then(|b| u32::try_from(b).ok())
        .filter(|k| *k != 0)
        .or(boot.selected_fx)?;
    host.run(e, key, &[])
}

/// Whether the body record `did` passes the §3.1 test: `IsSelectable` true and, when
/// present, `ClientIsSelectableOverride` true (`None` when the record does not read).
fn body_selectable(boot: &Boot, did: Did) -> Option<bool> {
    let set = boot.data.data().record(did).ok().flatten()?;
    let flag = |pid: Option<u32>| {
        pid.and_then(|p| set.get(p))
            .and_then(|v| v.as_f64())
            .map(|x| x != 0.0)
    };
    let [sel, over] = boot.selectable;
    // An unset `IsSelectable` reads its dictionary default; the override counts only when
    // present.
    let default = sel
        .and_then(|p| boot.data.data().default_value(p))
        .and_then(|v| v.as_f64())
        .map(|x| x != 0.0);
    Some(flag(sel).or(default).unwrap_or(true) && flag(over).unwrap_or(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cue waits for its records and clips at most [`MAX_CUE_WAIT`] from its own start:
    /// a run of cues that never become ready is handled together once they are overdue,
    /// not one wait after another; a ready cue behind a waiting one still waits (§9.6
    /// order).
    #[test]
    fn overdue_cues_are_handled_together() {
        let now = Instant::now() + MAX_CUE_WAIT * 4;
        let old = now - MAX_CUE_WAIT - Duration::from_millis(1);
        let fresh = now - Duration::from_millis(5);
        // Three cues that never got ready, all queued long ago, then a ready one.
        let cues = [(false, old), (false, old), (false, old), (true, fresh)];
        assert_eq!(handled_cues(cues.into_iter(), now), (4, 3));
        // A cue that is still within its wait stops the count; the ready one behind it
        // waits too, and the ones behind are not asked.
        let mut asked = 0;
        let cues = [(true, fresh), (false, fresh), (true, fresh), (true, fresh)];
        let got = handled_cues(cues.into_iter().inspect(|_| asked += 1), now);
        assert_eq!(got, (1, 0));
        assert_eq!(asked, 2, "evaluated up to the first waiting cue");
        assert_eq!(handled_cues(std::iter::empty(), now), (0, 0));
    }

    fn item(slot: Option<u32>, default: u32, compatible: &[u32], category: &str) -> HeldItem {
        HeldItem {
            slot,
            default_bits: slot_bit(default),
            compatible_bits: compatible
                .iter()
                .map(|k| slot_bit(*k))
                .fold(0, |a, b| a | b),
            category: category.to_owned(),
        }
    }

    /// Keys invented for the test: primary 17, secondary 18, ranged 19.
    const HANDS: Option<[u32; 2]> = Some([17, 18]);

    #[test]
    fn the_hands_follow_the_slot_then_the_default_slot() {
        // A ranged-slot bow listed first takes no hand: sword and shield.
        let items = [
            item(None, 19, &[], "Bow"),
            item(None, 17, &[17, 18], "1HSword"),
            item(None, 18, &[], "Shield"),
        ];
        let (m, o) = hand_categories(&items, HANDS);
        assert_eq!(
            stance_for(m.as_deref(), o.as_deref(), true),
            "combat_1hshield"
        );
        // An off-hand dagger by its slot alone: main hand empty.
        let items = [item(Some(18), 17, &[17, 18], "Dagger")];
        let (m, o) = hand_categories(&items, HANDS);
        assert_eq!((m.as_deref(), o.as_deref()), (None, Some("Dagger")));
        assert_eq!(stance_for(m.as_deref(), o.as_deref(), true), "combat_ua");
        // Two one-handers without slots: the second fits the secondary hand.
        let items = [
            item(None, 17, &[17, 18], "1HAxe"),
            item(None, 17, &[17, 18], "1HSword"),
        ];
        let (m, o) = hand_categories(&items, HANDS);
        assert_eq!(
            stance_for(m.as_deref(), o.as_deref(), true),
            "combat_dualwield"
        );
        // A main-hand item of another category: combat_1h.
        let items = [item(None, 17, &[], "")];
        let (m, o) = hand_categories(&items, HANDS);
        assert_eq!(stance_for(m.as_deref(), o.as_deref(), true), "combat_1h");
        // Without the hand keys the order rule applies.
        let items = [item(None, 0, &[], "Shield"), item(None, 0, &[], "Spear")];
        let (m, o) = hand_categories(&items, None);
        assert_eq!(
            stance_for(m.as_deref(), o.as_deref(), true),
            "combat_spearshield"
        );
    }

    /// Instance-ID run parameters (`CombatStrikeData_AttackerID`) name the runtime's entity,
    /// so a Property target over them reaches the attacker's figure.
    #[test]
    fn instance_id_parameters_become_runtime_ids() {
        let params = [
            (1, PropertyValue::InstanceId(7)),
            (2, PropertyValue::InstanceId(8)),
            (3, PropertyValue::Float(0.5)),
        ];
        let out = runtime_params(&params, |e| (e.0 == 7).then_some(EntityId((1 << 32) + 4)));
        assert_eq!(
            out,
            vec![
                (1, PropertyValue::InstanceId((1 << 32) + 4)),
                (2, PropertyValue::InstanceId(0)),
                (3, PropertyValue::Float(0.5)),
            ]
        );
    }
}
