// SPDX-License-Identifier: GPL-3.0-or-later
//! The far view: distant-landscape sectors around the viewer, far-terrain batches, distant
//! sprites and far entities (`docs/specs/far-view.md`, `docs/specs/distant-landscape.md`
//! §5).
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`settings`] | *R*, *F*, *N*, *E*, the far-draw fade (FD1), the template choice, the sprite gate | `far-view.md` §3.1, §6.1, §7, §9.1 |
//! | [`grid`] | sector keys and the sector list, LOD by squared landblock distance, the near field, the per-landblock grid and the nine descriptors per sector | §3.3, §4; DL21, DL20 |
//! | [`residency`] | rebuild rules, listed / detached / freed sectors (FD2) | §3.4, §3.5 |
//! | [`mesh`] | far and near batches of the terrain and ceiling meshes, stitching, the generated index list | §5; `distant-landscape.md` §5.3 |
//! | [`sprites`] | sprite placement from the sector lists and imposter tables, fade-in distance, mirror flag | §9 |
//! | [`entities`] | the distant set of far entities | §10 |
//! | [`horizon`] | placement of the horizon fill mesh | §7.3 |
//! | [`shading`] | CPU twins of the shading terms (fog, fade, near-view mask, normal strength, coverage, sprite opacity) | §6.3–§6.5, §9.3 |
//! | [`load`] | the UNIQUEDB entries (templates, detail texture, horizon mesh), sector and imposter-table reads | §6.1, §7.3 |
//! | [`state`] | the per-frame driver: residency, records, grid, build and sprite jobs swapped in per sector (FD3) | §3.6 |
//!
//! The walker keeps its own full-detail rings (Tessera Decision FD9): it passes the
//! landblocks it draws at full detail as [`NearField::Set`], and the far batches skip and
//! stitch to them; [`NearField::Radius`]`(1)` is the original's 3 × 3 near field.
//!
//! # Hostile input
//!
//! Sector records and imposter tables come from the decoders of `tessera-formats`. The mesh
//! builder checks `block_size` and the sample blocks it uses and never indexes data-derived
//! values unchecked; sprite placement skips sprites with non-finite or non-positive sizes or
//! positions and caps each sector at [`sprites::MAX_SPRITES_PER_SECTOR`]; the far radius
//! is capped at [`settings::MAX_FAR_RADIUS`] (289 listed sectors at most).

pub mod entities;
pub mod grid;
pub mod horizon;
pub mod load;
pub mod mesh;
pub mod residency;
pub mod settings;
pub mod shading;
pub mod sprites;
pub mod state;

#[cfg(test)]
mod tests;

pub use entities::{DistantChanges, DistantEntry, DistantSet};
pub use grid::{
    Cell, Edge, LandblockDescriptor, LandblockGrid, NearField, SectorKey, lod_of, sector_list,
};
pub use horizon::{HorizonPlacement, MeshBounds};
pub use load::{FarViewRecords, read_imposter_table, read_sector};
pub use mesh::{Batch, FarMesh, MeshKind, build_batch, generated_index_list};
pub use residency::{Change, Residency, ResidencyUpdate, Viewer, classify};
pub use settings::{FarFade, FarViewSettings, Template};
pub use sprites::{FarSprite, SpriteView, place_sprites};
pub use state::{
    BuildJob, FarViewState, FarViewWork, SectorMeshes, SectorRecordState, SectorSlot, SpriteJob,
    sector_water,
};
