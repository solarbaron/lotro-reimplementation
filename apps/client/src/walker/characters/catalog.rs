// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-install character vocabulary, built once on the world start-up thread: the playable
//! bodies of the race table with their species labels (`appearance.md` §2,
//! `skeleton-anim.md` §2.2), the animation layers that idle, walk and run play on
//! (`script-table.md` §6.2), the role of each weenie type (`appearance.md` §7.1), and the
//! assembly and script vocabularies the figure jobs need.
//!
//! Labels are the specs' (looked up in the install's enum mappers at run time); no game
//! value is hard-coded. A missing piece is a [`CharacterCatalog::problems`] entry, never
//! an error: the walker then draws what it can.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_dat::ArchiveSet;
use tessera_formats::avatar::{RacePids, RaceTable};
use tessera_formats::script_table::resolve::ScriptVocabulary;
use tessera_formats::{Did, EnumMapper};
use tessera_mesh::assembly::AssemblyVocabulary;
use tessera_net::{Appearance, Body, DataId, Equipped};
use tessera_props::{EnumSource, GameData};

use crate::avatar::{AvatarBody, AvatarSpec, SEX_FEMALE, SEX_MALE, Species};

/// The game data the character jobs read (shared by every worker).
pub type Data = GameData<Arc<ArchiveSet>>;

/// One playable body of the race table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyEntry {
    /// Species key.
    pub species: u32,
    /// Its `RaceTable_Race` label (empty when the mapper lacks it).
    pub label: String,
    /// Sex value.
    pub sex: u32,
}

/// How the walker shows a weenie placement (`walker.md` §10 Decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeenieRole {
    /// `Monster`: an animated creature or humanoid.
    Creature,
    /// `RealNPC`: an animated NPC.
    Npc,
    /// `Door`: a static figure (its `PhysObj` drawn rigid).
    Door,
    /// Anything else (`Entity` housing hooks, `DistantBattleUnit`, unknown types): a marker,
    /// drawn only with `--markers`.
    Marker,
}

/// The vocabulary (module docs).
#[derive(Debug)]
pub struct CharacterCatalog {
    /// The game data.
    pub data: Arc<Data>,
    /// Playable bodies in race-table order, one per (species, sex).
    pub bodies: Vec<BodyEntry>,
    /// `AnimationScript_Layer` key of `idle`.
    pub idle_layer: Option<u32>,
    /// `AnimationScript_Layer` key of `movement`.
    pub movement_layer: Option<u32>,
    /// Roles of the `WeenieType` keys whose labels are known.
    pub weenie_roles: HashMap<u32, WeenieRole>,
    /// Assembly vocabulary (`None` when the registry could not be read).
    pub assembly: Option<AssemblyVocabulary>,
    /// Script vocabulary (`None` when a required name is missing).
    pub scripts: Option<ScriptVocabulary>,
    /// The script key labelled `Chargen` among the values of `ScriptTable_ScriptID`: the
    /// start-up script the character screens add to their figures
    /// (`character-screens.md` §2.1.4, CS28); `None` when the install lacks the label.
    pub chargen_script: Option<u32>,
    /// What is missing, for the log and the HUD.
    pub problems: Vec<String>,
}

/// The enum mapper a property's values are labelled by.
fn property_mapper(data: &Data, name: &str) -> Option<Did> {
    let registry = data.registry().ok()?;
    match registry.enum_source(registry.pid(name)?)? {
        EnumSource::Mapper(m) => Some(m),
        EnumSource::DidMap(_) => None,
    }
}

/// The key of `label` among the values of property `name`.
fn property_key(data: &Data, name: &str, label: &str) -> Option<u32> {
    let m = property_mapper(data, name)?;
    EnumMapper::key_in_chain_from(m, |d| data.enum_mapper(d).ok().flatten(), label)
}

/// The label of `key` in `mapper` or its bases.
fn enum_label(data: &Data, mapper: Did, key: u32) -> Option<String> {
    let mut did = mapper;
    for _ in 0..16 {
        if did.is_null() {
            return None;
        }
        let m = data.enum_mapper(did).ok().flatten()?;
        if let Some(n) = m.name(key) {
            return Some(n.to_owned());
        }
        did = m.base;
    }
    None
}

impl CharacterCatalog {
    /// Builds the catalog (reads records: call it on a start-up thread).
    #[must_use]
    pub fn build(set: Arc<ArchiveSet>) -> Self {
        let data = Arc::new(GameData::new(set));
        let mut out = Self {
            data: Arc::clone(&data),
            bodies: Vec::new(),
            idle_layer: None,
            movement_layer: None,
            weenie_roles: HashMap::new(),
            assembly: None,
            scripts: None,
            chargen_script: None,
            problems: Vec::new(),
        };
        let registry = match data.registry() {
            Ok(r) => Arc::clone(r),
            Err(e) => {
                out.problems.push(format!("property registry: {e}"));
                return out;
            }
        };
        let dict = registry.dictionary();
        // Race table (skeleton-anim.md §2.2).
        match data.lookup_path(&["WEENIECONTENT", "RaceTableDirectory"]) {
            Ok(Some(dir)) => match RacePids::from_names(dict) {
                Ok(pids) => {
                    match RaceTable::read(dir, registry.as_ref(), &pids, |d| data.read(d)) {
                        Ok(t) => {
                            let mapper = property_mapper(&data, "RaceTable_Race");
                            for e in &t.entries {
                                if out
                                    .bodies
                                    .iter()
                                    .any(|b| b.species == e.species && b.sex == e.sex)
                                {
                                    continue;
                                }
                                out.bodies.push(BodyEntry {
                                    species: e.species,
                                    label: mapper
                                        .and_then(|m| enum_label(&data, m, e.species))
                                        .unwrap_or_default(),
                                    sex: e.sex,
                                });
                            }
                        }
                        Err(e) => out.problems.push(format!("race table {dir}: {e}")),
                    }
                }
                Err(e) => out.problems.push(format!("race-table properties: {e}")),
            },
            Ok(None) => out
                .problems
                .push("no race table (RaceTableDirectory)".into()),
            Err(e) => out.problems.push(format!("race table: {e}")),
        }
        out.idle_layer = property_key(&data, "AnimationScript_Layer", "idle");
        out.movement_layer = property_key(&data, "AnimationScript_Layer", "movement");
        if out.idle_layer.is_none() || out.movement_layer.is_none() {
            out.problems
                .push("animation layers idle/movement not found by label".into());
        }
        for (label, role) in [
            ("Monster", WeenieRole::Creature),
            ("RealNPC", WeenieRole::Npc),
            ("Door", WeenieRole::Door),
        ] {
            if let Some(k) = property_key(&data, "WeenieType", label) {
                out.weenie_roles.insert(k, role);
            }
        }
        // Read by its label at run time (§2.1.4: key 0x100004AB of the script-key mapper in
        // the reference install).
        out.chargen_script = property_key(&data, "ScriptTable_ScriptID", "Chargen");
        if out.chargen_script.is_none() {
            out.problems
                .push("script key Chargen not found by label (creation-screen stances)".into());
        }
        match AssemblyVocabulary::from_game_data(&data) {
            Ok(v) => out.assembly = Some(v),
            Err(e) => out.problems.push(format!("assembly vocabulary: {e}")),
        }
        match ScriptVocabulary::from_data(dict, |d| {
            data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
        }) {
            Ok(v) => out.scripts = Some(v),
            Err(e) => out.problems.push(format!("script vocabulary: {e}")),
        }
        for p in &out.problems {
            tracing::warn!("characters: {p}");
        }
        tracing::info!(
            bodies = out.bodies.len(),
            weenie_roles = out.weenie_roles.len(),
            "character catalog ready"
        );
        out
    }

    /// The body of (`species`, `sex`), if the race table has it.
    #[must_use]
    pub fn body(&self, species: u32, sex: u32) -> Option<&BodyEntry> {
        self.bodies
            .iter()
            .find(|b| b.species == species && b.sex == sex)
    }

    /// The appearance of an `--avatar` spec: a playable body by label or key (the label
    /// matched case-insensitively), or an object body, with the spec's items.
    ///
    /// # Errors
    ///
    /// An unknown species label or a (species, sex) pair without a body, as text.
    pub fn appearance(&self, spec: &AvatarSpec) -> Result<Appearance, String> {
        let body = match &spec.body {
            AvatarBody::Object(d) => Body::Object(DataId(*d)),
            AvatarBody::Playable { species, sex } => {
                let key = match species {
                    Species::Key(k) => *k,
                    Species::Label(l) => self
                        .bodies
                        .iter()
                        .find(|b| b.label.eq_ignore_ascii_case(l))
                        .map(|b| b.species)
                        .ok_or_else(|| {
                            let mut known: Vec<String> =
                                self.bodies.iter().map(|b| b.label.to_lowercase()).collect();
                            known.sort();
                            known.dedup();
                            format!("unknown species {l:?} (known: {})", known.join(", "))
                        })?,
                };
                if !self.bodies.is_empty() && self.body(key, *sex).is_none() {
                    return Err(format!(
                        "no playable body for species {key:#x}, sex {}",
                        sex_text(*sex)
                    ));
                }
                Body::Character {
                    species: key,
                    sex: *sex,
                }
            }
        };
        let mut a = Appearance::new(body);
        for i in &spec.items {
            a.equipment.push(Equipped {
                item: DataId(i.did),
                dye: i.dye,
                secondary_dye: i.dye2,
                stowed: i.stowed,
                slot: None,
            });
        }
        Ok(a)
    }

    /// Appearances for a crowd: every playable body once, in race-table order.
    #[must_use]
    pub fn crowd(&self) -> Vec<Appearance> {
        self.bodies
            .iter()
            .map(|b| {
                Appearance::new(Body::Character {
                    species: b.species,
                    sex: b.sex,
                })
            })
            .collect()
    }

    /// The role of a weenie placement with `WeenieType` key `weenie_type`.
    #[must_use]
    pub fn weenie_role(&self, weenie_type: Option<u32>) -> WeenieRole {
        weenie_type
            .and_then(|k| self.weenie_roles.get(&k).copied())
            .unwrap_or(WeenieRole::Marker)
    }
}

/// `m`, `f` or the value in hex.
#[must_use]
pub fn sex_text(sex: u32) -> String {
    match sex {
        SEX_MALE => "m".into(),
        SEX_FEMALE => "f".into(),
        other => format!("{other:#x}"),
    }
}
