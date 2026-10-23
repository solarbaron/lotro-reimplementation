// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's `[UI]` settings keys applied to `tessera-ui` (`ui-scaling.md` §7.1): the art
//! filter (§6.3, §7.6), the text factor (§7.5), the text source (§6.4.3 item 4), the
//! layout-parameter reader of the crisp text path (§6.4.5) and the split of the §8 S6 pool
//! between the decoded-image LRU and the derived-glyph cache (§6.5). The host applies them at start and on
//! every live change ([`super::UiHost::interface_frame`], a change of *Se*).

use tessera_formats::property::PropertyValue;
use tessera_settings::{ArtFilter, TextSource, UiPrefs};
use tessera_ui::Ui;
use tessera_ui::paint::ArtFilter as UiArtFilter;
use tessera_ui::text::{TextParamsReader, UiTextSource};

use super::text::{TextPids, layout_params};

/// The derived-glyph cache takes one part in this many of the unpinned S6 pool of
/// `ui-scaling.md` §8 (Tessera choice: 8 MiB of the 64 MiB default at *Se* = 1, the
/// `tessera-ui` default); the decoded-image LRU gets the rest ([`s6_split`]).
pub const GLYPH_SHARE_DIVISOR: u64 = 8;

/// Smallest derived-glyph cache budget, bytes (Tessera bound: one ladder step of the
/// printable range at a large target height fits).
pub const MIN_GLYPH_CACHE_BYTES: u64 = 1 << 20;

/// Largest derived-glyph cache budget, bytes (Tessera bound against a hostile settings value
/// or scale).
pub const MAX_GLYPH_CACHE_BYTES: u64 = 512 << 20;

/// The `tessera-ui` art filter of `ui_art_filter` (`ui-scaling.md` §7.6), through
/// [`UiArtFilter::from_setting`].
#[must_use]
pub fn ui_art_filter(filter: ArtFilter) -> UiArtFilter {
    let name = match filter {
        ArtFilter::Sharp => "sharp",
        ArtFilter::Smooth => "smooth",
        ArtFilter::Nearest => "nearest",
    };
    UiArtFilter::from_setting(name).unwrap_or_default()
}

/// The `tessera-ui` text source of `ui_text_source` (`ui-scaling.md` §6.4.3 item 4), through
/// [`UiTextSource::from_setting`].
#[must_use]
pub fn ui_text_source(source: TextSource) -> UiTextSource {
    let name = match source {
        TextSource::Game => "game",
        TextSource::Tessera => "tessera",
    };
    UiTextSource::from_setting(name).unwrap_or_default()
}

/// Largest S6 pool, bytes (Tessera bound against a hostile settings value or scale).
pub const MAX_S6_POOL_BYTES: u64 = 1 << 40;

/// The unpinned S6 pool of `ui-scaling.md` §8 split between its caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct S6Split {
    /// The decoded-image LRU budget (decoded and scaled images), bytes.
    pub image_lru: u64,
    /// The derived-glyph cache budget (§6.5), bytes.
    pub glyph: usize,
}

/// The S6 pool (`ui-scaling.md` §8: "decoded images, scaled-image and glyph caches together
/// ≤ 64 MiB × max(1, *Se*²) unpinned", `image_lru_mib` overriding the 64 MiB) split between
/// the caches: the pool is `image_lru_bytes` × max(1, *Se*²) (at most
/// [`MAX_S6_POOL_BYTES`]); the glyph cache takes one [`GLYPH_SHARE_DIVISOR`]th of it, within
/// [`MIN_GLYPH_CACHE_BYTES`]–[`MAX_GLYPH_CACHE_BYTES`], and the image LRU the rest, so the
/// two together stay within the pool (only a pool under [`MIN_GLYPH_CACHE_BYTES`] is
/// exceeded, by the glyph floor). A non-finite or non-positive *Se* counts as 1.
#[must_use]
pub fn s6_split(image_lru_bytes: u64, se: f64) -> S6Split {
    let se = if se.is_finite() && se > 0.0 { se } else { 1.0 };
    let area = (se * se).max(1.0);
    let pool = image_lru_bytes as f64 * area;
    let pool = if pool.is_finite() {
        pool.min(MAX_S6_POOL_BYTES as f64) as u64
    } else {
        MAX_S6_POOL_BYTES
    };
    let glyph = (pool / GLYPH_SHARE_DIVISOR).clamp(MIN_GLYPH_CACHE_BYTES, MAX_GLYPH_CACHE_BYTES);
    S6Split {
        image_lru: pool.saturating_sub(glyph),
        glyph: usize::try_from(glyph).unwrap_or(usize::MAX),
    }
}

/// Applies the art filter, the text factor and the text source of `prefs` (`ui-scaling.md`
/// §7.1). Each setter does nothing when its value is unchanged; a change repaints or lays the
/// text out again live.
pub fn apply_text_and_art(ui: &mut Ui, prefs: &UiPrefs) {
    ui.set_art_filter(ui_art_filter(prefs.art_filter));
    ui.set_text_scale(f64::from(prefs.text_scale));
    ui.set_text_source(ui_text_source(prefs.text_source));
}

/// The layout-parameter reader of the crisp text path (`ui-scaling.md` §6.4.5, §7.5): the
/// shaper's own [`layout_params`] over `pids` with the locale leading factor `leading`, so a
/// text laid out again in physical pixels takes the same margins, justification, wrap switch
/// and leading as the shaper's layout.
pub fn text_params_reader(pids: TextPids, leading: f32) -> TextParamsReader {
    Box::new(
        move |props: &[(u32, PropertyValue)], size: (i32, i32), wrap: Option<i32>| {
            layout_params(&pids, props, size, wrap, leading)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_map_to_the_ui_options() {
        assert_eq!(ui_art_filter(ArtFilter::Sharp), UiArtFilter::Sharp);
        assert_eq!(ui_art_filter(ArtFilter::Smooth), UiArtFilter::Smooth);
        assert_eq!(ui_art_filter(ArtFilter::Nearest), UiArtFilter::Nearest);
        assert_eq!(ui_text_source(TextSource::Game), UiTextSource::Game);
        assert_eq!(ui_text_source(TextSource::Tessera), UiTextSource::Tessera);
    }

    /// §8 S6: the image LRU and the glyph cache share one pool of 64 MiB × max(1, *Se*²)
    /// (by default) instead of the glyph share coming on top of the image budget.
    #[test]
    fn s6_pool_is_shared() {
        let lru = 64u64 << 20;
        let s = s6_split(lru, 1.0);
        assert_eq!(s.glyph, 8 << 20, "the tessera-ui default");
        assert_eq!(s.image_lru, 56 << 20);
        assert_eq!(s.image_lru + s.glyph as u64, lru, "together the S6 total");
        assert_eq!(s6_split(lru, 0.5), s, "max(1, Se²)");
        assert_eq!(s6_split(lru, f64::NAN), s);
        assert_eq!(s6_split(lru, -1.0), s);
        let s2 = s6_split(lru, 2.0);
        assert_eq!(s2.glyph, 32 << 20);
        assert_eq!(
            s2.image_lru + s2.glyph as u64,
            256 << 20,
            "both grow with Se²"
        );
        let s0 = s6_split(0, 1.0);
        assert_eq!((s0.image_lru, s0.glyph), (0, 1 << 20), "floor");
        let big = s6_split(u64::MAX, 4.0);
        assert_eq!(big.glyph, 512 << 20, "ceiling");
        assert_eq!(big.image_lru, MAX_S6_POOL_BYTES - (512 << 20));
    }

    #[test]
    fn reader_is_the_shapers_layout() {
        let p = TextPids::default();
        let props = vec![
            (p.left_margin, PropertyValue::Int(3)),
            (p.one_line, PropertyValue::Bool(true)),
        ];
        let r = text_params_reader(p, 0.0);
        assert_eq!(
            r(&props, (80, 20), Some(40)),
            layout_params(&p, &props, (80, 20), Some(40), 0.0)
        );
    }
}
