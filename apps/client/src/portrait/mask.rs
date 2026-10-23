// SPDX-License-Identifier: GPL-3.0-or-later
//! The portraits' opacity masks (`ui-widgets.md` §12.3 step 2): a texture list (prefix
//! 0x40, `texture-list.md`) or a surface (prefix 0x41) read from the install and decoded to
//! RGBA8, the mask in the alpha channel; a surface without an alpha channel gives its red
//! channel (Tessera rule of §12.3). Pure functions over a [`RecordSource`] (no GPU, no
//! logging), run on a worker thread by [`super::CombatPortraits`] and included by path by
//! the fuzz target `client_portrait_mask`.

use tessera_formats::Record as _;
use tessera_formats::did::Did;
use tessera_formats::surface::{Surface, SurfaceFormat};
use tessera_formats::texture_list::TextureList;
use tessera_props::RecordSource;

/// The largest mask side decoded, pixels (Tessera bound; a portrait target is at most 256).
pub const MAX_MASK_SIDE: u32 = 1024;

/// What goes wrong reading an opacity mask. (Display and Error are written out rather than
/// derived so that the fuzz crate, which includes this file by path, needs no `thiserror`.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskError {
    /// No archive holds the record.
    Missing(u32),
    /// Reading failed.
    Read(u32, String),
    /// The record does not decode.
    Decode(u32, String),
    /// No level of the list resolves to a surface of at most [`MAX_MASK_SIDE`].
    NoLevel(u32),
}

impl std::fmt::Display for MaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(d) => write!(f, "record {d:#010x} not found"),
            Self::Read(d, e) => write!(f, "reading {d:#010x}: {e}"),
            Self::Decode(d, e) => write!(f, "decoding {d:#010x}: {e}"),
            Self::NoLevel(d) => write!(f, "texture list {d:#010x} has no usable level"),
        }
    }
}

impl std::error::Error for MaskError {}

/// A decoded opacity mask: RGBA8, its alpha the mask (`ui-widgets.md` §12.3 step 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, R, G, B, A.
    pub rgba: Vec<u8>,
}

/// Whether a surface format stores an alpha channel (the others take the mask from red,
/// `ui-widgets.md` §12.3 step 2). DXT1 counts as without one: a Tessera reading that the
/// spec does not settle (its 1-bit transparency taken as a cut-out key, not an alpha
/// channel; a DXT1 mask whose cut-out carries the shape would be read from red instead).
#[must_use]
pub fn has_alpha_channel(format: SurfaceFormat) -> bool {
    matches!(
        format,
        SurfaceFormat::A8R8G8B8 | SurfaceFormat::A8 | SurfaceFormat::Dxt3 | SurfaceFormat::Dxt5
    )
}

/// Copies every pixel's red channel into its alpha (a mask without an alpha channel).
pub fn alpha_from_red(rgba: &mut [u8]) {
    for p in rgba.as_chunks_mut::<4>().0 {
        p[3] = p[0];
    }
}

fn read(source: &dyn RecordSource, did: Did) -> Result<Vec<u8>, MaskError> {
    match source.read_record(did) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(MaskError::Missing(did.0)),
        Err(e) => Err(MaskError::Read(did.0, e.to_string())),
    }
}

/// Reads and decodes the opacity mask `did`: a texture list (prefix 0x40, its largest level
/// of at most [`MAX_MASK_SIDE`] that resolves: a level that cannot be read, parsed or
/// decoded, or is too large, is skipped), or a single surface (prefix 0x41).
///
/// # Errors
///
/// [`MaskError`] when the records are missing, unreadable or malformed, or no level
/// resolves ([`MaskError::NoLevel`] for a list; for a single surface the error of reading or
/// decoding it).
pub fn load_mask(source: &dyn RecordSource, did: Did) -> Result<MaskImage, MaskError> {
    let surfaces: Vec<Did> = if did.0 >> 24 == 0x40 {
        let bytes = read(source, did)?;
        let list = TextureList::parse_expecting(&bytes, did)
            .map_err(|e| MaskError::Decode(did.0, e.to_string()))?;
        list.levels().map(<[Did]>::to_vec).unwrap_or_default()
    } else {
        vec![did]
    };
    let single = surfaces.len() == 1 && surfaces.first() == Some(&did);
    let mut last: Option<MaskError> = None;
    for s in surfaces {
        if s.0 == 0 {
            continue;
        }
        let bytes = match read(source, s) {
            Ok(b) => b,
            Err(e) => {
                last = Some(e);
                continue;
            }
        };
        let surface = match Surface::parse_expecting(&bytes, s) {
            Ok(surface) => surface,
            Err(e) => {
                last = Some(MaskError::Decode(s.0, e.to_string()));
                continue;
            }
        };
        let (w, h) = surface.decoded_size().unwrap_or((0, 0));
        if w == 0 || h == 0 || w > MAX_MASK_SIDE || h > MAX_MASK_SIDE {
            continue;
        }
        let image = match surface.to_rgba8() {
            Ok(image) => image,
            Err(e) => {
                last = Some(MaskError::Decode(s.0, e.to_string()));
                continue;
            }
        };
        let mut rgba = image.pixels;
        if !has_alpha_channel(surface.format) {
            alpha_from_red(&mut rgba);
        }
        return Ok(MaskImage {
            width: image.width,
            height: image.height,
            rgba,
        });
    }
    match last {
        // A single surface reports why it did not resolve.
        Some(e) if single => Err(e),
        _ => Err(MaskError::NoLevel(did.0)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn red_becomes_alpha_without_an_alpha_channel() {
        let mut p = vec![10, 20, 30, 255, 200, 0, 0, 7];
        alpha_from_red(&mut p);
        assert_eq!(p, vec![10, 20, 30, 10, 200, 0, 0, 200]);
        assert!(has_alpha_channel(SurfaceFormat::Dxt5));
        assert!(has_alpha_channel(SurfaceFormat::A8R8G8B8));
        assert!(!has_alpha_channel(SurfaceFormat::R8G8B8));
        assert!(!has_alpha_channel(SurfaceFormat::Jpeg));
        assert!(!has_alpha_channel(SurfaceFormat::Dxt1));
    }

    /// A record source over a map (synthetic records).
    #[derive(Debug, Default)]
    struct Records(BTreeMap<u32, Vec<u8>>);

    impl RecordSource for Records {
        fn read_record(&self, did: Did) -> Result<Option<Vec<u8>>, tessera_props::SourceError> {
            Ok(self.0.get(&did.0).cloned())
        }
    }

    #[test]
    fn missing_and_malformed_masks_are_errors() {
        let mut r = Records::default();
        assert!(matches!(
            load_mask(&r, Did(0x4000_0001)),
            Err(MaskError::Missing(_))
        ));
        r.0.insert(0x4000_0001, vec![1, 2, 3]);
        assert!(matches!(
            load_mask(&r, Did(0x4000_0001)),
            Err(MaskError::Decode(..))
        ));
        r.0.insert(0x4100_0001, vec![0xFF; 7]);
        assert!(load_mask(&r, Did(0x4100_0001)).is_err());
    }

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    fn surface(did: u32, format: SurfaceFormat, w: u32, h: u32, data: &[u8]) -> Vec<u8> {
        let len = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let mut b = le(&[did, 0, w, h, format.code(), len]);
        b.extend_from_slice(data);
        b
    }

    #[test]
    fn a_list_gives_its_first_level_within_the_bound_and_red_without_alpha() {
        let list = 0x4000_0E4B;
        let (big, small) = (0x4100_1000, 0x4100_1001);
        let mut l = le(&[list]);
        l.extend_from_slice(&[2, 2, 0, 0, 0]);
        l.extend(le(&[big, small, 0, 15]));
        let mut r = Records::default();
        r.0.insert(list, l);
        // Level 0 is wider than the bound: skipped.
        r.0.insert(
            big,
            surface(big, SurfaceFormat::A8, MAX_MASK_SIDE + 1, 1, &[7; 1025]),
        );
        // Level 1 has no alpha channel: B, G, R stored; red becomes the mask.
        let px = [0, 0, 200, 0, 0, 10, 1, 2, 3, 9, 9, 255];
        r.0.insert(small, surface(small, SurfaceFormat::R8G8B8, 2, 2, &px));
        let m = load_mask(&r, Did(list)).expect("mask");
        assert_eq!((m.width, m.height), (2, 2));
        let alpha: Vec<u8> = m.rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
        assert_eq!(alpha, vec![200, 10, 3, 255]);
        // A surface with an alpha channel keeps it.
        let direct = 0x4100_2000;
        let argb = [1, 2, 3, 40, 4, 5, 6, 50];
        r.0.insert(
            direct,
            surface(direct, SurfaceFormat::A8R8G8B8, 2, 1, &argb),
        );
        let m = load_mask(&r, Did(direct)).expect("mask");
        assert_eq!(m.rgba, vec![3, 2, 1, 40, 6, 5, 4, 50]);
        // A level that parses but does not decode (a JPEG frame header without a scan)
        // is skipped like the others: the next level gives the mask.
        let broken = 0x4100_1002;
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x02, 0x00, 0x02, 0x01, 0x01, 0x11,
            0x00, 0xFF, 0xD9,
        ];
        r.0.insert(broken, surface(broken, SurfaceFormat::Jpeg, 0, 0, &jpeg));
        let mut l = le(&[list]);
        l.extend_from_slice(&[2, 2, 0, 0, 0]);
        l.extend(le(&[broken, small, 0, 15]));
        r.0.insert(list, l);
        let m = load_mask(&r, Did(list)).expect("the next level");
        assert_eq!((m.width, m.height), (2, 2));
        // Alone, the broken level is a decode error of its own.
        assert!(matches!(
            load_mask(&r, Did(broken)),
            Err(MaskError::Decode(d, _)) if d == broken
        ));
        let mut l = le(&[list]);
        l.extend_from_slice(&[2, 2, 0, 0, 0]);
        l.extend(le(&[big, small, 0, 15]));
        r.0.insert(list, l);
        // A list whose only level is too large has none.
        r.0.remove(&small);
        assert!(matches!(
            load_mask(&r, Did(list)),
            Err(MaskError::NoLevel(_))
        ));
    }
}
