// SPDX-License-Identifier: GPL-3.0-or-later
//! Labels of the ID fields of UI layouts through the enum mappers of `ui-layout.md` §3.3.
//!
//! Each kind of ID has a game-level enum mapper whose base mapper holds the engine-level
//! labels (keys below 0x10000000). The mappers are found at run time through the root DID
//! map label `EMAPPER` and the label in [`UiEnum::emapper_label`]; nothing here hard-codes a
//! DID. [`UiLabels::load`] takes the lookups as closures so that this crate stays
//! independent of the archive layer.

use std::collections::HashSet;

use crate::did::Did;
use crate::enum_mapper::{ENUM_BASE_CHAIN_LIMIT, EnumMapper};

/// The five ID enums of §3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UiEnum {
    /// Element IDs, table keys, base-element IDs, element-valued properties.
    ElementId,
    /// State IDs, default states, State media items.
    StateId,
    /// Element (widget) types.
    ElementType,
    /// Draw modes of Image, Animation and Coloured-image items.
    DrawMode,
    /// UI messages (Message media items).
    Message,
}

impl UiEnum {
    /// All five, in the order of the §3.3 table.
    pub const ALL: [Self; 5] = [
        Self::ElementId,
        Self::StateId,
        Self::ElementType,
        Self::DrawMode,
        Self::Message,
    ];

    /// The label of the mapper in the `EMAPPER` DID map (an interoperability identifier read
    /// from the install, §3.3).
    #[must_use]
    pub fn emapper_label(self) -> &'static str {
        match self {
            Self::ElementId => "UIElementID",
            Self::StateId => "UIStateID",
            Self::ElementType => "UIElementType",
            Self::DrawMode => "DrawModeType",
            Self::Message => "UIElementMessage",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The enum mapper chains of the five UI enums, for labelling IDs.
#[derive(Debug, Clone, Default)]
pub struct UiLabels {
    /// Per [`UiEnum`] (by its index), the mapper and its base chain, nearest first.
    chains: [Vec<EnumMapper>; 5],
}

impl UiLabels {
    /// Loads the five chains. `emapper` returns the DID that the `EMAPPER` map gives a label
    /// (or `None`); `fetch` loads and decodes an enum mapper. A missing mapper leaves its
    /// enum without labels; a base chain stops at a missing mapper, a cycle or
    /// [`ENUM_BASE_CHAIN_LIMIT`] links.
    pub fn load(
        mut emapper: impl FnMut(&str) -> Option<Did>,
        mut fetch: impl FnMut(Did) -> Option<EnumMapper>,
    ) -> Self {
        let mut out = Self::default();
        for which in UiEnum::ALL {
            let mut did = emapper(which.emapper_label()).unwrap_or_default();
            let mut seen = HashSet::new();
            let chain = &mut out.chains[which.index()];
            while !did.is_null() && chain.len() < ENUM_BASE_CHAIN_LIMIT && seen.insert(did) {
                let Some(m) = fetch(did) else { break };
                did = m.base;
                chain.push(m);
            }
            if chain.is_empty() {
                tracing::debug!(label = which.emapper_label(), "UI enum mapper not found");
            }
        }
        out
    }

    /// Builds the labels from chains already loaded (nearest mapper first), in the order of
    /// [`UiEnum::ALL`].
    #[must_use]
    pub fn from_chains(chains: [Vec<EnumMapper>; 5]) -> Self {
        Self { chains }
    }

    /// The chain of `which`, nearest first (empty when its mapper was not found).
    #[must_use]
    pub fn chain(&self, which: UiEnum) -> &[EnumMapper] {
        &self.chains[which.index()]
    }

    /// Whether any mapper of `which` was loaded.
    #[must_use]
    pub fn is_loaded(&self, which: UiEnum) -> bool {
        !self.chain(which).is_empty()
    }

    /// The label of `key` in `which`'s chain, the nearest mapper first.
    #[must_use]
    pub fn label(&self, which: UiEnum, key: u32) -> Option<&str> {
        self.chain(which).iter().find_map(|m| m.name(key))
    }

    /// The key whose label is `name` in `which`'s chain, the nearest mapper first.
    #[must_use]
    pub fn key(&self, which: UiEnum, name: &str) -> Option<u32> {
        self.chain(which).iter().find_map(|m| m.key_of(name))
    }

    /// Label of an element ID.
    #[must_use]
    pub fn element(&self, id: u32) -> Option<&str> {
        self.label(UiEnum::ElementId, id)
    }

    /// Label of a state ID.
    #[must_use]
    pub fn state(&self, id: u32) -> Option<&str> {
        self.label(UiEnum::StateId, id)
    }

    /// Label of an element type.
    #[must_use]
    pub fn element_type(&self, ty: u32) -> Option<&str> {
        self.label(UiEnum::ElementType, ty)
    }

    /// Label of a draw mode.
    #[must_use]
    pub fn draw_mode(&self, mode: u32) -> Option<&str> {
        self.label(UiEnum::DrawMode, mode)
    }

    /// Label of a UI message.
    #[must_use]
    pub fn message(&self, id: u32) -> Option<&str> {
        self.label(UiEnum::Message, id)
    }
}
