// SPDX-License-Identifier: GPL-3.0-or-later
//! User skins: discovery, `SkinDefinition.xml`, art replacement and panel geometry overrides
//! (`skin-definition.md`).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`skins_root_in`], [`default_skins_root`] | `<settings folder>/ui/skins`, letter case resolved | §3.1, D1, D2; `install-layout.md` §8 |
//! | [`SkinSet::discover`] | Depth-first walk for `SkinDefinition.xml`, sorted entries, limits, rejected files with reasons | §3.2, D3, D4, D9 |
//! | [`parse_definition`], [`Definition`] | Strict bounded XML (no DTD, no external entities), `SkinName`, `Mapping`, `PanelFile`/`Element`, integers, acceptance | §4, D5, D8 |
//! | [`SkinTables`], [`NameResolver`], [`LabelIndex`], [`BaseTable`], [`GameSkin`] | Name resolution and the art tables from the user's data | §5, §6.1, §6.2 |
//! | [`resolve_image`], [`ArtReplacement`], [`SkinFile`], [`SkinDirIndex`], [`SkinFileLedger`] | Per-image precedence, `FileName` → path (lexical), letter-case resolution and decoding through `tessera-material::ui_image` on a worker, load tickets, failed loads | §6.3–§6.6, §7, D2, D7, D11, D12, D16, D20, D25–D27 |
//! | [`apply_override`], [`scale_about_origin`] | Geometry overrides on the per-build resolved copy, the scaler origin | §8, D19 |
//! | [`SkinOverlay`], [`SkinSelection`] | The immutable overlay the element tree consults; activation by name | §9.3, D6, D20–D23 |
//!
//! # How the pieces fit
//!
//! At UI start the client loads [`SkinTables`] from the user's data, finds the skins root
//! ([`default_skins_root`] or Tessera's own setting, D1) and runs [`SkinSet::discover`]
//! once (§9.4). [`SkinSelection::apply_option`] activates the skin named by Tessera's
//! settings (`[UI] UserSkin`, D6) when the world view's UI is created, and
//! [`SkinSelection::deactivate`] when it is torn down. Each switch returns a
//! [`SkinSwitch`]: the client drops the skin images of the previous overlay from the
//! decoded-image cache, resets its [`SkinFileLedger`] and re-sets every base image from its
//! original DID (§6.6, D20, D22). No panel is rebuilt.
//!
//! The element tree consults the active [`SkinOverlay`]:
//!
//! - **Art.** For every base image set from a DID, [`SkinOverlay::resolve_image`] says
//!   whether to draw the DID or a skin file. For a file, [`SkinFileLedger::request`] says
//!   whether to schedule [`SkinFile::load_with`] (with [`SkinOverlay::dir_index`]) on an
//!   asset worker and report it with its [`LoadTicket`], wait (draw nothing, D16), take the
//!   decoded image from the cache under [`SkinFile::image_key`] (reporting
//!   [`SkinFileLedger::evicted`] on a miss), or draw the placeholder
//!   (`tessera_material::ui_image::placeholder`, §6.5).
//! - **Geometry.** Before creating an element from its per-build resolved descriptor,
//!   [`SkinOverlay::apply`] matches the override through the element-ID path of the
//!   current build and applies it (§8.2–§8.4, D19). Not for the plugin API's game-styled
//!   controls (D28).

mod art;
mod definition;
mod discover;
mod geometry;
mod overlay;
mod tables;
pub mod xml;

pub use art::{
    ArtReplacement, FileState, ImageChoice, LoadRequest, LoadTicket, MAX_INDEXED_ENTRIES,
    SkinDirIndex, SkinFile, SkinFileLedger, SkinImageError, SkinPathError, resolve_image,
    skin_file,
};
pub use definition::{
    Definition, DefinitionError, Diagnostic, DiagnosticKind, ElementOverride, MAX_DEFINITION_BYTES,
    MAX_ELEMENT_OVERRIDES, MAX_MAPPINGS, Mapping, PanelOverride, parse_definition, parse_int,
};
pub use discover::{
    DEFINITION_FILE, MAX_FOLDER_ENTRIES, MAX_SKINS, MAX_WALK_DEPTH, RejectedDefinition, Skin,
    SkinChoice, SkinSet, WalkIssue, default_skins_root, skins_root_in,
};
pub use geometry::{ScaleChange, apply_override, override_rect, scale_about_origin};
pub use overlay::{SkinEnvironment, SkinOverlay, SkinSelection, SkinSwitch};
pub use tables::{
    ART_ASSET_LABEL, BASE_SKIN_LABEL, BaseTable, ELEMENT_ID_LABEL, EMAPPER_LABEL, GAME_SKIN_LABELS,
    GameSkin, GeometryPids, LabelIndex, MappingPids, NameResolver, SkinDataError, SkinTables,
    UNIQUEDB_LABEL, names,
};
