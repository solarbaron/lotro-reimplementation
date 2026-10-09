// SPDX-License-Identifier: GPL-3.0-or-later
//! RGBA8 texture uploads (single level or a stored mip chain) and a generational
//! texture store.

use crate::error::{RenderError, Result};

/// Handle to a texture owned by a [`crate::Renderer`].
///
/// Generational and owner-tagged: a handle to a destroyed texture never
/// aliases a newer one, and a handle issued by one renderer is never
/// accepted by another (it behaves like a destroyed texture there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId {
    owner: u64,
    index: u32,
    generation: u32,
}

impl TextureId {
    /// A handle no renderer ever issues (owner 0): every renderer treats it
    /// as unknown. Useful as a default value and for checks that never
    /// resolve the handle (for example `UiEntry::geometry`).
    #[must_use]
    pub const fn placeholder() -> Self {
        Self {
            owner: 0,
            index: u32::MAX,
            generation: u32::MAX,
        }
    }

    /// A handle no renderer ever issues (owner 0).
    #[cfg(test)]
    pub(crate) fn dangling() -> Self {
        Self::placeholder()
    }
}

/// How texel values are interpreted when sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// Colour data stored sRGB-encoded (decoded to linear when sampled).
    #[default]
    Srgb,
    /// Data used as-is (normal maps, masks, lookup tables).
    Linear,
}

/// Texture filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Nearest-neighbour.
    Nearest,
    /// Bilinear.
    #[default]
    Linear,
}

/// Texture coordinate addressing outside `0..1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrap {
    /// Tile.
    #[default]
    Repeat,
    /// Clamp to the edge texel.
    Clamp,
}

/// Description of an RGBA8 upload (4 bytes per texel, rows tightly packed,
/// top row first).
#[derive(Debug, Clone, Copy)]
pub struct TextureUpload<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA8 data.
    pub rgba: &'a [u8],
    /// Interpretation of the colour channels.
    pub color_space: ColorSpace,
    /// Sampling filter.
    pub filter: Filter,
    /// Addressing mode.
    pub wrap: Wrap,
}

impl<'a> TextureUpload<'a> {
    /// sRGB, bilinear, repeating upload.
    #[must_use]
    pub fn rgba8(width: u32, height: u32, rgba: &'a [u8]) -> Self {
        Self {
            label: None,
            width,
            height,
            rgba,
            color_space: ColorSpace::Srgb,
            filter: Filter::Linear,
            wrap: Wrap::Repeat,
        }
    }
}

/// One stored level of a mip chain: tightly packed RGBA8, top row first.
#[derive(Debug, Clone, Copy)]
pub struct MipLevel<'a> {
    /// Width in texels of the stored level.
    pub width: u32,
    /// Height in texels of the stored level.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: &'a [u8],
}

/// A texture with a stored mip chain (`docs/specs/texture-list.md` §5.5).
///
/// The texture takes the size of `levels[0]` and one mip per level: level *i* fills mip
/// *i*, whose size is max(1, ⌊size / 2^i⌋). A level whose stored size differs from its
/// mip slot is copied clipped to the smaller width and height (its top-left part; a
/// smaller level leaves the rest of the slot zero), as the original does. Missing small
/// levels are not generated: sampling below the last level stays on it. Levels beyond a
/// full pyramid (more than ⌊log2(max(width, height))⌋ + 1) are ignored with a debug log.
#[derive(Debug, Clone, Copy)]
pub struct MipChainUpload<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// The active chain, largest level first (at least one level).
    pub levels: &'a [MipLevel<'a>],
    /// Interpretation of the colour channels.
    pub color_space: ColorSpace,
    /// Sampling filter (also between mips: `Linear` is trilinear).
    pub filter: Filter,
    /// Addressing mode.
    pub wrap: Wrap,
}

impl<'a> MipChainUpload<'a> {
    /// sRGB, trilinear, repeating upload of `levels`.
    #[must_use]
    pub fn new(levels: &'a [MipLevel<'a>]) -> Self {
        Self {
            label: None,
            levels,
            color_space: ColorSpace::Srgb,
            filter: Filter::Linear,
            wrap: Wrap::Repeat,
        }
    }
}

/// One copy of a [`MipChainUpload`]: which stored level goes to which mip, and the
/// clipped extent copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MipCopy {
    /// Index into `levels` (= mip level).
    pub level: u32,
    /// Copied width and height (≤ both the stored size and the slot size).
    pub width: u32,
    pub height: u32,
    /// Stored row length in texels (the source's `bytes_per_row` / 4).
    pub stored_width: u32,
}

/// Validated layout of a mip-chain upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MipPlan {
    pub width: u32,
    pub height: u32,
    pub copies: Vec<MipCopy>,
}

/// Number of levels of a full pyramid for a `width × height` texture.
pub(crate) fn full_mip_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

/// Check every level of a chain and plan the copies (texture-list.md §5.5). Every level
/// must be non-empty, within `max_dim`, and carry exactly `width * height * 4` bytes;
/// levels past a full pyramid are dropped.
pub(crate) fn plan_mip_chain(levels: &[MipLevel<'_>], max_dim: u32) -> Result<MipPlan> {
    let first = levels
        .first()
        .ok_or_else(|| RenderError::InvalidTexture("mip chain without levels".into()))?;
    let (width, height) = (first.width, first.height);
    let full = full_mip_count(width, height) as usize;
    if levels.len() > full {
        tracing::debug!(
            levels = levels.len(),
            full,
            width,
            height,
            "mip chain longer than a full pyramid; extra levels ignored"
        );
    }
    let mut copies = Vec::with_capacity(levels.len().min(full));
    for (i, level) in levels.iter().take(full).enumerate() {
        validate_rgba8(level.width, level.height, level.rgba.len(), max_dim)
            .map_err(|e| RenderError::InvalidTexture(format!("mip level {i}: {e}")))?;
        let i = u32::try_from(i).unwrap_or(u32::MAX);
        let slot_w = (width >> i).max(1);
        let slot_h = (height >> i).max(1);
        copies.push(MipCopy {
            level: i,
            width: level.width.min(slot_w),
            height: level.height.min(slot_h),
            stored_width: level.width,
        });
    }
    Ok(MipPlan {
        width,
        height,
        copies,
    })
}

/// Validate dimensions and data length; returns the expected byte length.
pub(crate) fn validate_rgba8(width: u32, height: u32, len: usize, max_dim: u32) -> Result<usize> {
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidTexture(format!(
            "zero-sized texture {width}x{height}"
        )));
    }
    if width > max_dim || height > max_dim {
        return Err(RenderError::InvalidTexture(format!(
            "{width}x{height} exceeds device limit {max_dim}"
        )));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| RenderError::InvalidTexture("size overflow".into()))?;
    if len != expected {
        return Err(RenderError::InvalidTexture(format!(
            "{width}x{height} RGBA8 needs {expected} bytes, got {len}"
        )));
    }
    Ok(expected)
}

#[derive(Debug)]
pub(crate) struct GpuTexture {
    pub texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
    /// View and sampler, for bind groups that combine several textures
    /// (mesh materials).
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
    /// Number of mip levels (1 for single-level uploads).
    pub mip_levels: u32,
    /// The texture decodes sRGB when sampled.
    pub srgb: bool,
    /// `Some(view)` when this entry samples an offscreen render target (it
    /// is then neither writable nor destroyed with the entry, and must not
    /// be sampled while rendering into that same view).
    pub target_view: Option<wgpu::TextureView>,
}

#[derive(Debug)]
struct Slot<T> {
    generation: u32,
    tex: Option<T>,
}

/// Samplers indexed by (filter, wrap).
#[derive(Debug)]
pub(crate) struct Samplers {
    all: [wgpu::Sampler; 4],
}

impl Samplers {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let make = |filter: Filter, wrap: Wrap| {
            let f = match filter {
                Filter::Nearest => wgpu::FilterMode::Nearest,
                Filter::Linear => wgpu::FilterMode::Linear,
            };
            let a = match wrap {
                Wrap::Repeat => wgpu::AddressMode::Repeat,
                Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
            };
            // The mip filter matters only for mip chains (single-level textures
            // sample level 0 either way): `Linear` is trilinear.
            let m = match filter {
                Filter::Nearest => wgpu::MipmapFilterMode::Nearest,
                Filter::Linear => wgpu::MipmapFilterMode::Linear,
            };
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("tessera-sampler"),
                address_mode_u: a,
                address_mode_v: a,
                address_mode_w: a,
                mag_filter: f,
                min_filter: f,
                mipmap_filter: m,
                ..Default::default()
            })
        };
        Self {
            all: [
                make(Filter::Nearest, Wrap::Repeat),
                make(Filter::Nearest, Wrap::Clamp),
                make(Filter::Linear, Wrap::Repeat),
                make(Filter::Linear, Wrap::Clamp),
            ],
        }
    }

    pub(crate) fn get(&self, filter: Filter, wrap: Wrap) -> &wgpu::Sampler {
        let i = match (filter, wrap) {
            (Filter::Nearest, Wrap::Repeat) => 0,
            (Filter::Nearest, Wrap::Clamp) => 1,
            (Filter::Linear, Wrap::Repeat) => 2,
            (Filter::Linear, Wrap::Clamp) => 3,
        };
        &self.all[i]
    }
}

/// Owns every texture of one renderer. Generic over the stored value so the
/// handle bookkeeping is unit-testable without a GPU.
#[derive(Debug)]
pub(crate) struct TextureStore<T = GpuTexture> {
    owner: u64,
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> TextureStore<T> {
    /// Empty store whose handles carry `owner` (unique per renderer).
    pub(crate) fn new(owner: u64) -> Self {
        Self {
            owner,
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    pub(crate) fn insert(&mut self, tex: T) -> Result<TextureId> {
        let index = if let Some(i) = self.free.pop() {
            i
        } else {
            let i = u32::try_from(self.slots.len())
                .map_err(|_| RenderError::InvalidTexture("texture store full".into()))?;
            self.slots.push(Slot {
                generation: 0,
                tex: None,
            });
            i
        };
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or_else(|| RenderError::InvalidTexture("texture slot missing".into()))?;
        slot.tex = Some(tex);
        Ok(TextureId {
            owner: self.owner,
            index,
            generation: slot.generation,
        })
    }

    fn slot(&self, id: TextureId) -> Option<&Slot<T>> {
        if id.owner != self.owner {
            return None;
        }
        self.slots
            .get(id.index as usize)
            .filter(|s| s.generation == id.generation)
    }

    pub(crate) fn get(&self, id: TextureId) -> Option<&T> {
        self.slot(id)?.tex.as_ref()
    }

    /// Remove and return the value; the handle (and every copy of it) is
    /// stale afterwards.
    pub(crate) fn take(&mut self, id: TextureId) -> Option<T> {
        self.slot(id)?;
        let slot = self.slots.get_mut(id.index as usize)?;
        let tex = slot.tex.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        // Retire a slot whose generation wrapped instead of reusing it, so a
        // very old handle can never alias a new texture.
        if slot.generation != 0 {
            self.free.push(id.index);
        }
        Some(tex)
    }

    pub(crate) fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.tex.is_some()).count()
    }
}

impl TextureStore<GpuTexture> {
    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        label: Option<&str>,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        samplers: &Samplers,
        desc: &TextureUpload<'_>,
    ) -> Result<TextureId> {
        let max_dim = device.limits().max_texture_dimension_2d;
        validate_rgba8(desc.width, desc.height, desc.rgba.len(), max_dim)?;
        let format = match desc.color_space {
            ColorSpace::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            ColorSpace::Linear => wgpu::TextureFormat::Rgba8Unorm,
        };
        let size = wgpu::Extent3d {
            width: desc.width,
            height: desc.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: desc.label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_rgba8(queue, &texture, desc.width, desc.height, desc.rgba);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = samplers.get(desc.filter, desc.wrap).clone();
        let bind_group = Self::bind_group(device, layout, desc.label, &view, &sampler);
        let id = self.insert(GpuTexture {
            texture,
            bind_group,
            view,
            sampler,
            width: desc.width,
            height: desc.height,
            mip_levels: 1,
            srgb: desc.color_space == ColorSpace::Srgb,
            target_view: None,
        })?;
        tracing::trace!(?id, desc.width, desc.height, "uploaded texture");
        Ok(id)
    }

    pub(crate) fn upload_mips(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        samplers: &Samplers,
        desc: &MipChainUpload<'_>,
    ) -> Result<TextureId> {
        let max_dim = device.limits().max_texture_dimension_2d;
        let plan = plan_mip_chain(desc.levels, max_dim)?;
        let format = match desc.color_space {
            ColorSpace::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            ColorSpace::Linear => wgpu::TextureFormat::Rgba8Unorm,
        };
        let mip_levels = u32::try_from(plan.copies.len()).unwrap_or(1).max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: desc.label,
            size: wgpu::Extent3d {
                width: plan.width,
                height: plan.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for copy in &plan.copies {
            let Some(level) = desc.levels.get(copy.level as usize) else {
                continue;
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: copy.level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                level.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(copy.stored_width.saturating_mul(4)),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: copy.width,
                    height: copy.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = samplers.get(desc.filter, desc.wrap).clone();
        let bind_group = Self::bind_group(device, layout, desc.label, &view, &sampler);
        let id = self.insert(GpuTexture {
            texture,
            bind_group,
            view,
            sampler,
            width: plan.width,
            height: plan.height,
            mip_levels,
            srgb: desc.color_space == ColorSpace::Srgb,
            target_view: None,
        })?;
        tracing::trace!(
            ?id,
            plan.width,
            plan.height,
            mip_levels,
            "uploaded mip chain"
        );
        Ok(id)
    }

    /// Register an existing render-target texture for sampling.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register_target(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        samplers: &Samplers,
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
        filter: Filter,
        wrap: Wrap,
    ) -> Result<TextureId> {
        let sampler = samplers.get(filter, wrap).clone();
        let bind_group = Self::bind_group(
            device,
            layout,
            Some("tessera-target-texture"),
            view,
            &sampler,
        );
        self.insert(GpuTexture {
            texture: texture.clone(),
            bind_group,
            view: view.clone(),
            sampler,
            width: texture.width(),
            height: texture.height(),
            mip_levels: 1,
            srgb: texture.format().is_srgb(),
            target_view: Some(view.clone()),
        })
    }

    pub(crate) fn update(&self, queue: &wgpu::Queue, id: TextureId, rgba: &[u8]) -> Result<()> {
        let tex = self.get(id).ok_or(RenderError::UnknownTexture(id))?;
        if tex.target_view.is_some() {
            return Err(RenderError::InvalidTexture(
                "render-target textures are written by rendering, not uploads".into(),
            ));
        }
        if tex.mip_levels > 1 {
            return Err(RenderError::InvalidTexture(
                "mip-chain textures cannot be updated in place; upload a new chain".into(),
            ));
        }
        validate_rgba8(tex.width, tex.height, rgba.len(), u32::MAX)?;
        write_rgba8(queue, &tex.texture, tex.width, tex.height, rgba);
        Ok(())
    }

    /// Destroy a texture. Render-target registrations only drop the
    /// sampling binding; the target itself stays usable.
    pub(crate) fn remove(&mut self, id: TextureId) -> bool {
        match self.take(id) {
            Some(tex) => {
                if tex.target_view.is_none() {
                    tex.texture.destroy();
                }
                true
            }
            None => false,
        }
    }
}

fn write_rgba8(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width.saturating_mul(4)),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert_eq!(validate_rgba8(2, 3, 24, 16).ok(), Some(24));
        assert!(validate_rgba8(0, 3, 0, 16).is_err());
        assert!(validate_rgba8(2, 3, 23, 16).is_err());
        assert!(validate_rgba8(32, 1, 128, 16).is_err());
        assert!(validate_rgba8(u32::MAX, u32::MAX, 0, u32::MAX).is_err());
    }

    fn level(width: u32, height: u32, data: &[u8]) -> MipLevel<'_> {
        MipLevel {
            width,
            height,
            rgba: data,
        }
    }

    #[test]
    fn mip_plan_halving_chain() {
        let a = vec![0u8; 64 * 16 * 4];
        let b = vec![0u8; 32 * 8 * 4];
        let c = vec![0u8; 16 * 4 * 4];
        let plan = plan_mip_chain(
            &[level(64, 16, &a), level(32, 8, &b), level(16, 4, &c)],
            4096,
        )
        .expect("valid chain");
        assert_eq!((plan.width, plan.height), (64, 16));
        assert_eq!(
            plan.copies
                .iter()
                .map(|c| (c.level, c.width, c.height))
                .collect::<Vec<_>>(),
            vec![(0, 64, 16), (1, 32, 8), (2, 16, 4)]
        );
        assert_eq!(full_mip_count(64, 16), 7);
        assert_eq!(full_mip_count(1, 1), 1);
        assert_eq!(full_mip_count(4096, 1), 13);
    }

    #[test]
    fn mip_plan_clips_non_halving_levels() {
        // texture-list.md §4.2 exception: two equal top levels; the second is copied
        // clipped into the half-size slot.
        let a = vec![0u8; 8 * 8 * 4];
        let plan = plan_mip_chain(&[level(8, 8, &a), level(8, 8, &a)], 4096).expect("chain");
        assert_eq!(plan.copies[1].width, 4);
        assert_eq!(plan.copies[1].height, 4);
        assert_eq!(plan.copies[1].stored_width, 8);
        // A level smaller than its slot is copied whole.
        let small = vec![0u8; 4];
        let plan = plan_mip_chain(&[level(8, 8, &a), level(1, 1, &small)], 4096).expect("chain");
        assert_eq!((plan.copies[1].width, plan.copies[1].height), (1, 1));
    }

    #[test]
    fn mip_plan_rejects_and_truncates() {
        assert!(plan_mip_chain(&[], 4096).is_err());
        let a = vec![0u8; 4 * 4 * 4];
        let short = vec![0u8; 3];
        assert!(plan_mip_chain(&[level(4, 4, &a), level(2, 2, &short)], 4096).is_err());
        assert!(plan_mip_chain(&[level(0, 4, &[])], 4096).is_err());
        assert!(plan_mip_chain(&[level(4, 4, &a)], 2).is_err());
        // 4x4 has three levels; a hostile list of 1,000 is cut there.
        let one = vec![0u8; 4];
        let mut levels = vec![level(4, 4, &a)];
        levels.extend(std::iter::repeat_n(level(1, 1, &one), 1000));
        let plan = plan_mip_chain(&levels, 4096).expect("chain");
        assert_eq!(plan.copies.len(), 3);
    }

    #[test]
    fn handles_are_owner_tagged() {
        let mut a: TextureStore<&str> = TextureStore::new(1);
        let mut b: TextureStore<&str> = TextureStore::new(2);
        let ia = a.insert("a").expect("insert");
        let ib = b.insert("b").expect("insert");
        // Same slot index and generation in both stores...
        assert_eq!((ia.index, ia.generation), (ib.index, ib.generation));
        // ...but a foreign handle never resolves or removes anything.
        assert_eq!(b.get(ia), None);
        assert_eq!(b.take(ia), None);
        assert_eq!(b.get(ib), Some(&"b"));
        assert_eq!(a.get(ib), None);
        assert_eq!(a.get(ia), Some(&"a"));
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn handles_are_generational() {
        let mut s: TextureStore<u32> = TextureStore::new(7);
        let first = s.insert(1).expect("insert");
        assert_eq!(s.take(first), Some(1));
        assert_eq!(s.take(first), None);
        let second = s.insert(2).expect("insert");
        assert_eq!(second.index, first.index, "slot reused");
        assert_eq!(s.get(first), None, "stale handle does not alias");
        assert_eq!(s.get(second), Some(&2));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn wrapped_generation_retires_slot() {
        let mut s: TextureStore<u32> = TextureStore::new(1);
        let id = s.insert(0).expect("insert");
        if let Some(slot) = s.slots.get_mut(id.index as usize) {
            slot.generation = u32::MAX;
        }
        let id = TextureId {
            generation: u32::MAX,
            ..id
        };
        assert_eq!(s.take(id), Some(0));
        let next = s.insert(1).expect("insert");
        assert_ne!(next.index, id.index, "wrapped slot is not reused");
    }
}
