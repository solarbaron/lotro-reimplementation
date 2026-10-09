// SPDX-License-Identifier: GPL-3.0-or-later
//! The immutable skin overlay and the activation of skins by name (`skin-definition.md`
//! §6.3, §6.6, §8.2–§8.3, §9.3; D6, D13, D20–D23).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tessera_formats::ui_layout::ElementDesc;

use super::art::{ArtReplacement, ImageChoice, SkinDirIndex, SkinFile, resolve_image};
use super::definition::ElementOverride;
use super::discover::{Skin, SkinSet};
use super::geometry::apply_override;
use super::tables::SkinTables;

/// Environment of an activation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkinEnvironment {
    /// The Wine/Proton prefix of the install, through which drive-letter `FileName`s are
    /// mapped (D12); `None` on a native install, where they fail.
    pub wine_prefix: Option<PathBuf>,
}

/// An active user skin as the element tree and the client consult it: art replacements by
/// art-asset value and panel overrides by element ID path. Immutable (apart from the
/// folder listings its asset workers share); shared as an `Arc`.
#[derive(Debug, Clone)]
pub struct SkinOverlay {
    skin: Arc<Skin>,
    tables: Arc<SkinTables>,
    replacements: HashMap<u32, ArtReplacement>,
    generation: u64,
    dirs: SkinDirIndex,
}

impl SkinOverlay {
    /// Builds the overlay of `skin` against `tables`. `generation` identifies the activation
    /// (it changes at every switch). No file-system work: every `FileName` is joined
    /// lexically (§6.4, D12) and resolved later, on the asset workers.
    #[must_use]
    pub fn new(
        skin: Arc<Skin>,
        tables: Arc<SkinTables>,
        env: &SkinEnvironment,
        generation: u64,
    ) -> Self {
        let replacements = skin
            .definition
            .mappings()
            .iter()
            .map(|m| {
                (
                    m.art_asset,
                    ArtReplacement::new(m, &skin.folder, env.wine_prefix.as_deref()),
                )
            })
            .collect();
        Self {
            skin,
            tables,
            replacements,
            generation,
            dirs: SkinDirIndex::new(),
        }
    }

    /// The folder listings of this activation, for [`SkinFile::load_with`] on the asset
    /// workers (each folder is listed at most once per activation; a switch starts anew,
    /// §6.6).
    #[must_use]
    pub fn dir_index(&self) -> &SkinDirIndex {
        &self.dirs
    }

    /// The skin.
    #[must_use]
    pub fn skin(&self) -> &Arc<Skin> {
        &self.skin
    }

    /// The tables it was resolved against.
    #[must_use]
    pub fn tables(&self) -> &Arc<SkinTables> {
        &self.tables
    }

    /// The activation generation.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// What the base-image setter draws for `did` (§6.3) with this user skin active and no
    /// game skin (the offline client never activates one, D13). Use
    /// [`resolve_image`](super::resolve_image) directly to include a game skin.
    ///
    /// Only the base-image setter consults this: Image items from data and from code,
    /// Animation frames, meter frame arrays and child images, the game-styled plugin
    /// controls, and the refresh after a switch for every base image made from a DID
    /// (D21). Coloured-image overlays, hit masks, cursors and plugin backgrounds when set
    /// do not.
    #[must_use]
    pub fn resolve_image(&self, did: u32) -> ImageChoice<'_> {
        resolve_image(did, &self.tables.base, Some(&self.replacements), None)
    }

    /// The replacement of an art-asset value.
    #[must_use]
    pub fn replacement(&self, art_asset: u32) -> Option<&ArtReplacement> {
        self.replacements.get(&art_asset)
    }

    /// The art replacement table (art-asset value → replacement).
    #[must_use]
    pub fn replacements(&self) -> &HashMap<u32, ArtReplacement> {
        &self.replacements
    }

    /// The distinct files of the skin (by cache key), for the client to drop from the image
    /// cache at a switch (§6.6) or to schedule ahead.
    #[must_use]
    pub fn files(&self) -> Vec<&SkinFile> {
        let mut out: Vec<&SkinFile> = self
            .replacements
            .values()
            .filter_map(|r| r.file.as_ref().ok())
            .collect();
        out.sort_by(|a, b| a.cache_key().cmp(b.cache_key()));
        out.dedup_by(|a, b| a.cache_key() == b.cache_key());
        out
    }

    /// The override for the element `element_id` built inside the ID path `path` (outermost
    /// first; empty for the element a build starts from), §8.3.
    #[must_use]
    pub fn find_override(&self, path: &[u32], element_id: u32) -> Option<&ElementOverride> {
        self.skin.definition.find_override(path, element_id)
    }

    /// Matches and applies the override of a descriptor about to be built (§8.2 step 2):
    /// `desc` must be the per-build resolved copy and `path` the element IDs enclosing it
    /// within the current build. Returns whether an override applied.
    pub fn apply(&self, desc: &mut ElementDesc, path: &[u32]) -> bool {
        match self.find_override(path, desc.element_id) {
            Some(ov) => apply_override(desc, ov, &self.tables.geometry),
            None => false,
        }
    }
}

/// The outcome of a switch: the client empties the skin entries of the decoded-image cache
/// (those of `previous`), resets its [`super::SkinFileLedger`] and runs the refresh of §6.6
/// over every existing element; no panel is rebuilt (D20).
#[derive(Debug, Clone)]
pub struct SkinSwitch {
    /// The overlay that was active before.
    pub previous: Option<Arc<SkinOverlay>>,
    /// The overlay active now.
    pub current: Option<Arc<SkinOverlay>>,
    /// The name requested, when no skin of that name exists (none is active then).
    pub not_found: Option<String>,
}

/// Selection and activation of user skins by name (§9.3, D6, D23).
///
/// The client passes the name stored in its settings (`[UI] UserSkin`, empty for no skin)
/// when its world view's UI is created and when the option is applied, and deactivates
/// when that UI is torn down. Pre-game screens are never skinned.
#[derive(Debug)]
pub struct SkinSelection {
    skins: Arc<SkinSet>,
    tables: Arc<SkinTables>,
    env: SkinEnvironment,
    active: Option<Arc<SkinOverlay>>,
    generation: u64,
}

impl SkinSelection {
    /// A selection with no skin active.
    #[must_use]
    pub fn new(skins: Arc<SkinSet>, tables: Arc<SkinTables>, env: SkinEnvironment) -> Self {
        Self {
            skins,
            tables,
            env,
            active: None,
            generation: 0,
        }
    }

    /// The discovered skins.
    #[must_use]
    pub fn skins(&self) -> &Arc<SkinSet> {
        &self.skins
    }

    /// The active overlay.
    #[must_use]
    pub fn active(&self) -> Option<&Arc<SkinOverlay>> {
        self.active.as_ref()
    }

    /// Activation generation (incremented at every activation or deactivation).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Applies the option (§9.3): deactivate, then, for a non-empty name, activate the
    /// first discovered skin whose name equals it ignoring ASCII case.
    pub fn apply_option(&mut self, name: &str) -> SkinSwitch {
        let previous = self.active.take();
        self.generation = self.generation.wrapping_add(1);
        let mut not_found = None;
        if !name.is_empty() {
            match self.skins.find(name) {
                Some(skin) => {
                    self.active = Some(Arc::new(SkinOverlay::new(
                        Arc::clone(skin),
                        Arc::clone(&self.tables),
                        &self.env,
                        self.generation,
                    )));
                }
                None => {
                    tracing::info!(name, "selected user skin not found; no skin active");
                    not_found = Some(name.to_owned());
                }
            }
        }
        SkinSwitch {
            previous,
            current: self.active.clone(),
            not_found,
        }
    }

    /// Deactivates the user skin (the world view's UI is torn down, §9.3).
    pub fn deactivate(&mut self) -> SkinSwitch {
        self.apply_option("")
    }
}
