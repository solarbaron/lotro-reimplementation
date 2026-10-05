// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared vertex/index buffers for meshes.
//!
//! Meshes are not given buffers of their own. The arena owns a few large
//! **pages**, each one vertex buffer plus one index buffer, and sub-allocates
//! every mesh's vertices and indices from a single page with a first-fit
//! [`RangeAllocator`]. Draws of meshes on the same page therefore share their
//! buffer bindings. A mesh larger than the default page gets a page sized for
//! it. The total size of all pages is capped by
//! [`MeshConfig::max_total_bytes`]; uploads beyond the cap fail with
//! [`RenderError::LimitExceeded`] instead of growing without bound.
//!
//! Indices are stored already rebased onto the page (index + first vertex),
//! so draws never need a base-vertex offset (not every downlevel backend
//! supports one).
//!
//! Skinned meshes also need a bone-influence stream. A page that holds a
//! skinned mesh gets a **skin buffer** parallel to its vertex buffer (12
//! bytes per vertex slot, so a vertex's influences sit at the same index as
//! the vertex itself and the same rebased indices address both streams).
//! It is created when the first skinned mesh lands on the page, counted in
//! the byte cap, and released when the page's last skinned mesh goes,
//! except on the warm page 0 (default size), which keeps it so that one
//! character streaming in and out does not reallocate a multi-MiB buffer
//! each time. An idle skin buffer is also released when an upload would
//! otherwise exceed the byte cap. Skinned uploads prefer pages that already
//! have one and never attach one to an oversized page (whose skin buffer
//! would be sized for its one large mesh).

use std::collections::BTreeMap;
use std::ops::Range;

use crate::error::{RenderError, Result};
use crate::mesh::{GpuVertex, MeshInfo, MeshLimits, PreparedMesh, PreparedSubmesh};
use crate::skin::GpuSkin;

/// First-fit allocator of `u32` element ranges inside a fixed capacity.
///
/// Pure bookkeeping (no GPU), so it is unit-tested and fuzzed on its own.
/// Freed neighbours are coalesced. Freeing a range that overlaps free space
/// or lies outside the capacity is rejected (returns `false`) and leaves the
/// allocator unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeAllocator {
    capacity: u32,
    /// Free blocks: start → length (never empty, never adjacent).
    free: BTreeMap<u32, u32>,
    used: u32,
}

impl RangeAllocator {
    /// Allocator with everything free.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        let mut free = BTreeMap::new();
        if capacity > 0 {
            free.insert(0, capacity);
        }
        Self {
            capacity,
            free,
            used: 0,
        }
    }

    /// Total capacity in elements.
    #[must_use]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Elements currently allocated.
    #[must_use]
    pub fn used(&self) -> u32 {
        self.used
    }

    /// Largest single allocation that would currently succeed.
    #[must_use]
    pub fn largest_free(&self) -> u32 {
        self.free.values().copied().max().unwrap_or(0)
    }

    /// Allocate `len` contiguous elements (first fit). `len == 0` returns
    /// `Some(0..0)` without changing anything.
    pub fn alloc(&mut self, len: u32) -> Option<Range<u32>> {
        if len == 0 {
            return Some(0..0);
        }
        let (&start, &size) = self.free.iter().find(|&(_, &size)| size >= len)?;
        self.free.remove(&start);
        if size > len {
            self.free.insert(start + len, size - len);
        }
        self.used += len;
        Some(start..start + len)
    }

    /// Return a range obtained from [`Self::alloc`]. Empty ranges are a
    /// no-op (`true`).
    pub fn free(&mut self, range: Range<u32>) -> bool {
        if range.start >= range.end {
            return range.start == range.end;
        }
        if range.end > self.capacity {
            return false;
        }
        let len = range.end - range.start;
        // Overlap checks against the nearest free blocks on both sides.
        let before = self
            .free
            .range(..=range.start)
            .next_back()
            .map(|(&s, &l)| (s, l));
        if let Some((s, l)) = before
            && s + l > range.start
        {
            return false;
        }
        let after = self.free.range(range.start..).next().map(|(&s, &l)| (s, l));
        if let Some((s, _)) = after
            && s < range.end
        {
            return false;
        }
        if len > self.used {
            return false;
        }
        let mut start = range.start;
        let mut end = range.end;
        if let Some((s, l)) = before
            && s + l == start
        {
            self.free.remove(&s);
            start = s;
        }
        if let Some((s, l)) = after
            && s == end
        {
            self.free.remove(&s);
            end = s + l;
        }
        self.free.insert(start, end - start);
        self.used -= len;
        true
    }

    /// Check the internal invariants (used by tests and the fuzz target):
    /// free blocks are non-empty, sorted, non-overlapping, non-adjacent,
    /// inside the capacity, and `used + free == capacity`.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        let mut prev_end: Option<u32> = None;
        let mut free_total: u64 = 0;
        for (&s, &l) in &self.free {
            if l == 0 || u64::from(s) + u64::from(l) > u64::from(self.capacity) {
                return false;
            }
            if let Some(pe) = prev_end
                && s <= pe
            {
                return false;
            }
            prev_end = Some(s + l);
            free_total += u64::from(l);
        }
        free_total + u64::from(self.used) == u64::from(self.capacity)
    }
}

/// Mesh arena settings (part of [`crate::RendererConfig`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshConfig {
    /// Vertex buffer bytes of a default page.
    pub page_vertex_bytes: u64,
    /// Index buffer bytes of a default page.
    pub page_index_bytes: u64,
    /// Cap on the bytes of all pages together.
    pub max_total_bytes: u64,
    /// Per-mesh limits checked before anything is allocated.
    pub limits: MeshLimits,
    /// Build line-list edge indices at upload so [`crate::MeshDebug`]
    /// wireframes can be drawn. Costs about as much index memory again;
    /// a client that never shows wireframes turns it off.
    pub wireframe_edges: bool,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            page_vertex_bytes: 16 << 20,
            page_index_bytes: 8 << 20,
            max_total_bytes: 1 << 30,
            limits: MeshLimits::default(),
            wireframe_edges: true,
        }
    }
}

/// Handle to a mesh uploaded with [`crate::Renderer::upload_mesh`].
///
/// Generational and owner-tagged like [`crate::TextureId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshId {
    owner: u64,
    index: u32,
    generation: u32,
}

/// Memory statistics of the mesh arena.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArenaStats {
    /// Live meshes.
    pub meshes: u32,
    /// Live pages (vertex + index buffer pairs).
    pub pages: u32,
    /// Bytes of all page buffers.
    pub capacity_bytes: u64,
    /// Bytes of vertex data in use.
    pub vertex_bytes_used: u64,
    /// Bytes of index data in use (triangles and wireframe edges).
    pub index_bytes_used: u64,
    /// Live skinned meshes.
    pub skinned_meshes: u32,
    /// Bytes of bone-influence data in use (skinned meshes).
    pub skin_bytes_used: u64,
}

const VERTEX_SIZE: u64 = size_of::<GpuVertex>() as u64;
const INDEX_SIZE: u64 = 4;
const SKIN_SIZE: u64 = size_of::<GpuSkin>() as u64;

#[derive(Debug)]
pub(crate) struct Page {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    vertices: RangeAllocator,
    indices: RangeAllocator,
    meshes: u32,
    bytes: u64,
    /// Sized for one large mesh rather than the default page size; never
    /// kept as the warm page once empty.
    oversized: bool,
    /// Bone influences parallel to `vertex_buffer` (one [`GpuSkin`] per
    /// vertex slot); present while the page holds a skinned mesh, and kept
    /// on the warm page 0 afterwards.
    pub skin_buffer: Option<wgpu::Buffer>,
    skinned_meshes: u32,
    skin_used: u64,
}

impl Page {
    fn skin_bytes(&self) -> u64 {
        u64::from(self.vertices.capacity()) * SKIN_SIZE
    }
}

/// One uploaded mesh.
#[derive(Debug)]
pub(crate) struct GpuMesh {
    pub page: usize,
    pub vertex_range: Range<u32>,
    /// Triangle indices then edge indices, absolute within the page.
    pub index_range: Range<u32>,
    pub submeshes: Vec<PreparedSubmesh>,
    pub info: MeshInfo,
    /// Bone influences live in the page's skin buffer at `vertex_range`.
    pub skinned: bool,
}

impl GpuMesh {
    /// Absolute index range of a submesh's triangles.
    pub(crate) fn triangles(&self, s: &PreparedSubmesh) -> Range<u32> {
        let start = self.index_range.start + s.first_index;
        start..start + s.index_count
    }

    /// Absolute index range of a submesh's edges.
    pub(crate) fn edges(&self, s: &PreparedSubmesh) -> Range<u32> {
        let start = self.index_range.start + self.info.index_count + s.first_edge;
        start..start + s.edge_count
    }
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    mesh: Option<GpuMesh>,
}

/// Pages plus the generational mesh table.
#[derive(Debug)]
pub(crate) struct MeshArena {
    owner: u64,
    config: MeshConfig,
    pages: Vec<Option<Page>>,
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    total_bytes: u64,
}

impl MeshArena {
    pub(crate) fn new(owner: u64, config: MeshConfig) -> Self {
        Self {
            owner,
            config,
            pages: Vec::new(),
            slots: Vec::new(),
            free_slots: Vec::new(),
            total_bytes: 0,
        }
    }

    pub(crate) fn config(&self) -> &MeshConfig {
        &self.config
    }

    pub(crate) fn page(&self, i: usize) -> Option<&Page> {
        self.pages.get(i).and_then(Option::as_ref)
    }

    pub(crate) fn get(&self, id: MeshId) -> Option<&GpuMesh> {
        if id.owner != self.owner {
            return None;
        }
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.mesh.as_ref()
    }

    pub(crate) fn stats(&self) -> ArenaStats {
        let mut s = ArenaStats {
            meshes: self.slots.iter().filter(|s| s.mesh.is_some()).count() as u32,
            capacity_bytes: self.total_bytes,
            ..ArenaStats::default()
        };
        for p in self.pages.iter().flatten() {
            s.pages += 1;
            s.vertex_bytes_used += u64::from(p.vertices.used()) * VERTEX_SIZE;
            s.index_bytes_used += u64::from(p.indices.used()) * INDEX_SIZE;
            s.skinned_meshes += p.skinned_meshes;
            s.skin_bytes_used += p.skin_used;
        }
        s
    }

    fn alloc_in(page: &mut Page, verts: u32, idx: u32) -> Option<(Range<u32>, Range<u32>)> {
        if page.vertices.largest_free() < verts || page.indices.largest_free() < idx {
            return None;
        }
        let v = page.vertices.alloc(verts)?;
        match page.indices.alloc(idx) {
            Some(i) => Some((v, i)),
            None => {
                page.vertices.free(v);
                None
            }
        }
    }

    fn new_page(
        &mut self,
        device: &wgpu::Device,
        verts: u32,
        idx: u32,
        skinned: bool,
    ) -> Result<usize> {
        let max_buffer = device.limits().max_buffer_size;
        // Default page capacity, never larger than the device allows.
        let default_v = (self.config.page_vertex_bytes / VERTEX_SIZE)
            .min(max_buffer / VERTEX_SIZE)
            .min(u64::from(u32::MAX));
        let default_i = (self.config.page_index_bytes / INDEX_SIZE)
            .min(max_buffer / INDEX_SIZE)
            .min(u64::from(u32::MAX));
        // A mesh that alone needs more than the device allows fails below.
        let vcap = default_v.max(u64::from(verts));
        let icap = default_i.max(u64::from(idx));
        let oversized = vcap > default_v || icap > default_i;
        let vbytes = vcap * VERTEX_SIZE;
        let ibytes = icap * INDEX_SIZE;
        for (what, bytes) in [
            ("mesh vertex buffer", vbytes),
            ("mesh index buffer", ibytes),
        ] {
            if bytes > max_buffer {
                return Err(RenderError::LimitExceeded {
                    what,
                    requested: bytes,
                    max: max_buffer,
                });
            }
        }
        let sbytes = if skinned { vcap * SKIN_SIZE } else { 0 };
        let bytes = vbytes + ibytes;
        if self.total_bytes + bytes + sbytes > self.config.max_total_bytes {
            self.release_idle_skin_buffers();
        }
        if self.total_bytes + bytes + sbytes > self.config.max_total_bytes {
            return Err(RenderError::LimitExceeded {
                what: "mesh arena bytes",
                requested: self.total_bytes + bytes + sbytes,
                max: self.config.max_total_bytes,
            });
        }
        let make = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let page = Page {
            vertex_buffer: make("tessera-mesh-vertices", vbytes, wgpu::BufferUsages::VERTEX),
            index_buffer: make("tessera-mesh-indices", ibytes, wgpu::BufferUsages::INDEX),
            vertices: RangeAllocator::new(vcap as u32),
            indices: RangeAllocator::new(icap as u32),
            meshes: 0,
            bytes,
            oversized,
            skin_buffer: skinned.then(|| make_skin_buffer(device, sbytes)),
            skinned_meshes: 0,
            skin_used: 0,
        };
        self.total_bytes += bytes + sbytes;
        tracing::debug!(
            vbytes,
            ibytes,
            sbytes,
            total = self.total_bytes,
            "new mesh page"
        );
        let i = match self.pages.iter().position(Option::is_none) {
            Some(i) => {
                self.pages[i] = Some(page);
                i
            }
            None => {
                self.pages.push(Some(page));
                self.pages.len() - 1
            }
        };
        Ok(i)
    }

    /// Copy a prepared mesh into a page and return its handle.
    pub(crate) fn insert(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: PreparedMesh,
    ) -> Result<MeshId> {
        let PreparedMesh {
            vertices,
            mut indices,
            submeshes,
            info,
            skin,
        } = prepared;
        let skinned = skin.is_some();
        if skin.as_ref().is_some_and(|s| s.len() != vertices.len()) {
            return Err(RenderError::InvalidSkin(
                "skin stream length differs from the vertex count".into(),
            ));
        }
        let verts = u32::try_from(vertices.len()).map_err(|_| RenderError::LimitExceeded {
            what: "mesh vertices",
            requested: vertices.len() as u64,
            max: u64::from(u32::MAX),
        })?;
        let idx = u32::try_from(indices.len()).map_err(|_| RenderError::LimitExceeded {
            what: "mesh indices",
            requested: indices.len() as u64,
            max: u64::from(u32::MAX),
        })?;
        // A slot for the handle before touching the pages, so a full table
        // cannot leak page space.
        if self.free_slots.is_empty() && self.slots.len() >= u32::MAX as usize {
            return Err(RenderError::LimitExceeded {
                what: "live meshes",
                requested: self.slots.len() as u64,
                max: u64::from(u32::MAX),
            });
        }
        // Skinned meshes first look at pages that already carry a skin
        // buffer, then at any default-size page (attaching a skin buffer to
        // it; never to an oversized page, whose skin buffer would be sized
        // for its one large mesh). When only the byte cap stands in the way,
        // idle skin buffers are released and the pages searched again before
        // a new page is made (`plan_page`).
        let fits: Vec<Option<PageFit>> = self
            .pages
            .iter()
            .map(|p| {
                p.as_ref().map(|p| PageFit {
                    fits: p.vertices.largest_free() >= verts && p.indices.largest_free() >= idx,
                    oversized: p.oversized,
                    has_skin: p.skin_buffer.is_some(),
                    idle_skin: p.skin_buffer.is_some() && p.skinned_meshes == 0,
                    skin_bytes: p.skin_bytes(),
                })
            })
            .collect();
        let plan = plan_page(
            &fits,
            skinned,
            self.total_bytes,
            self.config.max_total_bytes,
        );
        let (page_index, (vrange, irange)) = match plan {
            PagePlan::Existing { page, release_idle } => {
                if release_idle {
                    self.release_idle_skin_buffers();
                }
                let p = self
                    .pages
                    .get_mut(page)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| RenderError::Gpu("planned mesh page vanished".into()))?;
                let r = Self::alloc_in(p, verts, idx).ok_or_else(|| {
                    RenderError::Gpu("planned mesh page cannot hold its mesh".into())
                })?;
                (page, r)
            }
            PagePlan::NewPage => {
                let i = self.new_page(device, verts, idx, skinned)?;
                let page = self.pages[i]
                    .as_mut()
                    .ok_or_else(|| RenderError::Gpu("mesh page vanished after creation".into()))?;
                let r = Self::alloc_in(page, verts, idx).ok_or_else(|| {
                    RenderError::Gpu("fresh mesh page cannot hold its mesh".into())
                })?;
                (i, r)
            }
        };
        for i in &mut indices {
            *i += vrange.start;
        }
        let Some(page) = self.pages[page_index].as_mut() else {
            return Err(RenderError::Gpu("mesh page vanished".into()));
        };
        if let Some(skin) = &skin {
            if page.skin_buffer.is_none() {
                let sbytes = page.skin_bytes();
                page.skin_buffer = Some(make_skin_buffer(device, sbytes));
                self.total_bytes += sbytes;
                tracing::debug!(
                    page = page_index,
                    sbytes,
                    "skin buffer attached to mesh page"
                );
            }
            if let Some(buffer) = &page.skin_buffer {
                queue.write_buffer(
                    buffer,
                    u64::from(vrange.start) * SKIN_SIZE,
                    bytemuck::cast_slice(skin),
                );
            }
            page.skinned_meshes += 1;
            page.skin_used += u64::from(verts) * SKIN_SIZE;
        }
        page.meshes += 1;
        queue.write_buffer(
            &page.vertex_buffer,
            u64::from(vrange.start) * VERTEX_SIZE,
            bytemuck::cast_slice(&vertices),
        );
        queue.write_buffer(
            &page.index_buffer,
            u64::from(irange.start) * INDEX_SIZE,
            bytemuck::cast_slice(&indices),
        );
        let mesh = GpuMesh {
            page: page_index,
            vertex_range: vrange,
            index_range: irange,
            submeshes,
            info,
            skinned,
        };
        let index = match self.free_slots.pop() {
            Some(i) => i,
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    mesh: None,
                });
                (self.slots.len() - 1) as u32
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.mesh = Some(mesh);
        Ok(MeshId {
            owner: self.owner,
            index,
            generation: slot.generation,
        })
    }

    /// Free a mesh's ranges; drops a page that becomes empty, except page 0
    /// when it has the default size (kept warm for the next upload). An
    /// oversized page made for one large mesh is always released.
    pub(crate) fn remove(&mut self, id: MeshId) -> bool {
        self.remove_impl(id, false)
    }

    /// Like [`Self::remove`], but also releases page 0 when it becomes
    /// empty: used to undo an upload that failed on the GPU, whose page may
    /// itself be the failed allocation.
    pub(crate) fn discard(&mut self, id: MeshId) -> bool {
        self.remove_impl(id, true)
    }

    fn remove_impl(&mut self, id: MeshId, release_warm: bool) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return false;
        };
        let Some(mesh) = slot.mesh.take() else {
            return false;
        };
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation != 0 {
            self.free_slots.push(id.index);
        }
        let mut drop_page = false;
        if let Some(Some(page)) = self.pages.get_mut(mesh.page) {
            page.vertices.free(mesh.vertex_range.clone());
            page.indices.free(mesh.index_range.clone());
            page.meshes = page.meshes.saturating_sub(1);
            if mesh.skinned {
                page.skinned_meshes = page.skinned_meshes.saturating_sub(1);
                page.skin_used = page.skin_used.saturating_sub(
                    u64::from(mesh.vertex_range.end - mesh.vertex_range.start) * SKIN_SIZE,
                );
                // The warm page keeps its skin buffer (see the module docs).
                let warm = mesh.page == 0 && !page.oversized && !release_warm;
                if page.skinned_meshes == 0 && !warm && page.skin_buffer.take().is_some() {
                    self.total_bytes -= page.skin_bytes();
                    tracing::debug!(page = mesh.page, "released skin buffer of mesh page");
                }
            }
            drop_page = page.meshes == 0 && (mesh.page != 0 || page.oversized || release_warm);
        }
        if drop_page && let Some(page) = self.pages[mesh.page].take() {
            let skin = if page.skin_buffer.is_some() {
                page.skin_bytes()
            } else {
                0
            };
            self.total_bytes -= page.bytes + skin;
            tracing::debug!(page = mesh.page, "released empty mesh page");
        }
        true
    }

    /// Release the skin buffers of pages that hold no skinned mesh (the
    /// warm page's kept buffer), under byte-cap pressure.
    fn release_idle_skin_buffers(&mut self) {
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(page) = page
                && page.skinned_meshes == 0
                && page.skin_buffer.take().is_some()
            {
                self.total_bytes -= page.skin_bytes();
                tracing::debug!(page = i, "released idle skin buffer under the byte cap");
            }
        }
    }
}

/// What the page search needs to know about one live page (no GPU state, so
/// the policy is unit-tested on its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageFit {
    /// The mesh's vertices and indices both fit in a free block.
    fits: bool,
    oversized: bool,
    has_skin: bool,
    /// Holds a skin buffer but no skinned mesh (released under cap pressure).
    idle_skin: bool,
    /// Bytes of the page's skin buffer (whether or not it has one).
    skin_bytes: u64,
}

/// Where an upload goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PagePlan {
    /// An existing page; release the idle skin buffers first when set.
    Existing { page: usize, release_idle: bool },
    /// No existing page will do; make a new one.
    NewPage,
}

/// First page that takes the mesh: for skinned meshes, pages with a skin
/// buffer first, then default-size pages whose new skin buffer fits under
/// `cap` with `total` bytes already in use.
fn search_pages(pages: &[Option<PageFit>], skinned: bool, total: u64, cap: u64) -> Option<usize> {
    let passes: &[bool] = if skinned { &[true, false] } else { &[false] };
    passes.iter().find_map(|&want_skin| {
        pages.iter().position(|p| {
            p.is_some_and(|p| {
                p.fits
                    && (!want_skin || p.has_skin)
                    && (!skinned
                        || p.has_skin
                        || (!p.oversized && total.saturating_add(p.skin_bytes) <= cap))
            })
        })
    })
}

/// The page policy of [`MeshArena::insert`]. A skinned upload that finds no
/// page is searched again as if the idle skin buffers were released (the
/// module docs: an idle skin buffer yields when an upload would otherwise
/// exceed the byte cap), so a page rejected only for the cap can still take
/// it; a new page is the last resort.
fn plan_page(pages: &[Option<PageFit>], skinned: bool, total: u64, cap: u64) -> PagePlan {
    if let Some(page) = search_pages(pages, skinned, total, cap) {
        return PagePlan::Existing {
            page,
            release_idle: false,
        };
    }
    if skinned {
        let freed: u64 = pages
            .iter()
            .flatten()
            .filter(|p| p.idle_skin)
            .map(|p| p.skin_bytes)
            .sum();
        if freed > 0 {
            let after: Vec<Option<PageFit>> = pages
                .iter()
                .map(|p| {
                    p.map(|p| {
                        if p.idle_skin {
                            PageFit {
                                has_skin: false,
                                idle_skin: false,
                                ..p
                            }
                        } else {
                            p
                        }
                    })
                })
                .collect();
            if let Some(page) = search_pages(&after, skinned, total.saturating_sub(freed), cap) {
                return PagePlan::Existing {
                    page,
                    release_idle: true,
                };
            }
        }
    }
    PagePlan::NewPage
}

fn make_skin_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tessera-mesh-skin"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fit_and_coalescing() {
        let mut a = RangeAllocator::new(10);
        let r1 = a.alloc(3).expect("r1");
        let r2 = a.alloc(3).expect("r2");
        let r3 = a.alloc(3).expect("r3");
        assert_eq!((r1.clone(), r2.clone(), r3.clone()), (0..3, 3..6, 6..9));
        assert_eq!(a.alloc(2), None, "only one element left");
        assert!(a.free(r2.clone()));
        assert!(!a.free(r2.clone()), "double free rejected");
        assert!(!a.free(4..5), "overlapping free space rejected");
        assert!(a.is_consistent());
        assert_eq!(a.alloc(2), Some(3..5), "first fit reuses the hole");
        assert!(a.free(3..5));
        assert!(a.free(r1));
        assert!(a.free(r3));
        assert!(a.is_consistent());
        assert_eq!(a.largest_free(), 10, "everything coalesced");
        assert_eq!(a.used(), 0);
        assert!(!a.free(5..11), "outside capacity");
        assert_eq!(a.alloc(0), Some(0..0));
        assert!(a.free(0..0));
        let mut z = RangeAllocator::new(0);
        assert_eq!(z.alloc(1), None);
        assert!(z.is_consistent());
    }

    #[test]
    fn free_never_underflows_used() {
        let mut a = RangeAllocator::new(8);
        // Never allocated, but not overlapping free space either: the
        // allocator is full, so "used" bookkeeping rejects it.
        let _ = a.alloc(8);
        assert!(a.free(0..8));
        assert!(!a.free(0..8));
        assert!(a.is_consistent());
    }

    fn fit(fits: bool, has_skin: bool, idle_skin: bool) -> Option<PageFit> {
        Some(PageFit {
            fits,
            oversized: false,
            has_skin,
            idle_skin,
            skin_bytes: 100,
        })
    }

    #[test]
    fn skinned_uploads_prefer_pages_with_a_skin_buffer() {
        let pages = [fit(true, false, false), None, fit(true, true, false)];
        assert_eq!(
            plan_page(&pages, true, 0, 1000),
            PagePlan::Existing {
                page: 2,
                release_idle: false
            }
        );
        assert_eq!(
            plan_page(&pages, false, 0, 1000),
            PagePlan::Existing {
                page: 0,
                release_idle: false
            }
        );
        let oversized = [Some(PageFit {
            oversized: true,
            ..fit(true, false, false).expect("page")
        })];
        assert_eq!(plan_page(&oversized, true, 0, 1000), PagePlan::NewPage);
    }

    #[test]
    fn idle_skin_buffer_yields_to_an_existing_page_under_the_cap() {
        // Regression: page 0 is full of static meshes and keeps its idle skin
        // buffer; page 1 has room but attaching a skin buffer to it would pass
        // the cap. Releasing page 0's idle buffer was only tried when making a
        // new page, which needs far more bytes, so the upload failed although
        // page 1 would then have taken it.
        let pages = [fit(false, true, true), fit(true, false, false)];
        assert_eq!(
            search_pages(&pages, true, 950, 1000),
            None,
            "cap blocks page 1"
        );
        assert_eq!(
            plan_page(&pages, true, 950, 1000),
            PagePlan::Existing {
                page: 1,
                release_idle: true
            }
        );
        // Still no room after the release: a new page, nothing released here.
        assert_eq!(plan_page(&pages, true, 1000 + 50, 1000), PagePlan::NewPage);
        // A skin buffer in use is never counted as releasable.
        let busy = [fit(false, true, false), fit(true, false, false)];
        assert_eq!(plan_page(&busy, true, 950, 1000), PagePlan::NewPage);
        // Static uploads never release skin buffers.
        assert_eq!(
            plan_page(&[fit(false, true, true)], false, 950, 1000),
            PagePlan::NewPage
        );
        // Pages that are simply full lead to a new page, not a release.
        let full = [fit(false, true, true), fit(false, false, false)];
        assert_eq!(plan_page(&full, true, 0, 1000), PagePlan::NewPage);
    }

    #[test]
    fn default_config_is_sane() {
        let c = MeshConfig::default();
        assert!(c.page_vertex_bytes / VERTEX_SIZE > 100_000);
        assert!(c.max_total_bytes >= c.page_vertex_bytes + c.page_index_bytes);
    }
}
