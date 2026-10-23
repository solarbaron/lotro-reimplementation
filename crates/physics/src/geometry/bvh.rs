// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own bounding-volume hierarchy (`collision.md` §3.8, §12.1).
//!
//! A binary tree of axis-aligned boxes over item boxes, built top-down with a binned
//! surface-area heuristic (16 bins per axis) and stored as a flat array in depth-first
//! order: an internal node's left child follows it, its right child is at an index it
//! stores. Leaves hold up to [`MAX_LEAF_ITEMS`] items. The build is iterative within a
//! thread; the parallel fork/join of large inputs nests at most [`MAX_BUILD_THREADS`] levels,
//! so hostile inputs cannot overflow the stack. It is deterministic (same items, same tree,
//! whatever the threads) and bounded: node count ≤ 2 · items, depth ≤ [`MAX_DEPTH`] (deeper
//! ranges become leaves).
//!
//! **Threads.** A large build ([`PARALLEL_MIN_ITEMS`] items or more) can use helper threads
//! of its own, named `tessera-bvh` and `tessera-bvh-scan`, which live only during the build.
//! [`Bvh::build`] takes them from one process-wide budget shared by every concurrent build:
//! at most [`build_thread_budget`] threads work on parallel builds at once, the calling
//! threads included, so builds started together on many workers (a streamed ring of
//! landblocks) get no helpers and run on their own threads, and a build running alone gets
//! up to [`MAX_BUILD_THREADS`]. [`Bvh::build_with_threads`] lets the caller choose the count
//! instead (1: the calling thread only). A helper that cannot be had or started leaves its
//! work to the calling thread. Helpers do not observe the caller's priority or cancellation
//! (neither does the build on the calling thread).
//!
//! The quantised trees stored in the data (§3) are not used for collision: they omit
//! sliver triangles (§3.6) and exist per host, while a landblock set mixes many hosts.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::primitives::Aabb;

/// Items per leaf at most (except at [`MAX_DEPTH`]).
pub const MAX_LEAF_ITEMS: usize = 4;

/// Deepest node; a range still too large there becomes one leaf.
pub const MAX_DEPTH: usize = 48;

const BINS: usize = 16;

/// Ranges up to this size are split at the median instead of by the binned SAH.
const SMALL_NODE: usize = 32;

/// Inputs with fewer items are always built on the calling thread ([`Bvh::build`]).
pub const PARALLEL_MIN_ITEMS: usize = 16_384;

/// Most threads one build uses ([`Bvh::build`], [`Bvh::build_with_threads`]).
pub const MAX_BUILD_THREADS: usize = 8;

/// Cores the shared budget leaves to other threads ([`build_thread_budget`]): the asset
/// pool sizes itself the same way, keeping the main and audio threads' cores free.
pub const RESERVED_CORES: usize = 2;

/// Threads at work on parallel builds now, calling threads and helpers ([`BuildThread`]).
static BUILD_THREADS_BUSY: AtomicUsize = AtomicUsize::new(0);

/// The process-wide budget of [`Bvh::build`]: threads that may work on parallel builds at
/// once, calling threads included. The machine's available parallelism less
/// [`RESERVED_CORES`], at least 1; read once.
#[must_use]
pub fn build_thread_budget() -> usize {
    static BUDGET: OnceLock<usize> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .saturating_sub(RESERVED_CORES)
            .max(1)
    })
}

/// Where a build's helper threads come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Helpers {
    /// The shared budget ([`Bvh::build`]): a helper only while fewer than
    /// [`build_thread_budget`] threads are busy.
    Shared,
    /// The caller's own count ([`Bvh::build_with_threads`]); still counted as busy, so that
    /// concurrent shared builds hold back.
    Explicit,
}

/// One thread's place among [`BUILD_THREADS_BUSY`], given back on drop.
struct BuildThread(());

impl BuildThread {
    /// The calling thread of a parallel build (always counted).
    fn caller() -> Self {
        BUILD_THREADS_BUSY.fetch_add(1, Ordering::AcqRel);
        Self(())
    }

    /// A helper's place, or `None` when the shared budget is spent.
    fn helper(helpers: Helpers) -> Option<Self> {
        if helpers == Helpers::Explicit {
            return Some(Self::caller());
        }
        let budget = build_thread_budget();
        let mut busy = BUILD_THREADS_BUSY.load(Ordering::Acquire);
        while busy < budget {
            match BUILD_THREADS_BUSY.compare_exchange_weak(
                busy,
                busy + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self(())),
                Err(now) => busy = now,
            }
        }
        None
    }
}

impl Drop for BuildThread {
    fn drop(&mut self) {
        BUILD_THREADS_BUSY.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
thread_local! {
    /// Tests: bit `k % 32` set makes this thread's `k`-th helper start fail.
    pub(crate) static FAIL_SPAWNS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static SPAWNS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Tests: helper threads this thread started.
    static STARTED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Starts a helper thread in `scope` running `f`, with a place from `helpers`, or `None`
/// when no place is free or the thread does not start (`f` is dropped; the caller then does
/// the work itself, so it must be able to recover it).
fn spawn_helper<'scope, T, F>(
    scope: &'scope std::thread::Scope<'scope, '_>,
    name: &str,
    helpers: Helpers,
    f: F,
) -> Option<std::thread::ScopedJoinHandle<'scope, T>>
where
    T: Send + 'scope,
    F: FnOnce() -> T + Send + 'scope,
{
    #[cfg(test)]
    {
        let k = SPAWNS.with(|c| {
            let k = c.get();
            c.set(k.wrapping_add(1));
            k
        });
        if FAIL_SPAWNS.with(std::cell::Cell::get) & (1 << (k % 32)) != 0 {
            return None;
        }
    }
    let place = BuildThread::helper(helpers)?;
    let h = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn_scoped(scope, move || {
            let _place = place;
            f()
        })
        .inspect_err(|e| {
            tracing::debug!(error = %e, "collision hierarchy build: a helper thread did not start");
        })
        .ok();
    #[cfg(test)]
    if h.is_some() {
        STARTED.with(|c| c.set(c.get().wrapping_add(1)));
    }
    h
}

/// A range with fewer items is never split among threads (not worth the hand-over).
const MIN_TASK_ITEMS: usize = 4_096;

/// Ranges split on the calling thread with at least this many items have their scans
/// (bounds and bins) shared among the threads ([`step_with`]).
const PARALLEL_SCAN_ITEMS: usize = 65_536;

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
    ///
    /// Large inputs (at least [`PARALLEL_MIN_ITEMS`] items) can be built on up to
    /// [`MAX_BUILD_THREADS`] threads, the calling thread included, with helpers taken from
    /// the process-wide budget ([`build_thread_budget`], module docs): while that many
    /// threads already work on builds, the build stays on the calling thread. Smaller
    /// inputs are always built on the calling thread. The tree is the one
    /// [`Bvh::build_with_threads`] with one thread gives, node for node.
    #[must_use]
    pub fn build(boxes: &[Aabb]) -> Self {
        Self::build_using(boxes, MAX_BUILD_THREADS, Helpers::Shared)
    }

    /// [`Bvh::build`] on at most `threads` threads (the calling thread included; 0 and 1
    /// build on the calling thread alone). The result does not depend on `threads`: every
    /// range is split by the same rule as in a single-threaded build (the scans of large
    /// ranges are shared among threads and merged in range order, which gives the same bits),
    /// sibling subtrees are built on different threads, and their nodes are joined in
    /// depth-first order, so nodes, item order and therefore every query's visiting order
    /// are identical (`collision.md` §12.1). A subtree whose thread cannot be started is
    /// built on the calling thread.
    ///
    /// The count is the caller's choice (at most [`MAX_BUILD_THREADS`]) and is not limited
    /// by the shared budget of [`Bvh::build`]; its threads are counted in that budget while
    /// they work, so that concurrent [`Bvh::build`] calls hold back. A caller on a worker
    /// pool can size it from the pool's idle workers.
    #[must_use]
    pub fn build_with_threads(boxes: &[Aabb], threads: usize) -> Self {
        Self::build_using(boxes, threads, Helpers::Explicit)
    }

    fn build_using(boxes: &[Aabb], threads: usize, helpers: Helpers) -> Self {
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
        let threads = threads.clamp(1, MAX_BUILD_THREADS);
        // The budget is read only for large inputs (it is cached, but small sets and the
        // cell lookup never need it).
        let alone = threads == 1
            || prims.len() < PARALLEL_MIN_ITEMS
            || (helpers == Helpers::Shared && build_thread_budget() <= 1);
        let nodes = if alone {
            let mut nodes: Vec<Node> = Vec::with_capacity(2 * prims.len() / MAX_LEAF_ITEMS + 1);
            build_range(&mut prims, 0, 0, &mut nodes);
            nodes
        } else {
            let _caller = BuildThread::caller();
            build_parallel(&mut prims, threads, helpers)
        };
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

    /// Every node's fields as bit patterns, in node order (tests: two trees equal bit for
    /// bit, signed zeros included).
    #[cfg(test)]
    pub(crate) fn node_bits(&self) -> Vec<u32> {
        self.nodes
            .iter()
            .flat_map(|n| {
                n.bounds
                    .min
                    .iter()
                    .chain(n.bounds.max.iter())
                    .map(|c| c.to_bits())
                    .chain([n.first_or_right, n.count])
                    .collect::<Vec<_>>()
            })
            .collect()
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

/// What one build step does with a range: a leaf, or a split at `mid` (items `..mid` of
/// the range go left). The range is partitioned in place for a split.
enum Step {
    Leaf(Aabb),
    Split(Aabb, usize),
}

/// The build rule for one node over `range` at `depth` (module docs). Depends only on the
/// range's contents and the depth, which is what makes the parallel build equal to the
/// single-threaded one.
fn step(range: &mut [Prim], depth: usize) -> Step {
    step_with(range, depth, 1, Helpers::Explicit)
}

/// [`step`] with the two scans of a large range (its bounds and its bins) shared among up to
/// `threads` threads. The scans are folds of exact minima, maxima and counts, merged in
/// range order, so they give the bit patterns the single-threaded scans give (on equal
/// values the earlier item is kept either way); the partition stays on the calling thread.
fn step_with(range: &mut [Prim], depth: usize, threads: usize, helpers: Helpers) -> Step {
    let (bounds, cbounds) = if threads > 1 && range.len() >= PARALLEL_SCAN_ITEMS {
        par_fold(range, threads, helpers, scan, |a, b| {
            (fast_union(a.0, b.0), fast_union(a.1, b.1))
        })
    } else {
        scan(range)
    };
    let n = range.len();
    if n <= MAX_LEAF_ITEMS || depth >= MAX_DEPTH {
        return Step::Leaf(bounds);
    }
    let mid = if n <= SMALL_NODE {
        // Small ranges: a median split on the widest centre axis is as good and avoids the
        // binning's fixed cost.
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
    } else if let Some((axis, pivot_bin)) = choose_split(range, &cbounds, threads, helpers) {
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
    Step::Split(bounds, mid)
}

fn index_u32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Builds the subtree over `prims` (items `offset..offset + prims.len()` of the whole
/// build) whose root is at `depth`, appending its nodes to `nodes` in depth-first order.
/// Right-child indices are indices into `nodes`; leaf starts are indices into the whole
/// build's item order.
fn build_range(prims: &mut [Prim], offset: usize, depth: usize, nodes: &mut Vec<Node>) {
    // Tasks: (start, end, depth, right child of this node). The stack is LIFO and a split
    // pushes its right task before its left one, so a left subtree is finished before its
    // sibling starts: nodes come out in depth-first order and a left child always sits
    // right after its parent.
    let mut stack: Vec<(usize, usize, usize, Option<usize>)> = vec![(0, prims.len(), depth, None)];
    while let Some((start, end, depth, right_of)) = stack.pop() {
        let node = nodes.len();
        if let Some(parent) = right_of
            && let Some(p) = nodes.get_mut(parent)
        {
            p.first_or_right = index_u32(node);
        }
        let Some(range) = prims.get_mut(start..end) else {
            continue;
        };
        match step(range, depth) {
            Step::Leaf(bounds) => nodes.push(Node {
                bounds,
                first_or_right: index_u32(offset + start),
                count: index_u32(end - start),
            }),
            Step::Split(bounds, mid) => {
                nodes.push(Node {
                    bounds,
                    first_or_right: 0,
                    count: 0,
                });
                stack.push((start + mid, end, depth + 1, Some(node)));
                stack.push((start, start + mid, depth + 1, None));
            }
        }
    }
}

/// The parallel build ([`Bvh::build_with_threads`]): fork and join. A range with a thread
/// budget above 1 is split on the calling thread by the rule of [`step`] (its scans shared
/// among the budget, [`step_with`]); its right half is built on a new thread with a share of
/// the budget in proportion to its items, its left half on the calling thread with the rest,
/// and the halves' nodes are joined behind the parent in depth-first order. A range with a
/// budget of 1, or with fewer than [`MIN_TASK_ITEMS`] items, is built by [`build_range`].
/// The tree is therefore the single-threaded one, node for node. Every fork lowers both
/// halves' budgets by at least 1, so forks nest at most [`MAX_BUILD_THREADS`] deep and the
/// recursion is shallow.
fn build_parallel(prims: &mut [Prim], threads: usize, helpers: Helpers) -> Vec<Node> {
    build_task(prims, 0, 0, threads, helpers)
}

/// Builds the subtree over `prims` (items `offset..` of the whole build) whose root is at
/// `depth` with a budget of `threads` threads (helpers from `helpers`); right-child indices
/// are local to the result.
fn build_task(
    prims: &mut [Prim],
    offset: usize,
    depth: usize,
    threads: usize,
    helpers: Helpers,
) -> Vec<Node> {
    let n = prims.len();
    if threads <= 1 || n < MIN_TASK_ITEMS {
        let mut nodes = Vec::with_capacity(2 * n / MAX_LEAF_ITEMS + 1);
        build_range(prims, offset, depth, &mut nodes);
        return nodes;
    }
    let (bounds, mid) = match step_with(prims, depth, threads, helpers) {
        Step::Leaf(bounds) => {
            return vec![Node {
                bounds,
                first_or_right: index_u32(offset),
                count: index_u32(n),
            }];
        }
        Step::Split(bounds, mid) => (bounds, mid),
    };
    let (left, right) = prims.split_at_mut(mid.min(n));
    let right_len = right.len();
    // The right half's share of the budget, in proportion to its items, at least 1 each.
    let right_threads = (threads * right_len)
        .checked_div(n)
        .unwrap_or(1)
        .clamp(1, threads - 1);
    let left_threads = threads - right_threads;
    // The right half is handed over through a slot, so that the calling thread can still
    // build it when no helper can be had or started.
    let slot = std::sync::Mutex::new(Some(right));
    let take = || {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    };
    let right_offset = offset + mid;
    let (left_nodes, right_nodes) = std::thread::scope(|s| {
        let spawned = spawn_helper(s, "tessera-bvh", helpers, || {
            take().map(|r| build_task(r, right_offset, depth + 1, right_threads, helpers))
        });
        let left_nodes = build_task(left, offset, depth + 1, left_threads, helpers);
        let joined = match spawned {
            Some(h) => match h.join() {
                Ok(nodes) => nodes,
                Err(panic) => std::panic::resume_unwind(panic),
            },
            None => None,
        };
        // Not started (or the slot still full): build it here, after the left half, with
        // the right half's budget (its own forks may find helpers free by now).
        let right_nodes = joined.or_else(|| {
            take().map(|r| build_task(r, right_offset, depth + 1, right_threads, helpers))
        });
        (left_nodes, right_nodes.unwrap_or_default())
    });
    let mut nodes = Vec::with_capacity(1 + left_nodes.len() + right_nodes.len());
    nodes.push(Node {
        bounds,
        first_or_right: index_u32(1 + left_nodes.len()),
        count: 0,
    });
    append_moved(&mut nodes, left_nodes);
    append_moved(&mut nodes, right_nodes);
    nodes
}

/// Appends a subtree's nodes, moving its local right-child indices by where it starts.
fn append_moved(nodes: &mut Vec<Node>, sub: Vec<Node>) {
    let base = nodes.len();
    nodes.extend(sub.into_iter().map(|mut n| {
        if n.count == 0 {
            n.first_or_right = index_u32(n.first_or_right as usize + base);
        }
        n
    }));
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

/// The bounds of a range's boxes and of their centres, folded in range order.
fn scan(range: &[Prim]) -> (Aabb, Aabb) {
    let mut bounds = Aabb::EMPTY;
    let mut cbounds = Aabb::EMPTY;
    for p in range {
        bounds = fast_union(bounds, p.bounds);
        cbounds = fast_union(
            cbounds,
            Aabb {
                min: p.centre,
                max: p.centre,
            },
        );
    }
    (bounds, cbounds)
}

/// Bins of a range: per bin of the centres on `axis`, the union of the boxes (folded in
/// range order) and the count.
type Bins = [(Aabb, usize); BINS];

fn bin_range(range: &[Prim], axis: usize, lo: f32, scale: f32) -> Bins {
    let mut bins = [(Aabb::EMPTY, 0usize); BINS];
    for p in range {
        let bin = &mut bins[bin_of(p.centre[axis], lo, scale)];
        bin.0 = fast_union(bin.0, p.bounds);
        bin.1 += 1;
    }
    bins
}

/// Folds `range` in up to `threads` contiguous chunks, one per thread (the calling thread
/// takes the first), and merges the chunks' results in range order. A chunk whose helper
/// cannot be had (`helpers`), cannot be started or fails is folded on the calling thread, so the result never depends
/// on the threads.
fn par_fold<T: Send>(
    range: &[Prim],
    threads: usize,
    helpers: Helpers,
    fold: impl Fn(&[Prim]) -> T + Sync,
    merge: impl Fn(T, T) -> T,
) -> T {
    let chunk = range.len().div_ceil(threads.max(1)).max(1);
    let mut parts = range.chunks(chunk);
    let first = parts.next().unwrap_or(&[]);
    let fold = &fold;
    std::thread::scope(|s| {
        let handles: Vec<_> = parts
            .map(|part| {
                let h = spawn_helper(s, "tessera-bvh-scan", helpers, move || fold(part));
                (part, h)
            })
            .collect();
        let mut acc = fold(first);
        for (part, h) in handles {
            let r = match h {
                Some(h) => h.join().unwrap_or_else(|_| fold(part)),
                None => fold(part),
            };
            acc = merge(acc, r);
        }
        acc
    })
}

/// The best (axis, pivot bin) by the binned SAH over the widest centre axis, or `None`
/// when no split separates the items (all centres equal). A large range is binned on up to
/// `threads` threads ([`step_with`]).
fn choose_split(
    range: &[Prim],
    cbounds: &Aabb,
    threads: usize,
    helpers: Helpers,
) -> Option<(usize, usize)> {
    let extent: [f32; 3] = std::array::from_fn(|a| cbounds.max[a] - cbounds.min[a]);
    let axis = (0..3)
        .max_by(|&a, &b| extent[a].total_cmp(&extent[b]).then(b.cmp(&a)))
        .unwrap_or(0);
    let (lo, ext) = (cbounds.min[axis], extent[axis]);
    if ext <= 0.0 || !ext.is_finite() {
        return None;
    }
    let scale = bin_scale(ext);
    let bins = if threads > 1 && range.len() >= PARALLEL_SCAN_ITEMS {
        par_fold(
            range,
            threads,
            helpers,
            |r| bin_range(r, axis, lo, scale),
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    x.0 = fast_union(x.0, y.0);
                    x.1 += y.1;
                }
                a
            },
        )
    } else {
        bin_range(range, axis, lo, scale)
    };
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

#[cfg(test)]
mod thread_tests {
    use super::*;

    /// Pseudo-random boxes over a landblock (xorshift; synthetic), with runs of equal
    /// centres and invalid boxes mixed in.
    fn boxes(count: usize) -> Vec<Aabb> {
        let mut x = 0x9e37_79b9_u32;
        let mut rnd = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            #[allow(clippy::cast_precision_loss)]
            let v = x as f32 / u32::MAX as f32;
            v
        };
        (0..count)
            .map(|k| {
                if k % 997 == 0 {
                    return Aabb {
                        min: [2.0, 2.0, 2.0],
                        max: [1.0, 1.0, 1.0],
                    };
                }
                if k % 50 < 3 {
                    return Aabb {
                        min: [40.0, 40.0, 4.0],
                        max: [41.0, 41.0, 5.0],
                    };
                }
                let p = [rnd() * 160.0, rnd() * 160.0, rnd() * 50.0];
                Aabb {
                    min: p,
                    max: [p[0] + rnd(), p[1] + rnd(), p[2] + rnd()],
                }
            })
            .collect()
    }

    fn with_failing_spawns<R>(mask: u32, f: impl FnOnce() -> R) -> R {
        FAIL_SPAWNS.with(|c| c.set(mask));
        SPAWNS.with(|c| c.set(0));
        let r = f();
        FAIL_SPAWNS.with(|c| c.set(0));
        r
    }

    fn started() -> u32 {
        STARTED.with(std::cell::Cell::get)
    }

    /// Helpers that cannot be started (all, or some of them: the fork's slot hand-off and
    /// the scans' refold both run) leave their work to the calling thread, and the tree is
    /// still the single-threaded one bit for bit (`collision.md` §12.1).
    #[test]
    fn failed_helper_starts_give_the_single_threaded_tree() {
        // Large enough for shared scans at the top (PARALLEL_SCAN_ITEMS).
        let items = boxes(5 * PARALLEL_MIN_ITEMS);
        let one = Bvh::build_with_threads(&items, 1);
        let bits = one.node_bits();
        for mask in [u32::MAX, 0x5555_5555, 0xaaaa_aaaa, 0x0000_0001, 0x8000_0000] {
            for threads in [2, 3, MAX_BUILD_THREADS] {
                let before = started();
                let many = with_failing_spawns(mask, || Bvh::build_with_threads(&items, threads));
                if mask == u32::MAX {
                    assert_eq!(
                        started(),
                        before,
                        "no helper starts with every start failing"
                    );
                }
                assert!(
                    many == one,
                    "mask {mask:#x}, {threads} threads: the tree differs"
                );
                assert!(
                    many.node_bits() == bits,
                    "mask {mask:#x}, {threads} threads: node bits differ"
                );
            }
        }
        // Without failures the explicit build does use helpers.
        let before = started();
        let many = Bvh::build_with_threads(&items, 4);
        assert!(started() > before, "the explicit build started helpers");
        assert!(many.node_bits() == bits);
    }

    /// With the shared budget spent (every place held), [`Bvh::build`] starts no helper and
    /// builds on the calling thread; the tree is the same. Places are given back on drop.
    #[test]
    fn a_spent_shared_budget_builds_on_the_calling_thread() {
        let items = boxes(2 * PARALLEL_MIN_ITEMS);
        let one = Bvh::build_with_threads(&items, 1);
        let held: Vec<BuildThread> = (0..build_thread_budget())
            .map(|_| BuildThread::caller())
            .collect();
        assert!(BuildThread::helper(Helpers::Shared).is_none());
        assert!(BuildThread::helper(Helpers::Explicit).is_some());
        let before = started();
        let shared = Bvh::build(&items);
        assert_eq!(started(), before, "no helper while the budget is spent");
        assert!(shared.node_bits() == one.node_bits());
        drop(held);
    }

    /// The budget is the available parallelism less the reserved cores, at least 1.
    #[test]
    fn the_shared_budget_reserves_cores() {
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        assert_eq!(
            build_thread_budget(),
            cores.saturating_sub(RESERVED_CORES).max(1)
        );
    }
}
