// SPDX-License-Identifier: GPL-3.0-or-later
//! Public UI entry points on [`Renderer`], [`Frame`] and [`Pass`]
//! (render.md §6.2, §6.3, §6.4, §6.9–§6.11).

use super::state::UI_TEXTURE_FORMAT;
use super::{MAX_UI_TEXTURE_DIM, UiPass, UiViewMode};
use crate::context::GpuContext;
use crate::error::{RenderError, Result};
use crate::frame::{Frame, Pass, PassSpace};
use crate::renderer::Renderer;
use crate::texture::{Filter, GpuTexture, Samplers, TextureId, TextureStore, Wrap};

/// Mutable borrows of the renderer parts the UI code works with.
#[derive(Debug)]
pub(crate) struct UiParts<'a> {
    pub ctx: &'a GpuContext,
    pub textures: &'a mut TextureStore,
    pub texture_layout: &'a wgpu::BindGroupLayout,
    pub samplers: &'a Samplers,
    pub ui: &'a mut super::UiState,
}

fn invalid(msg: impl Into<String>) -> RenderError {
    RenderError::InvalidTexture(msg.into())
}

/// `x + w ≤ limit` without overflow.
fn fits(x: u32, w: u32, limit: u32) -> bool {
    u64::from(x) + u64::from(w) <= u64::from(limit)
}

/// A texture that sub-rectangle operations may write.
fn writable(textures: &TextureStore, id: TextureId) -> Result<&GpuTexture> {
    let t = textures
        .get(id)
        .ok_or_else(|| invalid(format!("unknown, destroyed or foreign texture {id:?}")))?;
    if t.target_view.is_some() {
        return Err(invalid(
            "render-target textures are written by rendering, not uploads",
        ));
    }
    if t.texture.mip_level_count() != 1 {
        return Err(invalid("texture has more than one level"));
    }
    Ok(t)
}

/// A UI texture (copies and clears work on UI textures only).
fn ui_texture(textures: &TextureStore, id: TextureId) -> Result<&GpuTexture> {
    let t = writable(textures, id)?;
    if t.texture.format() != UI_TEXTURE_FORMAT {
        return Err(invalid(format!("{id:?} is not a UI texture")));
    }
    Ok(t)
}

/// Submit the pending operations when the queue is full
/// ([`super::ops::MAX_PENDING_OPS`]).
fn make_room(p: &mut UiParts<'_>) -> Result<()> {
    if p.ui.ops.is_full() {
        tracing::debug!(
            pending = p.ui.ops.len(),
            "UI operation queue full: submitting it before the next frame"
        );
        p.ui.ops.flush(p.ctx, &p.ui.clear_pipeline)?;
    }
    Ok(())
}

fn check_rect(t: &GpuTexture, x: u32, y: u32, w: u32, h: u32) -> Result<()> {
    if fits(x, w, t.width) && fits(y, h, t.height) {
        Ok(())
    } else {
        Err(invalid(format!(
            "rectangle {w}x{h} at ({x}, {y}) outside the {}x{} texture",
            t.width, t.height
        )))
    }
}

impl Renderer<'_> {
    /// Create a **UI texture** (render.md §6.2): `Bgra8Unorm`, linear, one
    /// level, transparent black, read by the UI pass with `textureLoad` and
    /// written with [`Self::update_texture_rect`],
    /// [`Self::copy_texture_rect`] and [`Self::clear_texture_rect`]. A canvas
    /// pixel 0xAARRGGBB is uploaded as its little-endian bytes B, G, R, A,
    /// with straight alpha. [`Self::destroy_texture`] releases it.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTexture`] when a side is 0 or larger than
    /// `min(max_texture_dimension_2d, 16,384)`; [`RenderError::Gpu`] when the
    /// GPU refuses the allocation.
    pub fn create_ui_texture(
        &mut self,
        width: u32,
        height: u32,
        label: Option<&str>,
    ) -> Result<TextureId> {
        let p = self.ui_parts();
        let max = p
            .ctx
            .device
            .limits()
            .max_texture_dimension_2d
            .min(MAX_UI_TEXTURE_DIM);
        if width == 0 || height == 0 || width > max || height > max {
            return Err(invalid(format!(
                "UI texture {width}x{height} outside 1..={max}"
            )));
        }
        let device = &p.ctx.device;
        let (texture, gpu_error) = super::scoped(device, || {
            device.create_texture(&wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: UI_TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        if let Some(e) = gpu_error {
            texture.destroy();
            return Err(RenderError::Gpu(e));
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = p.samplers.get(Filter::Nearest, Wrap::Clamp).clone();
        // Lets ordinary quads draw the texture too (debug views).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: p.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let id = p.textures.insert(GpuTexture {
            texture,
            bind_group,
            view,
            sampler,
            width,
            height,
            mip_levels: 1,
            srgb: UI_TEXTURE_FORMAT.is_srgb(),
            target_view: None,
        })?;
        let bytes = u64::from(width) * u64::from(height) * 4;
        p.ui.textures.insert(id, bytes);
        p.ui.texture_bytes = p.ui.texture_bytes.saturating_add(bytes);
        tracing::trace!(?id, width, height, "created UI texture");
        Ok(id)
    }

    /// Write a `w` × `h` rectangle at (`x`, `y`) of a single-level texture
    /// with 4 bytes per texel (render.md §6.3): B, G, R, A for UI textures,
    /// R, G, B, A for textures from [`Self::upload_texture`]. Row *r* of the
    /// rectangle starts at `bytes[r * row_stride]`. The bytes are copied at
    /// the call. The write takes effect on the GPU in call order with
    /// [`Self::copy_texture_rect`] and [`Self::clear_texture_rect`], before
    /// the next frame reads the texture. A rectangle with `w` = 0 or `h` = 0
    /// does nothing.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTexture`] when the texture is unknown,
    /// destroyed, foreign or a registered target; when the rectangle leaves
    /// the texture; when `row_stride` < 4 `w`; or when `bytes` is shorter
    /// than `row_stride` × (`h` − 1) + 4 `w`. Nothing is written then.
    #[allow(clippy::too_many_arguments)]
    pub fn update_texture_rect(
        &mut self,
        id: TextureId,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        bytes: &[u8],
        row_stride: usize,
    ) -> Result<()> {
        let p = self.ui_parts();
        let t = writable(p.textures, id)?;
        if w == 0 || h == 0 {
            return Ok(());
        }
        check_rect(t, x, y, w, h)?;
        let row_bytes = w as usize * 4;
        if row_stride < row_bytes {
            return Err(invalid(format!(
                "row stride {row_stride} below {row_bytes}"
            )));
        }
        let need = row_stride
            .checked_mul(h as usize - 1)
            .and_then(|n| n.checked_add(row_bytes))
            .ok_or_else(|| invalid("rectangle size overflow"))?;
        if bytes.len() < need {
            return Err(invalid(format!(
                "{w}x{h} rectangle with stride {row_stride} needs {need} bytes, got {}",
                bytes.len()
            )));
        }
        let texture = t.texture.clone();
        let band = super::ops::PendingOps::rows_per_band(row_bytes);
        let mut row = 0usize;
        while row < h as usize {
            let rows = band.min(h as usize - row);
            if p.ui.ops.would_overflow(row_bytes, rows) || p.ui.ops.is_full() {
                p.ui.ops.flush(p.ctx, &p.ui.clear_pipeline)?;
            }
            let start = row * row_stride;
            p.ui.ops.stage_upload(
                &texture,
                [x, y + row as u32],
                [w, rows as u32],
                bytes.get(start..).unwrap_or(&[]),
                row_stride,
            );
            row += rows;
        }
        let s = &mut p.ui.op_stats;
        s.upload_bytes = s
            .upload_bytes
            .saturating_add(4 * u64::from(w) * u64::from(h));
        s.upload_rects = s.upload_rects.saturating_add(1);
        Ok(())
    }

    /// GPU copy of a `w` × `h` rectangle from (`sx`, `sy`) of UI texture
    /// `src` to (`dx`, `dy`) of a **different** UI texture `dst` (front to
    /// back, growing or shrinking a tile box; render.md §6.3). In call order
    /// with the other UI texture operations.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTexture`] when either texture is not a live UI
    /// texture of this renderer, when `src == dst`, or when a rectangle
    /// leaves its texture. A zero-size rectangle does nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_texture_rect(
        &mut self,
        src: TextureId,
        sx: u32,
        sy: u32,
        dst: TextureId,
        dx: u32,
        dy: u32,
        w: u32,
        h: u32,
    ) -> Result<()> {
        let mut p = self.ui_parts();
        if src == dst {
            return Err(invalid("copy within one texture"));
        }
        let s = ui_texture(p.textures, src)?;
        let d = ui_texture(p.textures, dst)?;
        if w == 0 || h == 0 {
            return Ok(());
        }
        check_rect(s, sx, sy, w, h)?;
        check_rect(d, dx, dy, w, h)?;
        let (s, d) = (s.texture.clone(), d.texture.clone());
        make_room(&mut p)?;
        p.ui.ops.push_copy(&s, [sx, sy], &d, [dx, dy], [w, h]);
        let st = &mut p.ui.op_stats;
        st.gpu_copies = st.gpu_copies.saturating_add(1);
        st.copy_bytes = st
            .copy_bytes
            .saturating_add(4 * u64::from(w) * u64::from(h));
        Ok(())
    }

    /// Set a rectangle of a UI texture to transparent black (a freed tile;
    /// render.md §6.3). It is a draw under a scissor equal to the rectangle,
    /// so no texel outside it changes. In call order with the other UI
    /// texture operations.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTexture`] when the texture is not a live UI
    /// texture of this renderer or the rectangle leaves it. A zero-size
    /// rectangle does nothing.
    pub fn clear_texture_rect(
        &mut self,
        id: TextureId,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<()> {
        let mut p = self.ui_parts();
        let t = ui_texture(p.textures, id)?;
        if w == 0 || h == 0 {
            return Ok(());
        }
        check_rect(t, x, y, w, h)?;
        let view = t.view.clone();
        make_room(&mut p)?;
        p.ui.ops.push_clear(&view, [x, y, w, h]);
        p.ui.op_stats.clears = p.ui.op_stats.clears.saturating_add(1);
        Ok(())
    }

    /// Submit the pending UI texture operations now (they are otherwise
    /// submitted by the next [`Self::end_frame`], before the frame's own
    /// commands, or as soon as 4,096 are pending or 32 MiB of upload bytes
    /// are staged). A client that keeps painting while it ends no frame (a
    /// minimised window) may call it to spread the work.
    ///
    /// # Errors
    ///
    /// [`RenderError::LimitExceeded`] or [`RenderError::Gpu`] when the
    /// staging buffer cannot be allocated (the pending operations are
    /// dropped; the next flush allocates again).
    pub fn flush_ui_ops(&mut self) -> Result<()> {
        let p = self.ui_parts();
        p.ui.ops.flush(p.ctx, &p.ui.clear_pipeline)
    }

    /// UI texture operations (uploads, copies, clears) waiting for the next
    /// submission; never more than 4,096.
    #[must_use]
    pub fn ui_pending_ops(&self) -> usize {
        self.ui_ref().0.ops.len()
    }

    /// Upload staging memory the UI keeps between submissions, CPU and GPU
    /// together (at most 8 MiB each once a submission has run; not part of
    /// [`Self::ui_texture_bytes`]).
    #[must_use]
    pub fn ui_staging_bytes(&self) -> u64 {
        self.ui_ref().0.ops.retained_bytes()
    }

    /// Sum of width × height × 4 over live UI textures (U5's GPU budget).
    #[must_use]
    pub fn ui_texture_bytes(&self) -> u64 {
        self.ui_ref().0.texture_bytes
    }

    /// Whether `id` is a live UI texture of this renderer.
    #[must_use]
    pub fn is_ui_texture(&self, id: TextureId) -> bool {
        self.ui_ref().0.textures.contains_key(&id)
    }

    /// How the UI pass reaches stored values on this renderer's main target
    /// (the surface, or offscreen `Rgba8Srgb` targets when headless;
    /// render.md §6.4).
    #[must_use]
    pub fn ui_view_mode(&self) -> UiViewMode {
        let (ui, surface_format) = self.ui_ref();
        let flags = self.context().adapter.get_downlevel_capabilities().flags;
        let (format, has_view) = match surface_format {
            Some(f) => (
                f,
                flags.contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS),
            ),
            None => (
                wgpu::TextureFormat::Rgba8UnormSrgb,
                flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS),
            ),
        };
        ui.view_mode(format, has_view)
    }

    /// Force the linear-light fallback of render.md §6.4 even where a
    /// non-sRGB view exists (for tests and comparisons).
    pub fn set_ui_linear_fallback(&mut self, on: bool) {
        self.ui_parts().ui.force_fallback = on;
    }
}

impl Frame {
    /// Append a UI pass (render.md §6.9). It is drawn after the ordinary
    /// passes recorded before this call and before those recorded after it,
    /// into the non-sRGB view of the target. With no pass recorded yet, an
    /// empty screen pass that clears to the renderer's clear colour is
    /// recorded first.
    pub fn ui_pass(&mut self) -> &mut UiPass {
        if self.passes.is_empty() {
            self.passes
                .push(Pass::new("clear".into(), PassSpace::Screen));
        }
        let position = self.passes.len();
        // Earlier UI passes can no longer grow, so their entries count
        // against this pass's share of the frame cap.
        let earlier = self
            .ui
            .passes
            .last()
            .map_or(0, |(_, p)| p.earlier.saturating_add(p.len()));
        self.ui.passes.push((
            position,
            UiPass {
                earlier,
                ..UiPass::default()
            },
        ));
        let last = self.ui.passes.len() - 1;
        &mut self.ui.passes[last].1
    }

    /// Draw the software cursor (render.md §6.11): `texture` (a UI texture,
    /// already at its final size; any other texture is not drawn) at 1 : 1 with its top-left corner at
    /// `position` (the pointer minus the scaled hot spot, physical pixels),
    /// opacity 1, no clip and no threshold, as the **last** draw of the
    /// frame, whenever this is called. It is not a composition entry. Do not
    /// call it while the pointer lock is on, while the pointer is outside the
    /// window, or for a screenshot.
    pub fn software_cursor(&mut self, texture: TextureId, position: [i32; 2]) -> &mut Self {
        self.ui.cursor = Some((texture, position));
        self
    }

    /// Remove the software cursor set for this frame.
    pub fn clear_software_cursor(&mut self) -> &mut Self {
        self.ui.cursor = None;
        self
    }

    /// The UI passes recorded so far.
    pub fn ui_passes(&self) -> impl Iterator<Item = &UiPass> {
        self.ui.passes.iter().map(|(_, p)| p)
    }
}

impl Pass {
    /// Prepare this pass for drawing **external content** of the UI
    /// (render.md §6.10): it clears the target to transparent black, so
    /// opaque draws (alpha 1) and source-over draws (whose alpha blends with
    /// `One`/`OneMinusSrcAlpha`) leave premultiplied colour, and empty pixels
    /// stay transparent. Draw into an `OffscreenFormat::Rgba8Unorm` target
    /// and compose it with [`super::UiSource::Premultiplied`].
    ///
    /// Every built-in pipeline blends alpha that way (or not at all, writing
    /// alpha 1), except the fixed blend states of the lit, instanced and sky
    /// pipelines (`FixedBlend`: additive, modulate, min, max), which keep the
    /// destination alpha (render.md §6.10 OPEN); the option only sets the
    /// clear. A unit test of this module fails when a pipeline with another
    /// blend state is added, so that this option is extended before such a
    /// pipeline can draw a portrait.
    pub fn premultiplied_alpha(&mut self) -> &mut Self {
        self.clear([0.0; 4])
    }
}

/// Run `f` inside wgpu error scopes and return the first GPU error it
/// caused.
pub(crate) fn scoped<T>(device: &wgpu::Device, f: impl FnOnce() -> T) -> (T, Option<String>) {
    let oom = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let out = f();
    let v = pollster::block_on(validation.pop());
    let o = pollster::block_on(oom.pop());
    (out, v.or(o).map(|e| e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The blend states `Pass::premultiplied_alpha` relies on (render.md §6.10):
    /// alpha blended with `One`/`OneMinusSrcAlpha`.
    const PREMULTIPLYING: [&str; 2] = ["ALPHA_BLENDING", "PREMULTIPLIED_ALPHA_BLENDING"];

    fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn built_in_blend_states_keep_external_content_premultiplied() {
        assert_eq!(
            wgpu::BlendState::ALPHA_BLENDING.alpha,
            wgpu::BlendComponent::OVER
        );
        assert_eq!(
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.alpha,
            wgpu::BlendComponent::OVER
        );
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let ui = src.join("ui");
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        assert!(files.len() > 5, "sources found under {}", src.display());
        let mut bad = Vec::new();
        for path in files.iter().filter(|p| !p.starts_with(&ui)) {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            // The one known exception (render.md §6.10 OPEN): `FixedBlend::blend_state`
            // in material.rs, which keeps the destination alpha.
            let fixed_blend = path.ends_with("material.rs");
            let mut in_fixed = false;
            for (n, line) in text.lines().enumerate() {
                if fixed_blend
                    && line.contains("pub(crate) fn blend_state(self) -> wgpu::BlendState")
                {
                    in_fixed = true;
                }
                if in_fixed {
                    if line == "    }" {
                        in_fixed = false;
                    }
                    continue;
                }
                let code = line.split("//").next().unwrap_or("");
                let mut rest = code;
                while let Some(at) = rest.find("BlendState") {
                    let after = &rest[at + "BlendState".len()..];
                    let name: String = after
                        .strip_prefix("::")
                        .unwrap_or("")
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !PREMULTIPLYING.contains(&name.as_str()) {
                        bad.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                    }
                    rest = after;
                }
                if code.contains("BlendComponent") {
                    bad.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "pipelines with a blend state other than ALPHA_BLENDING: extend \
             Pass::premultiplied_alpha (render.md §6.10) so that a portrait pass \
             blends alpha with One/OneMinusSrcAlpha, then allow them here:\n{}",
            bad.join("\n")
        );
    }
}
