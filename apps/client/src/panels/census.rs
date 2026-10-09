// SPDX-License-Identifier: GPL-3.0-or-later
//! Counts on an instantiated panel (`ui-layout.md` §17.2, second table): every element
//! resolved by §7, children resolved when they are created (§9.2 step 1), the text elements
//! and state blocks, and the distinct render surfaces, fonts and string references named
//! anywhere in the panel.
//!
//! The counting rules are those of the note under the §17.2 table, plus two columns the note
//! does not define, counted by Tessera's reading:
//!
//! - **Text elements**: resolved elements whose element type is the engine type `Text`.
//! - **States**: state blocks of the resolved elements, summed.
//!
//! - **Art surfaces**: distinct DIDs with prefix 0x41 in Image, Coloured-image and Cursor
//!   items and animation frames of elements and states, and in DID-valued properties
//!   (inside arrays and structs too).
//! - **Fonts**: distinct DIDs with prefix 0x42 in DID-valued properties.
//! - **String refs**: distinct StringInfo table references (table, token) in any property
//!   of any element or state block, inside arrays and structs too; literals not counted.

use std::collections::BTreeSet;
use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_formats::property_set::PropertySet;
use tessera_formats::string_info::StringSource;
use tessera_formats::ui_layout::{ElementDesc, MediaItem, StateBlock};
use tessera_formats::{Did, UiLayout};
use tessera_ui::resolve::resolve;
use tessera_ui::{MAX_ELEMENTS, MAX_TREE_DEPTH};

use super::PanelsError;
use super::catalog::PanelRef;

/// The counts of one instantiated panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelCensus {
    /// Elements instantiated (the root included).
    pub elements: usize,
    /// Elements whose base chain did not resolve (not counted in `elements`; UL §13).
    pub unresolved: usize,
    /// Resolved elements of the engine type `Text`.
    pub text_elements: usize,
    /// State blocks of the resolved elements, summed.
    pub states: usize,
    /// Distinct render surfaces (prefix 0x41).
    pub art_surfaces: BTreeSet<u32>,
    /// Distinct fonts (prefix 0x42).
    pub fonts: BTreeSet<u32>,
    /// Distinct string-table references (table DID, token).
    pub string_refs: BTreeSet<(u32, u32)>,
    /// Whether a cap ([`MAX_ELEMENTS`] or [`MAX_TREE_DEPTH`]) stopped the walk.
    pub truncated: bool,
}

/// Counts the panel `r`, resolving base chains through `lookup` (a decoded layout by DID,
/// `None` when missing or undecodable). `text_type` is the element type of the engine type
/// `Text` (looked up by label, `tessera_ui::Vocab::types`).
///
/// # Errors
/// [`PanelsError::NoLayout`] when the panel's layout is missing,
/// [`PanelsError::NoRoot`] when the root is not a top-level element of it, and
/// [`PanelsError::Unresolved`] when the root itself does not resolve.
pub fn panel_census(
    r: PanelRef,
    text_type: u32,
    lookup: &mut dyn FnMut(Did) -> Option<Arc<UiLayout>>,
) -> Result<PanelCensus, PanelsError> {
    let layout = lookup(r.layout).ok_or(PanelsError::NoLayout(r.layout.0))?;
    let top = layout
        .top_level(r.element_id)
        .ok_or(PanelsError::NoRoot(r.element_id))?;
    let root = resolve(top, lookup).map_err(|_| PanelsError::Unresolved(r.element_id))?;
    let mut c = PanelCensus::default();
    let mut stack: Vec<(ElementDesc, usize)> = vec![(root, 0)];
    while let Some((d, depth)) = stack.pop() {
        if c.elements >= MAX_ELEMENTS {
            c.truncated = true;
            break;
        }
        c.elements += 1;
        c.states += d.states.len();
        if d.element_type == text_type {
            c.text_elements += 1;
        }
        c.block(&d.block);
        for s in &d.states {
            c.block(s);
        }
        if depth >= MAX_TREE_DEPTH {
            if !d.children.is_empty() {
                c.truncated = true;
            }
            continue;
        }
        for child in d.children.iter().rev() {
            match resolve(child, lookup) {
                Ok(rc) => stack.push((rc, depth + 1)),
                Err(_) => c.unresolved += 1,
            }
        }
    }
    Ok(c)
}

impl PanelCensus {
    fn block(&mut self, b: &StateBlock) {
        for m in &b.media {
            match m {
                MediaItem::Image { did, .. }
                | MediaItem::ColouredImage { did, .. }
                | MediaItem::Cursor { did, .. } => self.surface(*did),
                MediaItem::Animation { frames, .. } => {
                    for f in frames {
                        self.surface(*f);
                    }
                }
                _ => {}
            }
        }
        self.properties(&b.properties);
    }

    fn properties(&mut self, set: &PropertySet) {
        set.walk(|v| match &v.entry.value {
            PropertyValue::DidRef(d) => {
                self.surface(*d);
                if d.0 >> 24 == 0x42 {
                    self.fonts.insert(d.0);
                }
            }
            PropertyValue::StringInfo(si) => {
                if let StringSource::Table { token, table } = &si.source {
                    self.string_refs.insert((table.0, *token));
                }
            }
            _ => {}
        });
    }

    fn surface(&mut self, d: Did) {
        if d.0 >> 24 == 0x41 {
            self.art_surfaces.insert(d.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::property::PropertyEntry;
    use tessera_formats::ui_layout::{DrawMode, Rect};

    fn image(did: u32) -> MediaItem {
        MediaItem::Image {
            did: Did(did),
            draw_mode: DrawMode(2),
            path: String::new(),
        }
    }

    #[test]
    fn counts_resolved_elements_and_distinct_surfaces() {
        let mut child = ElementDesc {
            element_id: 0x1000_0002,
            element_type: 16,
            ..ElementDesc::default()
        };
        child.block.media.push(image(0x4100_0002));
        child.block.media.push(MediaItem::Animation {
            duration: 1.0,
            draw_mode: DrawMode(2),
            looping: true,
            frames: vec![Did(0x4100_0002), Did(0x4100_0003)],
        });
        let mut state = StateBlock {
            state_id: 7,
            ..StateBlock::default()
        };
        state.media.push(MediaItem::Cursor {
            did: Did(0x4100_0004),
            hotspot_x: 0,
            hotspot_y: 0,
        });
        state.properties.entries.push(PropertyEntry {
            pid: 0x25,
            value: PropertyValue::DidRef(Did(0x4200_0001)),
        });
        child.states.push(state);
        let mut root = ElementDesc {
            element_id: 0x1000_0001,
            element_type: 14,
            ..ElementDesc::default()
        };
        root.block.rect = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        root.block.media.push(image(0x4100_0001));
        root.block.media.push(image(0));
        root.children.push(child);
        // A based child whose base is missing does not resolve.
        root.children.push(ElementDesc {
            element_id: 0x1000_0003,
            base_element: 0x1000_0099,
            base_layout: Did(0x2200_0099),
            ..ElementDesc::default()
        });
        let layout = Arc::new(UiLayout {
            did: Did(0x2200_0001),
            design_width: 1024,
            design_height: 768,
            elements_size: tessera_formats::TSize::default(),
            elements: vec![root],
        });
        let mut lookup = |d: Did| (d == layout.did).then(|| Arc::clone(&layout));
        let c = panel_census(
            PanelRef {
                layout: Did(0x2200_0001),
                element_id: 0x1000_0001,
            },
            16,
            &mut lookup,
        )
        .unwrap();
        assert_eq!(c.elements, 2);
        assert_eq!(c.text_elements, 1);
        assert_eq!(c.states, 1);
        assert_eq!(c.unresolved, 1);
        assert_eq!(c.art_surfaces.len(), 4);
        assert_eq!(c.fonts.len(), 1);
        assert!(!c.truncated);
        let missing = panel_census(
            PanelRef {
                layout: Did(0x2200_0001),
                element_id: 0x1000_0005,
            },
            16,
            &mut lookup,
        );
        assert!(matches!(missing, Err(PanelsError::NoRoot(_))));
        let no_layout = panel_census(
            PanelRef {
                layout: Did(0x2200_0002),
                element_id: 0x1000_0001,
            },
            16,
            &mut lookup,
        );
        assert!(matches!(no_layout, Err(PanelsError::NoLayout(_))));
    }
}
