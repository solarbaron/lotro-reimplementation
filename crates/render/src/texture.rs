// SPDX-License-Identifier: GPL-3.0-or-later
//! RGBA8 texture uploads and a generational texture store.

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
    /// A handle no renderer ever issues (owner 0).
    #[cfg(test)]
    pub(crate) fn dangling() -> Self {
        Self {
            owner: 0,
            index: u32::MAX,
            generation: u32::MAX,
        }
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
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("tessera-sampler"),
                address_mode_u: a,
                address_mode_v: a,
                address_mode_w: a,
                mag_filter: f,
                min_filter: f,
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
            target_view: None,
        })?;
        tracing::trace!(?id, desc.width, desc.height, "uploaded texture");
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
