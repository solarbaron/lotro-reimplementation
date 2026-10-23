// SPDX-License-Identifier: GPL-3.0-or-later
//! The composition list (decision 0015 Decision 1.6, 1.7; `render.md` §6.5–§6.8).
//!
//! The element tree describes its surfaces as a list of [`CompNode`]s in the screen root's
//! drawing order (pre-order: every node after its parent, every subtree contiguous, siblings
//! in drawing order), with the geometry the transition rule chooses (the last swapped
//! generation's, [`super::Scheduler::shown`]). [`build`] turns it into the ordered entries
//! the client hands to `tessera-render`'s UI pass:
//!
//! - one entry per displayed surface with a front texture, and one per external content;
//! - after each nested entry's whole subtree, its parent surface's **canvas above** for it,
//!   with exactly the parent's destination, transform, clip, opacity and filter;
//! - the **transform chain**: the rotation held by the entry's own surface, then each
//!   surfaced ancestor's, up to the root (nearest first);
//! - the **clip chain** (M5-1, `render.md` §6.8): the displayed boxes of the enclosing
//!   windows, mode-0 child windows drawn into an ancestor's canvas included, each tested in
//!   the space of the surface that draws it; consecutive items in the same space are
//!   intersected;
//! - **opacity**: the element's own only (Tessera rule, Decision 1.7), or own × ancestors
//!   by the [`OpacityRule`] switch; a stand-in surface composes at 1;
//! - the caps of §7: no entry for a surface under a hidden ancestor or below a Tile surface,
//!   none deeper than the depth cap, at most the entry cap per frame.

use std::ops::Range;

use super::PaintError;
use super::caps::{CapCounters, PaintLimits};
use super::coverage::holds_rotation;
use super::geom::Rect;
use super::painter::Layer;
use super::scheduler::{Scheduler, SurfaceId};

/// Most nodes in one composition input (Tessera cap).
pub const MAX_NODES: usize = 1 << 16;

/// Most ancestors of a node, surfaced or not (Tessera cap: deeper nodes are skipped).
pub const MAX_TREE_DEPTH: usize = 256;

/// Largest depth cap the builder accepts: `render.md` §6.5 takes at most 8 clip items and 9
/// rotations per entry, which a depth of 8 surfaced ancestors keeps.
pub const MAX_DEPTH: u32 = 8;

/// How a canvas covers its displayed box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Addressing {
    /// Scaled onto the box (displayed box ÷ canvas size).
    #[default]
    Fit,
    /// Repeated from the box's top-left at the display-node scale (Tile surfaces).
    Tile {
        /// Display-node scale `(ex, ey)`.
        node_scale: [f32; 2],
    },
}

/// Scale filter (Tessera setting `ui_scale_filter`, §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Bilinear on premultiplied texels.
    #[default]
    Linear,
    /// Nearest texel.
    Nearest,
}

/// Texel convention of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    /// Canvases: straight alpha.
    #[default]
    Straight,
    /// External content: already premultiplied.
    Premultiplied,
}

/// The opacity switch of Decision 1.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpacityRule {
    /// A nested entry takes its own `ObjectOpacity` only (the decided Tessera rule).
    #[default]
    Own,
    /// Own × every surfaced ancestor's (auto-fade then applies twice, *o*²).
    OwnTimesAncestors,
}

/// What a node draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeContent {
    /// The element owns a surface (a stand-in surface included).
    #[default]
    Canvas,
    /// GPU-drawn external content (a portrait viewport), by the client's target id.
    External(u64),
    /// No surface: a mode-0 child window drawn into an ancestor's canvas (it still clips),
    /// or a structural node.
    None,
}

/// One node of the composition input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompNode {
    /// The surface (or external content) id; also the id of the canvas above that the
    /// parent surface keeps for it ([`Layer::Above`]).
    pub id: SurfaceId,
    /// Index of the parent node in the list (must be smaller than this node's), or `None`
    /// for a screen root.
    pub parent: Option<usize>,
    /// Whether the element is a root panel (a plugin `Window`, a native root panel).
    pub window: bool,
    /// What it draws.
    pub content: NodeContent,
    /// Its own visibility flag.
    pub shown: bool,
    /// Its displayed box in physical pixels (the root's unrotated space): the destination of
    /// its canvas.
    pub displayed: Rect,
    /// Rows at the top of `displayed` that lie above the element's own box (a plugin window
    /// whose canvas extends over layout parts placed above its top edge; 0 otherwise). The
    /// own box, `displayed` without them ([`CompNode::own_box`]), is what its rotation turns
    /// about and what it clips nested entries to (`render.md` §6.5, §6.8).
    pub overhang: i32,
    /// Full canvas size (the layout box, or the base image's size for plugin mode 2).
    pub canvas: [u32; 2],
    /// Canvas origin offset of a canvas cropped by §7.
    pub canvas_origin: [i32; 2],
    /// Fit or Tile.
    pub addressing: Addressing,
    /// `UICore_Element_ObjectOpacity`.
    pub opacity: f32,
    /// Whether the surface is a stand-in surface of a mode-0 root panel (opacity 1).
    pub stand_in: bool,
    /// Angles (x, y, z) held by its surface, in degrees.
    pub rotation: [f32; 3],
    /// Alpha threshold (alpha-test meters only).
    pub alpha_threshold: f32,
}

impl CompNode {
    /// A shown, unrotated, opaque Fit node with a canvas of its box's size.
    #[must_use]
    pub fn new(id: SurfaceId, parent: Option<usize>, displayed: Rect) -> Self {
        Self {
            id,
            parent,
            window: false,
            content: NodeContent::Canvas,
            shown: true,
            displayed,
            overhang: 0,
            canvas: [displayed.width(), displayed.height()],
            canvas_origin: [0, 0],
            addressing: Addressing::Fit,
            opacity: 1.0,
            stand_in: false,
            rotation: [0.0; 3],
            alpha_threshold: 0.0,
        }
    }

    /// The element's own box: [`Self::displayed`] without the [`Self::overhang`] rows (never
    /// below an empty box at the bottom edge).
    #[must_use]
    pub fn own_box(&self) -> Rect {
        own_box(&self.displayed, self.overhang)
    }
}

/// `displayed` without its top `overhang` rows (a negative overhang counts as 0).
pub(crate) fn own_box(displayed: &Rect, overhang: i32) -> Rect {
    let o = overhang.max(0);
    if o == 0 {
        return *displayed;
    }
    Rect::new(
        displayed.x0,
        displayed.y0.saturating_add(o).min(displayed.y1),
        displayed.x1,
        displayed.y1,
    )
}

/// One rotation of a transform chain (`render.md` §6.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    /// Centre of the displayed box of the surface that holds it.
    pub centre: [f32; 2],
    /// Angles in degrees, non-finite ones replaced by 0.
    pub angles: [f32; 3],
}

/// One clip item (`render.md` §6.8): a half-open rectangle `[x0, y0, x1, y1]` tested after
/// the first `space` rotations of the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    /// `[x0, y0, x1, y1]`.
    pub rect: [i32; 4],
    /// Space index.
    pub space: u32,
}

/// What an entry draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryTexture {
    /// The front texture of a surface's layer.
    Surface {
        /// The surface.
        surface: SurfaceId,
        /// Its canvas below or a canvas above.
        layer: Layer,
    },
    /// External content by target id.
    External(u64),
    /// An untextured fill of the entry's `dest` with one straight-alpha colour 0xAARRGGBB
    /// (no texture, rotation or clip; `canvas` is the box's size). The UI adds these after
    /// [`build`] for the letterbox bars of `ui-scaling.md` §5.4 step 6, directly below the
    /// picture of a fit group's lowest shown member (`crate::Ui::paint`).
    Fill {
        /// 0xAARRGGBB, straight alpha.
        colour: u32,
    },
}

/// One composition entry (`render.md` §6.5); the client resolves the texture and the two
/// ranges into `tessera-render`'s `UiEntry`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// What to draw.
    pub texture: EntryTexture,
    /// Straight or premultiplied texels.
    pub source: Source,
    /// Full canvas size.
    pub canvas: [u32; 2],
    /// Canvas position of the texture's texel (0, 0): the tile-box offset plus the canvas
    /// origin offset.
    pub tex_offset: [i32; 2],
    /// Displayed box.
    pub dest: Rect,
    /// Fit or Tile.
    pub addressing: Addressing,
    /// Filter.
    pub filter: Filter,
    /// Opacity, 0–1.
    pub opacity: f32,
    /// Alpha threshold, 0–1.
    pub alpha_threshold: f32,
    /// Range into [`CompositionList::rotations`], nearest first.
    pub rotations: Range<usize>,
    /// Range into [`CompositionList::clips`].
    pub clips: Range<usize>,
}

/// The composition list of one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompositionList {
    /// Entries in drawing order.
    pub entries: Vec<Entry>,
    /// Rotations the entries' ranges point into.
    pub rotations: Vec<Rotation>,
    /// Clip items the entries' ranges point into.
    pub clips: Vec<Clip>,
    /// Cap counters (`tile_parent`, `depth`, `entries`).
    pub counters: CapCounters,
}

impl CompositionList {
    /// The rotations of an entry.
    #[must_use]
    pub fn rotations_of(&self, e: &Entry) -> &[Rotation] {
        self.rotations.get(e.rotations.clone()).unwrap_or(&[])
    }

    /// The clip chain of an entry.
    #[must_use]
    pub fn clips_of(&self, e: &Entry) -> &[Clip] {
        self.clips.get(e.clips.clone()).unwrap_or(&[])
    }
}

/// Where the builder finds the textures.
pub trait TextureLookup {
    /// The front texture box of a layer, or `None` when it has no texture.
    fn front_box(&self, surface: SurfaceId, layer: Layer) -> Option<Rect>;
    /// Whether the surface may be drawn (its first paint released or swapped).
    fn is_drawn(&self, surface: SurfaceId) -> bool;
}

impl<G> TextureLookup for Scheduler<G> {
    fn front_box(&self, surface: SurfaceId, layer: Layer) -> Option<Rect> {
        Scheduler::front_box(self, surface, layer)
    }
    fn is_drawn(&self, surface: SurfaceId) -> bool {
        Scheduler::is_drawn(self, surface)
    }
}

/// Builder settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComposeConfig {
    /// Scale filter for every entry.
    pub filter: Filter,
    /// Opacity rule for nested entries.
    pub opacity_rule: OpacityRule,
    /// Entry cap (§7).
    pub max_entries: usize,
    /// Depth cap (§7): levels of nested surfaces below a root (at most [`MAX_DEPTH`]; a
    /// larger value is treated as [`MAX_DEPTH`]).
    pub max_depth: u32,
}

impl ComposeConfig {
    /// The default filter and opacity rule with the entry and depth caps of `limits`, so a
    /// Tessera setting changed in [`PaintLimits`] reaches [`build`].
    #[must_use]
    pub fn from_limits(limits: &PaintLimits) -> Self {
        Self {
            filter: Filter::Linear,
            opacity_rule: OpacityRule::Own,
            max_entries: limits.max_entries,
            max_depth: limits.max_depth,
        }
    }
}

impl Default for ComposeConfig {
    /// [`ComposeConfig::from_limits`] of the default [`PaintLimits`].
    fn default() -> Self {
        Self::from_limits(&PaintLimits::default())
    }
}

#[derive(Debug, Clone, Default)]
struct Info {
    displayed: bool,
    /// Nearest ancestor with a canvas.
    surf_parent: Option<usize>,
    /// Surfaced ancestors.
    surf_depth: u32,
    tree_depth: usize,
    under_tile: bool,
    /// Rotating surfaces at or above this node (for a canvas node), nearest first.
    rot_chain: Vec<usize>,
    /// Nearest window at or above this node.
    window_at_or_above: Option<usize>,
    /// Entry parameters of this node's own surface, reused by its canvases above.
    params: Option<(Range<usize>, Range<usize>, f32)>,
    /// Whether this node is a nested entry that was eligible (its canvas above follows it).
    nested_eligible: bool,
    eff_opacity: f32,
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

fn finite_angles(a: [f32; 3]) -> [f32; 3] {
    a.map(|v| if v.is_finite() { v } else { 0.0 })
}

fn centre(r: &Rect) -> [f32; 2] {
    [
        ((f64::from(r.x0) + f64::from(r.x1)) * 0.5) as f32,
        ((f64::from(r.y0) + f64::from(r.y1)) * 0.5) as f32,
    ]
}

/// Builds the composition list (see the module documentation).
///
/// # Errors
/// [`PaintError::InvalidTree`] when a parent index is not smaller than its child's, the
/// subtrees are not contiguous, or there are more than [`MAX_NODES`] nodes.
pub fn build(
    nodes: &[CompNode],
    lookup: &dyn TextureLookup,
    config: &ComposeConfig,
) -> Result<CompositionList, PaintError> {
    if nodes.len() > MAX_NODES {
        return Err(PaintError::InvalidTree("too many nodes"));
    }
    let config = &ComposeConfig {
        max_depth: config.max_depth.min(MAX_DEPTH),
        ..*config
    };
    let mut out = CompositionList::default();
    let mut info: Vec<Info> = Vec::with_capacity(nodes.len());
    let mut stack: Vec<usize> = Vec::new();
    for (j, n) in nodes.iter().enumerate() {
        match n.parent {
            Some(p) if p >= j => return Err(PaintError::InvalidTree("parent after child")),
            Some(p) => {
                while stack.last().is_some_and(|&t| t != p) {
                    if let Some(t) = stack.pop() {
                        close(t, nodes, &info, lookup, config, &mut out);
                    }
                }
                if stack.is_empty() {
                    return Err(PaintError::InvalidTree("subtree not contiguous"));
                }
            }
            None => {
                while let Some(t) = stack.pop() {
                    close(t, nodes, &info, lookup, config, &mut out);
                }
            }
        }
        let i = open(j, nodes, &info, lookup, config, &mut out);
        info.push(i);
        stack.push(j);
    }
    while let Some(t) = stack.pop() {
        close(t, nodes, &info, lookup, config, &mut out);
    }
    Ok(out)
}

fn push_entry(out: &mut CompositionList, config: &ComposeConfig, e: Entry) {
    if out.entries.len() >= config.max_entries {
        out.counters.entries += 1;
    } else {
        out.entries.push(e);
    }
}

fn open(
    j: usize,
    nodes: &[CompNode],
    info: &[Info],
    lookup: &dyn TextureLookup,
    config: &ComposeConfig,
    out: &mut CompositionList,
) -> Info {
    let n = &nodes[j];
    let parent = n.parent.map(|p| (&nodes[p], &info[p]));
    let mut i = Info {
        displayed: n.shown && parent.is_none_or(|(_, pi)| pi.displayed),
        ..Info::default()
    };
    if let Some((pn, pi)) = parent {
        let parent_is_surface = pn.content == NodeContent::Canvas;
        i.surf_parent = if parent_is_surface {
            n.parent
        } else {
            pi.surf_parent
        };
        i.surf_depth = pi.surf_depth + u32::from(parent_is_surface);
        i.tree_depth = pi.tree_depth + 1;
        i.under_tile = pi.under_tile
            || (parent_is_surface && matches!(pn.addressing, Addressing::Tile { .. }));
        i.window_at_or_above = pi.window_at_or_above;
    }
    if n.window {
        i.window_at_or_above = Some(j);
    }
    // Past the depth caps nothing is emitted here or below (every descendant is at least as
    // deep), so no rotation chain is kept: chains stay within MAX_DEPTH + 1 items and a long
    // linear tree costs linear work.
    let beyond = i.surf_depth > config.max_depth || i.tree_depth > MAX_TREE_DEPTH;
    let mut chain = Vec::new();
    if !beyond {
        let parent_chain: &[usize] = i
            .surf_parent
            .map_or(&[], |sp| info[sp].rot_chain.as_slice());
        let own_rot = n.content == NodeContent::Canvas && holds_rotation(n.rotation);
        chain.reserve(parent_chain.len() + 1);
        if own_rot {
            chain.push(j);
        }
        chain.extend_from_slice(parent_chain);
    }
    let own_opacity = if n.stand_in { 1.0 } else { clamp01(n.opacity) };
    i.eff_opacity = match (config.opacity_rule, i.surf_parent) {
        (OpacityRule::OwnTimesAncestors, Some(sp)) => own_opacity * info[sp].eff_opacity,
        _ => own_opacity,
    };
    if n.content == NodeContent::Canvas {
        i.rot_chain.clone_from(&chain);
    }
    if n.content == NodeContent::None || !i.displayed {
        return i;
    }
    if i.under_tile {
        out.counters.tile_parent += 1;
        return i;
    }
    if beyond {
        out.counters.depth += 1;
        return i;
    }
    let is_nested = i.surf_parent.is_some();
    // Rotations of this entry.
    let r0 = out.rotations.len();
    for &s in &chain {
        out.rotations.push(Rotation {
            centre: centre(&nodes[s].own_box()),
            angles: finite_angles(nodes[s].rotation),
        });
    }
    let rotations = r0..out.rotations.len();
    // Clip chain from the enclosing windows (strictly above this node).
    let c0 = out.clips.len();
    let mut dropped = false;
    if is_nested {
        let mut w = n.parent.and_then(|p| info[p].window_at_or_above);
        let mut last: Option<Clip> = None;
        let mut items: Vec<Clip> = Vec::new();
        while let Some(wi) = w {
            let wn = &nodes[wi];
            // The surface that draws the window: its own, or the nearest above it.
            let drawer = if wn.content == NodeContent::Canvas {
                Some(wi)
            } else {
                info[wi].surf_parent
            };
            let drawer_rots = drawer.map_or(0, |d| info[d].rot_chain.len());
            let space = u32::try_from(chain.len().saturating_sub(drawer_rots)).unwrap_or(0);
            let r = wn.own_box();
            let item = Clip {
                rect: [r.x0, r.y0, r.x1, r.y1],
                space,
            };
            last = Some(match last {
                Some(l) if l.space == item.space => Clip {
                    rect: [
                        l.rect[0].max(item.rect[0]),
                        l.rect[1].max(item.rect[1]),
                        l.rect[2].min(item.rect[2]),
                        l.rect[3].min(item.rect[3]),
                    ],
                    space: l.space,
                },
                Some(l) => {
                    items.push(l);
                    item
                }
                None => item,
            });
            w = wn.parent.and_then(|p| info[p].window_at_or_above);
        }
        items.extend(last);
        for c in &items {
            if c.rect[2] <= c.rect[0] || c.rect[3] <= c.rect[1] {
                dropped = true;
            }
        }
        out.clips.extend(items);
    }
    let clips = c0..out.clips.len();
    // A nested entry's canvas above follows it even when the entry itself is cut away:
    // the canvas above holds parent content, clipped by the parent's chain.
    i.nested_eligible = is_nested;
    if dropped {
        return i;
    }
    i.params = Some((rotations.clone(), clips.clone(), i.eff_opacity));
    match n.content {
        NodeContent::Canvas => {
            if !lookup.is_drawn(n.id) {
                return i;
            }
            if let Some(b) = lookup.front_box(n.id, Layer::Below) {
                push_entry(
                    out,
                    config,
                    surface_entry(n, Layer::Below, b, config, i.eff_opacity, rotations, clips),
                );
            }
        }
        NodeContent::External(t) => push_entry(
            out,
            config,
            Entry {
                texture: EntryTexture::External(t),
                source: Source::Premultiplied,
                canvas: [n.canvas[0].max(1), n.canvas[1].max(1)],
                tex_offset: [0, 0],
                dest: n.displayed,
                addressing: Addressing::Fit,
                filter: config.filter,
                opacity: i.eff_opacity,
                alpha_threshold: clamp01(n.alpha_threshold),
                rotations,
                clips,
            },
        ),
        NodeContent::None => {}
    }
    i
}

fn surface_entry(
    n: &CompNode,
    layer: Layer,
    tile_box: Rect,
    config: &ComposeConfig,
    opacity: f32,
    rotations: Range<usize>,
    clips: Range<usize>,
) -> Entry {
    Entry {
        texture: EntryTexture::Surface {
            surface: n.id,
            layer,
        },
        source: Source::Straight,
        canvas: n.canvas,
        tex_offset: [
            tile_box.x0.saturating_add(n.canvas_origin[0]),
            tile_box.y0.saturating_add(n.canvas_origin[1]),
        ],
        dest: n.displayed,
        addressing: n.addressing,
        filter: config.filter,
        opacity,
        alpha_threshold: clamp01(n.alpha_threshold),
        rotations,
        clips,
    }
}

/// A node's subtree ended: a nested entry's parent canvas above follows it.
fn close(
    j: usize,
    nodes: &[CompNode],
    info: &[Info],
    lookup: &dyn TextureLookup,
    config: &ComposeConfig,
    out: &mut CompositionList,
) {
    let i = &info[j];
    if !i.nested_eligible {
        return;
    }
    let Some(p) = i.surf_parent else {
        return;
    };
    let pn = &nodes[p];
    let Some((rot, clips, opacity)) = info[p].params.clone() else {
        return;
    };
    if !lookup.is_drawn(pn.id) {
        return;
    }
    let layer = Layer::Above(nodes[j].id.0);
    if let Some(b) = lookup.front_box(pn.id, layer) {
        push_entry(
            out,
            config,
            surface_entry(pn, layer, b, config, opacity, rot, clips),
        );
    }
}
