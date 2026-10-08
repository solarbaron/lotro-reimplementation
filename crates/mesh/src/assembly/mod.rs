// SPDX-License-Identifier: GPL-3.0-or-later
//! Assemblies: setup records and appearance records combined into one dressed figure
//! (`appearance.md` §8; ARCHITECTURE §11.1).
//!
//! A [`CharacterAssembler`] turns either a playable body description ([`CharacterSpec`]:
//! species, sex, styles, slider and height values, worn and held items with dyes and
//! wield state) or a placed object (a 0x70 weenie object or a 0x47 entity, with an optional
//! instance property set) into an [`AssembledCharacter`]:
//!
//! - the **skeleton source**: the setup DID and its Havok tagfile bytes ([`SkeletonSource`]);
//! - the **body parts** after worn-item replacement and layering ([`AssembledPart`]): a
//!   slot that something offered a part for holds that one mesh, other slots keep their
//!   default parts; hidden slots marked, each with its material groups and dye layers as DIDs
//!   ([`MaterialLayer`]);
//! - the **held parts** ([`HeldPart`]): the item's setup and parts, its holding location
//!   and the location's anchor bone and frame on the wearer;
//! - the slider and height values, the model scale, and the **clip-selection inputs** the
//!   `tessera_formats::script_table::resolve` resolver takes ([`ClipInputs`]);
//! - **diagnostics**: every unresolved link (missing records, keys, meshes, holding
//!   locations, …) as a categorised [`Diagnostic`], never as an error. Only a body that
//!   cannot be resolved at all, or a failing record source, is an [`AssemblyError`].
//!
//! The algorithm is `appearance.md` §8 in order: setup default parts, appearance lists
//! (template section, class list, instance list; §7.4), worn items in ascending
//! `ContainerSlot` bit order (§4.7 Decision), slot resolution by highest priority with the
//! later source winning ties (§4.4, §4.7), material groups and dyes (§4.9, §4.10), held
//! items (§5.3–§5.5). Static objects keep every default part of their setup, in order;
//! the slot table applies to them as to any rig, so a part a list element offers replaces
//! **all** default parts of its slot (drawn once, at the first one's position; `appearance.md`
//! §15, implementation choice). A figure with a random-appearance type skips its
//! appearance lists (§7.4 Decision). Material-only list elements (dye tables and
//! selection tables, §4.11) layer onto the parts list elements of the same record
//! contributed, else onto the default parts; never onto a worn item's parts.
//!
//! Records are read through an [`AssemblySource`]: [`GameDataSource`] reads an install
//! through `tessera-props`, [`MapSource`] holds hand-built records for tests and fuzzing.
//! Property IDs and enum keys come from an [`AssemblyVocabulary`] looked up in the data at
//! run time; nothing here hard-codes a PID, a key or a DID.
//!
//! Every input is bounded ([`AssemblyLimits`]) and nothing recurses: entity and template
//! chains are followed by the bounded, cycle-checked resolvers of `tessera-formats`, and
//! appearance records name no other records. Material groups are stored once per distinct
//! (record, entry, block, layer kind) and referenced by index, so many list elements naming
//! one large record cost no more than one ([`AssemblyLimits::max_total_layers`]).

mod resolve;
pub mod shape;
mod source;
mod vocab;

use std::sync::Arc;

use tessera_formats::appearance::MaterialGroup;
use tessera_formats::setup::{Anchor, Frame};
use tessera_formats::{Did, PropertySet, Setup};

pub use resolve::CharacterAssembler;
pub use source::{AssemblySource, GAME_DATA_CACHE_ENTRIES, GameDataSource, Lookup, MapSource};
pub use vocab::{AssemblyVocabulary, StowRule};

/// Bounds on one assembly (hostile-input limits; real figures stay far below them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyLimits {
    /// Items of a [`CharacterSpec`] considered (worn and held together).
    pub max_items: usize,
    /// Appearance-list elements considered per figure (all lists together; observed at
    /// most a few dozen).
    pub max_list_elements: usize,
    /// Parts per figure: the setup's default parts and the parts offered to the slot
    /// table share this budget, so [`AssembledCharacter::parts`] never holds more.
    pub max_parts: usize,
    /// Held parts per figure.
    pub max_held: usize,
    /// Material layers kept per part.
    pub max_layers: usize,
    /// Material layers held per figure before they are attached to parts: the groups of
    /// every distinct (record, entry, block, layer kind) the figure uses, counted once
    /// however many list elements or parts share them. Bounds the memory of an assembly
    /// and the per-part layer scan.
    pub max_total_layers: usize,
    /// Diagnostics kept per figure (further ones are counted in
    /// [`AssembledCharacter::dropped_diagnostics`]).
    pub max_diagnostics: usize,
}

impl Default for AssemblyLimits {
    fn default() -> Self {
        Self {
            max_items: 64,
            max_list_elements: 512,
            max_parts: 4096,
            max_held: 16,
            max_layers: 256,
            max_total_layers: 8192,
            max_diagnostics: 1024,
        }
    }
}

/// Whether a held item is in the hand or stowed (`appearance.md` §5.3, §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WieldState {
    /// In the hand (in combat): the location of `appearance.md` §5.3.
    #[default]
    Wielded,
    /// Stowed (out of combat): the stowing location by category (§5.4 Decision); items
    /// that stay in hand use the wielded location.
    Sheathed,
}

/// One equipped item of a [`CharacterSpec`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquippedItem {
    /// The item: a game object (0x70–0x76) or its property set (0x79…).
    pub item: Did,
    /// The dye code of the first channel; `None` keeps the item's default
    /// (`Item_ClothingColor`, `appearance.md` §4.9), `Some(0.0)` means no dye
    /// (`material.md` §12.4: code 0.0 layers no dye material). An appearance-list dye
    /// element with modifier 0.0 instead layers its 0.00 block (`appearance.md` §3.2.1
    /// rule 1); which reading lists follow is OPEN (`appearance.md` §15).
    pub dye: Option<f32>,
    /// The dye code of the second channel, as `dye`.
    pub secondary_dye: Option<f32>,
    /// Wield state, for items that turn out to be held.
    pub state: WieldState,
    /// The `ContainerSlot` bits of the slot the item is equipped in; `None` (or 0) means
    /// its `Inventory_DefaultSlot`. It decides the holding location (`appearance.md` §5.3:
    /// `Weapon_Secondary` → `Wpn_LOffHand`), the stowing location (§5.4: a one-handed
    /// weapon in the secondary slot → `Sheath_RHip`) and the worn-item order (§4.7), so a
    /// one-handed weapon whose default slot is `Weapon_Primary` can be dual-wielded.
    pub slot: Option<u64>,
}

impl EquippedItem {
    /// An item with its default dyes, wielded if held.
    #[must_use]
    pub fn new(item: impl Into<Did>) -> Self {
        Self {
            item: item.into(),
            dye: None,
            secondary_dye: None,
            state: WieldState::Wielded,
            slot: None,
        }
    }

    /// The same item with the first-channel dye code `code`.
    #[must_use]
    pub fn dyed(mut self, code: f32) -> Self {
        self.dye = Some(code);
        self
    }

    /// The same item stowed or wielded.
    #[must_use]
    pub fn with_state(mut self, state: WieldState) -> Self {
        self.state = state;
        self
    }

    /// The same item equipped in the slot with `ContainerSlot` bits `bits` instead of its
    /// default slot (see [`EquippedItem::slot`]).
    #[must_use]
    pub fn in_slot(mut self, bits: u64) -> Self {
        self.slot = Some(bits);
        self
    }

    /// The slot bits in use: [`EquippedItem::slot`] when set and non-zero, else `default`.
    #[must_use]
    pub fn effective_slot(&self, default: Option<u64>) -> Option<u64> {
        self.slot.filter(|b| *b != 0).or(default)
    }
}

/// A chosen style: the modifier that replaces an appearance-list element's own modifier
/// for its key (`appearance.md` §8 step 3: head, hair and beard styles).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleChoice {
    /// The `AppearanceKey` of the list element (`FaceTexture`, `HeadMesh`, `BeardMesh`, …).
    pub key: u32,
    /// The block selector to use (style index / 100).
    pub modifier: f32,
}

/// A playable figure to assemble (`appearance.md` §2, §8).
///
/// The data has no body-type choice beyond sex (`appearance.md` §2.2): the body is the
/// (species, sex) pair, resolved through the race table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CharacterSpec {
    /// `SpeciesType` key.
    pub species: u32,
    /// `RaceTable_GenderType`: 0x1000 male, 0x2000 female.
    pub sex: u32,
    /// Chosen styles; elements without a choice keep the avatar's defaults.
    pub styles: Vec<StyleChoice>,
    /// The slider byte string (`AC_BoneSlider_Value_Array`); `None` keeps the avatar's.
    pub slider_values: Option<String>,
    /// `AC_BoneScale_Test`; `None` keeps the avatar's. It holds the body-shape deformer
    /// (`appearance.md` §6.2, §6.4).
    pub bone_scale: Option<f32>,
    /// `AC_Avatar_Height`; `None` keeps the avatar's. It holds the River-hobbit layer-110
    /// clip (`appearance.md` §6.2, `script-table.md` §6.8).
    pub avatar_height: Option<f32>,
    /// Worn and held items; each item decides worn versus held itself (`setup.md` §13.3).
    pub items: Vec<EquippedItem>,
}

impl CharacterSpec {
    /// A body with its defaults and nothing equipped.
    #[must_use]
    pub fn new(species: u32, sex: u32) -> Self {
        Self {
            species,
            sex,
            ..Self::default()
        }
    }

    /// The same body with `item` added.
    #[must_use]
    pub fn wearing(mut self, item: EquippedItem) -> Self {
        self.items.push(item);
        self
    }
}

/// Where an input came from, for diagnostics and part origins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceRef {
    /// The setup's default parts (`appearance.md` §3.1).
    Setup,
    /// Element *n* of the templates' appearance sections (§7.4 step 2).
    TemplateSection(u32),
    /// Element *n* of the entity's `Appearance_ClassList` (§7.4 step 3).
    ClassList(u32),
    /// Element *n* of the instance list in use (§7.4 step 4; see
    /// [`AssembledCharacter::instance_list_origin`]).
    InstanceList(u32),
    /// Item *n* of [`CharacterSpec::items`].
    Item(u32),
    /// NPC prop *n* (1–4, `NPC_Cosmetic_Inventory_Item<n>`, §5.5).
    NpcProp(u8),
    /// The figure as a whole (body, sliders, scale).
    Figure,
}

/// Which property set supplied the instance list (`appearance.md` §7.4 step 4 Decision:
/// an instance property replaces the entity property of the same name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ListOrigin {
    /// The entity's own `Appearance_InstanceList` (possibly empty).
    #[default]
    Entity,
    /// The game object's property set (a weenie object).
    Object,
    /// The instance property set passed by the caller.
    Instance,
}

/// What kind of figure a setup makes (`appearance.md` §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RigKind {
    /// A part sits in slot `UpperBody`: takes appearance lists and worn pieces by slot.
    Humanoid,
    /// Skinned parts but no `UpperBody` part.
    Creature,
    /// No skinned part: drawn as its setup's parts (`setup.md` §7).
    Static,
}

/// The skeleton of the figure: the Havok tagfile embedded in its setup
/// (`skeleton-anim.md` §2.1), for `tessera-havok` / `tessera-anim` to decode.
///
/// The bytes are shared: an assembler copies a setup's tagfile once and hands the same
/// `Arc` to every figure it assembles on that setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonSource {
    /// The 0x04 setup.
    pub setup: Did,
    /// The tagfile bytes, verbatim.
    pub tagfile: Arc<[u8]>,
}

/// Where a drawn part came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartOrigin {
    /// Part `index` of the setup's part list (`appearance.md` §3.1).
    Setup {
        /// Position in the setup's part list.
        index: u32,
    },
    /// A part of an appearance block.
    Appearance {
        /// The list element or item that offered it.
        source: SourceRef,
        /// The 0x20 record.
        record: Did,
        /// The entry key.
        key: u32,
        /// The block's `variant`.
        variant: f32,
    },
}

/// Which layer a [`MaterialLayer`] is (`material.md` §9.3, §12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    /// A material group of the part's own block (`appearance.md` §4.10).
    Group,
    /// A group of the first-channel dye block (`ClothingColor`, §4.9).
    Dye,
    /// A group of the second-channel dye block (`ClothingColorSecondary`).
    SecondaryDye,
}

/// A material group layered onto a part, as DIDs (`material.md` §9.3): it applies to the
/// part's submeshes whose surface record has `binding_key`. Layers are listed in the order
/// `material.md` §9.2 applies them: the block's groups, then the dye blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialLayer {
    /// What the layer is.
    pub kind: LayerKind,
    /// The group's priority (`appearance.md` §4.3; how priorities combine is OPEN, §4.10).
    pub priority: f32,
    /// Shader override (0x2B), if any.
    pub shader: Option<Did>,
    /// The material (0x30).
    pub material: Did,
    /// Matched against the surface records' binding keys.
    pub binding_key: u32,
    /// The group's slot field (`All` or a body slot).
    pub group_slot: u32,
    /// The 0x20 record the group comes from.
    pub record: Did,
    /// The block's `variant` (the dye code for dye blocks).
    pub variant: f32,
}

impl MaterialLayer {
    pub(crate) fn from_group(
        kind: LayerKind,
        priority: f32,
        g: &MaterialGroup,
        record: Did,
        variant: f32,
    ) -> Self {
        Self {
            kind,
            priority,
            shader: Some(g.shader).filter(|d| !d.is_null()),
            material: g.material,
            binding_key: g.binding_key,
            group_slot: g.slot(),
            record,
            variant,
        }
    }
}

/// One part of the assembled figure, in the setup's model space with an identity part
/// transform (`skeleton-anim.md` §9.1).
#[derive(Debug, Clone, PartialEq)]
pub struct AssembledPart {
    /// The `MeshTypeID` slot (0 when the default part's mesh could not be read).
    pub slot: u32,
    /// The mesh (0x06).
    pub mesh: Did,
    /// The winning priority; [`f32::NEG_INFINITY`] for a default part.
    pub priority: f32,
    /// Where it came from.
    pub origin: PartOrigin,
    /// The mesh is a stub: the slot is hidden (`appearance.md` §4.4). Whether stubs are
    /// drawn is OPEN (O4); they never matter for what is hidden.
    pub hidden: bool,
    /// The mesh has bone influences.
    pub skinned: bool,
    /// Material groups and dye layers, in application order.
    pub materials: Vec<MaterialLayer>,
}

impl AssembledPart {
    /// Whether the part came from the setup's default body.
    #[must_use]
    pub fn is_default(&self) -> bool {
        matches!(self.origin, PartOrigin::Setup { .. })
    }

    /// The input that offered the part (`None` for a default part).
    #[must_use]
    pub fn source(&self) -> Option<SourceRef> {
        match self.origin {
            PartOrigin::Setup { .. } => None,
            PartOrigin::Appearance { source, .. } => Some(source),
        }
    }
}

/// A held item or NPC prop placed at a holding location of the wearer (`setup.md` §13.1,
/// `skeleton-anim.md` §8.4).
#[derive(Debug, Clone, PartialEq)]
pub struct HeldPart {
    /// The input (an item of the spec or an NPC prop).
    pub source: SourceRef,
    /// The item (for spec items).
    pub item: Option<Did>,
    /// The entity resolved (the item's `PhysObj` or the prop entity).
    pub entity: Did,
    /// The entity chain to the item's setup, entity first and setup last.
    pub chain: Vec<Did>,
    /// The item's setup.
    pub setup: Did,
    /// The item's part meshes, drawn in its own object space.
    pub parts: Vec<Did>,
    /// The `HoldingLocation` key (the attach slot).
    pub location: u32,
    /// What the location's frame is relative to on the wearer (a bone or the origin).
    pub anchor: Anchor,
    /// The location's frame in the anchor's space: the item's offset.
    pub frame: Frame,
    /// The wield state used to choose the location.
    pub state: WieldState,
}

/// The whole-body scale (`appearance.md` §6.1 Decision): template × entity × instance.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelScale {
    /// The first template's uniform `vec3`.
    pub template: Option<f32>,
    /// The entity's `Render_AdjustableScale`.
    pub entity: Option<f32>,
    /// The instance set's `Render_AdjustableScale`.
    pub instance: Option<f32>,
}

impl ModelScale {
    /// The product of the present factors (1 when none is). Composition OPEN (O8).
    #[must_use]
    pub fn value(&self) -> f32 {
        self.template.unwrap_or(1.0) * self.entity.unwrap_or(1.0) * self.instance.unwrap_or(1.0)
    }
}

/// Slider and height inputs for `tessera-anim` (`appearance.md` §6.2, §6.3). The values are
/// passed through; applying them is the animation system's job.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliderInputs {
    /// The bone-slider definition record (0x78) the avatar's UI names (the figure uses the
    /// record its SliderMeshDeformer node names, `appearance.md` §6.3; the contents agree).
    pub definitions: Option<Did>,
    /// The value string's pieces, read by the client's rule
    /// ([`tessera_formats::avatar::slider_value_pieces`], `appearance.md` §6.3, A16, A29):
    /// the *k*-th piece belongs to the *k*-th slider of the definition array; sliders without
    /// a piece sit at 0.5. Empty when the avatar carries no value string (a string, even an
    /// empty one, has at least one piece).
    pub values: Vec<u32>,
    /// `AC_BoneScale_Test`.
    pub bone_scale: Option<f32>,
    /// `AC_Avatar_Height`.
    pub avatar_height: Option<f32>,
}

/// What the script-table clip resolver needs (`script-table.md` §8.1, §11.3):
/// `collect_entity_scripts(start, …)`, then the instance set added with
/// `EntityScripts::add_property_set`, then a motion state.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipInputs {
    /// The DID to collect scripts from: the game object (players, weenies) or the entity.
    pub start: Did,
    /// The 0x47 entity of the figure.
    pub entity: Did,
    /// The instance property set to add after the object's and entity's.
    pub instance_properties: Option<PropertySet>,
    /// `NPC_StartingIdleAnimation` (an idle animation enum value, `appearance.md` §7.2).
    pub starting_idle_animation: Option<u32>,
}

/// Categories of unresolved links and data gaps (`appearance.md` §9, §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticCategory {
    /// A list element names the null DID (AI8): nothing to show.
    NullAppearance,
    /// A named 0x20 record does not exist (AI8 / AI1).
    AppearanceMissing,
    /// A named 0x20 record does not decode.
    AppearanceUndecodable,
    /// The record lacks the element's or item's key (§10: item not drawn for that body).
    KeyMissing,
    /// No block matches the modifier: §3.2.1 rule 3 chose the base or first block.
    BlockFallback,
    /// The entry has no parts where parts were expected (a worn item's material-only
    /// block, an entry without blocks, a list block with neither parts nor groups).
    MaterialOnly,
    /// An item's wardrobe entry has no `variant` 1.0 block (contradicts AI1).
    NoBaseBlock,
    /// The item's property set does not exist.
    ItemMissing,
    /// The item's property set does not decode.
    ItemUndecodable,
    /// The item has neither a worn appearance nor a `PhysObj` (jewellery): invisible.
    ItemInvisible,
    /// The item has wardrobe structs, but none for the wearer after the §2.3 fallback.
    NoWardrobeStruct,
    /// The dye entry (`ClothingColor` / `ClothingColorSecondary`) is missing in the record.
    DyeEntryMissing,
    /// No dye block has exactly the code; the `material.md` §12.4 rule chose another.
    DyeInexact,
    /// A dye or material-only list element whose record contributed no parts through the
    /// appearance lists; applied to the default parts (Decision, `appearance.md` §15).
    UnpairedDye,
    /// A part's mesh does not exist: the slot keeps its previous winner (§10).
    MeshMissing,
    /// A part's mesh does not decode: the slot keeps its previous winner.
    MeshUndecodable,
    /// A block carries extras (setup DIDs, `setup.md` §8.3): not used by assembly.
    BlockExtraIgnored,
    /// A held item has no holding location (class items, instruments; §5.3 Decision: not
    /// drawn).
    NoHoldingLocation,
    /// The wearer's setup lacks the holding location: not drawn (§10).
    HoldingLocationMissing,
    /// A held item's or prop's entity chain does not resolve to a setup.
    HeldChainBroken,
    /// The figure has a random-appearance type: drawn with its default parts and held
    /// props, its appearance lists skipped (§7.4 Decision, O12).
    RandomAppearance,
    /// The slider byte string is not clean hex: pairs that are not hexadecimal read 0
    /// (`appearance.md` §6.3).
    SliderValuesInvalid,
    /// A bound of [`AssemblyLimits`] cut the input.
    LimitExceeded,
}

impl DiagnosticCategory {
    /// Every category, in a stable order.
    pub const ALL: [Self; 23] = [
        Self::NullAppearance,
        Self::AppearanceMissing,
        Self::AppearanceUndecodable,
        Self::KeyMissing,
        Self::BlockFallback,
        Self::MaterialOnly,
        Self::NoBaseBlock,
        Self::ItemMissing,
        Self::ItemUndecodable,
        Self::ItemInvisible,
        Self::NoWardrobeStruct,
        Self::DyeEntryMissing,
        Self::DyeInexact,
        Self::UnpairedDye,
        Self::MeshMissing,
        Self::MeshUndecodable,
        Self::BlockExtraIgnored,
        Self::NoHoldingLocation,
        Self::HoldingLocationMissing,
        Self::HeldChainBroken,
        Self::RandomAppearance,
        Self::SliderValuesInvalid,
        Self::LimitExceeded,
    ];

    /// A short kebab-case name for logs and reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NullAppearance => "null-appearance",
            Self::AppearanceMissing => "appearance-missing",
            Self::AppearanceUndecodable => "appearance-undecodable",
            Self::KeyMissing => "key-missing",
            Self::BlockFallback => "block-fallback",
            Self::MaterialOnly => "material-only",
            Self::NoBaseBlock => "no-base-block",
            Self::ItemMissing => "item-missing",
            Self::ItemUndecodable => "item-undecodable",
            Self::ItemInvisible => "item-invisible",
            Self::NoWardrobeStruct => "no-wardrobe-struct",
            Self::DyeEntryMissing => "dye-entry-missing",
            Self::DyeInexact => "dye-inexact",
            Self::UnpairedDye => "unpaired-dye",
            Self::MeshMissing => "mesh-missing",
            Self::MeshUndecodable => "mesh-undecodable",
            Self::BlockExtraIgnored => "block-extra-ignored",
            Self::NoHoldingLocation => "no-holding-location",
            Self::HoldingLocationMissing => "holding-location-missing",
            Self::HeldChainBroken => "held-chain-broken",
            Self::RandomAppearance => "random-appearance",
            Self::SliderValuesInvalid => "slider-values-invalid",
            Self::LimitExceeded => "limit-exceeded",
        }
    }
}

impl std::fmt::Display for DiagnosticCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// One unresolved link or data gap. The figure is still assembled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// What happened.
    pub category: DiagnosticCategory,
    /// Which input.
    pub source: SourceRef,
    /// The record concerned (item, 0x20 record, mesh, entity), if any.
    pub did: Option<Did>,
    /// A key concerned (appearance key, holding location, random-appearance type), if any.
    pub key: Option<u32>,
}

/// A dressed figure (`appearance.md` §8 steps 1–6 and 9 as data; steps 7–8 are per frame
/// in the animation system and the renderer).
#[derive(Debug, Clone, PartialEq)]
pub struct AssembledCharacter {
    /// The game object assembly started from, if any.
    pub object: Option<Did>,
    /// The 0x47 entity.
    pub entity: Did,
    /// The entity chain to the setup (`setup.md` §3.1), entity first and setup last.
    pub chain: Vec<Did>,
    /// What kind of figure the setup makes.
    pub rig: RigKind,
    /// The skeleton source.
    pub skeleton: SkeletonSource,
    /// The decoded setup (holding locations, attach map, default parts).
    pub setup: Arc<Setup>,
    /// Drawn and hidden parts: the setup's parts in order, each replaced by its slot's
    /// winner, then winners for slots the setup lacks, in first-filled order. At most one
    /// part per replaced slot: on every rig, static setups included, a winning offer
    /// replaces all default parts of its slot and is drawn once, at the first one's
    /// position (`appearance.md` §15). Slots nothing offered keep all their default parts.
    pub parts: Vec<AssembledPart>,
    /// Held items and props.
    pub held: Vec<HeldPart>,
    /// Whole-body scale.
    pub scale: ModelScale,
    /// Slider and height inputs.
    pub sliders: SliderInputs,
    /// Clip-selection inputs.
    pub clip_inputs: ClipInputs,
    /// Which set supplied the instance list.
    pub instance_list_origin: ListOrigin,
    /// The random-appearance type, if the figure has one other than `Undef` (§7.4).
    pub random_appearance: Option<u32>,
    /// Unresolved links, in the order they were found.
    pub diagnostics: Vec<Diagnostic>,
    /// Diagnostics beyond [`AssemblyLimits::max_diagnostics`], counted only.
    pub dropped_diagnostics: usize,
}

impl AssembledCharacter {
    /// The part in `slot` (the first, for a static setup with several parts in one slot).
    #[must_use]
    pub fn part_in(&self, slot: u32) -> Option<&AssembledPart> {
        self.parts.iter().find(|p| p.slot == slot)
    }

    /// Slots whose winning part is a stub: hidden.
    pub fn hidden_slots(&self) -> impl Iterator<Item = u32> + '_ {
        self.parts.iter().filter(|p| p.hidden).map(|p| p.slot)
    }

    /// The parts an input contributed (winners only).
    pub fn parts_from(&self, source: SourceRef) -> impl Iterator<Item = &AssembledPart> + '_ {
        self.parts
            .iter()
            .filter(move |p| p.source() == Some(source))
    }

    /// Number of diagnostics of `category` (kept ones).
    #[must_use]
    pub fn count(&self, category: DiagnosticCategory) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.category == category)
            .count()
    }

    /// The model scale ([`ModelScale::value`]).
    #[must_use]
    pub fn model_scale(&self) -> f32 {
        self.scale.value()
    }
}

/// Failures that leave nothing to assemble. Everything else is a [`Diagnostic`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssemblyError {
    /// The race table has no avatar for (species, sex).
    #[error("no playable body for species {species:#x}, sex {sex:#x}")]
    NoSuchBody {
        /// The `SpeciesType` key.
        species: u32,
        /// The sex key.
        sex: u32,
    },
    /// The start does not resolve to a body (no `PhysObj`, a broken entity chain, a
    /// missing or undecodable entity).
    #[error("{start} does not resolve to a body: {reason}")]
    Body {
        /// The DID assembly started from.
        start: Did,
        /// What failed.
        reason: String,
    },
    /// The body's setup is missing or does not decode.
    #[error("setup {did}: {reason}")]
    Setup {
        /// The setup.
        did: Did,
        /// What failed.
        reason: String,
    },
    /// The record source failed (I/O, archive corruption): not a data gap.
    #[error("record source failed: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}
