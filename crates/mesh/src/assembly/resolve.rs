// SPDX-License-Identifier: GPL-3.0-or-later
//! The assembly algorithm of `appearance.md` §8.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::appearance::{
    AppearanceBlock, AppearanceKeyInfo, DYE_TOLERANCE, MODIFIER_TOLERANCE, PartSource, SLOT_ALL,
    SlotTable,
};
use tessera_formats::avatar::{AvatarBody, parse_slider_values, slider_value_pairs, uniform_scale};
use tessera_formats::item::{ItemLook, ItemVisual, WornAppearanceRef, worn_block};
use tessera_formats::{Did, PropertySet, PropertyValue, Setup};

use super::source::{AssemblySource, Lookup};
use super::vocab::AssemblyVocabulary;
use super::{
    AssembledCharacter, AssembledPart, AssemblyError, AssemblyLimits, CharacterSpec, ClipInputs,
    Diagnostic, DiagnosticCategory as Cat, EquippedItem, HeldPart, LayerKind, ListOrigin,
    MaterialLayer, ModelScale, PartOrigin, RigKind, SkeletonSource, SliderInputs, SourceRef,
    WieldState,
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

/// Index of a layer set in [`Ctx::sets`].
type SetId = usize;

/// One appearance block whose parts were offered.
#[derive(Debug)]
struct Contribution {
    source: SourceRef,
    record: Did,
    key: u32,
    variant: f32,
    /// The block's own material groups.
    groups: SetId,
    /// Further layer sets for its parts (dye blocks, material-only list blocks), in
    /// application order; each set at most once.
    extra: Vec<SetId>,
}

#[derive(Debug, Clone, Copy)]
struct Offer {
    mesh: Did,
    contribution: usize,
    info: MeshInfo,
}

/// A material-only element of an appearance list (a dye element, or a material-only
/// block under another key), applied once all parts are known.
#[derive(Debug)]
struct PendingLayers {
    source: SourceRef,
    record: Did,
    set: SetId,
}

/// Identifies a layer set: (record, entry key, block index, kind).
type SetKey = (Did, u32, usize, LayerKind);

/// Per-assembly state.
#[derive(Debug)]
struct Ctx {
    limits: AssemblyLimits,
    diags: Vec<Diagnostic>,
    dropped: usize,
    /// Mesh lookups, with the category of a failed one.
    meshes: HashMap<Did, Result<MeshInfo, Cat>>,
    /// Material layer sets, each built once per distinct block and kind.
    sets: Vec<Vec<MaterialLayer>>,
    set_index: HashMap<SetKey, SetId>,
    /// Layers held in `sets` (bounded by `max_total_layers`).
    total_layers: usize,
    contributions: Vec<Contribution>,
    offers: Vec<Offer>,
    table: SlotTable,
    /// Default parts entered (they share `max_parts` with the offers).
    n_defaults: usize,
    pending: Vec<PendingLayers>,
    pending_sets: HashSet<SetId>,
    /// Layer sets for the default parts (unpaired list elements).
    default_layers: Vec<SetId>,
    held: Vec<HeldPart>,
}

impl Ctx {
    fn new(limits: AssemblyLimits) -> Self {
        Self {
            limits,
            diags: Vec::new(),
            dropped: 0,
            meshes: HashMap::new(),
            sets: Vec::new(),
            set_index: HashMap::new(),
            total_layers: 0,
            contributions: Vec::new(),
            offers: Vec::new(),
            table: SlotTable::new(),
            n_defaults: 0,
            pending: Vec::new(),
            pending_sets: HashSet::new(),
            default_layers: Vec::new(),
            held: Vec::new(),
        }
    }

    /// The layer set of `block` (one of `blocks`, the blocks of entry `key` of `record`)
    /// as `kind`: built once per assembly, shared by every list element, item and part
    /// that uses it. The layers of all sets together stay within `max_total_layers`.
    #[allow(clippy::too_many_arguments)]
    fn layer_set(
        &mut self,
        source: SourceRef,
        record: Did,
        key: u32,
        blocks: &[AppearanceBlock],
        block: &AppearanceBlock,
        kind: LayerKind,
    ) -> SetId {
        let index = blocks.iter().position(|b| std::ptr::eq(b, block));
        let set_key = index.map(|i| (record, key, i, kind));
        if let Some(id) = set_key.and_then(|k| self.set_index.get(&k)) {
            return *id;
        }
        let room = self
            .limits
            .max_total_layers
            .saturating_sub(self.total_layers);
        if block.groups.len() > room {
            self.diag(Cat::LimitExceeded, source, Some(record), Some(key));
        }
        let layers: Vec<MaterialLayer> = block
            .groups_with_priority()
            .take(room)
            .map(|(p, g)| MaterialLayer::from_group(kind, p, g, record, block.variant))
            .collect();
        self.total_layers += layers.len();
        let id = self.sets.len();
        self.sets.push(layers);
        if let Some(k) = set_key {
            self.set_index.insert(k, id);
        }
        id
    }

    /// Queues a list element's layer set for pairing (each set once).
    fn queue_layers(&mut self, source: SourceRef, record: Did, set: SetId) {
        if self.pending_sets.insert(set) {
            self.pending.push(PendingLayers {
                source,
                record,
                set,
            });
        }
    }

    /// The layers of `sets` that apply to `slot` (all of them when `slot` is `None`), at
    /// most `max_layers`, in order.
    fn layers_for(
        &self,
        sets: impl IntoIterator<Item = SetId>,
        slot: Option<u32>,
    ) -> Vec<MaterialLayer> {
        sets.into_iter()
            .filter_map(|id| self.sets.get(id))
            .flatten()
            .filter(|l| slot.is_none_or(|s| layer_applies(l, s)))
            .take(self.limits.max_layers)
            .copied()
            .collect()
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
}

/// What a placed figure brings besides its body.
struct Input<'a> {
    start: Did,
    object_set: Option<PropertySet>,
    instance: Option<&'a PropertySet>,
    spec: Option<&'a CharacterSpec>,
}

fn source_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> AssemblyError {
    AssemblyError::Source(Box::new(e))
}

/// Applies to `slot`: the group targets `All` or that slot (`appearance.md` §4.10).
fn layer_applies(layer: &MaterialLayer, slot: u32) -> bool {
    layer.group_slot == SLOT_ALL || layer.group_slot == slot
}

/// Whether `source` is an appearance-list element (template section, class or instance
/// list), whose material-only elements pair with parts of the same record.
fn is_list_source(source: SourceRef) -> bool {
    matches!(
        source,
        SourceRef::TemplateSection(_) | SourceRef::ClassList(_) | SourceRef::InstanceList(_)
    )
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

impl<S: AssemblySource> CharacterAssembler<S> {
    /// An assembler over `source` with `vocab` and the default [`AssemblyLimits`].
    #[must_use]
    pub fn new(source: S, vocab: AssemblyVocabulary) -> Self {
        Self {
            source,
            vocab,
            limits: AssemblyLimits::default(),
            tagfiles: HashMap::new(),
        }
    }

    /// The same assembler with other limits.
    #[must_use]
    pub fn with_limits(mut self, limits: AssemblyLimits) -> Self {
        self.limits = limits;
        self
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
        let avatar = self
            .source
            .race_avatar(spec.species, spec.sex)
            .map_err(source_error)?
            .ok_or(AssemblyError::NoSuchBody {
                species: spec.species,
                sex: spec.sex,
            })?;
        self.assemble_avatar(avatar, spec)
    }

    /// Assembles `spec` on the body of `avatar` (a player-avatar game object or a 0x47
    /// entity): the setup's default parts, the avatar's appearance lists with the chosen
    /// styles, the worn items, the held items, sliders and heights. `spec.species` and
    /// `spec.sex` choose the wardrobe structs of the items.
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
        let body = self.body(avatar)?;
        self.assemble_body(
            body,
            Input {
                start: avatar,
                object_set: None,
                instance: None,
                spec: Some(spec),
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
    ) -> Result<Option<Arc<tessera_formats::AppearanceRecord>>, AssemblyError> {
        if did.is_null() {
            cx.diag(Cat::NullAppearance, source, None, Some(key));
            return Ok(None);
        }
        Ok(match self.source.appearance(did).map_err(source_error)? {
            Lookup::Found(r) => Some(r),
            Lookup::Missing => {
                cx.diag(Cat::AppearanceMissing, source, Some(did), Some(key));
                None
            }
            Lookup::Invalid(_) => {
                cx.diag(Cat::AppearanceUndecodable, source, Some(did), Some(key));
                None
            }
        })
    }

    /// Offers every part of `block` to its slot (`appearance.md` §4.4 rule 2) and records
    /// the block's material groups. Returns the contribution's index.
    fn offer_block(
        &mut self,
        cx: &mut Ctx,
        source: SourceRef,
        record: Did,
        key: u32,
        blocks: &[AppearanceBlock],
        block: &AppearanceBlock,
    ) -> Result<usize, AssemblyError> {
        if !block.extras.is_empty() {
            cx.diag(Cat::BlockExtraIgnored, source, Some(record), Some(key));
        }
        let groups = cx.layer_set(source, record, key, blocks, block, LayerKind::Group);
        let c = cx.contributions.len();
        cx.contributions.push(Contribution {
            source,
            record,
            key,
            variant: block.variant,
            groups,
            extra: Vec::new(),
        });
        for p in &block.parts {
            if cx.n_defaults + cx.offers.len() >= cx.limits.max_parts {
                cx.diag(Cat::LimitExceeded, source, Some(record), Some(key));
                break;
            }
            let Some(info) = self.mesh_info(cx, p.mesh, source)? else {
                // §10: the slot keeps its previous winner.
                continue;
            };
            let index = cx.offers.len() as u32;
            cx.table.offer(p.slot, p.mesh, p.priority(), index);
            cx.offers.push(Offer {
                mesh: p.mesh,
                contribution: c,
                info,
            });
        }
        Ok(c)
    }

    /// One appearance-list element (`appearance.md` §3.2, §8 step 3).
    ///
    /// A block with parts is offered to the slot table. A material-only block (a dye
    /// element keyed `ClothingColor` / `ClothingColorSecondary`, or a material-only block
    /// of a selection table under any other key, §4.11) is queued: its groups go onto the
    /// parts that list elements naming the same record contributed, else onto the default
    /// parts (Decision, `appearance.md` §15).
    ///
    /// A dye element's modifier picks its block by §3.2.1 rule 1 then rule 2, so modifier
    /// 0.0 layers the 0.00 block. For worn items code 0.0 means "no dye" (`material.md`
    /// §12.4); which reading applies to list elements is OPEN (`appearance.md` §15).
    fn apply_list_element(
        &mut self,
        cx: &mut Ctx,
        source: SourceRef,
        key: u32,
        file: Did,
        modifier: f32,
    ) -> Result<(), AssemblyError> {
        let Some(rec) = self.record(cx, file, source, key)? else {
            return Ok(());
        };
        let Some(entry) = rec.entry(key) else {
            cx.diag(Cat::KeyMissing, source, Some(file), Some(key));
            return Ok(());
        };
        let exact = entry
            .blocks
            .iter()
            .find(|b| (b.variant - modifier).abs() <= MODIFIER_TOLERANCE);
        let dye_kind = if Some(key) == self.vocab.clothing_color {
            Some(LayerKind::Dye)
        } else if Some(key) == self.vocab.clothing_color_secondary {
            Some(LayerKind::SecondaryDye)
        } else {
            None
        };
        if let Some(kind) = dye_kind {
            // §3.2.1 rule 2: the dye rule of material.md §12.4.
            let Some(block) = exact.or_else(|| entry.dye_block(modifier)) else {
                cx.diag(Cat::MaterialOnly, source, Some(file), Some(key));
                return Ok(());
            };
            if (block.variant - modifier).abs() > DYE_TOLERANCE {
                cx.diag(Cat::DyeInexact, source, Some(file), Some(key));
            }
            let set = cx.layer_set(source, file, key, &entry.blocks, block, kind);
            cx.queue_layers(source, file, set);
            return Ok(());
        }
        let block = match exact {
            Some(b) => b,
            None => match entry.block_for_modifier(modifier) {
                Some(b) => {
                    cx.diag(Cat::BlockFallback, source, Some(file), Some(key));
                    b
                }
                None => {
                    cx.diag(Cat::MaterialOnly, source, Some(file), Some(key));
                    return Ok(());
                }
            },
        };
        if block.parts.is_empty() {
            if block.groups.is_empty() {
                cx.diag(Cat::MaterialOnly, source, Some(file), Some(key));
                return Ok(());
            }
            // §4.11: material-only entries apply together with the record's parts.
            let set = cx.layer_set(source, file, key, &entry.blocks, block, LayerKind::Group);
            cx.queue_layers(source, file, set);
            return Ok(());
        }
        self.offer_block(cx, source, file, key, &entry.blocks, block)?;
        Ok(())
    }

    /// A worn item (`appearance.md` §8 step 4, §4.9).
    fn apply_worn(
        &mut self,
        cx: &mut Ctx,
        source: SourceRef,
        reference: WornAppearanceRef,
        dyes: [Option<f32>; 2],
    ) -> Result<(), AssemblyError> {
        let (file, key) = (reference.appearance, reference.key);
        let Some(rec) = self.record(cx, file, source, key)? else {
            return Ok(());
        };
        let Some(worn_entry) = rec.entry(key) else {
            cx.diag(Cat::KeyMissing, source, Some(file), Some(key));
            return Ok(());
        };
        let Some(block) = worn_block(&rec, key) else {
            cx.diag(Cat::NoBaseBlock, source, Some(file), Some(key));
            return Ok(());
        };
        if block.parts.is_empty() {
            cx.diag(Cat::MaterialOnly, source, Some(file), Some(key));
            return Ok(());
        }
        let c = self.offer_block(cx, source, file, key, &worn_entry.blocks, block)?;
        let channels = [
            (dyes[0], self.vocab.clothing_color, LayerKind::Dye),
            (
                dyes[1],
                self.vocab.clothing_color_secondary,
                LayerKind::SecondaryDye,
            ),
        ];
        for (code, dye_key, kind) in channels {
            let (Some(code), Some(dye_key)) = (code, dye_key) else {
                continue;
            };
            let Some(entry) = rec.entry(dye_key) else {
                cx.diag(Cat::DyeEntryMissing, source, Some(file), Some(dye_key));
                continue;
            };
            let Some(b) = entry.dye_block(code) else {
                continue;
            };
            if (b.variant - code).abs() > DYE_TOLERANCE {
                cx.diag(Cat::DyeInexact, source, Some(file), Some(dye_key));
            }
            let set = cx.layer_set(source, file, dye_key, &entry.blocks, b, kind);
            if !cx.contributions[c].extra.contains(&set) {
                cx.contributions[c].extra.push(set);
            }
        }
        Ok(())
    }

    /// Places a held item or prop at `location` of the wearer (`skeleton-anim.md` §8.4).
    #[allow(clippy::too_many_arguments)]
    fn place_held(
        &mut self,
        cx: &mut Ctx,
        wearer: &Setup,
        source: SourceRef,
        item: Option<Did>,
        entity: Did,
        location: Option<u32>,
        state: WieldState,
    ) -> Result<(), AssemblyError> {
        let did = item.or(Some(entity));
        if cx.held.len() >= cx.limits.max_held {
            cx.diag(Cat::LimitExceeded, source, did, None);
            return Ok(());
        }
        let Some(location) = location else {
            cx.diag(Cat::NoHoldingLocation, source, did, None);
            return Ok(());
        };
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
        let setup_did = chain[chain.len() - 1];
        let Lookup::Found(setup) = self.source.setup(setup_did).map_err(source_error)? else {
            cx.diag(Cat::HeldChainBroken, source, Some(setup_did), None);
            return Ok(());
        };
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
        });
        Ok(())
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
            defaults.push((i as u32, mesh, info));
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

        // Instance properties replace object and entity properties of the same name.
        let sets: [Option<&PropertySet>; 3] = [
            input.instance,
            input.object_set.as_ref(),
            Some(&body.properties),
        ];
        let prop = |pid: Option<u32>| {
            let pid = pid?;
            sets.iter().flatten().find_map(|s| s.get(pid))
        };

        // Random appearance (§7.4 Decision): default parts and held props only, so the
        // appearance lists are skipped.
        let random_appearance = prop(self.vocab.random_appearance_type)
            .and_then(as_u32)
            .filter(|&v| self.vocab.is_random_appearance(v));
        if let Some(v) = random_appearance {
            cx.diag(
                Cat::RandomAppearance,
                SourceRef::Figure,
                Some(body.entity),
                Some(v),
            );
        }

        // Step 3: appearance lists (template sections, class list, instance list).
        let (instance_list, instance_list_origin) = self.instance_list(&input);
        let n_template = body.template_appearance.len();
        let n_class = body.class_list.len();
        let elements = if random_appearance.is_some() {
            Vec::new()
        } else {
            body.appearance_files_with_instance(instance_list.as_deref())
        };
        for (n, (info, file)) in elements.into_iter().enumerate() {
            let n32 = n as u32;
            let source = if n < n_template {
                SourceRef::TemplateSection(n32)
            } else if n < n_template + n_class {
                SourceRef::ClassList(n32 - n_template as u32)
            } else {
                SourceRef::InstanceList(n32 - (n_template + n_class) as u32)
            };
            if n >= self.limits.max_list_elements {
                cx.diag(Cat::LimitExceeded, source, Some(file), Some(info.key));
                break;
            }
            let modifier = input
                .spec
                .and_then(|s| s.styles.iter().take(64).find(|c| c.key == info.key))
                .map_or(info.modifier, |c| c.modifier);
            self.apply_list_element(&mut cx, source, info.key, file, modifier)?;
        }

        // Step 4: worn items in ascending slot order; held items are queued.
        let mut held_queue: Vec<(u32, EquippedItem, ItemVisual, Did)> = Vec::new();
        if let Some(spec) = input.spec {
            let mut order: Vec<(u32, u32, EquippedItem, ItemVisual)> = Vec::new();
            for (i, item) in spec.items.iter().enumerate() {
                let source = SourceRef::Item(i as u32);
                if i >= self.limits.max_items {
                    cx.diag(Cat::LimitExceeded, source, Some(item.item), None);
                    break;
                }
                match self.source.item(item.item).map_err(source_error)? {
                    Lookup::Found(mut v) => {
                        // The equipped slot replaces the default slot for the order, the
                        // holding location and the stowing location.
                        v.default_slot = item.effective_slot(v.default_slot);
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
                        // A chosen dye replaces the default (§4.9); code 0 means none.
                        let pick = |chosen: Option<f32>, default: Option<f32>| match chosen {
                            Some(c) => Some(c).filter(|c| *c != 0.0 && !c.is_nan()),
                            None => default,
                        };
                        let dyes = [pick(item.dye, dye), pick(item.secondary_dye, secondary_dye)];
                        self.apply_worn(&mut cx, source, reference, dyes)?;
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

        // Material-only list elements: onto the parts that list elements naming the same
        // record contributed (never onto a worn item's parts, whose dye is its own, §4.9),
        // else onto the default parts (Decision). Each layer set is stored once and
        // referenced by index.
        for d in std::mem::take(&mut cx.pending) {
            let mut paired = false;
            for c in cx
                .contributions
                .iter_mut()
                .filter(|c| c.record == d.record && is_list_source(c.source))
            {
                c.extra.push(d.set);
                paired = true;
            }
            if !paired {
                if !matches!(d.source, SourceRef::TemplateSection(_)) {
                    cx.diag(Cat::UnpairedDye, d.source, Some(d.record), None);
                }
                cx.default_layers.push(d.set);
            }
        }

        // Steps 5–6: resolve the slots and attach the materials.
        let parts = self.resolve_parts(&mut cx, &defaults);

        // Step 9: held items and NPC props.
        for (i, item, visual, phys_obj) in held_queue {
            let wield = self.vocab.holding.location(&visual, true);
            let location = match item.state {
                WieldState::Wielded => wield,
                WieldState::Sheathed => self.vocab.stow.location(&visual).or(wield),
            };
            self.place_held(
                &mut cx,
                &setup,
                SourceRef::Item(i),
                Some(item.item),
                phys_obj,
                location,
                item.state,
            )?;
        }
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
        for (n, entity, location) in props {
            self.place_held(
                &mut cx,
                &setup,
                SourceRef::NpcProp(n),
                None,
                entity,
                location,
                WieldState::Wielded,
            )?;
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
                .and_then(uniform_scale),
        };
        let spec = input.spec;
        let slider_string = spec
            .and_then(|s| s.slider_values.clone())
            .or_else(|| body.slider_values.clone());
        // The client's reading (`appearance.md` §6.3, A16): pairs from the start, a pair
        // that is not hexadecimal reads 0. A string that is not clean hex is still noted.
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
                slider_value_pairs(text)
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
            },
            instance_list_origin,
            random_appearance,
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
    /// slot), then the winners of slots the setup lacks, with their material layers.
    fn resolve_parts(
        &self,
        cx: &mut Ctx,
        defaults: &[(u32, Did, Option<MeshInfo>)],
    ) -> Vec<AssembledPart> {
        let offered = |cx: &Ctx, slot: u32, offer: usize, priority: f32| {
            let o = &cx.offers[offer];
            let c = &cx.contributions[o.contribution];
            let materials = cx.layers_for(
                std::iter::once(c.groups).chain(c.extra.iter().copied()),
                Some(slot),
            );
            AssembledPart {
                slot,
                mesh: o.mesh,
                priority,
                origin: PartOrigin::Appearance {
                    source: c.source,
                    record: c.record,
                    key: c.key,
                    variant: c.variant,
                },
                hidden: o.info.stub,
                skinned: o.info.skinned,
                materials,
            }
        };
        let winner = |cx: &Ctx, slot: u32| {
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
                    materials: cx.layers_for(cx.default_layers.iter().copied(), None),
                });
                continue;
            };
            default_slots.insert(info.slot);
            if let Some((offer, priority)) = winner(cx, info.slot) {
                // Replaced; a second default part in the same slot disappears with it.
                if emitted.insert(info.slot) {
                    parts.push(offered(cx, info.slot, offer, priority));
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
                materials: cx.layers_for(cx.default_layers.iter().copied(), Some(info.slot)),
            });
        }
        let extra: Vec<(u32, usize, f32)> = cx
            .table
            .iter()
            .filter(|w| !default_slots.contains(&w.slot) && !emitted.contains(&w.slot))
            .filter_map(|w| match w.source {
                PartSource::Offered(i) => Some((w.slot, i as usize, w.priority)),
                PartSource::Default => None,
            })
            .collect();
        for (slot, offer, priority) in extra {
            parts.push(offered(cx, slot, offer, priority));
        }
        parts
    }
}
