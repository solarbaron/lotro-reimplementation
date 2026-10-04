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

use std::collections::BTreeMap;
use std::ops::Range;

use crate::error::{RenderError, Result};
use crate::mesh::{GpuVertex, MeshInfo, MeshLimits, PreparedMesh, PreparedSubmesh};

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
}

const VERTEX_SIZE: u64 = size_of::<GpuVertex>() as u64;
const INDEX_SIZE: u64 = 4;

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

    fn new_page(&mut self, device: &wgpu::Device, verts: u32, idx: u32) -> Result<usize> {
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
        let bytes = vbytes + ibytes;
        if self.total_bytes + bytes > self.config.max_total_bytes {
            return Err(RenderError::LimitExceeded {
                what: "mesh arena bytes",
                requested: self.total_bytes + bytes,
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
        };
        self.total_bytes += bytes;
        tracing::debug!(vbytes, ibytes, total = self.total_bytes, "new mesh page");
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
        } = prepared;
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
        let mut found = None;
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(page) = page
                && let Some(r) = Self::alloc_in(page, verts, idx)
            {
                found = Some((i, r));
                break;
            }
        }
        let (page_index, (vrange, irange)) = match found {
            Some(f) => f,
            None => {
                let i = self.new_page(device, verts, idx)?;
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
            drop_page = page.meshes == 0 && (mesh.page != 0 || page.oversized || release_warm);
        }
        if drop_page && let Some(page) = self.pages[mesh.page].take() {
            self.total_bytes -= page.bytes;
            tracing::debug!(page = mesh.page, "released empty mesh page");
        }
        true
    }
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

    #[test]
    fn default_config_is_sane() {
        let c = MeshConfig::default();
        assert!(c.page_vertex_bytes / VERTEX_SIZE > 100_000);
        assert!(c.max_total_bytes >= c.page_vertex_bytes + c.page_index_bytes);
    }
}
