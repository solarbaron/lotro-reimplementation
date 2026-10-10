// SPDX-License-Identifier: GPL-3.0-or-later
//! The appearance of interior cells (`landblock.md` §4.14 Tessera Decision: a cell's
//! `Appearance_InstanceList` is applied to its render mesh as `appearance.md` §3.2 applies
//! an instance list to an entity).
//!
//! - **Elements** ([`cell_look`]): the elements of the cell's `Appearance_InstanceList`, in
//!   stored order. Each names an appearance record (the toggle record when its toggle
//!   property is true in the cell's set, `appearance.md` §3.3), an entry key and a
//!   modifier; the modifier picks the entry's block by §3.2.1 ([`choose_block`]).
//! - **Groups**: the material groups of every chosen block join one list ordered by
//!   ascending group priority, ties in application order (`appearance.md` §4.10 rule for
//!   Tessera), each kept as (priority, shader override, material, slot, binding key).
//! - **Drawing** ([`CellLook::layering`]): a submesh whose surface record has a group's
//!   binding key, and whose mesh slot the group's slot matches (`All` matches every part),
//!   is drawn with the surface record's materials followed by the matching groups'
//!   materials in list order, and with the shader override of the last matching group that
//!   has one, a group without a material included (`material.md` §9.3 rules 1–2).
//! - **Parts** a chosen block offers are not drawn on a cell (a cell has no slot table);
//!   they are counted and logged. No block of this install's cell lists has parts (own
//!   probe, 2026-10-08: 81,171 elements in 80,825 cell lists, every chosen block
//!   material-only, every group slot `All`).
//!
//! The object layer draws parts per mesh DID; a cell with a look is drawn through a part
//! id of its own ([`CellLooks::intern`]): `LOOK_PART_BASE | index`, which the layer
//! requests as the cell's 0x01 record with the look's index as the asset variant
//! ([`CellLooks::asset_key`]), so one decoded copy exists per (cell geometry, look) pair,
//! as the client draws a private copy of the mesh with the appearance applied (`landblock.md`
//! L34). Hostile input: lists, records and looks are bounded ([`MAX_ELEMENTS`],
//! [`MAX_GROUPS`], [`MAX_LOOKS`], [`MAX_CACHED_RECORDS`]); a cell whose look cannot be
//! interned is drawn with its default look.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::AssetKey;
use tessera_formats::appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceListPids, AppearanceRecord, SLOT_ALL,
};
use tessera_formats::{Did, PropertyNames, PropertySet, Record};
use tessera_material::Layering;
use tessera_world::WorldSource;

/// The top byte of the part ids of cells drawn with a look (no record kind uses it).
pub const LOOK_PART_BASE: u32 = 0xFE00_0000;
/// Most (cell geometry, look) pairs interned; later cells keep their default look.
pub const MAX_LOOKS: usize = 65_536;
/// Most list elements read per cell.
pub const MAX_ELEMENTS: usize = 64;
/// Most groups kept per look.
pub const MAX_GROUPS: usize = 256;
/// Most appearance records kept decoded (the cache is emptied when full).
pub const MAX_CACHED_RECORDS: usize = 4096;
/// Most layered materials per submesh (as the figure path).
pub const MAX_LAYERS: usize = 16;

/// One material group of a look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookGroup {
    /// The group's priority (`appearance.md` §4.3).
    pub priority: f32,
    /// Shader override (0x2B), if any.
    pub shader: Option<Did>,
    /// The material (0x30).
    pub material: Did,
    /// The `MeshTypeID` slot ([`SLOT_ALL`] for every part).
    pub slot: u32,
    /// Matched against surface records' binding keys.
    pub binding_key: u32,
}

/// What a cell's appearance list does to its mesh (module docs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellLook {
    /// The groups, ascending priority, ties in application order.
    pub groups: Vec<LookGroup>,
    /// Parts the chosen blocks offered (not drawn on a cell).
    pub parts_ignored: usize,
}

impl CellLook {
    /// The layering of a submesh whose surface record has `binding_key`, of a mesh in slot
    /// `mesh_slot` (`None` when unknown: only `All` groups apply): `None` when no group
    /// changes it (the surface's own look). The groups that match (`material.md` §9.3: the
    /// group's key equals the surface's or is 2, its slot is `All` or the mesh's, never 0)
    /// apply in list order through [`Layering::from_groups`], the one rule of every figure
    /// path: a shader-only group replaces the shader, the last override wins.
    #[must_use]
    pub fn layering(&self, binding_key: u32, mesh_slot: Option<u32>) -> Option<Layering> {
        Layering::from_groups(
            self.groups
                .iter()
                .filter(|g| {
                    tessera_material::group_binding_matches(g.binding_key, binding_key)
                        && g.slot != 0
                        && (g.slot == SLOT_ALL || Some(g.slot) == mesh_slot)
                })
                .map(|g| (g.shader, g.material)),
            MAX_LAYERS,
        )
    }

    /// Whether the look changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// The block an element's `modifier` picks in `entry` (`appearance.md` §3.2.1): none for a
/// modifier of exactly −1.0; otherwise the block with the greatest `variant` not above the
/// modifier, walking the blocks in stored order and stopping before the first greater one;
/// none when the first block's `variant` is already greater (a NaN modifier picks none).
#[must_use]
pub fn choose_block(entry: &AppearanceEntry, modifier: f32) -> Option<&AppearanceBlock> {
    #[allow(clippy::float_cmp)]
    if modifier == -1.0 {
        return None;
    }
    let mut chosen = None;
    for b in &entry.blocks {
        if b.variant > modifier || modifier.is_nan() {
            break;
        }
        chosen = Some(b);
    }
    chosen
}

/// The look of a cell with property set `set` (module docs): `None` when the set has no
/// `Appearance_InstanceList` or the list changes nothing. `record` reads an appearance
/// record (`None`: missing or broken; the element contributes nothing).
pub fn cell_look(
    set: &PropertySet,
    pids: &AppearanceListPids,
    mut record: impl FnMut(Did) -> Option<Arc<AppearanceRecord>>,
) -> Option<CellLook> {
    let list = pids.instance_list?;
    set.get(list)?;
    let mut look = CellLook::default();
    for el in pids.read_list(set, list).into_iter().take(MAX_ELEMENTS) {
        let file = el.file_in(set);
        if file.is_null() {
            continue;
        }
        let Some(rec) = record(file) else {
            continue;
        };
        let Some(block) = rec.entry(el.key).and_then(|e| choose_block(e, el.modifier)) else {
            continue;
        };
        look.parts_ignored = look.parts_ignored.saturating_add(block.parts.len());
        for (priority, g) in block.groups_with_priority() {
            if look.groups.len() >= MAX_GROUPS {
                break;
            }
            look.groups.push(LookGroup {
                priority,
                shader: Some(g.shader).filter(|d| !d.is_null()),
                material: g.material,
                slot: g.slot(),
                binding_key: g.binding_key,
            });
        }
    }
    // Ascending priority; the sort is stable, so ties keep the application order. A NaN
    // priority sorts last (`total_cmp`).
    look.groups
        .sort_by(|a, b| a.priority.total_cmp(&b.priority));
    if look.parts_ignored > 0 {
        tracing::debug!(
            parts = look.parts_ignored,
            "cell appearance: parts offered to a cell are not drawn"
        );
    }
    (!look.is_empty()).then_some(look)
}

#[derive(Default)]
struct State {
    /// Interned looks: index − 1 → (cell geometry, look).
    looks: Vec<(Did, Arc<CellLook>)>,
    by_pair: HashMap<(Did, Arc<CellLookKey>), u32>,
    records: HashMap<Did, Option<Arc<AppearanceRecord>>>,
}

/// A hashable form of a look (its groups' bits).
#[derive(Debug, PartialEq, Eq, Hash)]
struct CellLookKey(Vec<(u32, u32, u32, u32, u32)>);

impl CellLookKey {
    fn of(look: &CellLook) -> Self {
        Self(
            look.groups
                .iter()
                .map(|g| {
                    (
                        g.priority.to_bits(),
                        g.shader.map_or(0, |d| d.0),
                        g.material.0,
                        g.slot,
                        g.binding_key,
                    )
                })
                .collect(),
        )
    }
}

/// The looks of the walker's cells (module docs), shared by the block workers that intern
/// them and the object layer that requests and decodes them.
pub struct CellLooks {
    pids: AppearanceListPids,
    source: Arc<dyn WorldSource>,
    state: Mutex<State>,
}

impl std::fmt::Debug for CellLooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellLooks")
            .field("looks", &self.len())
            .finish_non_exhaustive()
    }
}

impl CellLooks {
    /// Looks read through `source`, with the appearance-list properties of `names`;
    /// `None` when the dictionary lacks `Appearance_Key` or `Appearance_AprFile`.
    #[must_use]
    pub fn new<N: PropertyNames + ?Sized>(names: &N, source: Arc<dyn WorldSource>) -> Option<Self> {
        let pids = AppearanceListPids::from_names(names)
            .map_err(|e| tracing::warn!("cell appearance off: {e}"))
            .ok()?;
        pids.instance_list?;
        Some(Self {
            pids,
            source,
            state: Mutex::new(State::default()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Looks interned so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().looks.len()
    }

    /// Whether no look is interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// An appearance record, cached (missing or broken: `None`, cached too).
    fn record(&self, did: Did) -> Option<Arc<AppearanceRecord>> {
        if let Some(hit) = self.lock().records.get(&did) {
            return hit.clone();
        }
        let rec = match self.source.read(did) {
            Ok(Some(bytes)) => AppearanceRecord::parse_expecting(&bytes, did)
                .map_err(|e| tracing::debug!(%did, "cell appearance record unusable: {e}"))
                .ok()
                .map(Arc::new),
            Ok(None) => None,
            Err(e) => {
                // Not cached: a pipeline failure may pass.
                tracing::debug!(%did, "cell appearance record unreadable: {e}");
                return None;
            }
        };
        let mut st = self.lock();
        if st.records.len() >= MAX_CACHED_RECORDS {
            st.records.clear();
        }
        st.records.insert(did, rec.clone());
        rec
    }

    /// The part id cell geometry `geometry` with property set `set` is drawn through: its
    /// own DID without a look, else the look's interned id (module docs).
    #[must_use]
    pub fn part_for(&self, geometry: Did, set: Option<&PropertySet>) -> u32 {
        let Some(look) = set.and_then(|s| cell_look(s, &self.pids, |d| self.record(d))) else {
            return geometry.0;
        };
        self.intern(geometry, look)
    }

    /// Interns (`geometry`, `look`): its part id, the same for equal pairs; `geometry`'s
    /// own DID for an empty look or once [`MAX_LOOKS`] are interned.
    #[must_use]
    pub fn intern(&self, geometry: Did, look: CellLook) -> u32 {
        if look.is_empty() {
            return geometry.0;
        }
        let key = (geometry, Arc::new(CellLookKey::of(&look)));
        let mut st = self.lock();
        if let Some(&i) = st.by_pair.get(&key) {
            return LOOK_PART_BASE | i;
        }
        if st.looks.len() >= MAX_LOOKS {
            tracing::debug!(%geometry, "cell appearance: look limit reached; default look");
            return geometry.0;
        }
        st.looks.push((geometry, Arc::new(look)));
        let Ok(i) = u32::try_from(st.looks.len()) else {
            return geometry.0;
        };
        st.by_pair.insert(key, i);
        LOOK_PART_BASE | i
    }

    /// The asset key part id `part` is requested by: a look's cell geometry with the look
    /// index as variant, else `part` itself with variant 0.
    #[must_use]
    pub fn asset_key(&self, part: u32) -> AssetKey {
        if let Some(i) = look_index(part)
            && let Some((geometry, _)) = self.lock().looks.get(i as usize - 1)
        {
            return AssetKey::new(geometry.0, i);
        }
        AssetKey::new(part, 0)
    }

    /// The look of asset variant `index` ([`CellLooks::asset_key`]).
    #[must_use]
    pub fn look(&self, index: u32) -> Option<Arc<CellLook>> {
        let i = usize::try_from(index).ok()?.checked_sub(1)?;
        self.lock().looks.get(i).map(|(_, l)| Arc::clone(l))
    }
}

/// The look index of a part id ([`LOOK_PART_BASE`] | index), if it is one.
#[must_use]
pub fn look_index(part: u32) -> Option<u32> {
    (part & 0xFF00_0000 == LOOK_PART_BASE)
        .then_some(part & 0x00FF_FFFF)
        .filter(|i| *i != 0)
}

/// The part id of an asset key ([`CellLooks::asset_key`] inverted): a look's id for a
/// variant other than 0, else the DID.
#[must_use]
pub fn part_of(key: AssetKey) -> u32 {
    if key.variant != 0 && key.variant <= 0x00FF_FFFF {
        LOOK_PART_BASE | key.variant
    } else {
        key.did
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::appearance::MaterialGroup;
    use tessera_formats::{PropertyEntry, PropertyValue};
    use tessera_world::MemoryWorldSource;

    fn block(variant: f32, groups: Vec<MaterialGroup>, first_priority: f32) -> AppearanceBlock {
        AppearanceBlock {
            variant,
            group_param: first_priority,
            groups,
            ..AppearanceBlock::default()
        }
    }

    fn group(material: u32, key: u32, next_priority: f32) -> MaterialGroup {
        MaterialGroup {
            shader: Did(0),
            material: Did(material),
            a: SLOT_ALL,
            binding_key: key,
            g: next_priority,
        }
    }

    #[test]
    fn blocks_follow_the_modifier_rule() {
        // §3.2.1: −1 none; the greatest variant not above; below the first: none.
        let e = AppearanceEntry {
            key: 0x1000_0001,
            blocks: vec![
                block(0.0, vec![], 0.0),
                block(0.37, vec![], 0.0),
                block(1.0, vec![], 0.0),
            ],
        };
        let v = |m: f32| choose_block(&e, m).map(|b| b.variant);
        assert_eq!(v(-1.0), None);
        assert_eq!(v(0.36), Some(0.0));
        assert_eq!(v(0.37), Some(0.37));
        assert_eq!(v(0.99), Some(0.37));
        assert_eq!(v(5.0), Some(1.0));
        assert_eq!(v(-0.5), None);
        assert_eq!(v(f32::NAN), None);
        let only_base = AppearanceEntry {
            key: 1,
            blocks: vec![block(1.0, vec![], 0.0)],
        };
        assert!(
            choose_block(&only_base, 0.0).is_none(),
            "below the first block"
        );
    }

    #[test]
    fn groups_layer_by_priority_and_binding_key() {
        let look = CellLook {
            groups: vec![
                LookGroup {
                    priority: 1.0,
                    shader: Some(Did(0x2B00_0001)),
                    material: Did(0x3000_0001),
                    slot: SLOT_ALL,
                    binding_key: 7,
                },
                LookGroup {
                    priority: 2.0,
                    shader: None,
                    material: Did(0x3000_0002),
                    slot: SLOT_ALL,
                    binding_key: 7,
                },
                LookGroup {
                    priority: 3.0,
                    shader: Some(Did(0x2B00_0003)),
                    material: Did(0x3000_0003),
                    slot: 99,
                    binding_key: 7,
                },
                LookGroup {
                    priority: 4.0,
                    shader: None,
                    material: Did(0x3000_0004),
                    slot: SLOT_ALL,
                    binding_key: 8,
                },
            ],
            parts_ignored: 0,
        };
        let l = look.layering(7, None).expect("matches");
        assert_eq!(l.materials, [Did(0x3000_0001), Did(0x3000_0002)]);
        assert_eq!(l.shader_override, Some(Did(0x2B00_0001)));
        // The slot-99 group applies to a mesh in slot 99, and its shader wins (later).
        let l = look.layering(7, Some(99)).expect("matches");
        assert_eq!(l.materials.len(), 3);
        assert_eq!(l.shader_override, Some(Did(0x2B00_0003)));
        assert!(look.layering(9, None).is_none(), "no group for this key");
    }

    /// `material.md` §9.3 rules 1–2: a group without a material still replaces the shader
    /// (it is not skipped), the last override applied wins, binding key 2 matches every
    /// surface and slot 0 matches no mesh.
    #[test]
    fn shader_only_groups_and_the_last_override() {
        let group =
            |priority: f32, shader: Option<u32>, material: u32, slot: u32, key: u32| LookGroup {
                priority,
                shader: shader.map(Did),
                material: Did(material),
                slot,
                binding_key: key,
            };
        let look = CellLook {
            groups: vec![
                group(1.0, Some(0x2B00_0001), 0x3000_0001, SLOT_ALL, 7),
                group(2.0, Some(0x2B00_0002), 0, SLOT_ALL, 7),
                group(3.0, None, 0x3000_0003, SLOT_ALL, 2),
                group(4.0, Some(0x2B00_0004), 0x3000_0004, 0, 7),
            ],
            parts_ignored: 0,
        };
        let l = look.layering(7, Some(5)).expect("matches");
        assert_eq!(l.shader_override, Some(Did(0x2B00_0002)));
        assert_eq!(l.materials, [Did(0x3000_0001), Did(0x3000_0003)]);
        // Only the key-2 group reaches another surface.
        let l = look.layering(8, None).expect("matches");
        assert_eq!(l.shader_override, None);
        assert_eq!(l.materials, [Did(0x3000_0003)]);
        // A shader-only group alone.
        let only = CellLook {
            groups: vec![group(1.0, Some(0x2B00_0009), 0, SLOT_ALL, 7)],
            parts_ignored: 0,
        };
        let l = only
            .layering(7, None)
            .expect("a shader-only group is applied");
        assert_eq!(l.shader_override, Some(Did(0x2B00_0009)));
        assert!(l.materials.is_empty());
    }

    /// A record with entry `key` whose blocks are `blocks`.
    fn record(did: u32, key: u32, blocks: Vec<AppearanceBlock>) -> AppearanceRecord {
        AppearanceRecord {
            did: Did(did),
            linked: Vec::new(),
            entries: vec![AppearanceEntry { key, blocks }],
            ..AppearanceRecord::default()
        }
    }

    const P_LIST: u32 = 0x1000_0901;
    const P_KEY: u32 = 0x1000_0902;
    const P_FILE: u32 = 0x1000_0903;
    const P_MOD: u32 = 0x1000_0904;

    fn pids() -> AppearanceListPids {
        AppearanceListPids {
            class_list: None,
            instance_list: Some(P_LIST),
            key: P_KEY,
            file: P_FILE,
            modifier: Some(P_MOD),
            toggle_file: None,
            toggle_property: None,
        }
    }

    fn element(key: u32, file: u32, modifier: f32) -> PropertyValue {
        PropertyValue::Struct(tessera_formats::PropertyStruct {
            size: tessera_formats::TSize::default(),
            fields: vec![
                PropertyEntry {
                    pid: P_KEY,
                    value: PropertyValue::Enum(key),
                },
                PropertyEntry {
                    pid: P_FILE,
                    value: PropertyValue::DidRef(Did(file)),
                },
                PropertyEntry {
                    pid: P_MOD,
                    value: PropertyValue::Float(modifier),
                },
            ],
        })
    }

    fn set_with(elements: Vec<PropertyValue>) -> PropertySet {
        PropertySet {
            size: tessera_formats::TSize::default(),
            entries: vec![PropertyEntry {
                pid: P_LIST,
                value: PropertyValue::Array(tessera_formats::PropertyArray {
                    count: u32::try_from(elements.len()).unwrap(),
                    elements: elements
                        .into_iter()
                        .map(|value| PropertyEntry { pid: 0, value })
                        .collect(),
                }),
            }],
        }
    }

    #[test]
    fn a_cell_list_becomes_one_priority_ordered_group_list() {
        let a = Arc::new(record(
            0x2000_0001,
            0x1000_0010,
            vec![
                block(0.0, vec![group(0x3000_0010, 5, 0.0)], 2.0),
                block(1.0, vec![group(0x3000_0011, 5, 0.0)], 2.0),
            ],
        ));
        let b = Arc::new(record(
            0x2000_0002,
            0x1000_0020,
            vec![block(0.5, vec![group(0x3000_0020, 5, 0.0)], 1.0)],
        ));
        let read = |d: Did| match d.0 {
            0x2000_0001 => Some(Arc::clone(&a)),
            0x2000_0002 => Some(Arc::clone(&b)),
            _ => None,
        };
        let set = set_with(vec![
            element(0x1000_0010, 0x2000_0001, 1.0),
            element(0x1000_0020, 0x2000_0002, 0.7),
            element(0x1000_0010, 0x2000_0001, -1.0),
            element(0x1000_0030, 0x2000_0001, 1.0),
            element(0x1000_0010, 0x2000_0099, 1.0),
        ]);
        let look = cell_look(&set, &pids(), read).expect("a look");
        // Block 1.0 of the first (priority 2) and block 0.5 of the second (priority 1):
        // ascending priority puts the second first; the −1 element, the missing key and
        // the missing record contribute nothing.
        let mats: Vec<u32> = look.groups.iter().map(|g| g.material.0).collect();
        assert_eq!(mats, [0x3000_0020, 0x3000_0011]);
        assert!(cell_look(&PropertySet::default(), &pids(), read).is_none());
        assert!(cell_look(&set_with(vec![]), &pids(), read).is_none());
    }

    #[test]
    fn looks_intern_per_geometry_and_content() {
        let looks = CellLooks {
            pids: pids(),
            source: Arc::new(MemoryWorldSource::new()),
            state: Mutex::new(State::default()),
        };
        let one = CellLook {
            groups: vec![LookGroup {
                priority: 0.0,
                shader: None,
                material: Did(0x3000_0001),
                slot: SLOT_ALL,
                binding_key: 1,
            }],
            parts_ignored: 0,
        };
        let geo = Did(0x0100_0001);
        let p = looks.intern(geo, one.clone());
        assert_eq!(look_index(p), Some(1));
        assert_eq!(looks.intern(geo, one.clone()), p, "equal pairs share an id");
        let q = looks.intern(Did(0x0100_0002), one.clone());
        assert_ne!(q, p, "another geometry");
        assert_eq!(looks.intern(geo, CellLook::default()), geo.0, "no look");
        let k = looks.asset_key(p);
        assert_eq!((k.did, k.variant), (geo.0, 1));
        assert_eq!(part_of(k), p);
        assert_eq!(looks.look(1).as_deref(), Some(&one));
        assert!(looks.look(0).is_none() && looks.look(99).is_none());
        // Real DIDs pass through.
        let k = looks.asset_key(0x0600_0001);
        assert_eq!((k.did, k.variant), (0x0600_0001, 0));
        assert_eq!(part_of(k), 0x0600_0001);
        assert_eq!(look_index(0x0600_0001), None);
        assert_eq!(look_index(LOOK_PART_BASE), None);
        // A cell set without a list keeps its geometry.
        assert_eq!(looks.part_for(geo, None), geo.0);
        assert_eq!(looks.part_for(geo, Some(&PropertySet::default())), geo.0);
    }
}
