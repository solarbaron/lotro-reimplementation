// SPDX-License-Identifier: GPL-3.0-or-later
//! UI viewport targets: the render-to-texture path of a UI `Viewport` or `TargetViewport`
//! element (`docs/specs/ui-widgets.md` §12, §12.3; `docs/specs/render.md` §6.10).
//!
//! A [`ViewportTarget`] is an `Rgba8Unorm` offscreen target of the viewport's
//! render-to-texture size, registered as a texture so that the UI pass can compose it as
//! the element's external content ([`crate::UiEntry`] with [`crate::UiSource::Premultiplied`]).
//! One drawing of it is a frame of its own, submitted before the frame whose UI pass
//! composes it (`render.md` §6.10 "Timing"):
//!
//! 1. [`Renderer::begin_viewport_frame`] starts the frame with a pass that clears the
//!    target;
//! 2. the caller draws the scene into it, in passes with the premultiplied option
//!    ([`crate::Pass::premultiplied_alpha`], which clears to transparent black), for example
//!    the figure through the shared figure path;
//! 3. [`Renderer::end_viewport_frame`] applies [`ViewportFinish`]: every pass of the frame
//!    that clears the colour target clears it to the background colour (premultiplied)
//!    instead, so the scene is drawn onto a target cleared to the background
//!    (`ui-widgets.md` §12.3 step 1) whatever blend its materials use; then a finishing
//!    screen pass applies the opacity mask, which multiplies every premultiplied pixel by the
//!    mask's alpha stretched over the target (§12.3 step 2, Tessera rule); and the frame is
//!    submitted.
//!
//! The target keeps its pixels between drawings, so a caller draws it again only when what
//! it shows changes (its figure, camera, size, background or mask); the UI pass composes
//! the last drawing every frame.

use crate::error::{RenderError, Result};
use crate::frame::{Frame, PassSpace, Rgba};
use crate::renderer::{FrameStats, OffscreenFormat, OffscreenTarget, Renderer, RgbaImage};
use crate::texture::{Filter, TextureId, Wrap};

/// The largest side of a viewport target, pixels (Tessera bound: the render-to-texture
/// sizes of `ui-widgets.md` §12 are 1–256, and a layout box larger than this is refused
/// rather than allocated).
pub const MAX_VIEWPORT_SIDE: u32 = 2048;

/// A UI viewport's render target (module docs).
#[derive(Debug)]
pub struct ViewportTarget {
    target: OffscreenTarget,
    texture: TextureId,
}

impl ViewportTarget {
    /// Size in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.target.size()
    }

    /// The registered texture the UI pass composes (external content).
    #[must_use]
    pub fn texture(&self) -> TextureId {
        self.texture
    }

    /// The offscreen target (for [`Renderer::read_pixels`] in tests and tools).
    #[must_use]
    pub fn target(&self) -> &OffscreenTarget {
        &self.target
    }
}

/// The finishing steps of one drawing of a [`ViewportTarget`] (`ui-widgets.md` §12.3).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewportFinish {
    /// The background colour (straight linear RGBA; `RenderToTextureBackgroundColor`;
    /// each channel clamped to 0–1, a non-finite one taken as 0) the target is cleared to
    /// before the scene; `None` clears to transparent black.
    pub background: Option<Rgba>,
    /// The opacity mask: a texture whose alpha multiplies the target, stretched over it.
    /// A mask without an alpha channel is uploaded with its red channel as the alpha.
    pub mask: Option<TextureId>,
}

/// The clear colour of a straight linear RGBA background, premultiplied (each channel
/// clamped to 0–1, a non-finite one taken as 0).
fn premultiplied_clear(color: Rgba) -> wgpu::Color {
    let c = color.map(|v| {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    let a = f64::from(c[3]);
    wgpu::Color {
        r: f64::from(c[0]) * a,
        g: f64::from(c[1]) * a,
        b: f64::from(c[2]) * a,
        a,
    }
}

impl Renderer<'_> {
    /// Creates a viewport target of `width` × `height` pixels, registered as a texture
    /// sampled with `filter` (linear when the target's size differs from the displayed
    /// box, `ui-widgets.md` §12.3 step 3) and clamped at its edges.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidTarget`] for a zero side or one above [`MAX_VIEWPORT_SIDE`] or
    /// the device limit; the errors of [`Renderer::create_offscreen_target`] and
    /// [`Renderer::register_target_texture`].
    pub fn create_viewport_target(
        &mut self,
        width: u32,
        height: u32,
        filter: Filter,
    ) -> Result<ViewportTarget> {
        if width == 0 || height == 0 || width > MAX_VIEWPORT_SIDE || height > MAX_VIEWPORT_SIDE {
            return Err(RenderError::InvalidTarget(format!(
                "viewport {width}x{height} (limit {MAX_VIEWPORT_SIDE})"
            )));
        }
        let target = self.create_offscreen_target(width, height, OffscreenFormat::Rgba8Unorm)?;
        let texture = self.register_target_texture(&target, filter, Wrap::Clamp)?;
        Ok(ViewportTarget { target, texture })
    }

    /// Releases a viewport target and its texture. Returns whether the texture was live.
    pub fn destroy_viewport_target(&mut self, target: ViewportTarget) -> bool {
        self.destroy_texture(target.texture)
    }

    /// Starts one drawing of `target` (module docs, step 1): a frame whose first pass
    /// clears the target (to the background of [`Renderer::end_viewport_frame`]'s
    /// [`ViewportFinish`], transparent black without one).
    ///
    /// # Errors
    ///
    /// As [`Renderer::begin_offscreen_frame`].
    pub fn begin_viewport_frame(&mut self, target: &ViewportTarget) -> Result<Frame> {
        let mut frame = self.begin_offscreen_frame(&target.target)?;
        frame
            .pass("viewport-clear", PassSpace::Screen)
            .premultiplied_alpha();
        Ok(frame)
    }

    /// Ends one drawing (module docs, step 3): applies `finish` (the background as the
    /// clear colour of every pass that clears, the opacity mask as a finishing pass) and
    /// submits the frame.
    ///
    /// # Errors
    ///
    /// As [`Renderer::end_frame`].
    pub fn end_viewport_frame(
        &mut self,
        mut frame: Frame,
        finish: &ViewportFinish,
    ) -> Result<FrameStats> {
        if let Some(c) = finish.background {
            let bg = premultiplied_clear(c);
            for pass in &mut frame.passes {
                if pass.clear_color.is_some() {
                    pass.clear_color = Some(bg);
                }
            }
        }
        if let Some(m) = finish.mask {
            frame
                .pass("viewport-finish", PassSpace::Screen)
                .opacity_mask(m);
        }
        self.end_frame(frame)
    }

    /// Reads a viewport target back (tests and tools).
    ///
    /// # Errors
    ///
    /// As [`Renderer::read_pixels`].
    pub fn read_viewport_pixels(&self, target: &ViewportTarget) -> Result<RgbaImage> {
        self.read_pixels(&target.target)
    }
}

#[cfg(test)]
mod tests {
    use super::premultiplied_clear;

    #[test]
    fn the_background_clear_is_premultiplied_and_clamped() {
        let c = premultiplied_clear([0.8, 0.2, 2.0, 0.5]);
        assert!((c.r - 0.4).abs() < 1e-6 && (c.g - 0.1).abs() < 1e-6);
        assert!((c.b - 0.5).abs() < 1e-6 && (c.a - 0.5).abs() < 1e-6);
        let c = premultiplied_clear([f32::NAN, 1.0, 1.0, f32::INFINITY]);
        assert_eq!((c.r, c.g, c.b, c.a), (0.0, 0.0, 0.0, 0.0));
        let c = premultiplied_clear([0.0, 0.0, 0.0, 1.0]);
        assert_eq!((c.r, c.g, c.b, c.a), (0.0, 0.0, 0.0, 1.0));
    }
}
