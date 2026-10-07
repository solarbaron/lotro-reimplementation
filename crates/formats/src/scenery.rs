// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenery records: terrain-scene tables (0x12) and scene-choice tables (0x11) named by the
//! region descriptor (`scenery.md` §4.2–§4.3), and scene records (0x02, `scenery.md` §5).
//!
//! The chain (`scenery.md` §2): a coarse-grid word gives a terrain-type id and a slot; the
//! region's [`TerrainSceneTable`] maps them to a scene key; the region's
//! [`SceneChoiceTable`] maps the key to weighted [`Scene`] DIDs; a scene holds placement
//! nodes whose children are objects (0x47), nested nodes or references to other scenes.
//!
//! Every decoder consumes its record exactly and treats the input as hostile
//! (`scenery.md` §13.1): every count is bounded by the remaining bytes, kept data is
//! charged to the reader's memory budget, placement nodes nest at most
//! [`MAX_PLACEMENT_NODE_DEPTH`] levels, child types outside 1–3 are rejected, and NaN or
//! infinite floats in node fields are malformed (the `z_range` bounds may be infinite).
//! Scene references between records are followed only by [`SceneReferenceChecker`], which
//! refuses cycles and paths deeper than [`MAX_SCENE_REFERENCE_DEPTH`].
//!
//! Values whose meaning is OPEN in the spec are decoded and kept for tools. `kind` and
//! `unknown_f` of terrain entries and the choice `flag` are accepted whatever their value
//! (Tessera Decision of §4.2); the scene's `flag_a` is a bool8 and accepts 0 or 1 (Tessera
//! Decision of §5.1). The presence bytes `has_default` and `has_default_pair` of §4.2 are
//! read as bool8 too (0 or 1; `scenery.md` §4.2 OPEN on their type).

use std::borrow::Borrow;
use std::collections::HashMap;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader, hash_index_bytes, index_u32};
use crate::record::{Record, read_prologue};

/// Type prefix of scene records.
pub const SCENE_PREFIX: u8 = 0x02;
/// Type prefix of scene-choice tables.
pub const SCENE_CHOICE_TABLE_PREFIX: u8 = 0x11;
/// Type prefix of terrain-scene tables.
pub const TERRAIN_SCENE_TABLE_PREFIX: u8 = 0x12;

/// Most levels of placement nodes the scene decoder accepts, the frill layer or a group
/// child being level 1 (decoder limit; nodes nest at most 2 deep in the observed install,
/// `scenery.md` §5.4). Deeper nesting fails with [`FormatError::DepthLimit`].
pub const MAX_PLACEMENT_NODE_DEPTH: usize = 16;
/// Largest [`ChildRef::depth`] of a decoded scene: an object or scene reference held by a
/// node at level [`MAX_PLACEMENT_NODE_DEPTH`]. Node children are at most
/// [`MAX_PLACEMENT_NODE_DEPTH`].
pub const MAX_CHILD_DEPTH: usize = MAX_PLACEMENT_NODE_DEPTH + 1;
/// Most scenes on one scene-reference path that [`SceneReferenceChecker`] follows, the
/// starting scene included (Tessera Decision of `scenery.md` §13.1: a cap of 16; the
/// observed maximum is 5).
pub const MAX_SCENE_REFERENCE_DEPTH: usize = 16;
/// Size of a placement node's fixed part, before its children (`scenery.md` §5.3).
pub const PLACEMENT_NODE_FIXED_LEN: usize = 187;
/// Smallest stored placement node: the fixed part, no children and the trailing f32.
pub const MIN_PLACEMENT_NODE_LEN: usize = PLACEMENT_NODE_FIXED_LEN + 4;
/// Most bytes of a terrain-entry name this decoder reads: the one-byte length form only
/// (`scenery.md` §4.2 OPEN: a first length byte of 0x80 or more is rejected).
pub const MAX_TERRAIN_ENTRY_NAME_LEN: usize = 0x7F;

/// `scale_mode` value of a uniform random scale (`scenery.md` §5.3, §7.4 step 3).
pub const SCALE_MODE_UNIFORM: u8 = 4;

/// Smallest stored child: the type byte and a DID.
const MIN_CHILD_LEN: usize = 5;
/// Smallest stored terrain entry: empty name, kind, id, `unknown_f`, `has_default_pair`,
/// `pair_count`.
const MIN_TERRAIN_ENTRY_LEN: usize = 1 + 4 + 4 + 4 + 1 + 4;
/// Stored size of a (scene key, slot) pair.
const PAIR_LEN: usize = 8;
/// Stored size of a scene-choice entry header (key, n).
const MIN_CHOICE_ENTRY_LEN: usize = 8;
/// Stored size of one choice (DID, weight, flag).
const CHOICE_LEN: usize = 9;
/// Stored size of a group header (id, n).
const MIN_GROUP_LEN: usize = 8;

/// Mask of the terrain-type id in a coarse-grid scenery word (`scenery.md` §3.1, bits 0–7).
pub const WORD_TERRAIN_MASK: u32 = 0xFF;
/// Shift of the scene slot in a coarse-grid scenery word (bits 8–14).
pub const WORD_SLOT_SHIFT: u32 = 8;
/// Mask of the scene slot after shifting (7 bits).
pub const WORD_SLOT_MASK: u32 = 0x7F;

// ---------------------------------------------------------------------------------------
// Terrain-scene table (0x12)
// ---------------------------------------------------------------------------------------

/// A (scene key, slot) pair of a terrain entry (`scenery.md` §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ScenePair {
    /// Key into the region's scene-choice table; 0 means no scene.
    pub scene_key: u32,
    /// The slot number this pair answers (bits 8–14 of the coarse word).
    pub slot: u32,
}

impl ScenePair {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            scene_key: r.u32()?,
            slot: r.u32()?,
        })
    }

    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.scene_key.to_le_bytes());
        out.extend_from_slice(&self.slot.to_le_bytes());
    }
}

/// A terrain entry of a terrain-scene table (`scenery.md` §4.2). The default entry and the
/// listed entries share this layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TerrainSceneEntry {
    /// Internal label of a scene zone or ground type (Latin-1, at most 127 bytes).
    pub name: String,
    /// Meaning OPEN (`scenery.md` §4.2 callout); not used by placement.
    pub kind: u32,
    /// Terrain-type id matched against bits 0–7 of the coarse word (0 in the default entry).
    pub id: u32,
    /// Meaning OPEN (0.7071 in every observed entry); not used by placement.
    pub unknown_f: f32,
    /// The pair used when no pair has the vertex's slot. Its presence byte
    /// `has_default_pair` is read as a bool8 (0 or 1; `scenery.md` §4.2 OPEN).
    pub default_pair: Option<ScenePair>,
    /// The pairs in stored order.
    pub pairs: Vec<ScenePair>,
}

impl TerrainSceneEntry {
    /// Reads one entry. `has_default_pair` is read as a bool8 (`scenery.md` §4.2 OPEN on
    /// its type), so a value of 2 or more fails with [`FormatError::InvalidBool`].
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let offset = r.position();
        let len = r.peek(1)?[0];
        if usize::from(len) > MAX_TERRAIN_ENTRY_NAME_LEN {
            return Err(FormatError::Unsupported {
                what: "terrain-scene entry name length form",
                offset,
                value: u32::from(len),
            });
        }
        let name = r.short_string()?;
        let kind = r.u32()?;
        let id = r.u32()?;
        let unknown_f = r.f32()?;
        let default_pair = if r.bool8()? {
            Some(ScenePair::read(r)?)
        } else {
            None
        };
        let count = r.u32()?;
        let mut pairs = r.alloc_vec(count, PAIR_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let p = ScenePair::read(r)?;
            r.push(&mut pairs, p, count)?;
        }
        Ok(Self {
            name,
            kind,
            id,
            unknown_f,
            default_pair,
            pairs,
        })
    }

    /// Appends the stored form. A name that [`TerrainSceneTable::new`] would reject is
    /// written lossily: characters above U+00FF become `?` and bytes past
    /// [`MAX_TERRAIN_ENTRY_NAME_LEN`] are dropped.
    fn write(&self, out: &mut Vec<u8>) {
        let name: Vec<u8> = self
            .name
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .take(MAX_TERRAIN_ENTRY_NAME_LEN)
            .collect();
        out.push(u8::try_from(name.len()).unwrap_or(0x7F));
        out.extend_from_slice(&name);
        out.extend_from_slice(&self.kind.to_le_bytes());
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.unknown_f.to_le_bytes());
        match self.default_pair {
            Some(p) => {
                out.push(1);
                p.write(out);
            }
            None => out.push(0),
        }
        out.extend_from_slice(&len_u32(self.pairs.len()).to_le_bytes());
        for p in &self.pairs {
            p.write(out);
        }
    }

    /// The pair for `slot` (`scenery.md` §4.2 lookup step 2): the first pair whose `slot`
    /// equals it (by value, not by position), else the default pair, else `None` (the
    /// vertex has no scene).
    ///
    /// This scans the pairs (the entry's fields are public, so it keeps no index); per-vertex
    /// lookups should go through [`TerrainSceneTable::scene_key`], which uses the table's
    /// slot index and gives the same answer.
    #[must_use]
    pub fn pair_for_slot(&self, slot: u32) -> Option<ScenePair> {
        self.pairs
            .iter()
            .find(|p| p.slot == slot)
            .copied()
            .or(self.default_pair)
    }
}

/// Slot → stored index of the first pair with that slot, sorted by slot (one entry's pairs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SlotIndex(Box<[(u32, u32)]>);

impl SlotIndex {
    fn new(entry: &TerrainSceneEntry) -> Self {
        let mut v: Vec<(u32, u32)> = entry
            .pairs
            .iter()
            .enumerate()
            .map(|(i, p)| (p.slot, index_u32(i)))
            .collect();
        // Sorting by (slot, index) and keeping the first of each slot keeps the first match
        // by value of §4.2 step 2.
        v.sort_unstable();
        v.dedup_by_key(|&mut (slot, _)| slot);
        Self(v.into_boxed_slice())
    }

    /// The pair for `slot` in `entry` (the entry this index was built from), else the
    /// entry's default pair.
    fn pair(&self, entry: &TerrainSceneEntry, slot: u32) -> Option<ScenePair> {
        match self.0.binary_search_by_key(&slot, |&(s, _)| s) {
            Ok(k) => entry.pairs.get(self.0[k].1 as usize).copied(),
            Err(_) => entry.default_pair,
        }
    }
}

/// A terrain-scene table (prefix 0x12, region descriptor slot 2; `scenery.md` §4.2).
///
/// The entries are fixed once built: lookups by terrain id and by slot use indexes built
/// then, so a lookup costs O(log *n*) in the entry's pairs whatever the record holds.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainSceneTable {
    /// Own DID.
    pub did: Did,
    /// The u32 after the DID (0 in every observed record).
    pub reserved: u32,
    default_entry: Option<TerrainSceneEntry>,
    entries: Vec<TerrainSceneEntry>,
    /// id → stored index of the first entry with that id.
    by_id: HashMap<u32, u32>,
    /// Slot index of the default entry (empty without one).
    default_slots: SlotIndex,
    /// Slot index of each listed entry, parallel to `entries`.
    slots: Vec<SlotIndex>,
}

impl TerrainSceneTable {
    /// Builds a table from its parts (for tools and tests). The DID must have prefix 0x12,
    /// and every name must be Latin-1 (no character above U+00FF) of at most
    /// [`MAX_TERRAIN_ENTRY_NAME_LEN`] bytes, so that [`Self::to_bytes`] round-trips
    /// ([`FormatError::Malformed`] otherwise, at offset 0).
    pub fn new(
        did: Did,
        reserved: u32,
        default_entry: Option<TerrainSceneEntry>,
        entries: Vec<TerrainSceneEntry>,
    ) -> Result<Self> {
        if did.kind() != DidKind::TerrainSceneTable {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: <Self as Record>::NAME,
            });
        }
        for e in default_entry.iter().chain(&entries) {
            check_entry_name(&e.name)?;
        }
        Ok(Self::index(did, reserved, default_entry, entries))
    }

    /// Builds the lookup indexes of a table whose parts are valid.
    fn index(
        did: Did,
        reserved: u32,
        default_entry: Option<TerrainSceneEntry>,
        entries: Vec<TerrainSceneEntry>,
    ) -> Self {
        let by_id = id_index(entries.iter().map(|e| e.id));
        let default_slots = default_entry
            .as_ref()
            .map(SlotIndex::new)
            .unwrap_or_default();
        let slots = entries.iter().map(SlotIndex::new).collect();
        Self {
            did,
            reserved,
            default_entry,
            entries,
            by_id,
            default_slots,
            slots,
        }
    }

    /// The entry used for terrain ids that no listed entry has (`has_default` = 1).
    #[must_use]
    pub fn default_entry(&self) -> Option<&TerrainSceneEntry> {
        self.default_entry.as_ref()
    }

    /// The listed entries in stored order (the default entry excluded).
    #[must_use]
    pub fn entries(&self) -> &[TerrainSceneEntry] {
        &self.entries
    }

    /// The listed entry with terrain-type id `id` (the first one if the data repeats an id;
    /// ids are distinct in every observed record, SC5). The default entry is not consulted.
    #[must_use]
    pub fn entry(&self, id: u32) -> Option<&TerrainSceneEntry> {
        let i = *self.by_id.get(&id)?;
        self.entries.get(i as usize)
    }

    /// The entry for terrain-type id `terrain` (`scenery.md` §4.2 lookup step 1): the
    /// listed entry with that id, else the default entry.
    #[must_use]
    pub fn entry_for(&self, terrain: u32) -> Option<&TerrainSceneEntry> {
        self.entry(terrain).or(self.default_entry.as_ref())
    }

    /// The pair for (terrain id, slot) (`scenery.md` §4.2 lookup steps 1–2): the entry of
    /// [`Self::entry_for`], then its first pair with that slot, else its default pair.
    /// Same answer as [`TerrainSceneEntry::pair_for_slot`], through the slot index.
    #[must_use]
    pub fn pair_for(&self, terrain: u32, slot: u32) -> Option<ScenePair> {
        match self.by_id.get(&terrain) {
            Some(&i) => {
                let i = i as usize;
                self.slots.get(i)?.pair(self.entries.get(i)?, slot)
            }
            None => self.default_slots.pair(self.default_entry.as_ref()?, slot),
        }
    }

    /// The scene key for (terrain id, slot) (`scenery.md` §4.2 lookup steps 1–3). `None`
    /// means no scene: no entry, no pair for the slot and no default pair, or key 0.
    #[must_use]
    pub fn scene_key(&self, terrain: u32, slot: u32) -> Option<u32> {
        self.pair_for(terrain, slot)
            .map(|p| p.scene_key)
            .filter(|&k| k != 0)
    }

    /// [`TerrainSceneTable::scene_key`] for a coarse-grid scenery word (`scenery.md` §3.1:
    /// terrain id in bits 0–7, slot in bits 8–14).
    #[must_use]
    pub fn scene_key_for_word(&self, word: u32) -> Option<u32> {
        self.scene_key(
            word & WORD_TERRAIN_MASK,
            (word >> WORD_SLOT_SHIFT) & WORD_SLOT_MASK,
        )
    }

    /// The stored form of the record.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.extend_from_slice(&self.reserved.to_le_bytes());
        match &self.default_entry {
            Some(e) => {
                out.push(1);
                e.write(&mut out);
            }
            None => out.push(0),
        }
        out.extend_from_slice(&len_u32(self.entries.len()).to_le_bytes());
        for e in &self.entries {
            e.write(&mut out);
        }
        out
    }
}

impl Record<'_> for TerrainSceneTable {
    const NAME: &'static str = "terrain-scene table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::TerrainSceneTable)?;
        let reserved = r.u32()?;
        // `has_default` is read as a bool8 (scenery.md §4.2 OPEN on its type).
        let default_entry = if r.bool8()? {
            Some(TerrainSceneEntry::read(&mut r)?)
        } else {
            None
        };
        let count = r.u32()?;
        let mut entries = r.alloc_vec(count, MIN_TERRAIN_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let e = TerrainSceneEntry::read(&mut r)?;
            r.push(&mut entries, e, count)?;
        }
        r.expect_end(Self::NAME)?;
        r.charge(hash_index_bytes::<u32, u32>(entries.len()))?;
        // Slot indexes: one (slot, index) per pair plus a boxed slice per entry.
        let pairs = default_entry
            .iter()
            .chain(&entries)
            .map(|e| e.pairs.len())
            .fold(0usize, usize::saturating_add);
        r.charge(
            pairs
                .saturating_mul(std::mem::size_of::<(u32, u32)>())
                .saturating_add(
                    entries
                        .len()
                        .saturating_mul(std::mem::size_of::<SlotIndex>()),
                ),
        )?;
        Ok(Self::index(did, reserved, default_entry, entries))
    }
}

// ---------------------------------------------------------------------------------------
// Scene-choice table (0x11)
// ---------------------------------------------------------------------------------------

/// One weighted choice of a scene-choice entry (`scenery.md` §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SceneChoice {
    /// The scene (prefix 0x02).
    pub scene: Did,
    /// Its weight.
    pub weight: f32,
    /// Meaning OPEN (1 in every observed choice).
    pub flag: u8,
}

/// An entry of a scene-choice table: a scene key and its weighted scenes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneChoiceEntry {
    /// The scene key (unique within an observed record; not sorted).
    pub key: u32,
    /// The choices in stored order.
    pub choices: Vec<SceneChoice>,
}

impl SceneChoiceEntry {
    /// The sum of the weights, accumulated in stored order as f32.
    #[must_use]
    pub fn total_weight(&self) -> f32 {
        self.choices.iter().map(|c| c.weight).sum()
    }

    /// Picks a choice for the fraction `u` (0 ≤ u < 1; `scenery.md` §6.1, §7.2 steps 3–4):
    /// T = u × total weight, then the first choice whose running sum of weights is strictly
    /// greater than T; if none of the first *n* − 1 qualifies, the last. `None` only when
    /// the entry has no choices.
    #[must_use]
    pub fn choose(&self, u: f32) -> Option<&SceneChoice> {
        let (last, rest) = self.choices.split_last()?;
        let threshold = u * self.total_weight();
        let mut sum = 0.0f32;
        for c in rest {
            sum += c.weight;
            if sum > threshold {
                return Some(c);
            }
        }
        Some(last)
    }
}

/// A scene-choice table (prefix 0x11, region descriptor slot 1; `scenery.md` §4.3).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneChoiceTable {
    /// The u32 before the DID (0 in every observed record; `records.md` §1.6 prologue xD).
    pub reserved: u32,
    /// Own DID.
    pub did: Did,
    entries: Vec<SceneChoiceEntry>,
    /// key → stored index of the first entry with that key.
    by_key: HashMap<u32, u32>,
}

impl SceneChoiceTable {
    /// Builds a table from its parts (for tools and tests). The DID must have prefix 0x11.
    pub fn new(reserved: u32, did: Did, entries: Vec<SceneChoiceEntry>) -> Result<Self> {
        if did.kind() != DidKind::SceneChoiceTable {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: <Self as Record>::NAME,
            });
        }
        let by_key = id_index(entries.iter().map(|e| e.key));
        Ok(Self {
            reserved,
            did,
            entries,
            by_key,
        })
    }

    /// Entries in stored order.
    #[must_use]
    pub fn entries(&self) -> &[SceneChoiceEntry] {
        &self.entries
    }

    /// The entry for scene key `key` (the first one if the data repeats a key).
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&SceneChoiceEntry> {
        let i = *self.by_key.get(&key)?;
        self.entries.get(i as usize)
    }

    /// The stored form of the record.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.reserved.to_le_bytes());
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.extend_from_slice(&len_u32(self.entries.len()).to_le_bytes());
        for e in &self.entries {
            out.extend_from_slice(&e.key.to_le_bytes());
            out.extend_from_slice(&len_u32(e.choices.len()).to_le_bytes());
            for c in &e.choices {
                out.extend_from_slice(&c.scene.0.to_le_bytes());
                out.extend_from_slice(&c.weight.to_le_bytes());
                out.push(c.flag);
            }
        }
        out
    }
}

impl Record<'_> for SceneChoiceTable {
    const NAME: &'static str = "scene-choice table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let reserved = r.u32()?;
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::SceneChoiceTable)?;
        let count = r.u32()?;
        let mut entries = r.alloc_vec(count, MIN_CHOICE_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let key = r.u32()?;
            let n = r.u32()?;
            let mut choices = r.alloc_vec(n, CHOICE_LEN, PREALLOC_CAP)?;
            for _ in 0..n {
                let c = SceneChoice {
                    scene: r.did()?,
                    weight: r.f32()?,
                    flag: r.u8()?,
                };
                r.push(&mut choices, c, n)?;
            }
            r.push(&mut entries, SceneChoiceEntry { key, choices }, count)?;
        }
        r.expect_end(Self::NAME)?;
        r.charge(hash_index_bytes::<u32, u32>(entries.len()))?;
        let by_key = id_index(entries.iter().map(|e| e.key));
        Ok(Self {
            reserved,
            did,
            entries,
            by_key,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Scene records (0x02)
// ---------------------------------------------------------------------------------------

/// A (min, max) pair of f32 (`scenery.md` §5.3 "Range (a, b)").
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FloatRange {
    /// First value (minimum).
    pub min: f32,
    /// Second value (maximum).
    pub max: f32,
}

/// How a placement node's height is snapped (`scenery.md` §5.3 offset 176, §7.4 step 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightMode {
    /// 0: snap to the terrain plane.
    Terrain,
    /// 1: snap to the water surface (reject where there is none).
    Water,
    /// 2–8: base height 0 (BIN only; unobserved).
    BaseZero,
    /// 9 and above: no snap (9 BIN only; above 9 unobserved).
    NoSnap,
}

impl HeightMode {
    /// The mode for the stored byte.
    #[must_use]
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Terrain,
            1 => Self::Water,
            2..=8 => Self::BaseZero,
            _ => Self::NoSnap,
        }
    }
}

/// A child of a group or placement node (`scenery.md` §5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum SceneChild {
    /// Type 1: an object, a 0x47 entity descriptor.
    Object(Did),
    /// Type 2: a nested placement node, placed relative to its parent.
    Node(Box<PlacementNode>),
    /// Type 3: a reference to another 0x02 scene, instantiated at the parent's placement.
    Scene(Did),
}

impl SceneChild {
    /// The stored type byte (1, 2 or 3).
    #[must_use]
    pub fn type_code(&self) -> u8 {
        match self {
            Self::Object(_) => 1,
            Self::Node(_) => 2,
            Self::Scene(_) => 3,
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.type_code());
        match self {
            Self::Object(d) | Self::Scene(d) => out.extend_from_slice(&d.0.to_le_bytes()),
            Self::Node(n) => n.write(out),
        }
    }
}

/// A placement node (`scenery.md` §5.3): a 187-byte fixed part, the children, then the
/// maximum of the depth-below-ground range. Field docs give the stored offset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacementNode {
    /// 0: minimum displacement (x, y, z) from the parent position.
    pub disp_min: [f32; 3],
    /// 12: maximum displacement.
    pub disp_max: [f32; 3],
    /// 24: heading range in degrees, clockwise seen from above.
    pub heading: FloatRange,
    /// 32: second rotation range in degrees, about the model X axis.
    pub angle2: FloatRange,
    /// 40: third rotation range in degrees, about the model Y axis.
    pub angle3: FloatRange,
    /// 48: probability that the node is placed at a vertex; ≥ 1 means always.
    pub frequency: f32,
    /// 52: minimum scale per axis.
    pub scale_min: [f32; 3],
    /// 64: maximum scale per axis.
    pub scale_max: [f32; 3],
    /// 76: divisor for the stretch scale modes 5–7.
    pub stretch_ref: f32,
    /// 80: upper bound on the terrain normal's z.
    pub slope_max_cos: f32,
    /// 84: lower bound on the terrain normal's z.
    pub slope_min_cos: f32,
    /// 88: tilt the object so that its up axis is the terrain normal.
    pub align_to_slope: bool,
    /// 89: add the bearing of the terrain normal's horizontal part plus 180° to the heading.
    pub face_direction: bool,
    /// 90: height-above-ground range.
    pub above_ground: FloatRange,
    /// 98: height-above-water range.
    pub above_water: FloatRange,
    /// 106: depth-below-water range.
    pub below_water: FloatRange,
    /// 114: water-depth range (water surface minus ground).
    pub water_depth: FloatRange,
    /// 122: minimum of the depth-below-ground range (see [`PlacementNode::below_ground`]).
    pub below_ground_min: f32,
    /// 126: absolute height range; may be infinite.
    pub z_range: FloatRange,
    /// 134: spacing radius.
    pub spacing: f32,
    /// 138: radius of the terrain clearance test; 0 disables it.
    pub clearance: f32,
    /// 142: multiply `clearance` by max(scale x, scale y).
    pub clearance_scaled: bool,
    /// 143: upper bound on the normal z of the terrain triangles in the clearance area.
    pub clear_max_cos: f32,
    /// 147: lower bound on those normals' z.
    pub clear_min_cos: f32,
    /// 151: smallest allowed dot product between two of those normals.
    pub clear_min_dot: f32,
    /// 155: aspect tolerance in degrees; the aspect test runs only below 180.
    pub clear_aspect_tol: f32,
    /// 159: stop after this many created objects; 0 = no limit.
    pub max_objects: u32,
    /// 163: not read by the placement code.
    pub unknown_163: f32,
    /// 167: multiply the displacement by the parent scale.
    pub disp_scaled: bool,
    /// 168: start from the parent scale (else from 1).
    pub inherit_scale: bool,
    /// 169: rotate the displacement by the parent orientation.
    pub disp_relative: bool,
    /// 170: compose the rotation with the parent orientation (else absolute).
    pub orient_relative: bool,
    /// 171: multiply `spacing` by the largest scale component.
    pub spacing_scaled: bool,
    /// 172: skip the terrain-type, height-range and spacing checks.
    pub no_checks: bool,
    /// 173: spacing uses all three scale components and tests in 3D.
    pub spacing_3d: bool,
    /// 174: not read by the placement code.
    pub unknown_174: bool,
    /// 175: spacing radius 0 and no test against placed footprints (also descendants).
    pub no_spacing: bool,
    /// 176: height mode byte as stored (typed: [`PlacementNode::height_mode_kind`]).
    pub height_mode: u8,
    /// 177: scale mode byte (4 = uniform; [`SCALE_MODE_UNIFORM`]).
    pub scale_mode: u8,
    /// 178: range mode of `above_ground` (`scenery.md` §7.5).
    pub above_ground_mode: u8,
    /// 179: range mode of `above_water`.
    pub above_water_mode: u8,
    /// 180: range mode of `below_water`.
    pub below_water_mode: u8,
    /// 181: range mode of `water_depth`.
    pub water_depth_mode: u8,
    /// 182: range mode of the depth-below-ground range.
    pub below_ground_mode: u8,
    /// 187: the children, in stored order.
    pub children: Vec<SceneChild>,
    /// After the children: maximum of the depth-below-ground range.
    pub below_ground_max: f32,
}

/// Reads an f32 that must be finite (`scenery.md` §13.1).
fn finite(r: &mut Reader<'_>) -> Result<f32> {
    let offset = r.position();
    let v = r.f32()?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(FormatError::malformed(
            "placement node",
            offset,
            "non-finite float",
        ))
    }
}

fn finite3(r: &mut Reader<'_>) -> Result<[f32; 3]> {
    Ok([finite(r)?, finite(r)?, finite(r)?])
}

fn finite_range(r: &mut Reader<'_>) -> Result<FloatRange> {
    Ok(FloatRange {
        min: finite(r)?,
        max: finite(r)?,
    })
}

impl PlacementNode {
    /// The height mode (offset 176) as a [`HeightMode`]; the raw byte is
    /// [`PlacementNode::height_mode`].
    #[must_use]
    pub fn height_mode_kind(&self) -> HeightMode {
        HeightMode::from_raw(self.height_mode)
    }

    /// The depth-below-ground range (`below_ground_min`, `below_ground_max`).
    #[must_use]
    pub fn below_ground(&self) -> FloatRange {
        FloatRange {
            min: self.below_ground_min,
            max: self.below_ground_max,
        }
    }

    /// Reads a node at nesting level `depth` (1 = the frill layer or a group child).
    fn read(r: &mut Reader<'_>, depth: usize) -> Result<Self> {
        if depth > MAX_PLACEMENT_NODE_DEPTH {
            return Err(FormatError::DepthLimit {
                offset: r.position(),
                limit: MAX_PLACEMENT_NODE_DEPTH,
            });
        }
        let disp_min = finite3(r)?;
        let disp_max = finite3(r)?;
        let heading = finite_range(r)?;
        let angle2 = finite_range(r)?;
        let angle3 = finite_range(r)?;
        let frequency = finite(r)?;
        let scale_min = finite3(r)?;
        let scale_max = finite3(r)?;
        let stretch_ref = finite(r)?;
        let slope_max_cos = finite(r)?;
        let slope_min_cos = finite(r)?;
        let align_to_slope = r.bool8()?;
        let face_direction = r.bool8()?;
        let above_ground = finite_range(r)?;
        let above_water = finite_range(r)?;
        let below_water = finite_range(r)?;
        let water_depth = finite_range(r)?;
        let below_ground_min = finite(r)?;
        let z_offset = r.position();
        let z_range = FloatRange {
            min: r.f32()?,
            max: r.f32()?,
        };
        if z_range.min.is_nan() || z_range.max.is_nan() {
            return Err(FormatError::malformed(
                "placement node",
                z_offset,
                "NaN in z_range",
            ));
        }
        let spacing = finite(r)?;
        let clearance = finite(r)?;
        let clearance_scaled = r.bool8()?;
        let clear_max_cos = finite(r)?;
        let clear_min_cos = finite(r)?;
        let clear_min_dot = finite(r)?;
        let clear_aspect_tol = finite(r)?;
        let max_objects = r.u32()?;
        let unknown_163 = finite(r)?;
        let disp_scaled = r.bool8()?;
        let inherit_scale = r.bool8()?;
        let disp_relative = r.bool8()?;
        let orient_relative = r.bool8()?;
        let spacing_scaled = r.bool8()?;
        let no_checks = r.bool8()?;
        let spacing_3d = r.bool8()?;
        let unknown_174 = r.bool8()?;
        let no_spacing = r.bool8()?;
        let [
            height_mode,
            scale_mode,
            above_ground_mode,
            above_water_mode,
            below_water_mode,
            water_depth_mode,
            below_ground_mode,
        ] = r.array::<7>()?;
        let children = read_children(r, depth)?;
        let below_ground_max = finite(r)?;
        Ok(Self {
            disp_min,
            disp_max,
            heading,
            angle2,
            angle3,
            frequency,
            scale_min,
            scale_max,
            stretch_ref,
            slope_max_cos,
            slope_min_cos,
            align_to_slope,
            face_direction,
            above_ground,
            above_water,
            below_water,
            water_depth,
            below_ground_min,
            z_range,
            spacing,
            clearance,
            clearance_scaled,
            clear_max_cos,
            clear_min_cos,
            clear_min_dot,
            clear_aspect_tol,
            max_objects,
            unknown_163,
            disp_scaled,
            inherit_scale,
            disp_relative,
            orient_relative,
            spacing_scaled,
            no_checks,
            spacing_3d,
            unknown_174,
            no_spacing,
            height_mode,
            scale_mode,
            above_ground_mode,
            above_water_mode,
            below_water_mode,
            water_depth_mode,
            below_ground_mode,
            children,
            below_ground_max,
        })
    }

    /// Appends the stored form (without a child type byte).
    pub fn write(&self, out: &mut Vec<u8>) {
        let f = |out: &mut Vec<u8>, v: f32| out.extend_from_slice(&v.to_le_bytes());
        let range = |out: &mut Vec<u8>, v: FloatRange| {
            out.extend_from_slice(&v.min.to_le_bytes());
            out.extend_from_slice(&v.max.to_le_bytes());
        };
        for v in self.disp_min.iter().chain(&self.disp_max) {
            f(out, *v);
        }
        range(out, self.heading);
        range(out, self.angle2);
        range(out, self.angle3);
        f(out, self.frequency);
        for v in self.scale_min.iter().chain(&self.scale_max) {
            f(out, *v);
        }
        f(out, self.stretch_ref);
        f(out, self.slope_max_cos);
        f(out, self.slope_min_cos);
        out.push(u8::from(self.align_to_slope));
        out.push(u8::from(self.face_direction));
        range(out, self.above_ground);
        range(out, self.above_water);
        range(out, self.below_water);
        range(out, self.water_depth);
        f(out, self.below_ground_min);
        range(out, self.z_range);
        f(out, self.spacing);
        f(out, self.clearance);
        out.push(u8::from(self.clearance_scaled));
        f(out, self.clear_max_cos);
        f(out, self.clear_min_cos);
        f(out, self.clear_min_dot);
        f(out, self.clear_aspect_tol);
        out.extend_from_slice(&self.max_objects.to_le_bytes());
        f(out, self.unknown_163);
        for b in [
            self.disp_scaled,
            self.inherit_scale,
            self.disp_relative,
            self.orient_relative,
            self.spacing_scaled,
            self.no_checks,
            self.spacing_3d,
            self.unknown_174,
            self.no_spacing,
        ] {
            out.push(u8::from(b));
        }
        out.extend_from_slice(&[
            self.height_mode,
            self.scale_mode,
            self.above_ground_mode,
            self.above_water_mode,
            self.below_water_mode,
            self.water_depth_mode,
            self.below_ground_mode,
        ]);
        out.extend_from_slice(&len_u32(self.children.len()).to_le_bytes());
        for c in &self.children {
            c.write(out);
        }
        f(out, self.below_ground_max);
    }
}

/// Reads a child count and the children of a group (`depth` 0) or a node at `depth`.
fn read_children(r: &mut Reader<'_>, depth: usize) -> Result<Vec<SceneChild>> {
    let count = r.u32()?;
    let mut children = r.alloc_vec(count, MIN_CHILD_LEN, PREALLOC_CAP)?;
    for _ in 0..count {
        let offset = r.position();
        let child = match r.u8()? {
            1 => SceneChild::Object(r.did()?),
            2 => {
                r.charge(std::mem::size_of::<PlacementNode>())?;
                SceneChild::Node(Box::new(PlacementNode::read(r, depth + 1)?))
            }
            3 => SceneChild::Scene(r.did()?),
            4 => {
                return Err(FormatError::Unsupported {
                    what: "scene child type",
                    offset,
                    value: 4,
                });
            }
            t => {
                return Err(FormatError::malformed(
                    "scene child",
                    offset,
                    format!("child type {t} outside 1–4"),
                ));
            }
        };
        r.push(&mut children, child, count)?;
    }
    Ok(children)
}

/// A group of a scene (`scenery.md` §5.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneGroup {
    /// Group id (1 in every observed group; the generator uses the first group with id 1).
    pub group_id: u32,
    /// The children in stored order.
    pub children: Vec<SceneChild>,
}

/// Where a child sits in a scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneSite {
    /// Under the frill layer (evaluated only by the frill engine, `scenery.md` §8).
    FrillLayer,
    /// Under the group with this stored index.
    Group(usize),
}

/// A child met by [`Scene::descendants`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChildRef<'a> {
    /// The frill layer or group it is under.
    pub site: SceneSite,
    /// One more than the nesting level of the child's container, groups being level 0 and
    /// the frill layer root level 1: 1 for a direct child of a group, 2 for a direct child
    /// of the frill layer root or of a node that is a group child, and so on. For a
    /// [`SceneChild::Node`] this is its nesting level as limited by
    /// [`MAX_PLACEMENT_NODE_DEPTH`] (so at most that); objects and scene references are at
    /// most [`MAX_CHILD_DEPTH`].
    pub depth: usize,
    /// The child.
    pub child: &'a SceneChild,
}

/// A scene record (prefix 0x02; `scenery.md` §5.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// The u32 before the DID (0 in every observed record; `records.md` §1.6 prologue xD).
    pub reserved: u32,
    /// Own DID.
    pub did: Did,
    /// Meaning OPEN (1 in every observed record); not read by scene instantiation.
    pub flag_a: bool,
    /// The frill layer (root placement node), if any.
    pub frill_layer: Option<PlacementNode>,
    /// The groups in stored order.
    pub groups: Vec<SceneGroup>,
}

impl Scene {
    /// The group the generator uses: the first group whose id is 1 (`scenery.md` §5.1).
    #[must_use]
    pub fn placement_group(&self) -> Option<&SceneGroup> {
        self.groups.iter().find(|g| g.group_id == 1)
    }

    /// Every child of the frill layer and of every group, depth first in stored order
    /// (a node is yielded before its own children). The frill layer root itself is not a
    /// child; see [`Scene::nodes`].
    pub fn descendants(&self) -> impl Iterator<Item = ChildRef<'_>> + '_ {
        let mut stack: Vec<(SceneSite, usize, std::slice::Iter<'_, SceneChild>)> = Vec::new();
        for (i, g) in self.groups.iter().enumerate().rev() {
            stack.push((SceneSite::Group(i), 1, g.children.iter()));
        }
        if let Some(f) = &self.frill_layer {
            stack.push((SceneSite::FrillLayer, 2, f.children.iter()));
        }
        std::iter::from_fn(move || {
            loop {
                let (site, depth, iter) = stack.last_mut()?;
                let (site, depth) = (*site, *depth);
                match iter.next() {
                    Some(child) => {
                        if let SceneChild::Node(n) = child {
                            stack.push((site, depth + 1, n.children.iter()));
                        }
                        return Some(ChildRef { site, depth, child });
                    }
                    None => {
                        stack.pop();
                    }
                }
            }
        })
    }

    /// Every placement node: the frill layer root and every nested node, with its site and
    /// nesting level (1 for the frill layer root and for group children, as counted by
    /// [`MAX_PLACEMENT_NODE_DEPTH`] and [`ChildRef::depth`]).
    pub fn nodes(&self) -> impl Iterator<Item = (SceneSite, usize, &PlacementNode)> + '_ {
        self.frill_layer
            .iter()
            .map(|n| (SceneSite::FrillLayer, 1, n))
            .chain(self.descendants().filter_map(|c| match c.child {
                SceneChild::Node(n) => Some((c.site, c.depth, n.as_ref())),
                _ => None,
            }))
    }

    /// The DIDs of every scene reference, in [`Scene::descendants`] order (repeats kept).
    pub fn scene_references(&self) -> impl Iterator<Item = Did> + '_ {
        self.descendants().filter_map(|c| match c.child {
            SceneChild::Scene(d) => Some(*d),
            _ => None,
        })
    }

    /// The DIDs of every object, in [`Scene::descendants`] order (repeats kept).
    pub fn objects(&self) -> impl Iterator<Item = Did> + '_ {
        self.descendants().filter_map(|c| match c.child {
            SceneChild::Object(d) => Some(*d),
            _ => None,
        })
    }

    /// Whether the scene has no children at all: none under the frill layer and none in any
    /// group, whatever the group ids. A scene with children may still place nothing from
    /// its groups, as the generator uses only [`Scene::placement_group`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frill_layer
            .as_ref()
            .is_none_or(|f| f.children.is_empty())
            && self.groups.iter().all(|g| g.children.is_empty())
    }

    /// The stored form of the record.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.reserved.to_le_bytes());
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.push(u8::from(self.flag_a));
        match &self.frill_layer {
            Some(f) => {
                out.push(1);
                f.write(&mut out);
            }
            None => out.push(0),
        }
        out.extend_from_slice(&len_u32(self.groups.len()).to_le_bytes());
        for g in &self.groups {
            out.extend_from_slice(&g.group_id.to_le_bytes());
            out.extend_from_slice(&len_u32(g.children.len()).to_le_bytes());
            for c in &g.children {
                c.write(&mut out);
            }
        }
        out
    }
}

impl Record<'_> for Scene {
    const NAME: &'static str = "scene";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let reserved = r.u32()?;
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::Scene)?;
        let flag_a = r.bool8()?;
        let frill_layer = if r.bool8()? {
            Some(PlacementNode::read(&mut r, 1)?)
        } else {
            None
        };
        let count = r.u32()?;
        let mut groups = r.alloc_vec(count, MIN_GROUP_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let group_id = r.u32()?;
            let children = read_children(&mut r, 0)?;
            r.push(&mut groups, SceneGroup { group_id, children }, count)?;
        }
        r.expect_end(Self::NAME)?;
        Ok(Self {
            reserved,
            did,
            flag_a,
            frill_layer,
            groups,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Scene-reference graph
// ---------------------------------------------------------------------------------------

/// Follows scene references between scene records with cycle and depth protection
/// (`scenery.md` §13.1, SC2).
///
/// [`SceneReferenceChecker::height`] walks every path from a scene, refusing a scene that
/// is already on the current path (a cycle) and paths longer than the limit, and reports a
/// referenced scene that `fetch` cannot load. Results are memoised, so checking every scene
/// of an install visits each record once.
#[derive(Debug, Clone)]
pub struct SceneReferenceChecker {
    limit: usize,
    heights: HashMap<Did, usize>,
}

impl Default for SceneReferenceChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneReferenceChecker {
    /// A checker with the limit [`MAX_SCENE_REFERENCE_DEPTH`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(MAX_SCENE_REFERENCE_DEPTH)
    }

    /// A checker allowing paths of at most `limit` scenes (clamped to 1 …
    /// [`MAX_SCENE_REFERENCE_DEPTH`] × 4, so the walk's recursion stays shallow).
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit: limit.clamp(1, MAX_SCENE_REFERENCE_DEPTH * 4),
            heights: HashMap::new(),
        }
    }

    /// The number of scenes on the longest reference path starting at `root` (1 for a
    /// scene without references).
    ///
    /// `fetch` loads a scene by DID (owned, borrowed or shared). Errors:
    /// [`FormatError::DanglingReference`] when a scene cannot be loaded (`from` is 0 for
    /// `root` itself), [`FormatError::ReferenceChain`] on a cycle or a path longer than the
    /// limit. Scenes already checked are not loaded again.
    pub fn height<S: Borrow<Scene>>(
        &mut self,
        root: Did,
        mut fetch: impl FnMut(Did) -> Option<S>,
    ) -> Result<usize> {
        let mut path = Vec::new();
        self.visit(Did(0), root, &mut fetch, &mut path)
    }

    fn too_deep(&self, did: Did) -> FormatError {
        FormatError::ReferenceChain {
            did: did.0,
            reason: format!("scene reference path longer than {}", self.limit).into(),
        }
    }

    fn visit<S: Borrow<Scene>>(
        &mut self,
        from: Did,
        did: Did,
        fetch: &mut impl FnMut(Did) -> Option<S>,
        path: &mut Vec<Did>,
    ) -> Result<usize> {
        if let Some(&h) = self.heights.get(&did) {
            if path.len() + h > self.limit {
                return Err(self.too_deep(did));
            }
            return Ok(h);
        }
        if path.contains(&did) {
            return Err(FormatError::ReferenceChain {
                did: did.0,
                reason: "scene reference cycle".into(),
            });
        }
        if path.len() >= self.limit {
            return Err(self.too_deep(did));
        }
        let scene = fetch(did).ok_or(FormatError::DanglingReference {
            from: from.0,
            to: did.0,
        })?;
        let mut refs: Vec<Did> = scene.borrow().scene_references().collect();
        drop(scene);
        refs.sort_unstable();
        refs.dedup();
        path.push(did);
        let mut height = 1usize;
        for child in refs {
            let h = self.visit(did, child, fetch, path);
            match h {
                Ok(h) => height = height.max(h.saturating_add(1)),
                Err(e) => {
                    path.pop();
                    return Err(e);
                }
            }
        }
        path.pop();
        if path.len() + height > self.limit {
            return Err(self.too_deep(did));
        }
        self.heights.insert(did, height);
        Ok(height)
    }
}

// ---------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------

/// A stored count for a collection length (saturating; decoded collections always fit).
fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Checks that a terrain-entry name encodes losslessly in the one-byte length form.
fn check_entry_name(name: &str) -> Result<()> {
    if name.chars().any(|c| u32::from(c) > 0xFF) {
        return Err(FormatError::malformed(
            "terrain-scene entry",
            0,
            "name has a character above U+00FF",
        ));
    }
    let len = name.chars().count();
    if len > MAX_TERRAIN_ENTRY_NAME_LEN {
        return Err(FormatError::malformed(
            "terrain-scene entry",
            0,
            format!("name of {len} bytes is longer than {MAX_TERRAIN_ENTRY_NAME_LEN}"),
        ));
    }
    Ok(())
}

/// Value → stored index of its first occurrence.
fn id_index(ids: impl ExactSizeIterator<Item = u32>) -> HashMap<u32, u32> {
    let mut map = HashMap::with_capacity(ids.len());
    for (i, id) in ids.enumerate() {
        map.entry(id).or_insert(index_u32(i));
    }
    map
}

/// Decodes a terrain-scene table payload (for fuzz targets and tools).
pub fn decode_terrain_scene_table(data: &[u8]) -> Result<TerrainSceneTable> {
    TerrainSceneTable::parse(data)
}

/// Decodes a scene-choice table payload (for fuzz targets and tools).
pub fn decode_scene_choice_table(data: &[u8]) -> Result<SceneChoiceTable> {
    SceneChoiceTable::parse(data)
}

/// Decodes a scene record payload (for fuzz targets and tools).
pub fn decode_scene(data: &[u8]) -> Result<Scene> {
    Scene::parse(data)
}
