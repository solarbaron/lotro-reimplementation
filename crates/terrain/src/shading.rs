// SPDX-License-Identifier: GPL-3.0-or-later
//! Height/slope shading: Tessera's own vertex-colour palette for terrain until the
//! surface-layer material table is decoded (`landblock.md` §3.8.2 OPEN (1); roadmap M3
//! "height/slope shading before that").
//!
//! A colour is interpolated from height bands and then blended towards a cliff colour as
//! the slope steepens. The palette is an original choice, not taken from any client.

/// Largest number of height bands kept (further stops are dropped).
pub const MAX_COLOR_STOPS: usize = 64;

/// One height band of a [`HeightSlopeShading`] palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    /// Height in world units at which `color` applies exactly.
    pub height: f32,
    /// Linear RGB.
    pub color: [f32; 3],
}

/// Height/slope vertex colouring.
///
/// The palette is sanitised on use: non-finite stops are skipped, stops are taken in
/// height order, colours are clamped to 0–1 and only the first [`MAX_COLOR_STOPS`] stops
/// count. An empty palette gives mid grey.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightSlopeShading {
    /// Height bands (any order).
    pub stops: Vec<ColorStop>,
    /// Linear RGB of steep ground.
    pub cliff: [f32; 3],
    /// Slope in degrees where blending towards `cliff` starts.
    pub cliff_start_deg: f32,
    /// Slope in degrees where the colour is fully `cliff`.
    pub cliff_full_deg: f32,
}

/// Converts an sRGB-encoded byte triple to linear RGB.
#[must_use]
pub fn srgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| {
        let c = f32::from(c) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
}

impl Default for HeightSlopeShading {
    /// Tessera's default palette: shore sand, grass, dark green, heath, rock and snow by
    /// height; rock on slopes from 32° to 48°.
    fn default() -> Self {
        let stop = |height: f32, rgb: [u8; 3]| ColorStop {
            height,
            color: srgb8_to_linear(rgb),
        };
        Self {
            stops: vec![
                stop(0.0, [186, 172, 128]),
                stop(30.0, [128, 142, 86]),
                stop(140.0, [86, 120, 60]),
                stop(420.0, [70, 98, 52]),
                stop(820.0, [118, 110, 82]),
                stop(1_200.0, [126, 122, 116]),
                stop(1_550.0, [232, 234, 238]),
            ],
            cliff: srgb8_to_linear([112, 104, 94]),
            cliff_start_deg: 32.0,
            cliff_full_deg: 48.0,
        }
    }
}

impl HeightSlopeShading {
    /// Linear RGBA (alpha 1) for ground at `height` with unit-normal z component
    /// `normal_z` (cosine of the slope). Always finite. Colouring many vertices is cheaper
    /// through [`crate::TerrainMesh::build`], which sanitises the palette once.
    #[must_use]
    pub fn color(&self, height: f32, normal_z: f32) -> [f32; 4] {
        self.prepare().color(height, normal_z)
    }

    /// The sanitised palette.
    pub(crate) fn prepare(&self) -> Palette {
        let mut stops: Vec<ColorStop> = self
            .stops
            .iter()
            .take(MAX_COLOR_STOPS)
            .filter(|s| s.height.is_finite())
            .map(|s| ColorStop {
                height: s.height,
                color: s.color.map(clamp01),
            })
            .collect();
        stops.sort_by(|a, b| a.height.total_cmp(&b.height));
        Palette {
            stops,
            cliff: self.cliff.map(clamp01),
            cliff_start_deg: self.cliff_start_deg,
            cliff_full_deg: self.cliff_full_deg,
        }
    }
}

/// A sanitised [`HeightSlopeShading`]: finite stops in height order, clamped colours.
#[derive(Debug, Clone)]
pub(crate) struct Palette {
    stops: Vec<ColorStop>,
    cliff: [f32; 3],
    cliff_start_deg: f32,
    cliff_full_deg: f32,
}

impl Palette {
    pub(crate) fn color(&self, height: f32, normal_z: f32) -> [f32; 4] {
        let base = self.band_color(height);
        let t = self.cliff_weight(normal_z);
        let mix = |k: usize| base[k] + (self.cliff[k] - base[k]) * t;
        [mix(0), mix(1), mix(2), 1.0]
    }

    fn band_color(&self, height: f32) -> [f32; 3] {
        let stops = &self.stops;
        let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
            return [0.5; 3];
        };
        if !height.is_finite() || height <= first.height {
            return first.color;
        }
        if height >= last.height {
            return last.color;
        }
        for w in stops.windows(2) {
            let (a, b) = (w[0], w[1]);
            if height <= b.height {
                let span = b.height - a.height;
                let t = if span > 0.0 {
                    clamp01((height - a.height) / span)
                } else {
                    1.0
                };
                return std::array::from_fn(|k| a.color[k] + (b.color[k] - a.color[k]) * t);
            }
        }
        last.color
    }

    /// 0 (no cliff) to 1 (full cliff) for a normal with z component `nz`.
    fn cliff_weight(&self, nz: f32) -> f32 {
        if !nz.is_finite() {
            return 0.0;
        }
        let slope = nz.clamp(-1.0, 1.0).acos().to_degrees();
        let (lo, hi) = (self.cliff_start_deg, self.cliff_full_deg);
        if !(lo.is_finite() && hi.is_finite()) {
            return 0.0;
        }
        if hi <= lo {
            return if slope >= lo { 1.0 } else { 0.0 };
        }
        let t = clamp01((slope - lo) / (hi - lo));
        // Smoothstep.
        t * t * (3.0 - 2.0 * t)
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_and_cliffs() {
        let s = HeightSlopeShading::default();
        let low = s.color(-10.0, 1.0);
        assert_eq!(&low[..3], &s.stops[0].color[..]);
        let top = s.color(5_000.0, 1.0);
        assert_eq!(&top[..3], &s.stops.last().unwrap().color[..]);
        let mid = s.color(85.0, 1.0);
        for (k, m) in mid.iter().take(3).enumerate() {
            let (a, b) = (s.stops[1].color[k], s.stops[2].color[k]);
            assert!(*m >= a.min(b) - 1e-6 && *m <= a.max(b) + 1e-6);
        }
        // Vertical wall: pure cliff colour.
        let wall = s.color(300.0, 0.0);
        for (w, c) in wall.iter().zip(s.cliff) {
            assert!((w - c).abs() < 1e-6);
        }
        // 20° slope: no cliff yet.
        assert_eq!(
            s.color(300.0, 20f32.to_radians().cos()),
            s.color(300.0, 1.0)
        );
    }

    #[test]
    fn hostile_palettes_stay_finite() {
        let s = HeightSlopeShading {
            stops: vec![
                ColorStop {
                    height: f32::NAN,
                    color: [1.0; 3],
                },
                ColorStop {
                    height: 10.0,
                    color: [f32::INFINITY, -3.0, f32::NAN],
                },
                ColorStop {
                    height: 10.0,
                    color: [0.2; 3],
                },
                ColorStop {
                    height: -1e30,
                    color: [0.0; 3],
                },
            ],
            cliff: [f32::NAN; 3],
            cliff_start_deg: f32::INFINITY,
            cliff_full_deg: 3.0,
        };
        for h in [f32::NAN, f32::NEG_INFINITY, 0.0, 10.0, 1e30] {
            for nz in [f32::NAN, -2.0, 0.0, 0.5, 1.0, 7.0] {
                let c = s.color(h, nz);
                assert!(c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
            }
        }
        let empty = HeightSlopeShading {
            stops: Vec::new(),
            ..HeightSlopeShading::default()
        };
        assert_eq!(&empty.color(5.0, 1.0)[..3], &[0.5; 3]);
    }

    #[test]
    fn srgb_conversion() {
        assert_eq!(srgb8_to_linear([0, 255, 0]), [0.0, 1.0, 0.0]);
        let mid = srgb8_to_linear([188, 188, 188])[0];
        assert!((mid - 0.5).abs() < 0.01);
    }
}
