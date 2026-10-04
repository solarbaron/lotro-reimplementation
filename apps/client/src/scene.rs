// SPDX-License-Identifier: GPL-3.0-or-later
//! The M0 test scene: a ground grid, an upright textured quad showing either a surface
//! decoded from the user's archives or an original procedural checkerboard, the sandbox
//! entities as boxes, and a screen-space thumbnail.

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Filter, Frame, OrbitCamera, OrbitConfig, PassSpace, Quad, Renderer, Rgba, TextureId,
    TextureUpload, UpAxis, Wrap,
};

use crate::data::DecodedTexture;
use crate::net::EntityView;

/// Background colour (linear).
pub const CLEAR_COLOR: Rgba = [0.02, 0.03, 0.05, 1.0];
/// Height of the showcase quad in metres.
pub const QUAD_HEIGHT: f32 = 3.0;
/// Colour of the opaque panel behind the showcase quad (shows through transparent texels).
pub const BACKING_COLOR: Rgba = [0.09, 0.08, 0.07, 1.0];
/// Border of the backing panel around the quad, in metres.
pub const BACKING_BORDER: f32 = 0.08;
/// Edge of the screen-space thumbnail in pixels.
pub const THUMBNAIL: f32 = 128.0;
/// Edge of the procedural checkerboard in texels.
pub const CHECKER_SIZE: u32 = 64;

/// A texture shown by the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// GPU handle.
    pub id: TextureId,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Source surface DID, `None` for the procedural texture.
    pub did: Option<u32>,
}

/// Scene state that outlives frames.
#[derive(Debug)]
pub struct Scene {
    /// The orbit camera.
    pub camera: OrbitCamera,
    checker: Shown,
    surface: Option<Shown>,
}

/// An original two-tone checkerboard (RGBA8), `size × size` with `cell`-texel squares.
#[must_use]
pub fn checkerboard(size: u32, cell: u32) -> Vec<u8> {
    let cell = cell.max(1);
    let mut out = Vec::with_capacity((size as usize) * (size as usize) * 4);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / cell) + (y / cell)).is_multiple_of(2);
            out.extend_from_slice(if on {
                &[0xd8, 0xa0, 0x3c, 0xff]
            } else {
                &[0x2a, 0x4a, 0x6e, 0xff]
            });
        }
    }
    out
}

/// Ground-grid half extent in cells (1 m each). Kept short of the horizon: with the
/// level default camera, a 20-cell grid packs dozens of 1-px lines into the last few rows
/// above the horizon, where line rasterisation is implementation-defined; Mesa lavapipe
/// (the CI golden-image reference) then fills a solid block there that hardware drivers
/// do not draw, and the golden comparison fails.
pub const GRID_HALF_CELLS: u32 = 10;

/// The default camera: Z-up (the world model's convention), looking at the quad centre.
#[must_use]
pub fn default_camera() -> OrbitCamera {
    OrbitCamera::new(OrbitConfig {
        up: UpAxis::Z,
        pivot_height: QUAD_HEIGHT * 0.5,
        distance: 6.0,
        min_distance: 1.0,
        max_distance: 60.0,
        pitch: 0.0,
        ..OrbitConfig::default()
    })
}

impl Scene {
    /// Uploads the procedural texture.
    pub fn new(renderer: &mut Renderer<'_>) -> tessera_render::Result<Self> {
        let rgba = checkerboard(CHECKER_SIZE, 8);
        let id = renderer.upload_texture(&TextureUpload {
            label: Some("tessera-checker"),
            filter: Filter::Nearest,
            ..TextureUpload::rgba8(CHECKER_SIZE, CHECKER_SIZE, &rgba)
        })?;
        Ok(Self {
            camera: default_camera(),
            checker: Shown {
                id,
                width: CHECKER_SIZE,
                height: CHECKER_SIZE,
                did: None,
            },
            surface: None,
        })
    }

    /// The texture currently on the quad.
    #[must_use]
    pub fn shown(&self) -> Shown {
        self.surface.unwrap_or(self.checker)
    }

    /// Upload a decoded surface and show it, replacing any previous one.
    pub fn set_surface(
        &mut self,
        renderer: &mut Renderer<'_>,
        tex: &DecodedTexture,
    ) -> tessera_render::Result<()> {
        let label = format!("surface-{:#010x}", tex.did);
        let id = renderer.upload_texture(&TextureUpload {
            label: Some(&label),
            wrap: Wrap::Clamp,
            ..TextureUpload::rgba8(tex.width, tex.height, &tex.rgba)
        })?;
        if let Some(old) = self.surface.replace(Shown {
            id,
            width: tex.width,
            height: tex.height,
            did: Some(tex.did),
        }) {
            renderer.destroy_texture(old.id);
        }
        Ok(())
    }

    /// Model matrix of the upright quad (local XY plane rotated to stand on the ground,
    /// facing the default camera on the -Y side).
    #[must_use]
    pub fn quad_transform(&self) -> Mat4 {
        self.upright(0.0, 0.0)
    }

    /// Model matrix of the opaque panel just behind the quad.
    #[must_use]
    pub fn backing_transform(&self) -> Mat4 {
        self.upright(0.02, BACKING_BORDER * 2.0)
    }

    fn upright(&self, behind: f32, grow: f32) -> Mat4 {
        let s = self.shown();
        let aspect = if s.height == 0 {
            1.0
        } else {
            (s.width as f32 / s.height as f32).clamp(0.25, 4.0)
        };
        Mat4::from_translation(Vec3::new(0.0, behind, QUAD_HEIGHT * 0.5))
            * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
            * Mat4::from_scale(Vec3::new(
                QUAD_HEIGHT * aspect + grow,
                QUAD_HEIGHT + grow,
                1.0,
            ))
    }

    /// Record the scene's passes into `frame`.
    pub fn draw<'e>(
        &self,
        frame: &mut Frame,
        entities: impl IntoIterator<Item = (bool, &'e EntityView)>,
    ) {
        let shown = self.shown();
        let camera = self.camera.camera();
        let world = frame.world_pass("world", &camera);
        world
            .clear(CLEAR_COLOR)
            .grid(
                Vec3::ZERO,
                Vec3::X,
                Vec3::Y,
                1.0,
                GRID_HALF_CELLS,
                [0.18, 0.2, 0.24, 1.0],
            )
            .axes(Vec3::new(0.0, 0.0, 0.01), 1.0)
            .quad(Quad::new(self.backing_transform()).with_color(BACKING_COLOR))
            .quad(Quad::new(self.quad_transform()).with_texture(shown.id));
        for (is_player, e) in entities {
            let p = e.transform.position;
            if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                continue;
            }
            let base = Vec3::new(p.x, p.y, p.z);
            let color = if is_player {
                [1.0, 0.85, 0.2, 1.0]
            } else {
                [0.3, 0.9, 0.5, 1.0]
            };
            world.aabb(
                base - Vec3::new(0.25, 0.25, 0.0),
                base + Vec3::new(0.25, 0.25, 1.8),
                color,
            );
        }

        let (fw, _) = frame.size();
        let (tw, th) = thumbnail_size(shown.width, shown.height);
        let x = fw as f32 - tw - 12.0;
        frame
            .pass("overlay", PassSpace::Screen)
            .quad(
                Quad::screen_rect(x - 2.0, 10.0, tw + 4.0, th + 4.0)
                    .with_color([0.0, 0.0, 0.0, 1.0]),
            )
            .quad(Quad::screen_rect(x, 12.0, tw, th).with_texture(shown.id));
    }
}

/// Thumbnail size keeping the aspect ratio inside a [`THUMBNAIL`] square.
#[must_use]
pub fn thumbnail_size(width: u32, height: u32) -> (f32, f32) {
    if width == 0 || height == 0 {
        return (THUMBNAIL, THUMBNAIL);
    }
    let (w, h) = (width as f32, height as f32);
    if w >= h {
        (THUMBNAIL, (THUMBNAIL * h / w).max(1.0))
    } else {
        ((THUMBNAIL * w / h).max(1.0), THUMBNAIL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkerboard_layout() {
        let c = checkerboard(16, 8);
        assert_eq!(c.len(), 16 * 16 * 4);
        assert_eq!(&c[0..4], &[0xd8, 0xa0, 0x3c, 0xff]);
        let px = |x: usize, y: usize| &c[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
        assert_eq!(px(8, 0), &[0x2a, 0x4a, 0x6e, 0xff]);
        assert_eq!(px(8, 8), px(0, 0));
    }

    #[test]
    fn thumbnail_keeps_aspect() {
        assert_eq!(thumbnail_size(256, 128), (128.0, 64.0));
        assert_eq!(thumbnail_size(64, 256), (32.0, 128.0));
        assert_eq!(thumbnail_size(0, 5), (128.0, 128.0));
    }

    #[test]
    fn camera_looks_at_quad_centre() {
        let cam = default_camera();
        let p = cam.pivot();
        assert!((p.z - QUAD_HEIGHT * 0.5).abs() < 1e-5);
        assert!(cam.eye().y < 0.0, "camera starts on the -Y side");
    }
}
