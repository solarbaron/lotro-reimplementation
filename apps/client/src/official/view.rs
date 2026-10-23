// SPDX-License-Identifier: GPL-3.0-or-later
//! Drawing of the official screens (`docs/specs/official-entry.md` §4.1, §5.4, §7.1, §8.3,
//! §8.4): Tessera's own code, Tessera's neutral colours and the bundled font of decision
//! 0022 (`tessera_ui::text::TesseraFont`), no install layout, no skin, no game code.
//!
//! [`build`] lays out the flow's current screen into a [`Scene`]: a list of draw operations
//! and the hit boxes of its controls. [`paint`] renders a scene on the CPU into one RGBA
//! image, which the window uploads as one texture and draws over the whole window.
//!
//! Tessera engineering note: the screens are painted by this module rather than built as
//! `tessera-ui` element trees, so that the warning screen's input never passes through the
//! UI host (§5.4 rule 1) and nothing of the game UI (layouts, skins, plugin windows) can
//! reach them; the text uses `tessera-ui`'s bundled-font faces.

use std::hash::{Hash, Hasher};
use std::time::Instant;

use tessera_compat::consent::strings::{fill, text};
use tessera_ui::text::{DEFAULT_RANGES, FontFace, TesseraFont};

use super::flow::{AcceptanceLine, Control, Flow, Screen};
use super::form::FormControl;
use super::login::LoginScreen;
use super::warning::WarningControl;

/// Largest canvas side in pixels (a larger window stretches the image).
pub const MAX_CANVAS: u32 = 4096;

const BACKGROUND: [u8; 4] = [22, 24, 28, 255];
const PANEL: [u8; 4] = [36, 40, 47, 255];
const TEXT: [u8; 4] = [236, 236, 230, 255];
const DIM_TEXT: [u8; 4] = [150, 150, 150, 255];
const BUTTON: [u8; 4] = [58, 88, 128, 255];
const BUTTON_OFF: [u8; 4] = [52, 54, 58, 255];
const FOCUS: [u8; 4] = [232, 200, 92, 255];
const LINK: [u8; 4] = [140, 182, 240, 255];
const CHANGED: [u8; 4] = [96, 66, 22, 255];
const ENTRY: [u8; 4] = [16, 17, 20, 255];
const SELECTED: [u8; 4] = [48, 70, 100, 255];
const SHADE: [u8; 4] = [0, 0, 0, 160];

/// The three faces of the screens, rasterised at the window's scale factor.
#[derive(Debug)]
pub struct Fonts {
    title: FontFace,
    body: FontFace,
    small: FontFace,
    /// The scale factor the faces were rasterised for (physical pixels per layout unit).
    scale: f64,
}

/// A face of [`Fonts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// Titles.
    Title,
    /// Buttons and body text.
    Body,
    /// Notes and small lines.
    Small,
}

impl Fonts {
    /// Rasterises the bundled font at the screens' sizes for the window scale `scale` (run on
    /// a worker; `fonts.md` §11).
    ///
    /// # Errors
    ///
    /// The text error of the rasteriser.
    pub fn load(scale: f64) -> Result<Self, tessera_ui::text::TextError> {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.4)
        } else {
            1.0
        };
        let font = TesseraFont::bundled()?;
        let px = |base: f64| ((base * scale).round() as u32).clamp(8, 64);
        Ok(Self {
            title: font.rasterise(px(26.0), DEFAULT_RANGES)?,
            body: font.rasterise(px(17.0), DEFAULT_RANGES)?,
            small: font.rasterise(px(15.0), DEFAULT_RANGES)?,
            scale,
        })
    }

    /// The scale factor the faces were made for.
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    fn face(&self, f: Face) -> &FontFace {
        match f {
            Face::Title => &self.title,
            Face::Body => &self.body,
            Face::Small => &self.small,
        }
    }

    fn line(&self, f: Face) -> i32 {
        i32::try_from(self.face(f).line_height())
            .unwrap_or(16)
            .max(1)
    }

    /// The width of `s` in physical pixels.
    fn width(&self, f: Face, s: &str) -> i32 {
        let face = self.face(f);
        s.encode_utf16()
            .filter_map(|u| face.lookup(u).glyph)
            .map(|g| g.advance())
            .sum()
    }

    /// `s` wrapped to `width` physical pixels; paragraphs (`\n`) stay separate, an empty paragraph is
    /// an empty line.
    fn wrap(&self, f: Face, s: &str, width: i32) -> Vec<String> {
        let mut out = Vec::new();
        for para in s.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty() && self.width(f, &candidate) > width {
                    out.push(std::mem::take(&mut line));
                    line = word.to_owned();
                } else {
                    line = candidate;
                }
            }
            out.push(line);
        }
        out
    }
}

/// A rectangle in layout units (physical pixels ÷ the scale factor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl Rect {
    const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// Whether (`x`, `y`) is inside.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x + self.w)
            && y < f64::from(self.y + self.h)
    }
}

/// One draw operation. Its `Debug` form prints no text (the account name is drawn).
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Draw {
    /// A filled rectangle (alpha-blended).
    Fill(Rect, [u8; 4]),
    /// A rectangle's outline of `t` pixels.
    Frame(Rect, [u8; 4], i32),
    /// One line of text with its line box's top-left corner at (`x`, `y`).
    Text {
        /// Left.
        x: i32,
        /// Top of the line box.
        y: i32,
        /// Face.
        face: Face,
        /// The text.
        text: String,
        /// Colour.
        colour: [u8; 4],
    },
}

/// A laid-out screen. Its `Debug` form prints counts only (the account name is drawn).
#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct Scene {
    /// Draw operations in order.
    pub draws: Vec<Draw>,
    /// Control hit boxes (the last match wins).
    pub hits: Vec<(Rect, Control)>,
    /// The warning's text is laid out in full with the bundled font (§5.4 rule 7).
    pub warning_text_ready: bool,
    /// The warning body's largest scroll offset in lines.
    pub warning_scroll_max: u32,
    /// Physical pixels per layout unit, in thousandths.
    pub scale_milli: u32,
}

impl std::fmt::Debug for Draw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fill(r, c) => f.debug_tuple("Fill").field(r).field(c).finish(),
            Self::Frame(r, c, t) => f.debug_tuple("Frame").field(r).field(c).field(t).finish(),
            Self::Text {
                x, y, face, text, ..
            } => f
                .debug_struct("Text")
                .field("x", x)
                .field("y", y)
                .field("face", face)
                .field("bytes", &text.len())
                .finish_non_exhaustive(),
        }
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("draws", &self.draws.len())
            .field("hits", &self.hits.len())
            .field("warning_text_ready", &self.warning_text_ready)
            .field("warning_scroll_max", &self.warning_scroll_max)
            .field("scale_milli", &self.scale_milli)
            .finish()
    }
}

impl Scene {
    /// Physical pixels per layout unit.
    #[must_use]
    pub fn scale(&self) -> f64 {
        f64::from(self.scale_milli.max(1)) / 1000.0
    }

    /// The control under the window position (`x`, `y`) (physical pixels).
    #[must_use]
    pub fn hit(&self, x: f64, y: f64) -> Option<Control> {
        let k = self.scale();
        let (x, y) = (x / k, y / k);
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|(_, c)| *c)
    }

    /// A hash of the scene (the window repaints only when it changes).
    #[must_use]
    pub fn key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }

    fn fill(&mut self, r: Rect, c: [u8; 4]) {
        self.draws.push(Draw::Fill(r, c));
    }

    fn frame(&mut self, r: Rect, c: [u8; 4], t: i32) {
        self.draws.push(Draw::Frame(r, c, t));
    }

    fn text(&mut self, x: i32, y: i32, face: Face, s: impl Into<String>, colour: [u8; 4]) {
        self.draws.push(Draw::Text {
            x,
            y,
            face,
            text: s.into(),
            colour,
        });
    }
}

/// Builder state: the scene, the fonts (if loaded), the window size.
struct B<'a> {
    s: Scene,
    fonts: Option<&'a Fonts>,
    w: i32,
    h: i32,
    /// Physical pixels per layout unit.
    k: f64,
}

impl B<'_> {
    fn units(&self, physical: i32) -> i32 {
        (f64::from(physical) / self.k).round() as i32
    }

    fn line(&self, f: Face) -> i32 {
        self.fonts.map_or(18, |x| self.units(x.line(f)).max(1))
    }

    fn width(&self, f: Face, s: &str) -> i32 {
        self.fonts.map_or(0, |x| self.units(x.width(f, s)))
    }

    fn wrap(&self, f: Face, s: &str, width: i32) -> Vec<String> {
        let physical = (f64::from(width.max(1)) * self.k).floor() as i32;
        self.fonts
            .map_or_else(Vec::new, |x| x.wrap(f, s, physical.max(1)))
    }

    /// A button with its label centred.
    fn button(&mut self, r: Rect, label: &str, enabled: bool, focused: bool, c: Control) {
        self.s.fill(r, if enabled { BUTTON } else { BUTTON_OFF });
        if focused {
            self.s.frame(r, FOCUS, 2);
        }
        if self.fonts.is_some() {
            let tw = self.width(Face::Body, label);
            let lh = self.line(Face::Body);
            self.s.text(
                r.x + (r.w - tw) / 2,
                r.y + (r.h - lh) / 2,
                Face::Body,
                label,
                if enabled { TEXT } else { DIM_TEXT },
            );
        }
        if enabled {
            self.s.hits.push((r, c));
        }
    }

    /// Wrapped text from `y`; returns the next free `y`.
    fn para(&mut self, x: i32, y: i32, width: i32, face: Face, s: &str, colour: [u8; 4]) -> i32 {
        let lh = self.line(face);
        let mut y = y;
        for l in self.wrap(face, s, width) {
            self.s.text(x, y, face, l, colour);
            y += lh;
        }
        y
    }

    fn column(&self, max: i32) -> (i32, i32) {
        let cw = (self.w - 80).clamp(200, max);
        ((self.w - cw) / 2, cw)
    }
}

/// Lays out the flow's screen for a window of `size` physical pixels (module docs), in layout
/// units of the fonts' scale factor.
#[must_use]
pub fn build(flow: &Flow, fonts: Option<&Fonts>, size: (u32, u32), now: Instant) -> Scene {
    let k = fonts.map_or(1.0, Fonts::scale);
    let units = |v: u32| (f64::from(v.min(MAX_CANVAS)) / k).floor() as i32;
    let mut b = B {
        s: Scene {
            scale_milli: (k * 1000.0).round() as u32,
            ..Scene::default()
        },
        fonts,
        w: units(size.0).max(1),
        h: units(size.1).max(1),
        k,
    };
    b.s.fill(Rect::new(0, 0, b.w, b.h), BACKGROUND);
    match flow.screen() {
        Screen::Start => start(&mut b, flow),
        Screen::Checking => centred_line(&mut b, text("official.checking")),
        Screen::Warning => warning(&mut b, flow),
        Screen::Form => form(&mut b, flow, now),
        Screen::Login => login(&mut b, flow),
        Screen::Characters => characters(&mut b),
    }
    if let Some(n) = flow.notice() {
        notice(&mut b, &n.message.text, n.message.detail.as_deref());
    } else if let Some(m) = flow.login().and_then(|p| p.message()) {
        notice(&mut b, &m.text, m.detail.as_deref());
    }
    b.s
}

fn centred_line(b: &mut B<'_>, s: &str) {
    let tw = b.width(Face::Body, s);
    let lh = b.line(Face::Body);
    b.s.text((b.w - tw) / 2, (b.h - lh) / 2, Face::Body, s, TEXT);
}

fn start(b: &mut B<'_>, flow: &Flow) {
    let bw = 340.min(b.w - 40).max(80);
    let bh = 46;
    let x = (b.w - bw) / 2;
    // The line and its paragraph's line budget: the run-only texts are longer.
    let accepted = flow.acceptance_line().map(|l| match l {
        AcceptanceLine::Stored(date) => {
            (fill(text("official.start.accepted"), &[("date", date)]), 3)
        }
        AcceptanceLine::RunOnly(id) => (text(id).to_owned(), 4),
    });
    let extra = accepted
        .as_ref()
        .map_or(0, |(_, lines)| lines * b.line(Face::Small) + 8);
    let total = 3 * bh + 2 * 16 + extra + 16;
    let mut y = (b.h - total) / 2;
    b.button(
        Rect::new(x, y, bw, bh),
        text("official.start.offline"),
        true,
        false,
        Control::StartOffline,
    );
    y += bh + 16;
    b.button(
        Rect::new(x, y, bw, bh),
        text("official.start.official"),
        true,
        false,
        Control::StartOfficial,
    );
    y += bh + 8;
    if let Some((line, _)) = accepted {
        y = b.para(x - 60, y, bw + 120, Face::Small, &line, DIM_TEXT);
        let label = text("official.consent.withdraw");
        let lw = b.width(Face::Small, label);
        let lh = b.line(Face::Small);
        let r = Rect::new((b.w - lw) / 2, y, lw.max(1), lh);
        b.s.text(r.x, r.y, Face::Small, label, LINK);
        b.s.fill(Rect::new(r.x, r.y + lh - 2, r.w, 1), LINK);
        b.s.hits.push((r, Control::StartWithdraw));
        y += lh + 8;
    }
    y += 16;
    b.button(
        Rect::new(x, y, bw, bh),
        text("official.start.quit"),
        true,
        false,
        Control::StartQuit,
    );
}

fn warning(b: &mut B<'_>, flow: &Flow) {
    let Some(w) = flow.warning() else {
        return;
    };
    let (x, cw) = b.column(860);
    let bh = 46;
    let bottom = b.h - 30 - bh;
    if b.fonts.is_none() {
        // The text cannot be drawn: only Decline (§5.4 rule 7).
        let r = Rect::new(x, bottom, 220.min(cw), bh);
        b.button(r, "", true, true, Control::Warning(WarningControl::Decline));
        return;
    }
    let t = w.text();
    let mut y = 30;
    y = b.para(x, y, cw, Face::Title, &t.title, TEXT) + 12;
    if w.changed() {
        let lines = b.wrap(Face::Body, text("official.consent.changed"), cw - 24);
        let lh = b.line(Face::Body);
        let boxh = lines.len() as i32 * lh + 16;
        b.s.fill(Rect::new(x, y, cw, boxh), CHANGED);
        let mut ty = y + 8;
        for l in lines {
            b.s.text(x + 12, ty, Face::Body, l, TEXT);
            ty += lh;
        }
        y += boxh + 12;
    }
    // The check box and its label sit above the buttons.
    let box_side = b.line(Face::Body).clamp(14, 40);
    let label_lines = b.wrap(Face::Body, &t.checkbox, cw - box_side - 12);
    let label_h = (label_lines.len() as i32 * b.line(Face::Body)).max(box_side);
    let check_y = bottom - 20 - label_h;
    let body_bottom = check_y - 16;
    // The body in a scrollable area.
    let body = Rect::new(x, y, cw, (body_bottom - y).max(b.line(Face::Body)));
    b.s.fill(body, PANEL);
    let lines = b.wrap(Face::Body, &t.body, cw - 32);
    let lh = b.line(Face::Body);
    let visible = ((body.h - 24) / lh).max(1) as usize;
    let scroll_max = lines.len().saturating_sub(visible) as u32;
    let scroll = w.scroll().min(scroll_max) as usize;
    let mut ty = body.y + 12;
    for l in lines.iter().skip(scroll).take(visible) {
        b.s.text(x + 16, ty, Face::Body, l.clone(), TEXT);
        ty += lh;
    }
    if scroll_max > 0 {
        // A scroll indicator on the right edge.
        let track = body.h - 8;
        let thumb = (track * visible as i32 / lines.len().max(1) as i32).max(12);
        let pos = (track - thumb) * scroll as i32 / scroll_max.max(1) as i32;
        b.s.fill(
            Rect::new(body.x + body.w - 6, body.y + 4 + pos, 3, thumb),
            DIM_TEXT,
        );
    }
    b.s.warning_scroll_max = scroll_max;
    b.s.warning_text_ready = true;
    // Check box.
    let cb = Rect::new(x, check_y, box_side, box_side);
    b.s.fill(cb, ENTRY);
    b.s.frame(cb, TEXT, 1);
    if w.ticked() {
        b.s.fill(
            Rect::new(cb.x + 4, cb.y + 4, (cb.w - 8).max(1), (cb.h - 8).max(1)),
            TEXT,
        );
    }
    let mut ly = check_y;
    for l in label_lines {
        b.s.text(x + box_side + 12, ly, Face::Body, l, TEXT);
        ly += b.line(Face::Body);
    }
    if w.focus() == WarningControl::CheckBox {
        b.s.frame(Rect::new(cb.x - 3, cb.y - 3, cw + 6, label_h + 6), FOCUS, 2);
    }
    b.s.hits.push((
        Rect::new(x, check_y, cw, label_h),
        Control::Warning(WarningControl::CheckBox),
    ));
    // Decline (left) and Continue (right).
    let bw = ((cw - 24) / 2).max(80);
    b.button(
        Rect::new(x, bottom, bw, bh),
        &t.decline,
        true,
        w.focus() == WarningControl::Decline,
        Control::Warning(WarningControl::Decline),
    );
    b.button(
        Rect::new(x + cw - bw, bottom, bw, bh),
        &t.accept,
        w.continue_enabled(),
        w.focus() == WarningControl::Continue,
        Control::Warning(WarningControl::Continue),
    );
}

fn entry(b: &mut B<'_>, r: Rect, shown: &str, focused: bool, c: Control) {
    b.s.fill(r, ENTRY);
    b.s.frame(
        r,
        if focused { FOCUS } else { DIM_TEXT },
        if focused { 2 } else { 1 },
    );
    let lh = b.line(Face::Body);
    b.s.text(r.x + 10, r.y + (r.h - lh) / 2, Face::Body, shown, TEXT);
    b.s.hits.push((r, c));
}

fn form(b: &mut B<'_>, flow: &Flow, now: Instant) {
    let Some(f) = flow.form() else {
        return;
    };
    let (x, cw) = b.column(460);
    let eh = 40;
    let lh = b.line(Face::Body);
    let mut y = (b.h - (2 * (lh + eh + 18) + 46 + 120)).max(20) / 2;
    b.s.text(x, y, Face::Body, text("official.form.account"), TEXT);
    y += lh + 4;
    let account = f.account_text().to_owned();
    entry(
        b,
        Rect::new(x, y, cw, eh),
        &account,
        f.focus() == super::form::FormField::Account,
        Control::Form(FormControl::Field(super::form::FormField::Account)),
    );
    y += eh + 18;
    b.s.text(x, y, Face::Body, text("official.form.password"), TEXT);
    y += lh + 4;
    entry(
        b,
        Rect::new(x, y, cw, eh),
        &f.password_text(),
        f.focus() == super::form::FormField::Password,
        Control::Form(FormControl::Field(super::form::FormField::Password)),
    );
    y += eh + 24;
    let bw = ((cw - 16) / 2).max(60);
    b.button(
        Rect::new(x, y, bw, 46),
        text("official.form.back"),
        true,
        false,
        Control::Form(FormControl::Back),
    );
    b.button(
        Rect::new(x + cw - bw, y, bw, 46),
        text("official.form.login"),
        f.login_enabled(now),
        false,
        Control::Form(FormControl::LogIn),
    );
    y += 46 + 18;
    if let Some(left) = f.rate_limit_left(now) {
        let line = fill(
            text("official.error.rate_limited"),
            &[("seconds", &left.to_string())],
        );
        y = b.para(x, y, cw, Face::Small, &line, FOCUS) + 6;
    }
    b.para(
        x,
        y,
        cw,
        Face::Small,
        text("official.form.notice"),
        DIM_TEXT,
    );
}

fn login(b: &mut B<'_>, flow: &Flow) {
    let Some(p) = flow.login() else {
        return;
    };
    let (x, cw) = b.column(900);
    match p.screen() {
        LoginScreen::Progress { text: t } => {
            let lh = b.line(Face::Body);
            let tw = b.width(Face::Body, t);
            let y = b.h / 2 - lh - 20;
            b.s.text((b.w - tw) / 2, y, Face::Body, t.clone(), TEXT);
            let bw = 200;
            b.button(
                Rect::new((b.w - bw) / 2, y + lh + 24, bw, 46),
                text("official.login.cancel"),
                true,
                false,
                Control::Cancel,
            );
        }
        LoginScreen::Queue { lines } => {
            let lh = b.line(Face::Body);
            let mut y = b.h / 2 - (lines.len() as i32 * lh) - 30;
            for l in lines {
                y = b.para(x, y, cw, Face::Body, l, TEXT);
            }
            y += 24;
            let bw = ((cw - 16) / 2).clamp(60, 240);
            b.button(
                Rect::new(b.w / 2 - bw - 8, y, bw, 46),
                text("official.queue.leave"),
                true,
                false,
                Control::LeaveQueue,
            );
            b.button(
                Rect::new(b.w / 2 + 8, y, bw, 46),
                text("official.login.cancel"),
                true,
                false,
                Control::Cancel,
            );
        }
        LoginScreen::Worlds => {
            let lh = b.line(Face::Body);
            let rh = lh + 10;
            let cols = [0, cw * 30 / 100, cw * 58 / 100, cw * 72 / 100];
            let mut y = 40;
            for (i, id) in [
                "official.worlds.name",
                "official.worlds.status",
                "official.worlds.load",
                "official.worlds.note",
            ]
            .iter()
            .enumerate()
            {
                let cx = cols.get(i).copied().unwrap_or(0);
                b.s.text(x + 8 + cx, y, Face::Body, text(id), DIM_TEXT);
            }
            y += rh;
            let room = ((b.h - y - 120) / rh).max(1) as usize;
            let sel = p.selected().unwrap_or(0);
            let first = sel.saturating_sub(room.saturating_sub(1));
            for (i, row) in p.rows().iter().enumerate().skip(first).take(room) {
                let r = Rect::new(x, y, cw, rh);
                if p.selected() == Some(i) {
                    b.s.fill(r, SELECTED);
                }
                let colour = if row.choosable { TEXT } else { DIM_TEXT };
                let cells = [row.name.as_str(), row.status, row.load, row.note.as_str()];
                for (c, cell) in cells.iter().enumerate() {
                    let cx = cols.get(c).copied().unwrap_or(0);
                    b.s.text(x + 8 + cx, y + 5, Face::Body, *cell, colour);
                }
                if row.choosable {
                    b.s.hits.push((r, Control::WorldRow(i)));
                }
                y += rh;
            }
            let by = b.h - 30 - 46;
            let bw = 200.min((cw - 16) / 2).max(60);
            b.button(
                Rect::new(x, by, bw, 46),
                text("official.login.back"),
                true,
                false,
                Control::WorldsBack,
            );
            let can_join = p
                .selected()
                .and_then(|i| p.rows().get(i))
                .is_some_and(|r| r.choosable);
            b.button(
                Rect::new(x + cw - bw, by, bw, 46),
                text("official.login.join"),
                can_join,
                false,
                Control::Join,
            );
        }
    }
}

fn characters(b: &mut B<'_>) {
    let (x, cw) = b.column(560);
    let y = b.para(
        x,
        b.h / 2 - 60,
        cw,
        Face::Body,
        text("official.characters.unavailable"),
        TEXT,
    );
    let bw = 200;
    b.button(
        Rect::new((b.w - bw) / 2, y + 24, bw, 46),
        text("official.form.back"),
        true,
        false,
        Control::CharactersBack,
    );
}

/// A message panel with OK over the screen (§8.6); it takes every input while it shows.
fn notice(b: &mut B<'_>, message: &str, detail: Option<&str>) {
    b.s.hits.clear();
    b.s.fill(Rect::new(0, 0, b.w, b.h), SHADE);
    let (x, cw) = b.column(520);
    let inner = cw - 40;
    let mut lines = b.wrap(Face::Body, message, inner).len() as i32 * b.line(Face::Body);
    if let Some(d) = detail {
        lines += b.wrap(Face::Small, d, inner).len() as i32 * b.line(Face::Small) + 10;
    }
    let ph = lines + 40 + 46 + 20;
    let py = (b.h - ph) / 2;
    b.s.fill(Rect::new(x, py, cw, ph), PANEL);
    b.s.frame(Rect::new(x, py, cw, ph), DIM_TEXT, 1);
    let mut y = b.para(x + 20, py + 20, inner, Face::Body, message, TEXT);
    if let Some(d) = detail {
        y = b.para(x + 20, y + 10, inner, Face::Small, d, DIM_TEXT);
    }
    let bw = 140;
    b.button(
        Rect::new(x + (cw - bw) / 2, y + 20, bw, 46),
        text("official.ok"),
        true,
        true,
        Control::MessageOk,
    );
}

/// Renders `scene` into an RGBA image of `size` physical pixels (each side at most
/// [`MAX_CANVAS`]): layout units times the scene's scale factor, text at the fonts' own pixel
/// size, so it stays crisp.
#[must_use]
pub fn paint(scene: &Scene, fonts: Option<&Fonts>, size: (u32, u32)) -> (u32, u32, Vec<u8>) {
    let w = size.0.clamp(1, MAX_CANVAS);
    let h = size.1.clamp(1, MAX_CANVAS);
    let k = scene.scale();
    let mut px = vec![0u8; (w as usize) * (h as usize) * 4];
    let mut canvas = Canvas {
        w: w as i32,
        h: h as i32,
        px: &mut px,
    };
    let to_px = |v: i32| (f64::from(v) * k).round() as i32;
    let scaled = |r: Rect| {
        let x0 = to_px(r.x);
        let y0 = to_px(r.y);
        Rect::new(x0, y0, to_px(r.x + r.w) - x0, to_px(r.y + r.h) - y0)
    };
    for d in &scene.draws {
        match d {
            Draw::Fill(r, c) => canvas.fill(scaled(*r), *c),
            Draw::Frame(r, c, t) => {
                let r = scaled(*r);
                let t = to_px(*t).max(1);
                canvas.fill(Rect::new(r.x, r.y, r.w, t), *c);
                canvas.fill(Rect::new(r.x, r.y + r.h - t, r.w, t), *c);
                canvas.fill(Rect::new(r.x, r.y, t, r.h), *c);
                canvas.fill(Rect::new(r.x + r.w - t, r.y, t, r.h), *c);
            }
            Draw::Text {
                x,
                y,
                face,
                text,
                colour,
            } => {
                if let Some(f) = fonts {
                    canvas.text(f.face(*face), to_px(*x), to_px(*y), text, *colour);
                }
            }
        }
    }
    (w, h, px)
}

struct Canvas<'a> {
    w: i32,
    h: i32,
    px: &'a mut [u8],
}

impl Canvas<'_> {
    fn blend(&mut self, x: i32, y: i32, c: [u8; 4], cover: u8) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y as usize) * (self.w as usize) + x as usize) * 4;
        let Some(p) = self.px.get_mut(i..i + 4) else {
            return;
        };
        let a = u32::from(c[3]) * u32::from(cover) / 255;
        for k in 0..3 {
            let src = u32::from(c[k]);
            let dst = u32::from(p[k]);
            p[k] = ((src * a + dst * (255 - a)) / 255) as u8;
        }
        p[3] = 255;
    }

    fn fill(&mut self, r: Rect, c: [u8; 4]) {
        let x0 = r.x.max(0);
        let y0 = r.y.max(0);
        let x1 = (r.x + r.w).min(self.w);
        let y1 = (r.y + r.h).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, c, 255);
            }
        }
    }

    fn text(&mut self, face: &FontFace, x: i32, y: i32, s: &str, c: [u8; 4]) {
        let cover = face.foreground();
        let mut pen = x;
        for unit in s.encode_utf16() {
            let Some(g) = face.lookup(unit).glyph else {
                continue;
            };
            let gx = pen + i32::from(g.left_bearing);
            let gy = y + i32::from(g.top);
            for j in 0..i32::from(g.height) {
                for i in 0..i32::from(g.width) {
                    let a = cover.at(i32::from(g.x) + i, i32::from(g.y) + j);
                    if a > 0 {
                        self.blend(gx + i, gy + j, c, a);
                    }
                }
            }
            pen += g.advance();
            if pen > self.w {
                break;
            }
        }
    }
}
