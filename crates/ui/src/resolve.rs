// SPDX-License-Identifier: GPL-3.0-or-later
//! Base-element inheritance (`ui-layout.md` §7): following a base chain and merging
//! descriptors.
//!
//! [`resolve`] builds the resolved descriptor of a stored element descriptor: it follows the
//! base chain with `tessera_formats::ui_layout::base_chain` (§7.1, cycle and depth checks,
//! §12.1) and merges each derived descriptor into a copy of its resolved base (§7.2–§7.3).
//! Children are copied with their parent and never resolved during a merge (§7.3 step 6):
//! a merged child that still carries a base reference is resolved when the element tree is
//! created (§9.2 step 1).

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::ui_layout::{BaseChainError, ElementDesc, StateBlock, UiLayout, base_chain};

use crate::geom::IBox;
use crate::layout::design_time;

/// Merges a derived state block `d` into the base block `r` (`ui-layout.md` §7.2).
pub fn merge_block(r: &mut StateBlock, d: &StateBlock) {
    // Step 1: flags, rectangle, z-level and has_position come from the derived block.
    r.flags = d.flags;
    r.rect = d.rect;
    r.z_level = d.z_level;
    r.has_position = d.has_position;
    // Step 2: append the derived media after the base media.
    r.media.extend(d.media.iter().cloned());
    // Step 3: derived properties replace base properties with the same PID.
    for e in &d.properties.entries {
        if let Some(slot) = r.properties.entries.iter_mut().find(|x| x.pid == e.pid) {
            slot.value = e.value.clone();
        } else {
            r.properties.entries.push(e.clone());
        }
    }
    r.properties.size.count = r.properties.entries.len() as u32;
}

fn desc_box(d: &ElementDesc) -> IBox {
    let r = d.block.rect;
    IBox::from_xywh(r.x, r.y, r.width, r.height)
}

fn set_desc_box(d: &mut ElementDesc, b: IBox) {
    d.block.rect.x = b.l;
    d.block.rect.y = b.t;
    d.block.rect.width = b.w();
    d.block.rect.height = b.h();
}

/// Re-lays out a stored child with the design-time rule (`ui-layout.md` §8.2) when its
/// parent changes from `ow` × `oh` to `nw` × `nh`, then its own children recursively from
/// its old box to its new one.
pub fn relayout_design(child: &mut ElementDesc, ow: i32, oh: i32, nw: i32, nh: i32) {
    let old = desc_box(child);
    let new = design_time(old, child.edges.to_array().map(|e| e.0), ow, oh, nw, nh);
    set_desc_box(child, new);
    let (cow, coh, cnw, cnh) = (old.w(), old.h(), new.w(), new.h());
    for gc in &mut child.children {
        relayout_design(gc, cow, coh, cnw, cnh);
    }
}

/// Merges a derived element descriptor `d` into `r`, a copy of the resolved base
/// (`ui-layout.md` §7.3; recursively for overriding children, step 6).
pub fn merge_element(r: &mut ElementDesc, d: &ElementDesc) {
    let old = desc_box(r);
    // Step 1.
    merge_block(&mut r.block, &d.block);
    // Step 2: the element ID; type and base reference are never copied.
    if d.element_id != 0 {
        r.element_id = d.element_id;
    }
    // Step 3.
    r.default_state = d.default_state;
    r.edges = d.edges;
    r.read_order = d.read_order;
    // Step 4: states merged by key.
    for s in &d.states {
        if let Some(rs) = r.states.iter_mut().find(|x| x.state_id == s.state_id) {
            merge_block(rs, s);
        } else {
            r.states.push(s.clone());
        }
    }
    r.states_size.count = r.states.len() as u32;
    // Step 5: inherited children not overridden.
    let overridden: Vec<u32> = d
        .children
        .iter()
        .filter(|c| r.children.iter().any(|x| x.element_id == c.element_id))
        .map(|c| c.element_id)
        .collect();
    let c = r
        .children
        .iter()
        .filter(|x| !overridden.contains(&x.element_id))
        .count() as u32;
    let inherited_len = r.children.len();
    // Step 6 and 7.
    for dc in &d.children {
        if let Some(rc) = r
            .children
            .iter_mut()
            .take(inherited_len)
            .find(|x| x.element_id == dc.element_id)
        {
            merge_element(rc, dc);
        } else {
            let mut added = dc.clone();
            added.read_order = added.read_order.saturating_add(c);
            r.children.push(added);
        }
    }
    r.children_size.count = r.children.len() as u32;
    // Step 8: re-lay out the inherited children D does not override.
    let new = desc_box(r);
    if new.w() != old.w() || new.h() != old.h() {
        for rc in r.children.iter_mut().take(inherited_len) {
            if !overridden.contains(&rc.element_id) {
                relayout_design(rc, old.w(), old.h(), new.w(), new.h());
            }
        }
    }
}

/// Resolves a stored descriptor (`ui-layout.md` §7.1): a typed descriptor is returned as it
/// is; a based one is merged into its resolved base chain. `lookup` returns a decoded layout
/// by DID (`None` when missing or undecodable).
///
/// # Errors
/// The §13 resolution failures: missing base layout, missing top-level element, a repeated
/// (element, layout) pair or a chain over [`tessera_formats::ui_layout::MAX_BASE_CHAIN`].
pub fn resolve(
    desc: &ElementDesc,
    lookup: &mut dyn FnMut(Did) -> Option<Arc<UiLayout>>,
) -> Result<ElementDesc, BaseChainError> {
    if desc.base().is_none() {
        return Ok(desc.clone());
    }
    let chain = base_chain(desc, lookup)?;
    let Some(end) = chain.end() else {
        return Ok(desc.clone());
    };
    let mut r = end.clone();
    let n = chain.links.len();
    for i in (0..n.saturating_sub(1)).rev() {
        if let Some(e) = chain.links[i].element() {
            merge_element(&mut r, e);
        }
    }
    merge_element(&mut r, desc);
    Ok(r)
}
