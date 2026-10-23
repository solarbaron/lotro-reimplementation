// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's `[UI]` settings keys applied to `tessera-ui` (`ui-scaling.md` §7.1): the art
//! filter (§6.3, §7.6), the text factor (§7.5), the text source (§6.4.3 item 4), the
//! layout-parameter reader of the crisp text path (§6.4.5). The host applies them at start and on
//! every live change ([`super::UiHost::interface_frame`]). The §8 S6 split between the
//! decoded-image LRU and the glyph caches is `tessera-ui`'s (`Ui::set_image_memory`,
//! `Ui::s6_budget`); the host gives the images' share to its LRU after every change of *Se*.

use tessera_formats::property::PropertyValue;
use tessera_settings::{ArtFilter, TextSource, UiPrefs};
use tessera_ui::Ui;
use tessera_ui::paint::ArtFilter as UiArtFilter;
use tessera_ui::text::{TextParamsReader, UiTextSource};

use super::text::{TextPids, layout_params};

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
