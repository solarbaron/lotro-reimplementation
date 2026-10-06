// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own bounding-volume hierarchy (`collision.md` §3.8, §12.1).
//!
//! A binary tree of axis-aligned boxes over item boxes, built top-down with a binned
//! surface-area heuristic (16 bins per axis) and stored as a flat array in depth-first
//! order: an internal node's left child follows it, its right child is at an index it
//! stores. Leaves hold up to [`MAX_LEAF_ITEMS`] items. The build is iterative (no recursion,
//! so hostile inputs cannot overflow the stack), deterministic (same items, same tree) and
//! bounded: node count ≤ 2 · items, depth ≤ [`MAX_DEPTH`] (deeper ranges become leaves).
//!
//! The quantised trees stored in the data (§3) are not used for collision: they omit
//! sliver triangles (§3.6) and exist per host, while a landblock set mixes many hosts.

use super::primitives::Aabb;

/// Items per leaf at most (except at [`MAX_DEPTH`]).
pub const MAX_LEAF_ITEMS: usize = 4;

/// Deepest node; a range still too large there becomes one leaf.
pub const MAX_DEPTH: usize = 48;

const BINS: usize = 16;

/// Ranges up to this size are split at the median instead of by the binned SAH.
const SMALL_NODE: usize = 32;

/// One node.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Node {
    bounds: Aabb,
    /// Leaf: first index into `order`. Internal: index of the right child.
    first_or_right: u32,
    /// Leaf: item count (≥ 1). Internal: 0.
    count: u32,
}

/// A bounding-volume hierarchy over item boxes. `Send + Sync`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    /// Item numbers, grouped by leaf.
    order: Vec<u32>,
    /// The box of `order[k]`, for the per-item test in leaves.
    item_bounds: Vec<Aabb>,
}

impl Bvh {
    /// Builds a hierarchy over `boxes`; item *k* is `boxes[k]`. Invalid boxes (non-finite
    /// or inverted) are left out. At most `u32::MAX` items are indexed (larger inputs are
    /// truncated; the collision sets cap far below).
    #[must_use]
    pub fn build(boxes: &[Aabb]) -> Self {
        // Work on a contiguous copy (box, centre, item) that is partitioned in place, so
        // every pass reads memory in order.
        let mut prims: Vec<Prim> = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_valid())
            .filter_map(|(i, b)| {
                Some(Prim {
                    bounds: *b,
                    centre: b.centre(),
                    item: u32::try_from(i).ok()?,
                })
            })
            .collect();
        if prims.is_empty() {
            return Self::default();
        }
        let mut nodes: Vec<Node> = Vec::with_capacity(2 * prims.len() / MAX_LEAF_ITEMS + 1);
        // Tasks: (start, end, depth, right child of this node). The stack is LIFO and a
        // split pushes its right task before its left one, so a left subtree is finished
        // before its sibling starts: nodes come out in depth-first order and a left child
        // always sits right after its parent.
        let mut stack: Vec<(usize, usize, usize, Option<usize>)> = vec![(0, prims.len(), 0, None)];
        while let Some((start, end, depth, right_of)) = stack.pop() {
            let node = nodes.len();
            if let Some(parent) = right_of {
                nodes[parent].first_or_right = u32::try_from(node).unwrap_or(u32::MAX);
            }
            let range = &mut prims[start..end];
            let mut bounds = Aabb::EMPTY;
            let mut cbounds = Aabb::EMPTY;
            for p in range.iter() {
                bounds = fast_union(bounds, p.bounds);
                cbounds = fast_union(
                    cbounds,
                    Aabb {
                        min: p.centre,
                        max: p.centre,
                    },
                );
            }
            let n = end - start;
            if n <= MAX_LEAF_ITEMS || depth >= MAX_DEPTH {
                nodes.push(Node {
                    bounds,
                    first_or_right: u32::try_from(start).unwrap_or(u32::MAX),
                    count: u32::try_from(n).unwrap_or(u32::MAX),
                });
                continue;
            }
            let mid = if n <= SMALL_NODE {
                // Small ranges: a median split on the widest centre axis is as good and
                // avoids the binning's fixed cost.
                let extent: [f32; 3] = std::array::from_fn(|a| cbounds.max[a] - cbounds.min[a]);
                let axis = (0..3)
                    .max_by(|&a, &b| extent[a].total_cmp(&extent[b]).then(b.cmp(&a)))
                    .unwrap_or(0);
                let mid = n / 2;
                range.select_nth_unstable_by(mid, |a, b| {
                    a.centre[axis]
                        .total_cmp(&b.centre[axis])
                        .then(a.item.cmp(&b.item))
                });
                mid
            } else if let Some((axis, pivot_bin)) = choose_split(range, &cbounds) {
                // Partition: items whose centre bin is below the pivot go left.
                let lo = cbounds.min[axis];
                let scale = bin_scale(cbounds.max[axis] - cbounds.min[axis]);
                let mut mid = 0;
                for k in 0..range.len() {
                    if bin_of(range[k].centre[axis], lo, scale) < pivot_bin {
                        range.swap(k, mid);
                        mid += 1;
                    }
                }
                if mid == 0 || mid == n {
                    // Cannot happen for a split between non-empty bins; stay safe.
                    n / 2
                } else {
                    mid
                }
            } else {
                // Every centre is equal: split by count.
                n / 2
            };
            nodes.push(Node {
                bounds,
                first_or_right: 0,
                count: 0,
            });
            stack.push((start + mid, end, depth + 1, Some(node)));
            stack.push((start, start + mid, depth + 1, None));
        }
        let order = prims.iter().map(|p| p.item).collect();
        let item_bounds = prims.iter().map(|p| p.bounds).collect();
        Self {
            nodes,
            order,
            item_bounds,
        }
    }

    /// Number of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of indexed items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.order.len()
    }

    /// Bounds of everything, or `None` when empty.
    #[must_use]
    pub fn bounds(&self) -> Option<Aabb> {
        self.nodes.first().map(|n| n.bounds)
    }

    /// Calls `visit` with every item whose box overlaps `query` (touching counts, exactly
    /// the items a linear scan would find), in a fixed order (depth-first, left before right).
    pub fn query(&self, query: &Aabb, mut visit: impl FnMut(u32)) {
        self.query_while(query, |item| {
            visit(item);
            true
        });
    }

    /// [`Bvh::query`] that stops as soon as `visit` returns `false`. Returns `false` when it
    /// stopped early, `true` when it visited every overlapping item.
    pub fn query_while(&self, query: &Aabb, mut visit: impl FnMut(u32) -> bool) -> bool {
        if self.nodes.is_empty() || !query.is_valid() {
            return true;
        }
        let mut stack: Vec<u32> = Vec::with_capacity(64);
        stack.push(0);
        while let Some(i) = stack.pop() {
            let Some(n) = self.nodes.get(i as usize) else {
                continue;
            };
            if !n.bounds.overlaps(query) {
                continue;
            }
            if n.count > 0 {
                let s = n.first_or_right as usize;
                let e = s + n.count as usize;
                let items = self.order.get(s..e).unwrap_or(&[]);
                let bounds = self.item_bounds.get(s..e).unwrap_or(&[]);
                for (&item, b) in items.iter().zip(bounds) {
                    if b.overlaps(query) && !visit(item) {
                        return false;
                    }
                }
            } else {
                stack.push(n.first_or_right);
                stack.push(i + 1);
            }
        }
        true
    }

    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<Node>()
            + self.order.capacity() * 4
            + self.item_bounds.capacity() * std::mem::size_of::<Aabb>()
    }
}

/// The bin (0..BINS) of a centre coordinate.
fn bin_of(c: f32, lo: f32, scale: f32) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let b = ((c - lo) * scale) as usize;
    b.min(BINS - 1)
}

/// The factor that maps a centre offset to a bin: BINS / extent, or 0 for an empty or
/// invalid extent (everything in bin 0).
fn bin_scale(extent: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let s = BINS as f32 / extent;
    if extent > 0.0 && s.is_finite() {
        s
    } else {
        0.0
    }
}

/// One item during the build.
#[derive(Debug, Clone, Copy)]
struct Prim {
    bounds: Aabb,
    centre: [f32; 3],
    item: u32,
}

/// The best (axis, pivot bin) by the binned SAH over the widest centre axis, or `None`
/// when no split separates the items (all centres equal).
fn choose_split(range: &[Prim], cbounds: &Aabb) -> Option<(usize, usize)> {
    let extent: [f32; 3] = std::array::from_fn(|a| cbounds.max[a] - cbounds.min[a]);
    let axis = (0..3)
        .max_by(|&a, &b| extent[a].total_cmp(&extent[b]).then(b.cmp(&a)))
        .unwrap_or(0);
    let (lo, ext) = (cbounds.min[axis], extent[axis]);
    if ext <= 0.0 || !ext.is_finite() {
        return None;
    }
    let scale = bin_scale(ext);
    let mut bins = [(Aabb::EMPTY, 0usize); BINS];
    for p in range {
        let bin = &mut bins[bin_of(p.centre[axis], lo, scale)];
        bin.0 = fast_union(bin.0, p.bounds);
        bin.1 += 1;
    }
    // Prefix (left) and suffix (right) sweeps.
    let mut left = [(0.0f32, 0usize); BINS];
    let (mut acc, mut cnt) = (Aabb::EMPTY, 0);
    for k in 0..BINS {
        acc = fast_union(acc, bins[k].0);
        cnt += bins[k].1;
        left[k] = (acc.half_area(), cnt);
    }
    let mut best: Option<(f32, usize)> = None;
    let (mut acc, mut cnt) = (Aabb::EMPTY, 0);
    for k in (1..BINS).rev() {
        acc = fast_union(acc, bins[k].0);
        cnt += bins[k].1;
        let (la, lc) = left[k - 1];
        if lc == 0 || cnt == 0 {
            continue;
        }
        #[allow(clippy::cast_precision_loss)]
        let cost = la * lc as f32 + acc.half_area() * cnt as f32;
        if best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, k));
        }
    }
    best.map(|(_, k)| (axis, k))
}

/// Union of two boxes known to hold no NaN (plain comparisons, no NaN handling).
#[inline]
fn fast_union(a: Aabb, b: Aabb) -> Aabb {
    let mn = |x: f32, y: f32| if y < x { y } else { x };
    let mx = |x: f32, y: f32| if y > x { y } else { x };
    Aabb {
        min: [
            mn(a.min[0], b.min[0]),
            mn(a.min[1], b.min[1]),
            mn(a.min[2], b.min[2]),
        ],
        max: [
            mx(a.max[0], b.max[0]),
            mx(a.max[1], b.max[1]),
            mx(a.max[2], b.max[2]),
        ],
    }
}
