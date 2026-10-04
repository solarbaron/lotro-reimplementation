// SPDX-License-Identifier: GPL-3.0-or-later
//! Converts rendered message text ([`tessera_text::RichText`]) into an egui layout job,
//! keeping colours, emphasis, list items, link annotations and explorer placeholders
//! visible (`string-table.md` §4.7, §4.8, §8.3).

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId, Stroke};
use tessera_text::{RichText, Style};

/// Colours used for the markup that has no colour of its own.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Default text colour.
    pub text: Color32,
    /// Bold text colour (egui's default fonts have no bold face).
    pub strong: Color32,
    /// Explorer placeholders (`${NAME}`, missing references).
    pub placeholder: Color32,
    /// Background of placeholders.
    pub placeholder_bg: Color32,
    /// Link annotations.
    pub link: Color32,
}

impl Palette {
    /// A palette for a dark or light theme.
    #[must_use]
    pub fn for_dark_mode(dark: bool) -> Self {
        if dark {
            Self {
                text: Color32::from_gray(210),
                strong: Color32::WHITE,
                placeholder: Color32::from_rgb(0xE0, 0xB0, 0x60),
                placeholder_bg: Color32::from_rgba_unmultiplied(0xE0, 0xB0, 0x60, 28),
                link: Color32::from_rgb(0x70, 0xB0, 0xFF),
            }
        } else {
            Self {
                text: Color32::from_gray(30),
                strong: Color32::BLACK,
                placeholder: Color32::from_rgb(0x90, 0x60, 0x00),
                placeholder_bg: Color32::from_rgba_unmultiplied(0x90, 0x60, 0x00, 28),
                link: Color32::from_rgb(0x10, 0x50, 0xC0),
            }
        }
    }
}

/// Longest text put into one layout job; the rest is replaced by an ellipsis. Keeps a
/// pathological entry from making a frame slow.
pub const MAX_JOB_BYTES: usize = 16 * 1024;

fn format_for(style: &Style, font: &FontId, palette: &Palette) -> TextFormat {
    let mut color = match style.color {
        Some(c) => Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a.max(64)),
        None if style.bold => palette.strong,
        None => palette.text,
    };
    let mut fmt = TextFormat {
        font_id: font.clone(),
        italics: style.italic,
        ..TextFormat::default()
    };
    if style.link.is_some() {
        if style.color.is_none() {
            color = palette.link;
        }
        fmt.underline = Stroke::new(1.0, color);
    }
    if style.underline {
        fmt.underline = Stroke::new(1.0, color);
    }
    if style.placeholder {
        color = palette.placeholder;
        fmt.italics = true;
        fmt.background = palette.placeholder_bg;
    }
    fmt.color = color;
    fmt
}

/// Builds a layout job from rich text.
///
/// `single_line` turns line breaks into `⏎` and disables wrapping (for list rows);
/// otherwise the text wraps at `wrap_width`. List items get a bullet at the start of
/// each item run.
#[must_use]
pub fn layout_job(
    text: &RichText,
    font: &FontId,
    palette: &Palette,
    single_line: bool,
    wrap_width: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    if single_line {
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
    }
    job.wrap.max_width = wrap_width;
    let mut budget = MAX_JOB_BYTES;
    let mut prev_list = false;
    for span in text.spans() {
        if budget == 0 {
            break;
        }
        let fmt = format_for(&span.style, font, palette);
        if span.style.list_item && !prev_list {
            job.append("• ", 0.0, fmt.clone());
        }
        prev_list = span.style.list_item;
        let mut s: String = if single_line {
            span.text.replace("\r\n", "⏎").replace(['\n', '\r'], "⏎")
        } else {
            span.text.clone()
        };
        if s.len() > budget {
            let mut end = budget;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s.truncate(end);
            s.push('…');
            budget = 0;
        } else {
            budget -= s.len();
        }
        job.append(&s, 0.0, fmt);
    }
    if job.text.is_empty() {
        job.append(
            "(empty)",
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: palette.placeholder,
                italics: true,
                ..TextFormat::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_text::{Message, RenderOptions, Values, parse_markup, render};

    fn palette() -> Palette {
        Palette::for_dark_mode(true)
    }

    #[test]
    fn colours_and_placeholders() {
        let text = parse_markup("plain <rgb=#FF0000>red</rgb> <b>bold</b>");
        let job = layout_job(&text, &FontId::monospace(12.0), &palette(), false, 100.0);
        assert_eq!(job.text, "plain red bold");
        let red = job
            .sections
            .iter()
            .find(|s| job.text.get(s.byte_range.start.0..s.byte_range.end.0) == Some("red"))
            .unwrap();
        assert_eq!(red.format.color, Color32::from_rgb(255, 0, 0));
        let bold = job
            .sections
            .iter()
            .find(|s| job.text.get(s.byte_range.start.0..s.byte_range.end.0) == Some("bold"))
            .unwrap();
        assert_eq!(bold.format.color, palette().strong);

        let msg = Message::from_format_string("Hello ${PLAYER}!");
        let rendered = render(&msg, &Values::new(), &RenderOptions::explorer(None));
        let job = layout_job(
            &rendered.text,
            &FontId::monospace(12.0),
            &palette(),
            true,
            100.0,
        );
        assert!(job.text.contains("${PLAYER}"), "{}", job.text);
        assert!(
            job.sections
                .iter()
                .any(|s| s.format.color == palette().placeholder)
        );
    }

    #[test]
    fn single_line_and_limits() {
        let text = parse_markup("a\nb");
        let job = layout_job(&text, &FontId::monospace(12.0), &palette(), true, 50.0);
        assert_eq!(job.text, "a⏎b");
        assert_eq!(job.wrap.max_rows, 1);
        let empty = layout_job(
            &RichText::new(),
            &FontId::monospace(12.0),
            &palette(),
            true,
            50.0,
        );
        assert_eq!(empty.text, "(empty)");
        let long = RichText::plain(&"é".repeat(MAX_JOB_BYTES));
        let job = layout_job(&long, &FontId::monospace(12.0), &palette(), false, 50.0);
        assert!(job.text.len() <= MAX_JOB_BYTES + 4);
        assert!(job.text.ends_with('…'));
    }

    #[test]
    fn list_items_get_bullets() {
        let text = parse_markup("x<li>one</li>");
        let job = layout_job(&text, &FontId::monospace(12.0), &palette(), false, 50.0);
        assert!(job.text.contains("• "), "{}", job.text);
    }
}
