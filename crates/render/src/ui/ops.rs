// SPDX-License-Identifier: GPL-3.0-or-later
//! Operations on UI textures (render.md §6.3): sub-rectangle uploads, GPU
//! copies and scissored clears, kept in call order.
//!
//! `Queue::write_texture` would run before every command buffer of its
//! submission and so overtake an earlier copy or clear. Uploads are
//! therefore staged in a buffer and copied in the same encoder as the copies
//! and clears, which runs the three kinds strictly in call order. The
//! pending operations are submitted by the next `end_frame` (before the
//! frame's own commands), by `Renderer::flush_ui_ops`, before a texture is
//! destroyed, before `Renderer::update_texture` writes a texture, whenever
//! the staged bytes would pass [`STAGING_FLUSH_BYTES`], and whenever
//! [`MAX_PENDING_OPS`] operations are pending (so that a caller that keeps
//! painting while no frame is ended cannot grow the queue without bound).
//! Consecutive clears of one texture share one render pass.

use crate::context::GpuContext;
use crate::error::{RenderError, Result};

/// Staged upload bytes that trigger a submission (bounds the CPU and GPU
/// staging memory).
pub(crate) const STAGING_FLUSH_BYTES: usize = 32 << 20;

/// Pending operations that trigger a submission (bounds the queue and the
/// work of one flush).
pub(crate) const MAX_PENDING_OPS: usize = 4096;

/// Staging memory kept between flushes, on the CPU and on the GPU each: U3's
/// per-frame upload cap (8 MiB, decision 0015 §6). After a larger flush the
/// excess is released, so one large first paint does not stay resident.
pub(crate) const STAGING_RETAIN_BYTES: usize = 8 << 20;

/// One pending operation; GPU objects are cloned in at call time.
#[derive(Debug)]
enum Op {
    Upload {
        offset: u64,
        bytes_per_row: u32,
        texture: wgpu::Texture,
        origin: [u32; 2],
        size: [u32; 2],
    },
    Copy {
        src: wgpu::Texture,
        src_origin: [u32; 2],
        dst: wgpu::Texture,
        dst_origin: [u32; 2],
        size: [u32; 2],
    },
    Clear {
        view: wgpu::TextureView,
        rect: [u32; 4],
    },
}

/// Pending operations and their staged upload bytes.
#[derive(Debug, Default)]
pub(crate) struct PendingOps {
    ops: Vec<Op>,
    staging: Vec<u8>,
    gpu_staging: Option<wgpu::Buffer>,
}

impl PendingOps {
    /// Whether nothing is pending.
    pub(crate) fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Number of pending operations.
    pub(crate) fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the queue is full and must be flushed before another
    /// operation is added.
    pub(crate) fn is_full(&self) -> bool {
        self.ops.len() >= MAX_PENDING_OPS
    }

    /// Staging bytes held between flushes (CPU capacity plus the GPU
    /// staging buffer).
    pub(crate) fn retained_bytes(&self) -> u64 {
        self.staging.capacity() as u64 + self.gpu_staging.as_ref().map_or(0, wgpu::Buffer::size)
    }

    /// Bytes a band of `rows` rows of `row_bytes` bytes takes when staged.
    fn padded_row(row_bytes: usize) -> usize {
        row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
    }

    /// Rows of `row_bytes` that fit into one staging submission.
    pub(crate) fn rows_per_band(row_bytes: usize) -> usize {
        (STAGING_FLUSH_BYTES / Self::padded_row(row_bytes).max(1)).max(1)
    }

    /// Whether staging `rows` rows would pass the flush threshold.
    pub(crate) fn would_overflow(&self, row_bytes: usize, rows: usize) -> bool {
        !self.staging.is_empty()
            && self.staging.len() + Self::padded_row(row_bytes) * rows > STAGING_FLUSH_BYTES
    }

    /// Stage `rows` rows of `row_bytes` bytes each, read from `bytes` with
    /// `stride` (already bounds-checked by the caller), for an upload into
    /// `texture` at `origin`.
    pub(crate) fn stage_upload(
        &mut self,
        texture: &wgpu::Texture,
        origin: [u32; 2],
        size: [u32; 2],
        bytes: &[u8],
        stride: usize,
    ) {
        let row_bytes = size[0] as usize * 4;
        let padded = Self::padded_row(row_bytes);
        let offset = self.staging.len();
        self.staging.resize(offset + padded * size[1] as usize, 0);
        for row in 0..size[1] as usize {
            let src = row * stride;
            let dst = offset + row * padded;
            if let (Some(s), Some(d)) = (
                bytes.get(src..src + row_bytes),
                self.staging.get_mut(dst..dst + row_bytes),
            ) {
                d.copy_from_slice(s);
            }
        }
        self.ops.push(Op::Upload {
            offset: offset as u64,
            bytes_per_row: u32::try_from(padded).unwrap_or(u32::MAX),
            texture: texture.clone(),
            origin,
            size,
        });
    }

    pub(crate) fn push_copy(
        &mut self,
        src: &wgpu::Texture,
        src_origin: [u32; 2],
        dst: &wgpu::Texture,
        dst_origin: [u32; 2],
        size: [u32; 2],
    ) {
        self.ops.push(Op::Copy {
            src: src.clone(),
            src_origin,
            dst: dst.clone(),
            dst_origin,
            size,
        });
    }

    pub(crate) fn push_clear(&mut self, view: &wgpu::TextureView, rect: [u32; 4]) {
        self.ops.push(Op::Clear {
            view: view.clone(),
            rect,
        });
    }

    /// Submit every pending operation in call order.
    pub(crate) fn flush(
        &mut self,
        ctx: &GpuContext,
        clear_pipeline: &wgpu::RenderPipeline,
    ) -> Result<()> {
        if self.ops.is_empty() {
            return Ok(());
        }
        let device = &ctx.device;
        if !self.staging.is_empty() {
            let need = self.staging.len() as u64;
            let have = self.gpu_staging.as_ref().map_or(0, wgpu::Buffer::size);
            if have < need {
                let max = device.limits().max_buffer_size;
                let size = need.next_power_of_two().min(max);
                if size < need {
                    self.clear();
                    return Err(RenderError::LimitExceeded {
                        what: "UI staging bytes",
                        requested: need,
                        max,
                    });
                }
                let (buffer, gpu_error) = super::scoped(device, || {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("tessera-ui-staging"),
                        size,
                        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                });
                if let Some(e) = gpu_error {
                    // An invalid buffer still reports its size: never keep it,
                    // so the next flush allocates again.
                    buffer.destroy();
                    self.clear();
                    return Err(RenderError::Gpu(e));
                }
                self.gpu_staging = Some(buffer);
            }
            if let Some(buf) = &self.gpu_staging {
                ctx.queue.write_buffer(buf, 0, &self.staging);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tessera-ui-ops"),
        });
        let mut i = 0;
        while let Some(op) = self.ops.get(i) {
            i += 1;
            match op {
                Op::Upload {
                    offset,
                    bytes_per_row,
                    texture,
                    origin,
                    size,
                } => {
                    let Some(buffer) = &self.gpu_staging else {
                        continue;
                    };
                    encoder.copy_buffer_to_texture(
                        wgpu::TexelCopyBufferInfo {
                            buffer,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: *offset,
                                bytes_per_row: Some(*bytes_per_row),
                                rows_per_image: Some(size[1]),
                            },
                        },
                        texel(texture, *origin),
                        extent(*size),
                    );
                }
                Op::Copy {
                    src,
                    src_origin,
                    dst,
                    dst_origin,
                    size,
                } => {
                    encoder.copy_texture_to_texture(
                        texel(src, *src_origin),
                        texel(dst, *dst_origin),
                        extent(*size),
                    );
                }
                Op::Clear { view, rect } => {
                    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("tessera-ui-clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                // `Clear` would ignore the scissor.
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    rp.set_pipeline(clear_pipeline);
                    let [x, y, w, h] = *rect;
                    rp.set_scissor_rect(x, y, w, h);
                    rp.draw(0..3, 0..1);
                    // The clears of the same texture that follow share the pass
                    // (nothing else runs between them, so the order holds).
                    while let Some(Op::Clear { view: next, rect }) = self.ops.get(i) {
                        if next != view {
                            break;
                        }
                        let [x, y, w, h] = *rect;
                        rp.set_scissor_rect(x, y, w, h);
                        rp.draw(0..3, 0..1);
                        i += 1;
                    }
                }
            }
        }
        ctx.queue.submit(Some(encoder.finish()));
        self.clear();
        Ok(())
    }

    /// Drop the pending operations and release staging memory above
    /// [`STAGING_RETAIN_BYTES`] (after a submission, wgpu keeps a dropped
    /// buffer alive until the GPU is done with it).
    fn clear(&mut self) {
        self.ops.clear();
        self.staging.clear();
        if self.staging.capacity() > STAGING_RETAIN_BYTES {
            self.staging.shrink_to(STAGING_RETAIN_BYTES);
        }
        if self
            .gpu_staging
            .as_ref()
            .is_some_and(|b| b.size() > STAGING_RETAIN_BYTES as u64)
        {
            self.gpu_staging = None;
        }
    }
}

fn texel(texture: &wgpu::Texture, origin: [u32; 2]) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d {
            x: origin[0],
            y: origin[1],
            z: 0,
        },
        aspect: wgpu::TextureAspect::All,
    }
}

fn extent(size: [u32; 2]) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_and_padding() {
        assert_eq!(PendingOps::padded_row(4), 256);
        assert_eq!(PendingOps::padded_row(256), 256);
        assert_eq!(PendingOps::padded_row(260), 512);
        assert_eq!(
            PendingOps::rows_per_band(16_384 * 4),
            STAGING_FLUSH_BYTES / 65_536
        );
        assert!(PendingOps::rows_per_band(usize::MAX / 2) >= 1);
        let p = PendingOps::default();
        assert!(
            !p.would_overflow(65_536, 100_000),
            "empty staging never overflows"
        );
        assert!(!p.is_full());
    }

    #[test]
    fn large_staging_is_released_after_a_flush() {
        let mut p = PendingOps::default();
        // What a banded first paint of a 3840 x 2160 surface leaves behind.
        p.staging.resize(STAGING_FLUSH_BYTES - 1, 0);
        p.staging.reserve(STAGING_FLUSH_BYTES);
        assert!(p.retained_bytes() > 2 * STAGING_RETAIN_BYTES as u64);
        p.clear();
        assert!(p.staging.is_empty());
        assert!(p.retained_bytes() <= STAGING_RETAIN_BYTES as u64);
        // Small staging keeps its capacity (no churn at U3's per-frame cap).
        p.staging.resize(1 << 20, 0);
        let cap = p.staging.capacity();
        p.clear();
        assert_eq!(p.staging.capacity(), cap);
    }
}
