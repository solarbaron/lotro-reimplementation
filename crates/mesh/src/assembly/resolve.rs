// SPDX-License-Identifier: GPL-3.0-or-later
//! The assembly algorithm of `appearance.md` §8.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::appearance::{
    AppearanceBlock, AppearanceKeyInfo, PartSource, SLOT_ALL, SlotTable, key_application_order,
};
use tessera_formats::avatar::{AvatarBody, parse_slider_values, scale_vector, slider_value_pieces};
use tessera_formats::item::{ItemLook, ItemVisual, WORN_MODIFIER};
use tessera_formats::{AppearanceRecord, Did, PropertySet, PropertyValue, Setup};

use super::source::{AssemblySource, Lookup};
use super::vocab::{AssemblyVocabulary, PlacementScale};
use super::{
    AssembledCharacter, AssembledPart, AssemblyError, AssemblyLimits, AssemblyOptions,
    CharacterSpec, ClipInputs, Diagnostic, DiagnosticCategory as Cat, EquippedItem, HeldPart,
    HeldPlacement, LayerKind, ListOrigin, MAX_LINK_DEPTH, MaterialLayer, ModelScale, PartOrigin,
    RigKind, ScriptAppearance, SkeletonSource, SliderInputs, SourceRef, WieldState,
};

/// Assembles figures (`appearance.md` §8) from the records of an [`AssemblySource`].
///
/// ```no_run
/// use tessera_mesh::assembly::{
///     AssemblyVocabulary, CharacterAssembler, CharacterSpec, EquippedItem, GameDataSource,
/// };
/// use tessera_props::GameData;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = GameData::open("/path/to/install")?;
/// let vocab = AssemblyVocabulary::from_game_data(&data)?;
/// let mut assembler = CharacterAssembler::new(GameDataSource::new(&data)?, vocab);
/// let spec = CharacterSpec::new(0x17, 0x1000).wearing(EquippedItem::new(0x7900_0420));
/// let figure = assembler.assemble_character(&spec)?;
/// for part in figure.parts.iter().filter(|p| !p.hidden) {
///     // Draw part.mesh in the setup's model space, skinned to figure.skeleton.
///     let _ = (part.mesh, &part.materials);
/// }
/// for d in &figure.diagnostics {
///     eprintln!("{}: {:?} {:?}", d.category, d.source, d.did);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct CharacterAssembler<S> {
    source: S,
    vocab: AssemblyVocabulary,
    limits: AssemblyLimits,
    options: AssemblyOptions,
    /// Tagfile bytes per setup, shared by every figure on that setup (keyed by the
    /// setup's `Arc` so that a replaced setup record is copied again).
    tagfiles: HashMap<Did, (Arc<Setup>, Arc<[u8]>)>,
}

/// Setups whose tagfile an assembler keeps shared; a full cache is emptied.
const TAGFILE_CACHE_ENTRIES: usize = 256;

/// What assembly needs to know about a part mesh.
#[derive(Debug, Clone, Copy)]
struct MeshInfo {
    slot: u32,
    stub: bool,
    skinned: bool,
}

/// One part offered to the slot table.
#[derive(Debug, Clone, Copy)]
struct Offer {
    mesh: Did,
    info: MeshInfo,
    source: SourceRef,
    record: Did,
    key: u32,
    variant: f32,
}

/// One (key, modifier) pair of a record in the figure's appearance (§4.7 rule 2).
#[derive(Debug, Clone, Copy)]
struct KeyUse {
    key: u32,
    modifier: f32,
    /// The element that set it last (for diagnostics and part origins).
    source: SourceRef,
}

/// A record of the figure's appearance with its keys, in the order they were added.
#[derive(Debug)]
struct RecordUse {
    did: Did,
    keys: Vec<KeyUse>,
}

/// Per-assembly state.
#[derive(Debug)]
struct Ctx {
    limits: AssemblyLimits,
    diags: Vec<Diagnostic>,
    dropped: usize,
    /// Mesh lookups, with the category of a failed one.
    meshes: HashMap<Did, Result<MeshInfo, Cat>>,
    /// Decoded records of this assembly, with the category of a failed lookup.
    records: HashMap<Did, Result<Arc<AppearanceRecord>, Cat>>,
    /// The figure's appearance: records in first-entry order (§4.7 rule 2).
    uses: Vec<RecordUse>,
    use_index: HashMap<Did, usize>,
    /// Elements taken into `uses` (bounded by `max_list_elements`).
    elements: usize,
    /// The figure-wide material-group list (§4.10), kept sorted by priority.
    groups: Vec<MaterialLayer>,
    group_limit_reported: bool,
    /// Block applications so far.
    applications: usize,
    offers: Vec<Offer>,
    table: SlotTable,
    /// Default parts entered (they share `max_parts` with the offers).
    n_defaults: usize,
    held: Vec<HeldPart>,
}

impl Ctx {
    fn new(limits: AssemblyLimits) -> Self {
        Self {
            limits,
            diags: Vec::new(),
            dropped: 0,
            meshes: HashMap::new(),
            records: HashMap::new(),
            uses: Vec::new(),
            use_index: HashMap::new(),
            elements: 0,
            groups: Vec::new(),
            group_limit_reported: false,
            applications: 0,
            offers: Vec::new(),
            table: SlotTable::new(),
            n_defaults: 0,
            held: Vec::new(),
        }
    }

    fn diag(&mut self, category: Cat, source: SourceRef, did: Option<Did>, key: Option<u32>) {
        if self.diags.len() < self.limits.max_diagnostics {
            self.diags.push(Diagnostic {
                category,
                source,
                did,
                key,
            });
        } else {
            self.dropped += 1;
        }
    }

    /// Adds one element to the figure's appearance (§4.7 rule 2): a record named for the
    /// first time is appended; a record named again keeps its position, and the element's
    /// key replaces that key's modifier or is added.
    fn add_element(&mut self, source: SourceRef, record: Did, key: u32, modifier: f32) {
        if self.elements >= self.limits.max_list_elements {
            self.diag(Cat::LimitExceeded, source, Some(record), Some(key));
            return;
        }
        self.elements += 1;
        let use_ = KeyUse {
            key,
            modifier,
            source,
        };
        match self.use_index.get(&record) {
            Some(&i) => {
                let Some(r) = self.uses.get_mut(i) else {
                    return;
                };
                match r.keys.iter_mut().find(|k| k.key == key) {
                    Some(k) => *k = use_,
                    None => r.keys.push(use_),
                }
            }
            None => {
                self.use_index.insert(record, self.uses.len());
                self.uses.push(RecordUse {
                    did: record,
                    keys: vec![use_],
                });
            }
        }
    }

    /// Inserts the groups of `block` into the figure-wide list (§4.10): ascending group
    /// priority, a group after the groups of equal priority already there.
    fn insert_groups(
        &mut self,
        source: SourceRef,
        record: Did,
        block: &AppearanceBlock,
        kind: LayerKind,
    ) {
        for (priority, g) in block.groups_with_priority() {
            if self.groups.len() >= self.limits.max_total_layers {
                if !self.group_limit_reported {
                    self.group_limit_reported = true;
                    self.diag(Cat::LimitExceeded, source, Some(record), None);
                }
                return;
            }
            let at = self
                .groups
                .partition_point(|l| l.priority.total_cmp(&priority) != Ordering::Greater);
            self.groups.insert(
                at,
                MaterialLayer::from_group(kind, priority, g, record, block.variant),
            );
        }
    }

    /// The groups of the figure-wide list that apply to a part in `slot` (`All` or that
    /// slot; every group when `slot` is `None`), in list order, at most `max_layers`. A
    /// group whose slot is 0 matches no part (`material.md` §9.3 binding rule).
    fn layers_for(&self, slot: Option<u32>) -> Vec<MaterialLayer> {
        self.groups
            .iter()
            .filter(|l| l.group_slot != 0)
            .filter(|l| slot.is_none_or(|s| l.group_slot == SLOT_ALL || l.group_slot == s))
            .take(self.limits.max_layers)
            .copied()
            .collect()
    }
}

/// What a placed figure brings besides its body.
struct Input<'a> {
    start: Did,
    object_set: Option<PropertySet>,
    instance: Option<&'a PropertySet>,
    spec: Option<&'a CharacterSpec>,
    scripts: &'a [ScriptAppearance],
}

fn source_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> AssemblyError {
    AssemblyError::Source(Box::new(e))
}

/// Sort key of a worn item: its lowest `ContainerSlot` bit (`appearance.md` §4.7
/// Decision: ascending slot order); items without a slot last.
fn slot_order(bits: Option<u64>) -> u32 {
    match bits {
        Some(b) if b != 0 => b.trailing_zeros(),
        _ => u32::MAX,
    }
}

fn as_u32(v: &PropertyValue) -> Option<u32> {
    v.as_bits().and_then(|b| u32::try_from(b).ok())
}

/// A boolean read: a boolean, or a number other than 0.
fn as_true(v: &PropertyValue) -> bool {
    v.as_f64().is_some_and(|x| x != 0.0)
}

impl<S: AssemblySource> CharacterAssembler<S> {
    /// An assembler over `source` with `vocab`, the default [`AssemblyLimits`] and the
    /// default [`AssemblyOptions`].
    #[must_use]
    pub fn new(source: S, vocab: AssemblyVocabulary) -> Self {
        Self {
            source,
            vocab,
            limits: AssemblyLimits::default(),
            options: AssemblyOptions::default(),
            tagfiles: HashMap::new(),
        }
    }

    /// The same assembler with other limits.
    #[must_use]
    pub fn with_limits(mut self, limits: AssemblyLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The same assembler with other options (the updated-avatar option of §3.3).
    #[must_use]
    pub fn with_options(mut self, options: AssemblyOptions) -> Self {
        self.options = options;
        self
    }

    /// The options in use.
    #[must_use]
    pub fn options(&self) -> AssemblyOptions {
        self.options
    }

    /// The vocabulary in use.
    #[must_use]
    pub fn vocabulary(&self) -> &AssemblyVocabulary {
        &self.vocab
    }

    /// The record source.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// Consumes the assembler, returning the source.
    pub fn into_source(self) -> S {
        self.source
    }

    /// Assembles a playable figure: (species, sex) through the race table to the avatar
    /// object, then [`CharacterAssembler::assemble_avatar`].
    ///
    /// # Errors
    ///
    /// [`AssemblyError::NoSuchBody`] when the race table has no such entry; otherwise as
    /// [`CharacterAssembler::assemble_avatar`].
    pub fn assemble_character(
        &mut self,
        spec: &CharacterSpec,
    ) -> Result<AssembledCharacter, AssemblyError> {
        self.assemble_character_with(spec, &[])
    }

    /// [`CharacterAssembler::assemble_character`] with the Appearance nodes the figure's
    /// start-up scripts ran (§7.4), applied after the worn items.
    ///
    /// # Errors
    ///
    /// As [`CharacterAssembler::assemble_character`].
    pub fn assemble_character_with(
        &mut self,
        spec: &CharacterSpec,
        scripts: &[ScriptAppearance],
    ) -> Result<AssembledCharacter, AssemblyError> {
        let avatar = self
            .source
            .race_avatar(spec.species, spec.sex)
            .map_err(source_error)?
            .ok_or(AssemblyError::NoSuchBody {
                species: spec.species,
                sex: spec.sex,
            })?;
        self.assemble_avatar_with(avatar, spec, scripts)
    }

    /// Assembles `spec` on the body of `avatar` (a player-avatar game object or a 0x47
    /// entity): the setup's default parts, the avatar's appearance lists with the chosen
    /// styles, the worn items, the held items, sliders and heights. `spec.species` and
    /// `spec.sex` choose the wardrobe structs of the items and the species rows of the
    /// placements.
    ///
    /// # Errors
    ///
    /// [`AssemblyError::Body`] / [`AssemblyError::Setup`] when there is no body to dress;
    /// [`AssemblyError::Source`] when the source fails. Everything else is a diagnostic.
    pub fn assemble_avatar(
        &mut self,
        avatar: Did,
        spec: &CharacterSpec,
    ) -> Result<AssembledCharacter, AssemblyError> {
        self.assemble_avatar_with(avatar, spec, &[])
    }

    /// [`CharacterAssembler::assemble_avatar`] with script elements (§7.4).
    ///
    /// # Errors
    ///
    /// As [`CharacterAssembler::assemble_avatar`].
    pub fn assemble_avatar_with(
        &mut self,
        avatar: Did,
        spec: &CharacterSpec,
        scripts: &[ScriptAppearance],
    ) -> Result<AssembledCharacter, AssemblyError> {
        let body = self.body(avatar)?;
        self.assemble_body(
            body,
            Input {
                start: avatar,
                object_set: None,
                instance: None,
                spec: Some(spec),
                scripts,
            },
        )
    }

    /// Assembles a placed figure (`appearance.md` §7.4): a weenie game object (0x70–0x76,
    /// through its `PhysObj`) or a 0x47 entity (a Physics SED's object), with the
    /// instance property set of its placement if there is one (the landblock weenie list,
    /// §7.2). Sources: the setup's default parts, the templates' appearance sections, the
    /// entity's class list, then the instance list (the instance set's, else the game
    /// object's, else the entity's own), then the NPC props (§5.5).
    ///
    /// # Errors
    ///
    /// As [`CharacterAssembler::assemble_avatar`].
    pub fn assemble_object(
        &mut self,
        start: Did,
        instance: Option<&PropertySet>,
    ) -> Result<AssembledCharacter, AssemblyError> {
        self.assemble_object_with(start, instance, &[])
    }

    /// [`CharacterAssembler::assemble_object`] with the Appearance nodes the figure's
    /// start-up scripts ran (§7.4 Rule: the random-appearance outfit), applied after the
    /// lists.
    ///
    /// # Errors
    ///
    /// As [`CharacterAssembler::assemble_avatar`].
    pub fn assemble_object_with(
        &mut self,
        start: Did,
        instance: Option<&PropertySet>,
        scripts: &[ScriptAppearance],
    ) -> Result<AssembledCharacter, AssemblyError> {
        let body = self.body(start)?;
        let object_set = if matches!(start.prefix(), 0x70..=0x76) {
            self.source
                .object_properties(start)
                .map_err(source_error)?
                .found()
        } else {
            None
        };
        self.assemble_body(
            body,
            Input {
                start,
                object_set,
                instance,
                spec: None,
                scripts,
            },
        )
    }

    fn body(&mut self, start: Did) -> Result<AvatarBody, AssemblyError> {
        match self.source.body(start).map_err(source_error)? {
            Lookup::Found(b) => Ok(b),
            Lookup::Missing => Err(AssemblyError::Body {
                start,
                reason: "missing".into(),
            }),
            Lookup::Invalid(reason) => Err(AssemblyError::Body { start, reason }),
        }
    }

    fn mesh_info(
        &mut self,
        cx: &mut Ctx,
        mesh: Did,
        source: SourceRef,
    ) -> Result<Option<MeshInfo>, AssemblyError> {
        let info = match cx.meshes.get(&mesh) {
            Some(i) => *i,
            None => {
                let i = match self.source.mesh(mesh).map_err(source_error)? {
                    Lookup::Found(s) => Ok(MeshInfo {
                        slot: s.slot(),
                        stub: s.is_stub(),
                        skinned: s.is_skinned(),
                    }),
                    Lookup::Missing => Err(Cat::MeshMissing),
                    Lookup::Invalid(_) => Err(Cat::MeshUndecodable),
                };
                cx.meshes.insert(mesh, i);
                i
            }
        };
        // Every reference is reported, with the category of the first lookup.
        Ok(info
            .map_err(|cat| cx.diag(cat, source, Some(mesh), None))
            .ok())
    }

    /// The 0x20 record `did`, or a diagnostic.
    fn record(
        &mut self,
        cx: &mut Ctx,
        did: Did,
        source: SourceRef,
        key: u32,
    ) -> Result<Option<Arc<AppearanceRecord>>, AssemblyError> {
        if did.is_null() {
            cx.diag(Cat::NullAppearance, source, None, Some(key));
            return Ok(None);
        }
        let r = match cx.records.get(&did) {
            Some(r) => r.clone(),
            None => {
                let r = match self.source.appearance(did).map_err(source_error)? {
                    Lookup::Found(r) => Ok(r),
                    Lookup::Missing => Err(Cat::AppearanceMissing),
                    Lookup::Invalid(_) => Err(Cat::AppearanceUndecodable),
                };
                cx.records.insert(did, r.clone());
                r
            }
        };
        Ok(r.map_err(|cat| cx.diag(cat, source, Some(did), Some(key)))
            .ok())
    }

    /// The layer kind of the blocks of `key`.
    fn kind_of(&self, key: u32) -> LayerKind {
        if Some(key) == self.vocab.clothing_color {
            LayerKind::Dye
        } else if Some(key) == self.vocab.clothing_color_secondary {
            LayerKind::SecondaryDye
        } else {
            LayerKind::Group
        }
    }

    /// Applies one (key, modifier) pair of record `did` (`appearance.md` §4.7 rules 1 and
    /// 4, §3.2.1, §4.10): first the same pair to each linked record (recursively, a record
    /// already on `path` skipped), then the block the modifier chooses in the record's own
    /// entry for the key: its groups into the figure-wide list, its parts offered to their
    /// slots.
    #[allow(clippy::too_many_arguments)]
    fn apply_key(
        &mut self,
        cx: &mut Ctx,
        did: Did,
        key: u32,
        modifier: f32,
        source: SourceRef,
        path: &mut Vec<Did>,
    ) -> Result<(), AssemblyError> {
        if cx.applications >= cx.limits.max_applications {
            cx.diag(Cat::LimitExceeded, source, Some(did), Some(key));
            return Ok(());
        }
        cx.applications += 1;
        let top = path.is_empty();
        let Some(rec) = self.record(cx, did, source, key)? else {
            return Ok(());
        };
        if !rec.linked.is_empty() {
            path.push(did);
            for &linked in &rec.linked {
                if path.contains(&linked) || linked == did {
                    cx.diag(Cat::LinkedRecordCycle, source, Some(linked), Some(key));
                    continue;
                }
                if path.len() >= MAX_LINK_DEPTH {
                    cx.diag(Cat::LimitExceeded, source, Some(linked), Some(key));
                    continue;
                }
                self.apply_key(cx, linked, key, modifier, source, path)?;
            }
            path.pop();
        }
        let kind = self.kind_of(key);
        let Some(entry) = rec.entry(key) else {
            // A linked record lacking the key contributes nothing (rule 4).
            if top {
                let cat = if kind == LayerKind::Group {
                    Cat::KeyMissing
                } else {
                    Cat::DyeEntryMissing
                };
                cx.diag(cat, source, Some(did), Some(key));
            }
            return Ok(());
        };
        let Some((_, block)) = entry.choose_block(modifier) else {
            cx.diag(Cat::NoBlock, source, Some(did), Some(key));
            return Ok(());
        };
        let item_own = matches!(source, SourceRef::Item(_)) && kind == LayerKind::Group;
        if kind != LayerKind::Group && block.variant != modifier {
            cx.diag(Cat::DyeInexact, source, Some(did), Some(key));
        }
        if item_own && top {
            if block.variant != WORN_MODIFIER {
                cx.diag(Cat::NoBaseBlock, source, Some(did), Some(key));
            }
            if block.parts.is_empty() {
                cx.diag(Cat::MaterialOnly, source, Some(did), Some(key));
            }
        }
        if !block.extras.is_empty() {
            cx.diag(Cat::BlockExtraIgnored, source, Some(did), Some(key));
        }
        cx.insert_groups(source, did, block, kind);
        for p in &block.parts {
            if cx.n_defaults + cx.offers.len() >= cx.limits.max_parts {
                cx.diag(Cat::LimitExceeded, source, Some(did), Some(key));
                break;
            }
            let Some(info) = self.mesh_info(cx, p.mesh, source)? else {
                // §10: the slot keeps its previous winner.
                continue;
            };
            let index = u32::try_from(cx.offers.len()).unwrap_or(u32::MAX);
            cx.table.offer(p.slot, p.mesh, p.priority(), index);
            cx.offers.push(Offer {
                mesh: p.mesh,
                info,
                source,
                record: did,
                key,
                variant: block.variant,
            });
        }
        Ok(())
    }

    /// Applies the figure's appearance: records in first-entry order, each record's keys in
    /// key-table order (§4.7 rules 2–3).
    fn apply_records(&mut self, cx: &mut Ctx) -> Result<(), AssemblyError> {
        let uses = std::mem::take(&mut cx.uses);
        let mut path = Vec::new();
        for r in &uses {
            let keys: Vec<u32> = r.keys.iter().map(|k| k.key).collect();
            for i in key_application_order(&keys) {
                let Some(k) = r.keys.get(i) else {
                    continue;
                };
                path.clear();
                self.apply_key(cx, r.did, k.key, k.modifier, k.source, &mut path)?;
            }
        }
        cx.uses = uses;
        Ok(())
    }

    /// The slot bits an equipped item takes: its own [`EquippedItem::slot`] when set, else
    /// its `Inventory_DefaultSlot`, with the hand rule of `character-panels.md` §7 (Tessera
    /// rule, for items equipped without a slot): when an earlier entry of the list already
    /// took the primary hand and the item's `Inventory_CompatibleSlot` has
    /// `Weapon_Secondary`, it goes to the secondary hand. `hands` collects the hands of the
    /// entries seen so far, in list order.
    fn hand_slot(
        &mut self,
        item: &EquippedItem,
        default: Option<u64>,
        hands: &mut Vec<super::WeaponSlot>,
    ) -> Result<Option<u64>, AssemblyError> {
        use super::WeaponSlot;
        let rule = &self.vocab.placement;
        let explicit = item.slot.filter(|b| *b != 0);
        let mut slot = item.effective_slot(default);
        let hand = slot.and_then(|s| rule.weapon_slot(s));
        if explicit.is_none()
            && hand == Some(WeaponSlot::Primary)
            && hands.contains(&WeaponSlot::Primary)
        {
            let secondary = rule.bits_of(WeaponSlot::Secondary);
            let fits = secondary != 0
                && self
                    .source
                    .compatible_slots(item.item)
                    .map_err(source_error)?
                    .is_some_and(|c| c & secondary != 0);
            if fits {
                slot = Some(secondary);
            }
        }
        if let Some(h) = slot.and_then(|s| rule.weapon_slot(s)) {
            hands.push(h);
        }
        Ok(slot)
    }

    /// Places a held item or prop at `location` of the wearer (`skeleton-anim.md` §8.4)
    /// with `placement`.
    #[allow(clippy::too_many_arguments)]
    fn place_held(
        &mut self,
        cx: &mut Ctx,
        wearer: &Setup,
        source: SourceRef,
        item: Option<Did>,
        entity: Did,
        location: u32,
        state: WieldState,
        placement: HeldPlacement,
    ) -> Result<(), AssemblyError> {
        let did = item.or(Some(entity));
        if cx.held.len() >= cx.limits.max_held {
            cx.diag(Cat::LimitExceeded, source, did, None);
            return Ok(());
        }
        let Some(hl) = wearer.holding_location(location) else {
            cx.diag(Cat::HoldingLocationMissing, source, did, Some(location));
            return Ok(());
        };
        let chain = match self.source.setup_chain(entity).map_err(source_error)? {
            Lookup::Found(c) if !c.is_empty() => c,
            _ => {
                cx.diag(Cat::HeldChainBroken, source, Some(entity), None);
                return Ok(());
            }
        };
        let Some(&setup_did) = chain.last() else {
            cx.diag(Cat::HeldChainBroken, source, Some(entity), None);
            return Ok(());
        };
        let Lookup::Found(setup) = self.source.setup(setup_did).map_err(source_error)? else {
            cx.diag(Cat::HeldChainBroken, source, Some(setup_did), None);
            return Ok(());
        };
        let scale = self
            .source
            .held_scale(entity)
            .map_err(source_error)?
            .unwrap_or([1.0; 3]);
        cx.held.push(HeldPart {
            source,
            item,
            entity,
            chain,
            setup: setup_did,
            parts: setup.parts.clone(),
            location,
            anchor: hl.anchor,
            frame: hl.frame,
            state,
            placement,
            scale,
        });
        Ok(())
    }

    /// The attachments of the sheathed-instrument script for the ranged item `item` with
    /// entity `phys_obj` on the figure whose scripts start at `start` (§5.6 "Stowed
    /// instruments"): `Script_Item_PhysObj_Ranged` is the entity and
    /// `Music_Cached_Instrument_Type` the item's `Item_Music_Instrument_Type`.
    fn instrument_attachments(
        &mut self,
        start: Did,
        item: Did,
        phys_obj: Did,
        key: u32,
    ) -> Result<Vec<super::ScriptMeshFx>, AssemblyError> {
        let mut props: Vec<(u32, PropertyValue)> = Vec::new();
        if let Some(p) = self.vocab.physobj_ranged {
            props.push((p, PropertyValue::DidRef(phys_obj)));
        }
        if let (Some(from), Some(to)) = (
            self.vocab.item_instrument_type,
            self.vocab.cached_instrument_type,
        ) && let Lookup::Found(set) =
            self.source.object_properties(item).map_err(source_error)?
            && let Some(v) = set.get(from)
        {
            props.push((to, v.clone()));
        }
        let mut out = self
            .source
            .script_mesh_fx(start, key, &props)
            .map_err(source_error)?;
        out.truncate(self.limits.max_held);
        Ok(out)
    }

    /// The instance list in use and where it came from (§7.4 step 4 Decision): a list
    /// property present in the instance set, else in the game object's set, else the
    /// entity's own.
    fn instance_list(&self, input: &Input<'_>) -> (Option<Vec<AppearanceKeyInfo>>, ListOrigin) {
        let present = |set: &PropertySet| {
            let lists = self.vocab.lists?;
            let pid = lists.instance_list?;
            set.get(pid)?;
            Some(lists.read_list(set, pid))
        };
        if let Some(l) = input.instance.and_then(present) {
            return (Some(l), ListOrigin::Instance);
        }
        if let Some(l) = input.object_set.as_ref().and_then(present) {
            return (Some(l), ListOrigin::Object);
        }
        (None, ListOrigin::Entity)
    }

    #[allow(clippy::too_many_lines)]
    fn assemble_body(
        &mut self,
        body: AvatarBody,
        input: Input<'_>,
    ) -> Result<AssembledCharacter, AssemblyError> {
        let setup = match self.source.setup(body.setup).map_err(source_error)? {
            Lookup::Found(s) => s,
            Lookup::Missing => {
                return Err(AssemblyError::Setup {
                    did: body.setup,
                    reason: "missing".into(),
                });
            }
            Lookup::Invalid(reason) => {
                return Err(AssemblyError::Setup {
                    did: body.setup,
                    reason,
                });
            }
        };
        let mut cx = Ctx::new(self.limits);

        // Step 2: the default body.
        let mut defaults = Vec::with_capacity(setup.parts.len().min(self.limits.max_parts));
        for (i, &mesh) in setup.parts.iter().enumerate() {
            if i >= self.limits.max_parts {
                cx.diag(Cat::LimitExceeded, SourceRef::Setup, Some(setup.did), None);
                break;
            }
            let info = self.mesh_info(&mut cx, mesh, SourceRef::Setup)?;
            defaults.push((u32::try_from(i).unwrap_or(u32::MAX), mesh, info));
        }
        cx.n_defaults = defaults.len();
        let rig = if defaults
            .iter()
            .any(|(_, _, i)| i.is_some_and(|i| Some(i.slot) == self.vocab.upper_body))
        {
            RigKind::Humanoid
        } else if defaults
            .iter()
            .any(|(_, _, i)| i.is_some_and(|i| i.skinned))
        {
            RigKind::Creature
        } else {
            RigKind::Static
        };

        // Instance properties replace object and entity properties of the same name: the
        // figure's own property lookup (§7.4 step 4 Decision, §15).
        let sets: [Option<&PropertySet>; 3] = [
            input.instance,
            input.object_set.as_ref(),
            Some(&body.properties),
        ];
        let prop = |pid: Option<u32>| {
            let pid = pid?;
            sets.iter().flatten().find_map(|s| s.get(pid))
        };
        // §3.3 Rule: the option store answers `GOP_AvatarUpdateVisible` before the figure's
        // values; any other toggle property reads through the figure's lookup (instance,
        // game object, entity) and, when absent, as its dictionary default, false when the
        // dictionary has none (`script-table.md` T28; confirmed 2026-10-09).
        let options = self.options;
        let update_visible = self.vocab.avatar_update_visible;
        let mut toggle_defaults: Vec<(u32, bool)> = Vec::new();
        {
            let (instance_list, _) = self.instance_list(&input);
            let toggles: Vec<u32> = body
                .template_appearance
                .iter()
                .chain(&body.class_list)
                .chain(instance_list.as_deref().unwrap_or(&body.instance_list))
                .filter_map(|e| e.watched_toggle())
                .filter(|&p| Some(p) != update_visible)
                .collect();
            for p in toggles {
                if toggle_defaults.len() >= 64 || toggle_defaults.iter().any(|(q, _)| *q == p) {
                    continue;
                }
                let d = self
                    .source
                    .property_default(p)
                    .map_err(source_error)?
                    .is_some_and(|v| as_true(&v));
                toggle_defaults.push((p, d));
            }
        }
        let toggle_true = |pid: u32| {
            if Some(pid) == update_visible {
                options.avatar_update_visible
            } else {
                match prop(Some(pid)) {
                    Some(v) => as_true(v),
                    None => toggle_defaults
                        .iter()
                        .find(|(q, _)| *q == pid)
                        .is_some_and(|(_, d)| *d),
                }
            }
        };

        let random_appearance = prop(self.vocab.random_appearance_type)
            .and_then(as_u32)
            .filter(|&v| self.vocab.is_random_appearance(v));
        if let Some(v) = random_appearance
            && input.scripts.is_empty()
        {
            cx.diag(
                Cat::RandomAppearance,
                SourceRef::Figure,
                Some(body.entity),
                Some(v),
            );
        }

        // Step 3: the appearance elements (§4.7 rule 2, §8 step 3).
        let (instance_list, instance_list_origin) = self.instance_list(&input);
        let mut watched: Vec<u32> = Vec::new();
        let lists = body
            .template_appearance
            .iter()
            .enumerate()
            .map(|(n, e)| (SourceRef::TemplateSection(n as u32), e))
            .chain(
                body.class_list
                    .iter()
                    .enumerate()
                    .map(|(n, e)| (SourceRef::ClassList(n as u32), e)),
            )
            .chain(
                instance_list
                    .as_deref()
                    .unwrap_or(&body.instance_list)
                    .iter()
                    .enumerate()
                    .map(|(n, e)| (SourceRef::InstanceList(n as u32), e)),
            );
        for (source, info) in lists {
            if info.is_ignored() {
                if info.file.is_null() {
                    cx.diag(Cat::NullAppearance, source, None, Some(info.key));
                }
                continue;
            }
            if let Some(p) = info.watched_toggle()
                && !watched.contains(&p)
                && watched.len() < 64
            {
                watched.push(p);
            }
            let file = info.file_for(toggle_true);
            // A player's chosen style replaces the modifier for its key (§8 step 3).
            let modifier = input
                .spec
                .and_then(|s| s.styles.iter().take(64).find(|c| c.key == info.key))
                .map_or(info.modifier, |c| c.modifier);
            cx.add_element(source, file, info.key, modifier);
        }

        // Worn items in ascending slot order, each with its dye elements (§4.7 Decision,
        // §4.9); held items are queued.
        let mut held_queue: Vec<(u32, EquippedItem, ItemVisual, Did)> = Vec::new();
        if let Some(spec) = input.spec {
            let mut order: Vec<(u32, u32, EquippedItem, ItemVisual)> = Vec::new();
            // The hands earlier entries took (`character-panels.md` §7 Tessera rule).
            let mut hands: Vec<super::WeaponSlot> = Vec::new();
            for (i, item) in spec.items.iter().enumerate() {
                let source = SourceRef::Item(i as u32);
                if i >= self.limits.max_items {
                    cx.diag(Cat::LimitExceeded, source, Some(item.item), None);
                    break;
                }
                match self.source.item(item.item).map_err(source_error)? {
                    Lookup::Found(mut v) => {
                        // The equipped slot replaces the default slot for the order and the
                        // placement script; without one, the hand rule picks it.
                        v.default_slot = self.hand_slot(item, v.default_slot, &mut hands)?;
                        order.push((slot_order(v.default_slot), i as u32, *item, v));
                    }
                    Lookup::Missing => cx.diag(Cat::ItemMissing, source, Some(item.item), None),
                    Lookup::Invalid(_) => {
                        cx.diag(Cat::ItemUndecodable, source, Some(item.item), None);
                    }
                }
            }
            order.sort_by_key(|(slot, i, _, _)| (*slot, *i));
            for (_, i, item, visual) in order {
                let source = SourceRef::Item(i);
                match visual.look(spec.species, spec.sex, &self.vocab.fallbacks) {
                    ItemLook::Worn {
                        reference,
                        dye,
                        secondary_dye,
                    } => {
                        cx.add_element(source, reference.appearance, reference.key, WORN_MODIFIER);
                        // A chosen dye replaces the default (§4.9); code 0 emits no dye
                        // element (§4.9 Decision).
                        let pick = |chosen: Option<f32>, default: Option<f32>| match chosen {
                            Some(c) => Some(c).filter(|c| *c != 0.0 && !c.is_nan()),
                            None => default,
                        };
                        for (code, key) in [
                            (pick(item.dye, dye), self.vocab.clothing_color),
                            (
                                pick(item.secondary_dye, secondary_dye),
                                self.vocab.clothing_color_secondary,
                            ),
                        ] {
                            if let (Some(code), Some(key)) = (code, key) {
                                cx.add_element(source, reference.appearance, key, code);
                            }
                        }
                    }
                    ItemLook::Held { phys_obj } => held_queue.push((i, item, visual, phys_obj)),
                    ItemLook::Invisible => {
                        let cat = if visual.worn.is_empty() {
                            Cat::ItemInvisible
                        } else {
                            Cat::NoWardrobeStruct
                        };
                        cx.diag(cat, source, Some(item.item), None);
                    }
                }
            }
        }

        // The Appearance nodes the start-up scripts ran (§7.4), after the lists.
        for (n, e) in input.scripts.iter().enumerate() {
            let source = SourceRef::Script(n as u32);
            if e.record.is_null() || e.key == 0 {
                if e.record.is_null() {
                    cx.diag(Cat::NullAppearance, source, None, Some(e.key));
                }
                continue;
            }
            cx.add_element(source, e.record, e.key, e.modifier);
        }

        // Step 4: apply.
        self.apply_records(&mut cx)?;

        // Steps 5–6: resolve the slots and attach the materials.
        let parts = Self::resolve_parts(&cx, &defaults);

        // Step 9: held items at their placements (§5.4, §5.6).
        let species = input.spec.map_or(0, |s| s.species);
        let ranged = input.spec.is_some_and(|s| s.ranged_skill_mode);
        let mounted = input.spec.is_some_and(|s| s.mounted);
        let secondary_scale = prop(self.vocab.secondary_scale)
            .and_then(scale_vector)
            .unwrap_or([1.0; 3]);
        for (i, item, visual, phys_obj) in held_queue {
            let source = SourceRef::Item(i);
            let stowed = item.state == WieldState::Sheathed;
            let slot = visual.default_slot.unwrap_or(0);
            let category = visual.equipment_category.unwrap_or(0);
            // §5.6: the stowed shield goes on the back in the ranged skill mode, and is not
            // drawn while the figure is also mounted.
            let row = self
                .vocab
                .placement
                .row(slot, category, species, ranged, stowed)
                .filter(|r| {
                    !(mounted
                        && r.stowed
                        && r.slot == super::WeaponSlot::Secondary
                        && r.skill_mode == super::SkillModeMatch::Ranged)
                });
            let placements: Vec<_> = row.map(|r| r.placements.clone()).unwrap_or_default();
            // §5.6 "Stowed instruments" (A38): a stowed ranged-slot instrument is attached by
            // the sheathed-instrument script of the wearer's binding table, evaluated with
            // the item's entity and `Music_Cached_Instrument_Type` set to its instrument type
            // (Decision of 2026-10-09).
            if placements.is_empty()
                && stowed
                && self.vocab.instrument_category & category != 0
                && self.vocab.placement.weapon_slot(slot) == Some(super::WeaponSlot::Ranged)
                && let Some(key) = self.vocab.sheathed_instrument_script
            {
                let attached =
                    self.instrument_attachments(input.start, item.item, phys_obj, key)?;
                if attached.is_empty() {
                    cx.diag(Cat::NoPlacement, source, Some(item.item), None);
                }
                for a in attached {
                    self.place_held(
                        &mut cx,
                        &setup,
                        source,
                        Some(item.item),
                        a.entity,
                        a.location,
                        item.state,
                        HeldPlacement {
                            offset: a.offset,
                            rotation: a.rotation,
                            scale: a.scale,
                        },
                    )?;
                }
                continue;
            }
            if placements.is_empty() {
                cx.diag(Cat::NoPlacement, source, Some(item.item), None);
                continue;
            }
            for p in placements {
                let scale = match p.scale {
                    PlacementScale::Fixed(s) => [s; 3],
                    PlacementScale::SecondaryVector => secondary_scale,
                };
                self.place_held(
                    &mut cx,
                    &setup,
                    source,
                    Some(item.item),
                    phys_obj,
                    p.location,
                    item.state,
                    HeldPlacement {
                        offset: p.offset,
                        rotation: p.rotation,
                        scale,
                    },
                )?;
            }
        }
        // NPC props while `NPC_Cosmetic_Inventory_Visible` is true (dictionary default
        // true; §5.5 Rule).
        let props: Vec<(u8, Did, Option<u32>)> = self
            .vocab
            .npc_items
            .iter()
            .take(4)
            .enumerate()
            .filter_map(|(n, &(item, loc))| {
                let entity = prop(Some(item))?.as_did().filter(|d| !d.is_null())?;
                Some((n as u8 + 1, entity, prop(loc).and_then(as_u32)))
            })
            .collect();
        let props_visible = prop(self.vocab.npc_props_visible).is_none_or(as_true);
        if !props_visible && !props.is_empty() {
            cx.diag(Cat::PropsHidden, SourceRef::Figure, Some(body.entity), None);
        }
        for (n, entity, location) in props.into_iter().filter(|_| props_visible) {
            let source = SourceRef::NpcProp(n);
            let Some(location) = location else {
                cx.diag(Cat::NoHoldingLocation, source, Some(entity), None);
                continue;
            };
            self.place_held(
                &mut cx,
                &setup,
                source,
                None,
                entity,
                location,
                WieldState::Wielded,
                HeldPlacement::default(),
            )?;
        }

        // Creature cosmetic slots 1–4 while the props are visible (§5.5, A39): server
        // objects carry them; each slot with a prop and `_Hide` false is attached by the
        // rule of script 0x100007C3 ([`super::CosmeticSlotVocabulary::placement`]).
        let cos = &self.vocab.cosmetic;
        let code = |pid: Option<u32>| prop(pid).and_then(as_u32);
        if props_visible && cos.applies(&code) {
            let sheathed = cos.sheathed(
                code(cos.is_sheathed),
                prop(cos.combat_violent).is_some_and(as_true),
            );
            let float = |pid: Option<u32>| {
                #[allow(clippy::cast_possible_truncation)]
                prop(pid)
                    .and_then(PropertyValue::as_f64)
                    .map(|v| v as f32)
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
            };
            let placements: Vec<(u8, super::CosmeticPlacement)> = cos
                .slots
                .iter()
                .take(4)
                .enumerate()
                .filter_map(|(n, s)| {
                    let entity = prop(s.entity)?.as_did().filter(|d| !d.is_null())?;
                    if prop(s.hide).is_some_and(as_true) {
                        return None;
                    }
                    let scale = prop(s.scale).and_then(scale_vector).unwrap_or([1.0; 3]);
                    let rotation = [float(s.yaw), float(s.pitch), float(s.roll)];
                    let location = code(s.holding_location).unwrap_or(0);
                    cos.placement(entity, location, scale, rotation, sheathed)
                        .map(|p| (n as u8 + 1, p))
                })
                .collect();
            for (n, p) in placements {
                self.place_held(
                    &mut cx,
                    &setup,
                    SourceRef::CosmeticSlot(n),
                    None,
                    p.entity,
                    p.location,
                    WieldState::Wielded,
                    HeldPlacement {
                        offset: p.offset,
                        rotation: p.rotation,
                        scale: p.scale,
                    },
                )?;
            }
        }

        let starting_idle_animation = prop(self.vocab.idle_animation).and_then(as_u32);

        // Step 7 inputs: scale, sliders, heights.
        let scale = ModelScale {
            template: body.template_scale,
            entity: body.entity_scale,
            instance: input
                .instance
                .zip(self.vocab.render_scale)
                .and_then(|(s, p)| s.get(p))
                .and_then(scale_vector),
        };
        let spec = input.spec;
        let slider_string = spec
            .and_then(|s| s.slider_values.clone())
            .or_else(|| body.slider_values.clone());
        // The client's reading (`appearance.md` §6.3, A16, A29): pieces of two characters
        // from the start, each read like `strtol` in base 16, an empty piece after an
        // even-length string. A string that is not clean hex pairs is still noted.
        let values = match slider_string.as_deref() {
            Some(text) => {
                if parse_slider_values(text).is_none() {
                    cx.diag(
                        Cat::SliderValuesInvalid,
                        SourceRef::Figure,
                        Some(body.entity),
                        None,
                    );
                }
                slider_value_pieces(text)
            }
            None => Vec::new(),
        };
        #[allow(clippy::cast_possible_truncation)]
        let float = |chosen: Option<f32>, pid: Option<u32>| {
            chosen.or_else(|| {
                pid.and_then(|p| body.properties.get(p))
                    .and_then(PropertyValue::as_f64)
                    .map(|v| v as f32)
            })
        };
        let sliders = SliderInputs {
            definitions: body.bone_slider_definitions,
            values,
            bone_scale: float(spec.and_then(|s| s.bone_scale), self.vocab.bone_scale),
            avatar_height: float(spec.and_then(|s| s.avatar_height), self.vocab.avatar_height),
        };

        let tagfile = self.shared_tagfile(&setup);
        let groups = std::mem::take(&mut cx.groups);
        Ok(AssembledCharacter {
            object: body.object,
            entity: body.entity,
            chain: body.chain.clone(),
            rig,
            skeleton: SkeletonSource {
                setup: setup.did,
                tagfile,
            },
            parts,
            held: std::mem::take(&mut cx.held),
            scale,
            sliders,
            clip_inputs: ClipInputs {
                start: input.start,
                entity: body.entity,
                instance_properties: input.instance.cloned(),
                starting_idle_animation,
                appearance_values: spec
                    .map(|s| s.appearance_values.clone())
                    .unwrap_or_default(),
            },
            instance_list_origin,
            random_appearance,
            watched_properties: watched,
            groups,
            setup,
            diagnostics: std::mem::take(&mut cx.diags),
            dropped_diagnostics: cx.dropped,
        })
    }

    /// The setup's tagfile as an `Arc` shared with earlier figures on the same setup
    /// record (copied once per setup, not once per figure).
    fn shared_tagfile(&mut self, setup: &Arc<Setup>) -> Arc<[u8]> {
        if let Some((s, t)) = self.tagfiles.get(&setup.did)
            && Arc::ptr_eq(s, setup)
        {
            return Arc::clone(t);
        }
        if self.tagfiles.len() >= TAGFILE_CACHE_ENTRIES {
            self.tagfiles.clear();
        }
        let t: Arc<[u8]> = Arc::from(setup.tagfile.as_slice());
        self.tagfiles
            .insert(setup.did, (Arc::clone(setup), Arc::clone(&t)));
        t
    }

    /// Steps 5–6: the setup's parts in order, each replaced by its slot's winner (once per
    /// slot), then the winners of slots the setup lacks; each part with the groups of the
    /// figure-wide list that match its slot (§4.10).
    fn resolve_parts(cx: &Ctx, defaults: &[(u32, Did, Option<MeshInfo>)]) -> Vec<AssembledPart> {
        let offered = |slot: u32, offer: usize, priority: f32| {
            let o = cx.offers.get(offer)?;
            Some(AssembledPart {
                slot,
                mesh: o.mesh,
                priority,
                origin: PartOrigin::Appearance {
                    source: o.source,
                    record: o.record,
                    key: o.key,
                    variant: o.variant,
                },
                hidden: o.info.stub,
                skinned: o.info.skinned,
                materials: cx.layers_for(Some(slot)),
            })
        };
        let winner = |slot: u32| {
            cx.table.get(slot).and_then(|w| match w.source {
                PartSource::Offered(i) => Some((i as usize, w.priority)),
                PartSource::Default => None,
            })
        };
        let mut parts = Vec::with_capacity(defaults.len() + cx.table.len());
        let mut emitted: HashSet<u32> = HashSet::new();
        let mut default_slots: HashSet<u32> = HashSet::new();
        for &(index, mesh, info) in defaults {
            let origin = PartOrigin::Setup { index };
            let Some(info) = info else {
                parts.push(AssembledPart {
                    slot: 0,
                    mesh,
                    priority: f32::NEG_INFINITY,
                    origin,
                    hidden: false,
                    skinned: false,
                    materials: cx.layers_for(None),
                });
                continue;
            };
            default_slots.insert(info.slot);
            if let Some((offer, priority)) = winner(info.slot) {
                // Replaced; a second default part in the same slot disappears with it.
                if emitted.insert(info.slot)
                    && let Some(p) = offered(info.slot, offer, priority)
                {
                    parts.push(p);
                }
                continue;
            }
            parts.push(AssembledPart {
                slot: info.slot,
                mesh,
                priority: f32::NEG_INFINITY,
                origin,
                hidden: info.stub,
                skinned: info.skinned,
                materials: cx.layers_for(Some(info.slot)),
            });
        }
        for w in cx.table.iter() {
            if default_slots.contains(&w.slot) || emitted.contains(&w.slot) {
                continue;
            }
            if let PartSource::Offered(i) = w.source
                && let Some(p) = offered(w.slot, i as usize, w.priority)
            {
                parts.push(p);
            }
        }
        parts
    }
}

#[cfg(test)]
mod hand_rule_tests {
    //! The held-item hand rule of `character-panels.md` §7 (Tessera rule) on synthetic
    //! records.

    use std::collections::HashMap;
    use std::convert::Infallible;

    use super::*;
    use crate::assembly::source::MapSource;
    use crate::assembly::vocab::PlacementRule;
    use crate::summary::MeshSummary;
    use tessera_formats::avatar::AvatarBody;

    const PRIMARY: u64 = 0x1_0000;
    const SECONDARY: u64 = 0x2_0000;
    const RANGED: u64 = 0x4_0000;

    /// A map source whose items also report compatible slots.
    struct Source {
        map: MapSource,
        compatible: HashMap<Did, u64>,
    }

    impl AssemblySource for Source {
        type Error = Infallible;
        fn race_avatar(&mut self, species: u32, sex: u32) -> Result<Option<Did>, Infallible> {
            self.map.race_avatar(species, sex)
        }
        fn body(&mut self, start: Did) -> Result<Lookup<AvatarBody>, Infallible> {
            self.map.body(start)
        }
        fn setup(&mut self, did: Did) -> Result<Lookup<Arc<Setup>>, Infallible> {
            self.map.setup(did)
        }
        fn setup_chain(&mut self, start: Did) -> Result<Lookup<Vec<Did>>, Infallible> {
            self.map.setup_chain(start)
        }
        fn appearance(&mut self, did: Did) -> Result<Lookup<Arc<AppearanceRecord>>, Infallible> {
            self.map.appearance(did)
        }
        fn mesh(&mut self, did: Did) -> Result<Lookup<MeshSummary>, Infallible> {
            self.map.mesh(did)
        }
        fn item(&mut self, did: Did) -> Result<Lookup<ItemVisual>, Infallible> {
            self.map.item(did)
        }
        fn object_properties(&mut self, object: Did) -> Result<Lookup<PropertySet>, Infallible> {
            self.map.object_properties(object)
        }
        fn compatible_slots(&mut self, item: Did) -> Result<Option<u64>, Infallible> {
            Ok(self.compatible.get(&item).copied())
        }
    }

    fn assembler(compatible: &[(u32, u64)]) -> CharacterAssembler<Source> {
        let vocab = AssemblyVocabulary {
            placement: PlacementRule::from_labels(
                |l| match l {
                    "Weapon_Primary" => Some(PRIMARY),
                    "Weapon_Secondary" => Some(SECONDARY),
                    "Weapon_Ranged" => Some(RANGED),
                    _ => None,
                },
                |_| None,
                |_| None,
                |_| None,
            ),
            ..AssemblyVocabulary::default()
        };
        CharacterAssembler::new(
            Source {
                map: MapSource::default(),
                compatible: compatible.iter().map(|&(d, b)| (Did(d), b)).collect(),
            },
            vocab,
        )
    }

    fn slots(
        asm: &mut CharacterAssembler<Source>,
        items: &[(EquippedItem, Option<u64>)],
    ) -> Vec<Option<u64>> {
        let mut hands = Vec::new();
        items
            .iter()
            .map(|(i, default)| asm.hand_slot(i, *default, &mut hands).unwrap())
            .collect()
    }

    #[test]
    fn second_primary_item_goes_to_the_secondary_hand_when_it_fits() {
        let mut asm = assembler(&[(2, PRIMARY | SECONDARY)]);
        let got = slots(
            &mut asm,
            &[
                (EquippedItem::new(Did(1)), Some(PRIMARY)),
                (EquippedItem::new(Did(2)), Some(PRIMARY)),
            ],
        );
        assert_eq!(got, vec![Some(PRIMARY), Some(SECONDARY)]);
    }

    #[test]
    fn second_primary_item_stays_when_it_does_not_fit_the_secondary_hand() {
        let mut asm = assembler(&[(2, PRIMARY)]);
        let got = slots(
            &mut asm,
            &[
                (EquippedItem::new(Did(1)), Some(PRIMARY)),
                (EquippedItem::new(Did(2)), Some(PRIMARY)),
            ],
        );
        assert_eq!(got, vec![Some(PRIMARY), Some(PRIMARY)]);
    }

    #[test]
    fn an_explicit_slot_wins_and_takes_its_hand() {
        let mut asm = assembler(&[(2, PRIMARY | SECONDARY), (3, PRIMARY | SECONDARY)]);
        let got = slots(
            &mut asm,
            &[
                // Held in the secondary hand by its slot, although its default is primary.
                (EquippedItem::new(Did(2)).in_slot(SECONDARY), Some(PRIMARY)),
                // The primary hand is still free.
                (EquippedItem::new(Did(3)), Some(PRIMARY)),
            ],
        );
        assert_eq!(got, vec![Some(SECONDARY), Some(PRIMARY)]);
    }

    #[test]
    fn worn_and_ranged_items_are_not_moved() {
        let mut asm = assembler(&[(5, RANGED | SECONDARY)]);
        let got = slots(
            &mut asm,
            &[
                (EquippedItem::new(Did(4)), Some(0x4)),
                (EquippedItem::new(Did(5)), Some(RANGED)),
                (EquippedItem::new(Did(6)), Some(RANGED)),
                (EquippedItem::new(Did(7)), None),
            ],
        );
        assert_eq!(got, vec![Some(0x4), Some(RANGED), Some(RANGED), None]);
    }
}
