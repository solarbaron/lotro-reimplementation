// SPDX-License-Identifier: GPL-3.0-or-later
//! On-screen text for the walker: lines rasterised on the CPU with an 8 × 8 bitmap font
//! (the public-domain glyphs of the `font8x8` crate: Basic Latin and Latin-1) into one
//! RGBA texture, re-rendered only when the text changes and drawn as a screen quad.
//!
//! Text from the data (area names) is hostile: lines are sanitised and capped
//! ([`super::parse::sanitize_line`]), characters without a glyph become `?`, and the
//! canvas size is bounded.

use font8x8::legacy::{BASIC_LEGACY, LATIN_LEGACY};
use tessera_render::{Filter, Pass, Quad, Renderer, TextureId, TextureUpload, Wrap};

use super::parse::{MAX_HUD_LINE, sanitize_line};

/// Most lines in one panel.
pub const MAX_LINES: usize = 24;
/// Glyph cell edge in font pixels.
const CELL: u32 = 8;
/// Line pitch in font pixels (cell plus leading).
const PITCH: u32 = 10;
/// Panel padding in font pixels.
const PAD: u32 = 4;

/// The 8 × 8 bitmap of `c` (row 0 at the top, bit 0 = leftmost pixel).
#[must_use]
pub fn glyph(c: char) -> [u8; 8] {
    let u = c as u32;
    match u {
        0x20..=0x7E => BASIC_LEGACY[u as usize],
        0xA0..=0xFF => LATIN_LEGACY[(u - 0xA0) as usize],
        _ => BASIC_LEGACY[b'?' as usize],
    }
}

/// Most text columns of a panel that fits in `width` pixels at `scale` (1–4), at least 1
/// and at most [`MAX_HUD_LINE`] (`rasterize`'s width is `(cols · 8 + 2 · 4) · scale`).
#[must_use]
pub fn columns(width: u32, scale: u32) -> usize {
    let scale = scale.clamp(1, 4);
    let cols = (width / scale).saturating_sub(2 * PAD) / CELL;
    (cols as usize).clamp(1, MAX_HUD_LINE)
}

/// A rasterised panel: straight-alpha RGBA8, `width × height`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels, top row first.
    pub rgba: Vec<u8>,
}

/// Rasterises `lines` (text, sRGB colour) at integer `scale` (1–4) on a translucent dark
/// background. Lines are sanitised and capped at [`MAX_LINES`] × [`MAX_HUD_LINE`].
#[must_use]
pub fn rasterize(lines: &[(String, [u8; 3])], scale: u32) -> Panel {
    let scale = scale.clamp(1, 4);
    let lines: Vec<(Vec<char>, [u8; 3])> = lines
        .iter()
        .take(MAX_LINES)
        .map(|(t, c)| (sanitize_line(t).chars().collect(), *c))
        .collect();
    let cols = lines
        .iter()
        .map(|(t, _)| t.len())
        .max()
        .unwrap_or(0)
        .min(MAX_HUD_LINE) as u32;
    let rows = lines.len() as u32;
    let width = ((cols * CELL + 2 * PAD) * scale).max(1);
    let height = ((rows * PITCH + 2 * PAD) * scale).max(1);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..width * height {
        rgba.extend_from_slice(&[8, 10, 14, 170]);
    }
    for (row, (text, color)) in lines.iter().enumerate() {
        for (col, &c) in text.iter().enumerate() {
            let bits = glyph(c);
            let x0 = (PAD + col as u32 * CELL) * scale;
            let y0 = (PAD + row as u32 * PITCH) * scale;
            for (gy, byte) in bits.iter().enumerate() {
                for gx in 0..8u32 {
                    if byte & (1 << gx) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = x0 + gx * scale + sx;
                            let y = y0 + gy as u32 * scale + sy;
                            if x < width && y < height {
                                let i = ((y * width + x) * 4) as usize;
                                rgba[i..i + 4]
                                    .copy_from_slice(&[color[0], color[1], color[2], 255]);
                            }
                        }
                    }
                }
            }
        }
    }
    Panel {
        width,
        height,
        rgba,
    }
}

/// A text panel kept as a GPU texture.
#[derive(Debug, Default)]
pub struct TextPanel {
    texture: Option<(TextureId, u32, u32)>,
    content: Vec<(String, [u8; 3])>,
    /// Scale `content` was rasterised at (0: nothing rasterised).
    scale: u32,
}

impl TextPanel {
    /// Re-rasterises and uploads when `lines` or `scale` differ from what is shown.
    pub fn set(&mut self, renderer: &mut Renderer<'_>, lines: Vec<(String, [u8; 3])>, scale: u32) {
        if self.texture.is_some() && !self.needs_update(&lines, scale) {
            return;
        }
        let panel = rasterize(&lines, scale);
        self.content = lines;
        self.scale = scale;
        if let Some((id, w, h)) = self.texture
            && w == panel.width
            && h == panel.height
            && renderer.update_texture(id, &panel.rgba).is_ok()
        {
            return;
        }
        if let Some((id, _, _)) = self.texture.take() {
            renderer.destroy_texture(id);
        }
        match renderer.upload_texture(&TextureUpload {
            label: Some("walker-hud"),
            filter: Filter::Nearest,
            wrap: Wrap::Clamp,
            ..TextureUpload::rgba8(panel.width, panel.height, &panel.rgba)
        }) {
            Ok(id) => self.texture = Some((id, panel.width, panel.height)),
            Err(e) => tracing::warn!("HUD upload failed: {e}"),
        }
    }

    /// Whether `lines` at `scale` differ from what was last rasterised (then
    /// [`TextPanel::set`] re-rasterises; it also does when nothing is uploaded).
    #[must_use]
    pub fn needs_update(&self, lines: &[(String, [u8; 3])], scale: u32) -> bool {
        self.scale == 0 || lines != self.content.as_slice() || scale != self.scale
    }

    /// The panel's size in pixels, once uploaded.
    #[must_use]
    pub fn size(&self) -> Option<(u32, u32)> {
        self.texture.map(|(_, w, h)| (w, h))
    }

    /// Draws the panel with its top-left corner at (`x`, `y`).
    pub fn draw(&self, pass: &mut Pass, x: f32, y: f32) {
        if let Some((id, w, h)) = self.texture {
            pass.quad(Quad::screen_rect(x, y, w as f32, h as f32).with_texture(id));
        }
    }

    /// Frees the texture.
    pub fn release(&mut self, renderer: &mut Renderer<'_>) {
        if let Some((id, _, _)) = self.texture.take() {
            renderer.destroy_texture(id);
        }
        self.content.clear();
        self.scale = 0;
    }
}

/// The HUD's network line: backend, session state (and the protocol state of a remote
/// session), round-trip time, clock offset and interpolation delay.
#[must_use]
pub fn net_line(s: &crate::net::NetStatus) -> String {
    let mut t = format!("Net {} {:?}", s.backend, s.state);
    if let Some(link) = s.link {
        t.push_str(&format!(" ({link})"));
        match s.rtt {
            Some(r) => t.push_str(&format!("  rtt {:.1} ms", r.as_secs_f64() * 1e3)),
            None => t.push_str("  rtt -"),
        }
        match s.clock_offset_us {
            Some(o) => t.push_str(&format!("  clock {:+.1} ms", o as f64 / 1e3)),
            None => t.push_str("  clock -"),
        }
        if let Some(d) = s.delay {
            t.push_str(&format!("  interp {} ms", d.as_millis()));
        }
        t.push_str(&format!("  {} remote", s.entities));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_cover_latin1_and_fall_back() {
        assert_eq!(glyph(' '), [0; 8]);
        assert_ne!(glyph('A'), [0; 8]);
        assert_ne!(glyph('ú'), [0; 8]);
        assert_eq!(glyph('\u{4e00}'), glyph('?'));
        assert_eq!(glyph('\u{7f}'), glyph('?'));
    }

    #[test]
    fn net_line_shows_state_rtt_and_offset() {
        use std::time::Duration;
        let mut s = crate::net::NetStatus {
            backend: "sandbox".into(),
            state: tessera_net::SessionState::InWorld,
            link: None,
            rtt: None,
            clock_offset_us: None,
            delay: None,
            entities: 0,
        };
        assert_eq!(net_line(&s), "Net sandbox InWorld");
        s.backend = "tessera-server".into();
        s.link = Some("InWorld");
        s.rtt = Some(Duration::from_micros(12_340));
        s.clock_offset_us = Some(-2_500);
        s.delay = Some(Duration::from_millis(150));
        s.entities = 3;
        assert_eq!(
            net_line(&s),
            "Net tessera-server InWorld (InWorld)  rtt 12.3 ms  clock -2.5 ms  interp 150 ms  3 remote"
        );
        s.rtt = None;
        s.clock_offset_us = None;
        assert!(net_line(&s).contains("rtt -  clock -"));
    }

    #[test]
    fn columns_fit_the_width() {
        for (w, scale) in [(1910, 2), (1260, 1), (100, 1), (5000, 1), (0, 3)] {
            let cols = columns(w, scale);
            let panel = rasterize(&[("x".repeat(cols), [1, 1, 1])], scale);
            assert!(
                panel.width <= w.max((CELL + 2 * PAD) * scale),
                "{w} px at {scale}: {cols} cols"
            );
        }
        assert_eq!(columns(1910, 2), 118);
        assert_eq!(columns(1900, 2), 117);
        assert_eq!(columns(1260, 1), 156);
        assert_eq!(columns(5000, 1), MAX_HUD_LINE);
        assert_eq!(columns(0, 1), 1);
    }

    #[test]
    fn a_scale_change_re_rasterises() {
        let lines = vec![("hi".to_owned(), [1, 2, 3])];
        let mut p = TextPanel::default();
        assert!(p.needs_update(&lines, 1), "nothing rasterised yet");
        // As after `set` at scale 1 (no GPU in unit tests).
        p.content.clone_from(&lines);
        p.scale = 1;
        assert!(!p.needs_update(&lines, 1));
        assert!(
            p.needs_update(&lines, 2),
            "the window crossed the scale threshold"
        );
        assert!(p.needs_update(&[], 1));
    }

    #[test]
    fn rasterize_sizes_and_draws() {
        let p = rasterize(&[("AB".into(), [255, 0, 0]), ("C".into(), [0, 255, 0])], 2);
        assert_eq!(p.width, (2 * 8 + 8) * 2);
        assert_eq!(p.height, (2 * 10 + 8) * 2);
        assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        assert!(p.rgba.as_chunks::<4>().0.contains(&[255, 0, 0, 255]));
        assert!(p.rgba.as_chunks::<4>().0.contains(&[0, 255, 0, 255]));
        let empty = rasterize(&[], 9);
        assert_eq!((empty.width, empty.height), (32, 32));
        let many: Vec<(String, [u8; 3])> = (0..100).map(|_| ("x".repeat(500), [1, 2, 3])).collect();
        let big = rasterize(&many, 1);
        assert_eq!(big.height, MAX_LINES as u32 * 10 + 8);
        assert_eq!(big.width, MAX_HUD_LINE as u32 * 8 + 8);
    }
}
