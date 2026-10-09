// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Gameplay" view: read-only cards of the gameplay records that
//! `tessera_formats::gameplay` decodes, built on asset workers.
//!
//! A card shows a record's decoded fields by dictionary label, with DIDs as links: the
//! typed summary of its family, derived values the specs define (item value and DPS from
//! the value and DPS tables, damage range, armour from its progression, the creature
//! panel level), tables of nested entries, then every property of the set grouped by the
//! labels the family reads, and the rest. Text values are rendered from the user's own
//! string tables; nothing is stored or bundled.
//!
//! | Module | Records | Spec |
//! |---|---|---|
//! | [`card`] | sections, property rows, bounded dumps | — |
//! | `item` | item cards with icon layers (the icon view's composer) and table stats | `items.md` |
//! | `combat` | skill, effect and trait cards; other combat objects | `combat-data.md` |
//! | `creature` | creature templates, AI records, the encounter table (0x14) | `creatures.md` |
//! | `quest` | quests and deeds, the other quest-family kinds | `quests.md` |
//! | `economy` | loot lists, recipes, barter, vendors, currency | `economy.md` |
//! | `map` | map kinds, map-note properties, the places, map-note and quest-note tables (0x0E) | `map-data.md` |
//! | `particle` | particle systems (0x39) and fork systems (0x44) | `particles.md` |
//! | [`view`] | the UI | — |
//!
//! Records are recognised as the specs say: combat objects by class ancestry over the
//! class library (with the family signatures as a fallback when the library is missing),
//! creature templates and items by `WeenieType` labels and signatures, the other kinds by
//! their characteristic properties, the 0x0E tables by their UNIQUEDB labels. No DID or
//! class index of one install is written here.

pub mod card;
mod combat;
mod creature;
mod economy;
mod item;
mod map;
mod particle;
mod quest;
pub mod view;

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use tessera_assets::{AssetKey, Decoder, JobContext};
use tessera_formats::gameplay::combat::{CombatClasses, CombatPids};
use tessera_formats::gameplay::common::{
    ObjectRoot, Pid, UNIQUEDB_LABEL, WEENIECONTENT_LABEL, is_object_did,
};
use tessera_formats::gameplay::creatures::{CreaturePids, LabelIndex, WeenieKinds};
use tessera_formats::gameplay::economy::EconomyReader;
use tessera_formats::gameplay::items::{ItemSchema, ItemWeenieKinds};
use tessera_formats::gameplay::map::{MapPids, MapWeenieKeys};
use tessera_formats::gameplay::quests::{QuestLabels, QuestSingletons};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, PropertySet, PropertySetRecord, object_property_set_did,
};
use tessera_props::{EnumSource, PropertyRegistry};
use tessera_wstate::{ClassLibrary, LIBRARY_DID};

use card::{Card, Labels, property_section, remaining_section};
pub use card::{Row, Section};

use crate::data::DataContext;
use crate::error::ExplorerError;

/// The family a card belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    /// Items and item tables (`items.md`).
    Item,
    /// Skills, effects, traits and the other combat objects (`combat-data.md`).
    Combat,
    /// Creature templates and AI records (`creatures.md`).
    Creature,
    /// Quests, deeds and the quest family (`quests.md`).
    Quest,
    /// Loot, crafting, vendors, barter and currency (`economy.md`).
    Economy,
    /// Map kinds and the 0x0E map tables (`map-data.md`).
    Map,
    /// Particle and fork systems (`particles.md`).
    Particle,
    /// A game object no family recognises: its properties only.
    Object,
}

impl Family {
    /// Display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Combat => "combat",
            Self::Creature => "creature",
            Self::Quest => "quest",
            Self::Economy => "economy",
            Self::Map => "map",
            Self::Particle => "particle",
            Self::Object => "object",
        }
    }
}

/// A gameplay card ready to draw.
#[derive(Debug, Clone)]
pub struct GameplayProduct {
    /// The selected DID.
    pub did: u32,
    /// The game object the card describes (the 0x70–0x76 DID for an object or its set).
    pub object: Option<u32>,
    /// The property set read, if any.
    pub property_set: Option<u32>,
    /// The family.
    pub family: Family,
    /// The kind within the family (for example "skill", "trophy list").
    pub kind: String,
    /// The record's name, rendered from the user's string tables, when it has one.
    pub title: Option<String>,
    /// Root class of the object record.
    pub root_class: Option<u32>,
    /// The card's sections.
    pub sections: Vec<Section>,
    /// Classification notes and the errors of typed readers.
    pub notes: Vec<String>,
}

/// Whether the gameplay view applies to `did`: game objects and their property sets
/// (0x70–0x7F), the encounter table (0x14), particle (0x39) and fork systems (0x44), and
/// the 0x0E tables (only the map tables decode).
#[must_use]
pub fn applies_to(did: u32) -> bool {
    matches!(did >> 24, 0x70..=0x7F | 0x14 | 0x39 | 0x44 | 0x0E)
}

/// The labels, kind tables and class library of one install, built once on a worker.
pub(crate) struct Schemas {
    pub registry: Arc<PropertyRegistry>,
    pub items: ItemSchema,
    pub item_weenie: ItemWeenieKinds,
    pub combat: CombatPids,
    pub combat_classes: CombatClasses,
    pub library: Option<ClassLibrary>,
    pub creatures: CreaturePids,
    pub creature_weenie: WeenieKinds,
    pub quests: QuestLabels,
    pub singletons: QuestSingletons,
    pub economy: EconomyReader,
    pub map: MapPids,
    pub map_weenie: MapWeenieKeys,
    pub uniquedb: Option<Arc<DataIdMap>>,
    /// Problems met while building (shown on every card).
    pub problems: Vec<String>,
}

/// Longest enum-mapper base chain followed.
const MAX_MAPPER_CHAIN: usize = 16;

/// The mapper the dictionary names for `pid`, then its bases (nearest first).
fn mapper_chain(
    ctx: &DataContext,
    registry: &PropertyRegistry,
    pid: Pid,
    problems: &mut Vec<String>,
) -> Vec<Arc<EnumMapper>> {
    let mut out: Vec<Arc<EnumMapper>> = Vec::new();
    let Some(Some(EnumSource::Mapper(first))) = pid.id().map(|p| registry.enum_source(p)) else {
        problems.push(format!("{} has no enum mapper", pid.label()));
        return out;
    };
    let mut next = first;
    let mut seen: HashSet<u32> = HashSet::new();
    while !next.is_null() && out.len() < MAX_MAPPER_CHAIN && seen.insert(next.0) {
        match ctx.game.enum_mapper(next) {
            Ok(Some(m)) => {
                next = m.base;
                out.push(m);
            }
            Ok(None) => break,
            Err(e) => {
                problems.push(format!("enum mapper {next}: {e}"));
                break;
            }
        }
    }
    out
}

impl Schemas {
    fn build(ctx: &DataContext) -> Result<Self, ExplorerError> {
        let registry = Arc::clone(ctx.game.registry()?);
        let dict = registry.dictionary();
        let mut problems = Vec::new();
        let items = ItemSchema::resolve(dict);
        let chain = mapper_chain(ctx, &registry, items.item.weenie_type, &mut problems);
        let item_weenie = ItemWeenieKinds::from_mapper_chain(&chain);
        let creature_weenie = WeenieKinds::from_mapper_chain(&chain);
        let map_weenie = MapWeenieKeys::from_mapper_chain(&chain);
        let library = match ctx.game.read(LIBRARY_DID) {
            Ok(Some(bytes)) => match ClassLibrary::parse(&bytes) {
                Ok(l) => Some(l),
                Err(e) => {
                    problems.push(format!("class library: {e}; combat objects by signature"));
                    None
                }
            },
            Ok(None) => {
                problems.push("no class library; combat objects by signature".into());
                None
            }
            Err(e) => {
                problems.push(format!("class library: {e}; combat objects by signature"));
                None
            }
        };
        let singletons = match ctx.game.did_map_by_label(WEENIECONTENT_LABEL) {
            Ok(Some(m)) => QuestSingletons::from_weenie_content(&m),
            Ok(None) => QuestSingletons::default(),
            Err(e) => {
                problems.push(format!("{WEENIECONTENT_LABEL}: {e}"));
                QuestSingletons::default()
            }
        };
        let uniquedb = match ctx.game.did_map_by_label(UNIQUEDB_LABEL) {
            Ok(m) => m,
            Err(e) => {
                problems.push(format!("{UNIQUEDB_LABEL}: {e}"));
                None
            }
        };
        let labels = LabelIndex::from_dictionary(dict);
        Ok(Self {
            items,
            item_weenie,
            combat: CombatPids::resolve(dict),
            combat_classes: CombatClasses::observed(),
            library,
            creatures: CreaturePids::resolve(&labels),
            creature_weenie,
            quests: QuestLabels::from_dictionary(dict),
            singletons,
            economy: EconomyReader::new(dict),
            map: MapPids::resolve(dict),
            map_weenie,
            uniquedb,
            problems,
            registry,
        })
    }
}

/// What a family module contributes to an object card.
#[derive(Debug, Default)]
pub(crate) struct Built {
    pub kind: String,
    pub title: Option<String>,
    pub card: Card,
    /// Property groups the family reads, shown after its own sections.
    pub groups: Vec<(&'static str, Vec<Pid>)>,
    pub notes: Vec<String>,
}

/// The context of one card build.
pub(crate) struct Cx<'a> {
    pub ctx: &'a DataContext,
    pub s: &'a Schemas,
    pub labels: Labels<'a>,
}

impl Cx<'_> {
    /// The property set of another game object (a table the card evaluates), or why not.
    pub fn object_set(&self, did: Did) -> Result<PropertySet, String> {
        let set_did = object_property_set_did(did).ok_or_else(|| format!("{did} has no set"))?;
        read_set(self.ctx, self.s, set_did).map_err(|e| e.to_string())
    }
}

/// Reads and decodes property set `set_did` (bytes after the set are allowed).
fn read_set(ctx: &DataContext, s: &Schemas, set_did: Did) -> Result<PropertySet, ExplorerError> {
    let bytes = ctx
        .game
        .read(set_did)?
        .ok_or(ExplorerError::NotFound(set_did.0))?;
    let (rec, trailing) = PropertySetRecord::parse_prefix(&bytes, s.registry.dictionary())?;
    if rec.did != set_did {
        return Err(tessera_formats::FormatError::DidMismatch {
            expected: set_did.0,
            found: rec.did.0,
        }
        .into());
    }
    if trailing > 0 {
        tracing::debug!(%set_did, trailing, "bytes after the property set");
    }
    Ok(rec.set)
}

/// A decoded game object: its DID, root class and property set.
pub(crate) struct Object<'a> {
    pub did: Did,
    pub root: Option<u32>,
    pub set: &'a PropertySet,
}

/// Builds the card of `did`.
pub(crate) fn build(
    ctx: &DataContext,
    s: &Schemas,
    did: u32,
    job: &JobContext<'_>,
) -> Result<GameplayProduct, ExplorerError> {
    let labels = Labels {
        registry: Some(&s.registry),
        game: Some(&ctx.game),
        strings: ctx.strings.as_ref(),
    };
    let cx = Cx { ctx, s, labels };
    let mut product = match did >> 24 {
        0x70..=0x7F => object_product(&cx, did, job)?,
        0x39 | 0x44 => particle::system_product(&cx, did)?,
        0x14 => creature::encounter_product(&cx, did)?,
        0x0E => map::table_product(&cx, did)?,
        _ => {
            return Err(ExplorerError::NotApplicable(format!(
                "{did:#010x} is not a gameplay record"
            )));
        }
    };
    product.notes.extend(s.problems.iter().cloned());
    Ok(product)
}

/// An empty product of `family`.
pub(crate) fn product(did: u32, family: Family, kind: impl Into<String>) -> GameplayProduct {
    GameplayProduct {
        did,
        object: None,
        property_set: None,
        family,
        kind: kind.into(),
        title: None,
        root_class: None,
        sections: Vec::new(),
        notes: Vec::new(),
    }
}

fn object_product(
    cx: &Cx<'_>,
    did: u32,
    job: &JobContext<'_>,
) -> Result<GameplayProduct, ExplorerError> {
    // 0x79–0x7F are the sets of 0x70–0x76; 0x78 sets stand alone (`misc-records.md` §12).
    let (object, set_did) = match did >> 24 {
        0x70..=0x77 => (Some(Did(did)), object_property_set_did(Did(did))),
        0x78 => (None, Some(Did(did))),
        _ => (Some(Did(did.wrapping_sub(0x0900_0000))), Some(Did(did))),
    };
    let mut notes = Vec::new();
    let root = match object {
        Some(o) if is_object_did(o) => match cx.ctx.game.read(o) {
            Ok(Some(bytes)) => match ObjectRoot::read_expecting(&bytes, o) {
                Ok(r) => Some(r.root_class),
                Err(e) => {
                    notes.push(format!("object record: {e}"));
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                notes.push(format!("object record: {e}"));
                None
            }
        },
        _ => None,
    };
    let set = match set_did {
        Some(sd) => match read_set(cx.ctx, cx.s, sd) {
            Ok(set) => set,
            Err(ExplorerError::NotFound(_)) => {
                notes.push("no property set (no properties)".into());
                PropertySet::default()
            }
            Err(e) => return Err(e),
        },
        None => PropertySet::default(),
    };
    if job.is_cancelled() {
        return Err(ExplorerError::Cancelled);
    }
    let obj = Object {
        did: object.unwrap_or(Did(did)),
        root,
        set: &set,
    };
    let mut recognised: Vec<(Family, Built)> = Vec::new();
    if let Some(b) = combat::card(cx, &obj) {
        recognised.push((Family::Combat, b));
    }
    if let Some(b) = creature::template_card(cx, &obj) {
        recognised.push((Family::Creature, b));
    }
    if let Some(b) = item::card(cx, &obj) {
        recognised.push((Family::Item, b));
    }
    if let Some(b) = quest::card(cx, &obj) {
        recognised.push((Family::Quest, b));
    }
    if let Some(b) = economy::card(cx, &obj) {
        recognised.push((Family::Economy, b));
    }
    if let Some(b) = creature::ai_card(cx, &obj) {
        recognised.push((Family::Creature, b));
    }
    if let Some(b) = map::card(cx, &obj) {
        recognised.push((Family::Map, b));
    }
    let mut iter = recognised.into_iter();
    let (family, mut built) = iter.next().unwrap_or_else(|| {
        (
            Family::Object,
            Built {
                kind: "unclassified object".into(),
                ..Built::default()
            },
        )
    });
    for (f, b) in iter {
        built
            .notes
            .push(format!("also recognised as {} ({})", b.kind, f.name()));
    }
    // Sections every object may carry, whatever its family.
    economy::vendor_sections(cx, &obj, &mut built.card);
    map::object_sections(cx, &obj, &mut built.card);
    let mut shown: HashSet<u32> = HashSet::new();
    let mut sections = built.card.sections;
    for (title, pids) in &built.groups {
        let s = property_section(*title, &cx.labels, &set, pids, &mut shown);
        if !s.rows.is_empty() {
            sections.push(s);
        }
    }
    let rest = remaining_section(
        if built.groups.is_empty() {
            "Properties"
        } else {
            "Other properties"
        },
        &cx.labels,
        &set,
        &shown,
    );
    if !rest.rows.is_empty() {
        sections.push(rest);
    }
    notes.extend(built.notes);
    Ok(GameplayProduct {
        did,
        object: object.map(|d| d.0),
        property_set: set_did.map(|d| d.0),
        family,
        kind: built.kind,
        title: built.title,
        root_class: root,
        sections,
        notes,
    })
}

/// Builds gameplay cards on the asset workers.
pub struct GameplayDecoder {
    ctx: Arc<DataContext>,
    schemas: OnceLock<Result<Arc<Schemas>, String>>,
}

impl std::fmt::Debug for GameplayDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameplayDecoder").finish_non_exhaustive()
    }
}

impl GameplayDecoder {
    /// A decoder over `ctx`; the install's schemas are built on first use.
    #[must_use]
    pub fn new(ctx: Arc<DataContext>) -> Self {
        Self {
            ctx,
            schemas: OnceLock::new(),
        }
    }

    fn schemas(&self) -> Result<Arc<Schemas>, ExplorerError> {
        self.schemas
            .get_or_init(|| {
                Schemas::build(&self.ctx)
                    .map(Arc::new)
                    .map_err(|e| e.to_string())
            })
            .clone()
            .map_err(|e| ExplorerError::NotApplicable(format!("gameplay schemas: {e}")))
    }

    /// Builds the card of `did` (tests and tools; the view goes through the asset store).
    pub fn card(&self, did: u32, job: &JobContext<'_>) -> Result<GameplayProduct, ExplorerError> {
        let s = self.schemas()?;
        build(&self.ctx, &s, did, job)
    }
}

impl Decoder for GameplayDecoder {
    type Output = GameplayProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-gameplay";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<GameplayProduct, ExplorerError> {
        self.card(key.did, job)
    }

    fn size_of(&self, v: &GameplayProduct) -> usize {
        v.sections
            .iter()
            .flat_map(|s| &s.rows)
            .map(|r| r.cells.iter().map(String::len).sum::<usize>() + 64)
            .sum::<usize>()
            + 512
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_applies_to_gameplay_prefixes() {
        for did in [
            0x7000_0001,
            0x7900_0001,
            0x7800_0001,
            0x1400_0001,
            0x3900_0001,
        ] {
            assert!(applies_to(did), "{did:#x}");
        }
        for did in [0x4100_0001, 0x2500_0001, 0x0100_0001] {
            assert!(!applies_to(did), "{did:#x}");
        }
        assert_eq!(Family::Combat.name(), "combat");
    }
}
