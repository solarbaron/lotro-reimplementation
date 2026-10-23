// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine dialog boxes (`ui-widgets.md` §18, `ui-behaviour.md` §5.5): the wait,
//! confirmation, message and text-input dialogs of layout `Dialog`, one at a time with a
//! queue, filled from a **description** ([`DialogRequest`]), closed by their buttons and keys
//! with the result written back into the description.
//!
//! An owner (a panel of the client) keeps one [`DialogBoxes`]: it asks for a dialog with
//! [`DialogBoxes::request`], hands every UI event to [`DialogBoxes::handle_event`], calls
//! [`DialogBoxes::update`] once per frame (countdowns, the queue) and takes the finished
//! descriptions with [`DialogBoxes::take_results`]. The result reaches the requesting panel
//! this way instead of a class-library callback (§18.6 Tessera rule).
//!
//! What §18 gives, as built here:
//!
//! - **Elements** (§18.1): `DialogWaitField`, `DialogConfirmationField`,
//!   `DialogMessageField` or `DialogTextInputField` of layout `Dialog`, or the
//!   description's layout and element overrides; parts found by element ID below the
//!   dialog, a missing part skipped.
//! - **One at a time** (§18.2): a request is shown at once when no engine dialog is shown,
//!   else queued first in, first out; a `topmost` request takes the shown dialog down
//!   (without a result, back to the front of the queue) unless that one cannot be
//!   overridden, in which case the request goes to the front of the queue. The dialog is
//!   activated and brought to the front unless `StartInactive`.
//! - **Parts** (§18.3), the **countdown** (variable [`COUNTDOWN_VARIABLE`], the stall rule
//!   of [`STALL_GAP`]) and the **results** (§18.5).
//! - **Keys** (§18.4, `ui-behaviour.md` §5.5): the UI core's dialog key handler, given the
//!   dialog's kind with [`Ui::set_dialog_keys`] (`crate::DialogKeys`): a confirmation gives
//!   yes on `AcceptKey` or `AcceptInput` and no on `EscapeKey`; a message gives OK on any of
//!   the three; both only while the dialog is the active panel and the description does not
//!   ignore the keyboard (`UICore_Dialog_ignorekeyboardinput` is read for those two kinds
//!   only). A text input gives done on `AcceptInput` in its entry and quit on `EscapeKey` in
//!   its entry while the quit button is shown; a wait dialog handles no keys. A press and
//!   its release go to the same receiver (§18.4 revision 5, Tessera rule): the release of a
//!   key whose press gave a result is consumed even after the dialog is gone, and the
//!   release of a key pressed before the dialog appeared goes past it. `DialogBoxes` is
//!   pushed while the dialog is active (§18.6 Tessera rule).
//! - **Modal** (§18.4): a modal description's dialog is made modal in the core
//!   ([`Ui::set_dialog_modal`]): while it is shown the core consumes every other key press
//!   and the release of every key pressed meanwhile, so no key reaches the broadcast that
//!   feeds the plugins and game code ([`modal_dialog_shown`] tells a host the same). Keys
//!   held from before the dialog stay held, and their releases go past it. The client's UI
//!   host applies the rule to its own game keys too: `UiHost::modal_dialog_shown` keeps the
//!   chat entry and the game band from keys, `UiHost::keyboard_in_dialog` decides whether
//!   the UI sees a key under the dialog at all, and the host's modal holds consume the
//!   repeats and the release of every key pressed while the dialog was shown, even after it
//!   closed. Callers with raw keys of their own (the walker's world keys, the NPC windows'
//!   Escape and Enter, the map and finder keys) leave them alone while it is shown.
//!
//! **Tessera rules.**
//!
//! - One engine dialog at a time holds across owners: a dialog is "shown" when an element
//!   carrying Tessera's mark ([`MARK_PID`]) is displayed, whoever opened it. A `topmost`
//!   request of one owner takes another owner's dialog down by marking it; that owner puts
//!   it back at the front of its own queue at its next [`DialogBoxes::update`]. Requests of
//!   different owners wait in their own queues; the first owner to update after a dialog
//!   closes shows its next one.
//! - A confirmation whose duration has a magnitude of at most [`NO_ELEMENT_DURATION`] gets
//!   no element and closes at once with the timeout result (§18.3 OPEN rule).
//! - No range check runs offline (§18.3 OPEN rule).
//! - A description that is modal while its element is not (the element's own
//!   `UICore_Dialog_modal` already gives the container of `ui-layout.md` §9.1) gets a
//!   code-made screen-sized container behind it that blocks clicks.
//! - A dialog whose element lies in a modal container (its element is modal, `ui-layout.md`
//!   §9.1) is modal for keys too, whatever the description says.
//! - Without the install's `Dialog` layout (no install, or an element that does not
//!   resolve) the dialog is a code-made element with the §18.1 boxes and Tessera's art
//!   ([`DialogArt`]), centred on the screen. Its question, statement and wait texts are
//!   centred both ways (§18.1 gives that for the confirmation question; Tessera uses it for
//!   all three) and its button labels are centred (Tessera choice).
//! - A shown dialog whose element the core hid on a key while the result event never
//!   reached [`DialogBoxes::handle_event`] (an owner that filters its events) is shown
//!   again at the next [`DialogBoxes::update`] instead of waiting hidden forever; the key
//!   press is lost, the buttons still answer it.
//! - The `Button_click` handlers follow the names in use: [`DialogBoxes::set_refs`] takes
//!   effect at the next request or update, and the names a shown dialog was built with keep
//!   their handlers until it closes.
//!
//! # Bounds
//!
//! At most [`MAX_QUEUED`] waiting requests per owner (more are refused, a `topmost` one
//! too; a dialog taken down by another owner's `topmost` request is put back at the front
//! of a full queue by ending the newest waiting request as [`DialogOutcome::Dismissed`])
//! and texts cut at [`MAX_TEXT_CHARS`] characters.

use std::collections::VecDeque;

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;

use super::context_menu::label_table;
use crate::env::ImageRef;
use crate::events::UiEvent;
use crate::game::GameError;
use crate::game::chat::notice::{Arg, TextRef, TextSource};
use crate::game::npc::kit::{FNode, FallbackArt, colours, open_root};
use crate::text::JUSTIFY_CENTER;
use crate::tree::ElementKey;
use crate::ui::Ui;

/// The string variable set to the remaining whole seconds of a countdown (§18.3, W30).
pub const COUNTDOWN_VARIABLE: u32 = 0x0007_B035;

/// A confirmation whose duration has at most this magnitude gets no element (§18.3, W30).
pub const NO_ELEMENT_DURATION: f64 = 0.0002;

/// More than this many seconds between two dialog updates move the end time later by the
/// gap (§18.3, W30).
pub const STALL_GAP: f64 = 1.0;

/// The repeat of a range check, seconds (§18.3, W30; Tessera runs none offline).
pub const RANGE_CHECK_INTERVAL: f64 = 2.0;

/// Most waiting requests per owner (Tessera bound).
pub const MAX_QUEUED: usize = 64;

/// Most characters of a dialog text (Tessera bound).
pub const MAX_TEXT_CHARS: usize = 4096;

/// The `UILAYOUT` label of the dialog layout (§18.1, `Dialog`).
pub const LAYOUT_LABEL: &str = "Dialog";

/// Tessera's run-time mark on a shown engine dialog's element (a placeholder PID no
/// dictionary uses): an integer of [`MARK_SHOWN`], [`MARK_CANNOT_OVERRIDE`] and
/// [`MARK_TAKEN_DOWN`] bits.
pub const MARK_PID: u32 = 0xFFFE_7D01;
/// The element is a shown engine dialog.
pub const MARK_SHOWN: i32 = 1;
/// Its description has `UICore_Dialog_cannotbeoverridden` true.
pub const MARK_CANNOT_OVERRIDE: i32 = 2;
/// A `topmost` request of another owner took it down (its owner requeues it).
pub const MARK_TAKEN_DOWN: i32 = 4;

/// The engine dialog types (`UICore_Dialog_type`, §18.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogKind {
    /// `Wait` (1).
    Wait,
    /// `Confirmation` (2).
    Confirmation,
    /// `Message` (3).
    Message,
    /// `TextInput` (4).
    TextInput,
}

impl DialogKind {
    /// The kind of a `UICore_Dialog_type` value; the engine builds only 1–4.
    #[must_use]
    pub fn from_raw(v: u32) -> Option<Self> {
        Some(match v {
            1 => Self::Wait,
            2 => Self::Confirmation,
            3 => Self::Message,
            4 => Self::TextInput,
            _ => return None,
        })
    }
}

/// A text of a description (a StringInfo in the original).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogText {
    /// A rendered text.
    Plain(String),
    /// A string-table entry, rendered through the owner's [`TextSource`] (a countdown sets
    /// [`COUNTDOWN_VARIABLE`] on it).
    Ref(TextRef),
}

impl DialogText {
    /// A rendered text.
    #[must_use]
    pub fn plain(s: impl Into<String>) -> Self {
        Self::Plain(s.into())
    }

    /// The text, with the countdown variable set to `seconds` when given.
    pub fn render(&self, src: &mut dyn TextSource, seconds: Option<i64>) -> String {
        let s = match self {
            Self::Plain(s) => s.clone(),
            Self::Ref(r) => {
                let mut r = r.clone();
                if let Some(n) = seconds {
                    r.args.retain(|(id, _)| *id != COUNTDOWN_VARIABLE);
                    r.args.push((COUNTDOWN_VARIABLE, Arg::Int(n)));
                }
                r.render(src)
            }
        };
        s.chars().take(MAX_TEXT_CHARS).collect()
    }
}

/// How a dialog ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialogOutcome {
    /// Confirmation yes.
    Yes,
    /// Confirmation no.
    No,
    /// Confirmation or message timeout.
    TimedOut,
    /// Message OK.
    Ok,
    /// Text input done.
    Done,
    /// Text input quit.
    Quit,
    /// Closed by its owner without a result ([`DialogBoxes::dismiss`]: a wait dialog
    /// removed by game code, or the owner going away).
    Dismissed,
}

/// A dialog description (§18.1–§18.5): what to show, the queue flags and, after a result,
/// the result properties.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogRequest {
    /// `UICore_Dialog_type`.
    pub kind: DialogKind,
    /// The owner's own token, passed back with the result (Tessera).
    pub tag: u64,
    /// `UICore_Dialog_UILayout_Override`.
    pub layout_override: Option<Did>,
    /// `UICore_Dialog_UIElement_Override`.
    pub element_override: Option<u32>,
    /// `UICore_Dialog_topmost`.
    pub topmost: bool,
    /// `UICore_Dialog_cannotbeoverridden`.
    pub cannot_be_overridden: bool,
    /// `UICore_Dialog_StartInactive`.
    pub start_inactive: bool,
    /// `UICore_Dialog_modal`.
    pub modal: bool,
    /// `UICore_Dialog_ignorekeyboardinput`.
    pub ignore_keyboard: bool,
    /// `UICore_Dialog_wait_text`.
    pub wait_text: Option<DialogText>,
    /// `UICore_Dialog_confirmation_questiontext`.
    pub question: Option<DialogText>,
    /// `UICore_Dialog_confirmation_yesbuttontext`.
    pub yes_text: Option<DialogText>,
    /// `UICore_Dialog_confirmation_nobuttontext`.
    pub no_text: Option<DialogText>,
    /// `UICore_Dialog_confirmation_duration` / `…message_duration`, seconds.
    pub duration: Option<f64>,
    /// `UICore_Dialog_message_statementtext`.
    pub statement: Option<DialogText>,
    /// `UICore_Dialog_message_okbuttontext`.
    pub ok_text: Option<DialogText>,
    /// `UICore_Dialog_title_text`.
    pub title: Option<DialogText>,
    /// `UICore_Dialog_textinput_questiontext`.
    pub input_question: Option<DialogText>,
    /// `UICore_Dialog_icon_field` (an image).
    pub icon: Option<ImageRef>,
    /// `UICore_Dialog_icon_text`.
    pub icon_text: Option<DialogText>,
    /// `UICore_Dialog_textinput_donebuttontext`.
    pub done_text: Option<DialogText>,
    /// `UICore_Dialog_textinput_cancelbuttontext`.
    pub cancel_text: Option<DialogText>,
    /// `UICore_Dialog_textinput_displaycancelbutton` (default true).
    pub display_cancel: bool,
    /// `UICore_Dialog_textinput_response`: the entry's initial text, and after a result the
    /// entered text (empty after quit).
    pub response: String,
    /// `UICore_Dialog_textinput_activateandtakefocus`.
    pub activate_and_take_focus: bool,
    /// `UICore_Dialog_textinput_maxchars` (> 0 sets the entry's `UICore_Text_max_chars`).
    pub max_chars: i32,
    /// `UICore_Dialog_Range_Check_Entity` and `…_Distance` (recorded; no check offline).
    pub range_check: Option<(u64, f32)>,
    /// Result: `UICore_Dialog_confirmation_response`.
    pub confirmation_response: Option<bool>,
    /// Result: `UICore_Dialog_confirmation_timedout`.
    pub confirmation_timed_out: bool,
    /// Result: `UICore_Dialog_message_response`.
    pub message_response: Option<bool>,
    /// How the dialog ended (Tessera; `None` while it waits or is shown).
    pub outcome: Option<DialogOutcome>,
}

impl DialogRequest {
    /// A description of `kind` with the dictionary defaults.
    #[must_use]
    pub fn new(kind: DialogKind, tag: u64) -> Self {
        Self {
            kind,
            tag,
            layout_override: None,
            element_override: None,
            topmost: false,
            cannot_be_overridden: false,
            start_inactive: false,
            modal: false,
            ignore_keyboard: false,
            wait_text: None,
            question: None,
            yes_text: None,
            no_text: None,
            duration: None,
            statement: None,
            ok_text: None,
            title: None,
            input_question: None,
            icon: None,
            icon_text: None,
            done_text: None,
            cancel_text: None,
            display_cancel: true,
            response: String::new(),
            activate_and_take_focus: false,
            max_chars: 0,
            range_check: None,
            confirmation_response: None,
            confirmation_timed_out: false,
            message_response: None,
            outcome: None,
        }
    }

    /// A confirmation with its question.
    #[must_use]
    pub fn confirmation(tag: u64, question: DialogText) -> Self {
        let mut r = Self::new(DialogKind::Confirmation, tag);
        r.question = Some(question);
        r
    }

    /// A message with its statement.
    #[must_use]
    pub fn message(tag: u64, statement: DialogText) -> Self {
        let mut r = Self::new(DialogKind::Message, tag);
        r.statement = Some(statement);
        r
    }

    /// A text input with its question.
    #[must_use]
    pub fn text_input(tag: u64, question: DialogText) -> Self {
        let mut r = Self::new(DialogKind::TextInput, tag);
        r.input_question = Some(question);
        r
    }

    /// A wait dialog with its text.
    #[must_use]
    pub fn wait(tag: u64, text: DialogText) -> Self {
        let mut r = Self::new(DialogKind::Wait, tag);
        r.wait_text = Some(text);
        r
    }

    /// Writes the result properties of `outcome` (§18.5); `text` is the entry's text for
    /// a text input's done.
    fn finish(&mut self, outcome: DialogOutcome, text: Option<String>) {
        match outcome {
            DialogOutcome::Yes => self.confirmation_response = Some(true),
            DialogOutcome::No => self.confirmation_response = Some(false),
            DialogOutcome::TimedOut => {
                if self.kind == DialogKind::Message {
                    self.message_response = Some(false);
                } else {
                    self.confirmation_response = Some(false);
                    self.confirmation_timed_out = true;
                }
            }
            DialogOutcome::Ok => self.message_response = Some(true),
            DialogOutcome::Done => self.response = text.unwrap_or_default(),
            DialogOutcome::Quit => self.response.clear(),
            DialogOutcome::Dismissed => {}
        }
        self.outcome = Some(outcome);
    }
}

label_table! {
    /// Element IDs of layout `Dialog` (`UIElementID` labels, §18.1).
    DialogIds {
        /// `DialogWaitField`.
        wait_field = ("DialogWaitField", 0xFFFB_7101),
        /// `DialogWaitText`.
        wait_text = ("DialogWaitText", 0xFFFB_7102),
        /// `DialogConfirmationField`.
        confirmation_field = ("DialogConfirmationField", 0xFFFB_7103),
        /// `DialogConfirmationQuestionText`.
        question = ("DialogConfirmationQuestionText", 0xFFFB_7104),
        /// `DialogConfirmationYesButton`.
        yes = ("DialogConfirmationYesButton", 0xFFFB_7105),
        /// `DialogConfirmationNoButton`.
        no = ("DialogConfirmationNoButton", 0xFFFB_7106),
        /// `DialogMessageField`.
        message_field = ("DialogMessageField", 0xFFFB_7107),
        /// `DialogMessageStatementText`.
        statement = ("DialogMessageStatementText", 0xFFFB_7108),
        /// `DialogMessageOkButton`.
        ok = ("DialogMessageOkButton", 0xFFFB_7109),
        /// `DialogTextInputField`.
        text_input_field = ("DialogTextInputField", 0xFFFB_710A),
        /// `DialogTitleText`.
        title = ("DialogTitleText", 0xFFFB_710B),
        /// `DialogTextInputQuestionText`.
        input_question = ("DialogTextInputQuestionText", 0xFFFB_710C),
        /// `DialogIconField`.
        icon_field = ("DialogIconField", 0xFFFB_710D),
        /// `DialogIconText`.
        icon_text = ("DialogIconText", 0xFFFB_710E),
        /// `DialogTextInputTextEntry`.
        entry = ("DialogTextInputTextEntry", 0xFFFB_710F),
        /// `DialogTextInputDoneButton`.
        done = ("DialogTextInputDoneButton", 0xFFFB_7110),
        /// `DialogTextInputQuitButton`.
        quit = ("DialogTextInputQuitButton", 0xFFFB_7111),
    }
}

label_table! {
    /// Property IDs the fallback dialogs write (property-dictionary names).
    DialogPids {
        /// `UICore_Text_font_color` (the same placeholder as the P3 vocabulary's).
        text_font_color = ("UICore_Text_font_color", 0xFFFD_3006),
        /// `UICore_Text_one_line` (the same placeholder as the P3 vocabulary's).
        text_one_line = ("UICore_Text_one_line", 0xFFFD_3010),
        /// `UICore_Text_horizontal_justification` (the same placeholder as the combat
        /// look's).
        text_horizontal = ("UICore_Text_horizontal_justification", 0xFFFD_2F04),
        /// `UICore_Text_vertical_justification` (the same placeholder as the combat look's).
        text_vertical = ("UICore_Text_vertical_justification", 0xFFFD_2F05),
    }
}

/// Tessera's own art of the fallback dialogs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogArt {
    /// The dialog frame.
    pub window: Option<ImageRef>,
    /// A button.
    pub button: Option<ImageRef>,
    /// A ghosted button.
    pub button_ghosted: Option<ImageRef>,
    /// The text entry's frame.
    pub entry: Option<ImageRef>,
}

impl DialogArt {
    /// The dialog art taken from the fallback widgets' art.
    #[must_use]
    pub fn from_fallback(a: &FallbackArt) -> Self {
        Self {
            window: a.window.clone(),
            button: a.button.clone(),
            button_ghosted: a.button_ghosted.clone(),
            entry: a.entry.clone(),
        }
    }
}

/// The install's names of the engine dialogs, or the fallback's placeholders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogRefs {
    /// Element IDs.
    pub ids: DialogIds,
    /// Property IDs.
    pub pids: DialogPids,
    /// The `Dialog` layout, when it resolves (else the dialogs are code-made).
    pub layout: Option<Did>,
    /// The fallback dialogs' art.
    pub art: DialogArt,
}

impl DialogRefs {
    /// No install: placeholder IDs, code-made dialogs.
    #[must_use]
    pub fn fallback(art: DialogArt) -> Self {
        Self {
            art,
            ..Self::default()
        }
    }

    /// Resolves the names through `layout` (`UILAYOUT` label → DID), `element`
    /// (`UIElementID` label → ID) and `pid` (property name → PID). The layout is kept only
    /// when it is a layout DID (prefix 0x22) and the four dialog elements resolve.
    #[must_use]
    pub fn resolve(
        layout: &dyn Fn(&str) -> Option<Did>,
        element: &dyn Fn(&str) -> Option<u32>,
        pid: &dyn Fn(&str) -> Option<u32>,
        art: DialogArt,
    ) -> Self {
        let mut ids = DialogIds::default();
        ids.resolve_with(&mut |l| element(l));
        let mut pids = DialogPids::default();
        pids.resolve_with(&mut |n| pid(n));
        let complete = [
            "DialogWaitField",
            "DialogConfirmationField",
            "DialogMessageField",
            "DialogTextInputField",
        ]
        .iter()
        .all(|l| element(l).is_some());
        let layout = layout(LAYOUT_LABEL).filter(|d| d.0 >> 24 == 0x22 && complete);
        Self {
            ids,
            pids,
            layout,
            art,
        }
    }

    /// The dialog element of `kind`.
    #[must_use]
    pub fn element_of(&self, kind: DialogKind) -> u32 {
        match kind {
            DialogKind::Wait => self.ids.wait_field,
            DialogKind::Confirmation => self.ids.confirmation_field,
            DialogKind::Message => self.ids.message_field,
            DialogKind::TextInput => self.ids.text_input_field,
        }
    }
}

/// The parts of a shown dialog.
#[derive(Debug, Clone, Copy, Default)]
struct Parts {
    /// The text the countdown renders (question or statement).
    text: Option<ElementKey>,
    yes: Option<ElementKey>,
    no: Option<ElementKey>,
    ok: Option<ElementKey>,
    done: Option<ElementKey>,
    quit: Option<ElementKey>,
    entry: Option<ElementKey>,
}

/// The shown dialog.
#[derive(Debug, Clone)]
struct Shown {
    req: DialogRequest,
    /// The element IDs the dialog was built with (its buttons' handlers stay registered
    /// until it closes, whatever [`DialogBoxes::set_refs`] does meanwhile).
    ids: DialogIds,
    root: ElementKey,
    container: Option<ElementKey>,
    parts: Parts,
    end: Option<f64>,
    last_update: f64,
    last_whole: Option<i64>,
}

/// One owner's engine dialogs (module docs).
#[derive(Debug, Clone)]
pub struct DialogBoxes {
    refs: DialogRefs,
    shown: Option<Shown>,
    queue: VecDeque<DialogRequest>,
    done: Vec<DialogRequest>,
    handlers: Vec<(u32, u32)>,
}

/// Whether an engine dialog (of any owner) is displayed, and whether it cannot be
/// overridden: the displayed element with Tessera's mark ([`MARK_PID`]).
fn shown_elsewhere(ui: &Ui, except: Option<ElementKey>) -> Option<(ElementKey, bool)> {
    ui.focus
        .dialogs
        .iter()
        .copied()
        .filter(|k| Some(*k) != except && ui.is_displayed(*k))
        .find_map(|k| {
            let m = ui.prop(k, MARK_PID).int();
            (m & MARK_SHOWN != 0 && m & MARK_TAKEN_DOWN == 0)
                .then_some((k, m & MARK_CANNOT_OVERRIDE != 0))
        })
}

/// Whether an engine dialog is shown (any owner).
#[must_use]
pub fn engine_dialog_shown(ui: &Ui) -> bool {
    shown_elsewhere(ui, None).is_some()
}

/// Whether a shown engine dialog is modal (§18.4: no other key reaches game code below
/// it; the core consumes those keys, [`Ui::modal_dialog`]).
#[must_use]
pub fn modal_dialog_shown(ui: &Ui) -> bool {
    ui.modal_dialog().is_some_and(|k| {
        let m = ui.prop(k, MARK_PID).int();
        m & MARK_SHOWN != 0 && m & MARK_TAKEN_DOWN == 0
    })
}

impl DialogBoxes {
    /// An owner's dialogs over `refs`.
    #[must_use]
    pub fn new(refs: DialogRefs) -> Self {
        Self {
            refs,
            shown: None,
            queue: VecDeque::new(),
            done: Vec::new(),
            handlers: Vec::new(),
        }
    }

    /// The names in use.
    #[must_use]
    pub fn refs(&self) -> &DialogRefs {
        &self.refs
    }

    /// Replaces the names (the install's names arrived); a shown dialog keeps its element.
    /// The button handlers move to the new element IDs at the next [`Self::request`] or
    /// [`Self::update`] (the shown dialog's own IDs keep theirs until it closes).
    pub fn set_refs(&mut self, refs: DialogRefs) {
        self.refs = refs;
    }

    /// The shown dialog's element and description.
    #[must_use]
    pub fn shown(&self) -> Option<(ElementKey, &DialogRequest)> {
        self.shown.as_ref().map(|s| (s.root, &s.req))
    }

    /// The waiting requests, front first.
    #[must_use]
    pub fn queued(&self) -> Vec<&DialogRequest> {
        self.queue.iter().collect()
    }

    /// The shown dialog's text entry (a text input).
    #[must_use]
    pub fn entry(&self) -> Option<ElementKey> {
        self.shown.as_ref().and_then(|s| s.parts.entry)
    }

    /// The shown dialog's button for `outcome` (yes, no, OK, done, quit).
    #[must_use]
    pub fn button(&self, outcome: DialogOutcome) -> Option<ElementKey> {
        let p = self.shown.as_ref()?.parts;
        match outcome {
            DialogOutcome::Yes => p.yes,
            DialogOutcome::No => p.no,
            DialogOutcome::Ok => p.ok,
            DialogOutcome::Done => p.done,
            DialogOutcome::Quit => p.quit,
            _ => None,
        }
    }

    /// Takes the descriptions that got a result since the last call, in order.
    pub fn take_results(&mut self) -> Vec<DialogRequest> {
        std::mem::take(&mut self.done)
    }

    /// Asks for a dialog (§18.2). A request is refused (logged) when [`MAX_QUEUED`]
    /// requests already wait.
    pub fn request(&mut self, ui: &mut Ui, req: DialogRequest, src: &mut dyn TextSource) {
        self.register(ui);
        let mine = self
            .shown
            .as_ref()
            .map(|s| (s.root, s.req.cannot_be_overridden));
        let other = shown_elsewhere(ui, mine.map(|m| m.0));
        let shown = mine.or(other);
        match shown {
            None => {
                if self.queue.len() >= MAX_QUEUED {
                    tracing::warn!("dialog queue full; request {} refused", req.tag);
                } else {
                    self.queue.push_back(req);
                }
                self.show_next(ui, src);
            }
            Some(_) if !req.topmost => {
                if self.queue.len() >= MAX_QUEUED {
                    tracing::warn!("dialog queue full; request {} refused", req.tag);
                } else {
                    self.queue.push_back(req);
                }
            }
            Some((_, true)) => {
                if self.queue.len() >= MAX_QUEUED {
                    tracing::warn!("dialog queue full; request {} refused", req.tag);
                } else {
                    self.queue.push_front(req);
                }
            }
            Some((k, false)) => {
                if mine.is_some() {
                    // The shown description goes back to the queue: one more waits.
                    if self.queue.len() >= MAX_QUEUED {
                        tracing::warn!("dialog queue full; topmost request {} refused", req.tag);
                        return;
                    }
                    if let Some(s) = self.shown.take() {
                        self.remove_elements(ui, &s);
                        self.queue.push_front(s.req);
                    }
                } else {
                    let m = ui.prop(k, MARK_PID).int();
                    ui.set_property(k, MARK_PID, PropertyValue::Int(m | MARK_TAKEN_DOWN));
                    ui.set_shown(k, false);
                }
                self.queue.push_front(req);
                self.show_next(ui, src);
            }
        }
    }

    /// Closes the shown dialog without a result (a wait dialog that game code removes) and
    /// reports it as [`DialogOutcome::Dismissed`]; the next request is shown.
    pub fn dismiss(&mut self, ui: &mut Ui, src: &mut dyn TextSource) {
        if let Some(mut s) = self.shown.take() {
            self.remove_elements(ui, &s);
            s.req.finish(DialogOutcome::Dismissed, None);
            self.done.push(s.req);
        }
        self.show_next(ui, src);
    }

    /// Closes everything (the owner goes away): the shown dialog and every waiting request
    /// end as [`DialogOutcome::Dismissed`], and the handlers are released.
    pub fn close_all(&mut self, ui: &mut Ui) {
        if let Some(mut s) = self.shown.take() {
            self.remove_elements(ui, &s);
            s.req.finish(DialogOutcome::Dismissed, None);
            self.done.push(s.req);
        }
        for mut r in self.queue.drain(..) {
            r.finish(DialogOutcome::Dismissed, None);
            self.done.push(r);
        }
        for (id, msg) in self.handlers.drain(..) {
            ui.unregister_handler(id, msg);
        }
    }

    /// The `Button_click` registrations needed now: the buttons of the names in use and
    /// those of the shown dialog's element.
    fn wanted_handlers(&self, ui: &Ui) -> Vec<(u32, u32)> {
        let m = ui.vocab().msgs.button_click;
        let mut out = Vec::new();
        for ids in std::iter::once(self.refs.ids).chain(self.shown.as_ref().map(|s| s.ids)) {
            for id in [ids.yes, ids.no, ids.ok, ids.done, ids.quit] {
                if id != 0 && !out.contains(&(id, m)) {
                    out.push((id, m));
                }
            }
        }
        out
    }

    /// Registers the wanted handlers and releases the ones no longer wanted (the names
    /// changed and the dialog built with the old ones closed).
    fn register(&mut self, ui: &mut Ui) {
        let want = self.wanted_handlers(ui);
        if want.len() == self.handlers.len() && want.iter().all(|h| self.handlers.contains(h)) {
            return;
        }
        self.handlers.retain(|h| {
            let keep = want.contains(h);
            if !keep {
                ui.unregister_handler(h.0, h.1);
            }
            keep
        });
        for h in want {
            if !self.handlers.contains(&h) {
                ui.register_handler(h.0, h.1);
                self.handlers.push(h);
            }
        }
    }

    fn remove_elements(&self, ui: &mut Ui, s: &Shown) {
        if ui.exists(s.root) {
            ui.destroy(s.root);
        }
        if let Some(c) = s.container
            && ui.exists(c)
        {
            ui.destroy(c);
        }
    }

    /// Shows the request at the front of the queue when no engine dialog is shown.
    fn show_next(&mut self, ui: &mut Ui, src: &mut dyn TextSource) {
        let mut guard = 0;
        while self.shown.is_none() && shown_elsewhere(ui, None).is_none() {
            guard += 1;
            if guard > MAX_QUEUED + 1 {
                break;
            }
            let Some(req) = self.queue.pop_front() else {
                return;
            };
            self.show(ui, req, src);
        }
    }

    /// Shows one description (§18.2, §18.3).
    fn show(&mut self, ui: &mut Ui, mut req: DialogRequest, src: &mut dyn TextSource) {
        if req.kind == DialogKind::Confirmation
            && req.duration.is_some_and(|d| d.abs() <= NO_ELEMENT_DURATION)
        {
            // §18.3 OPEN rule: no element, the timeout result at once.
            req.finish(DialogOutcome::TimedOut, None);
            self.done.push(req);
            return;
        }
        let before = ui.active_panel();
        let root = match self.build_install(ui, &req) {
            Ok(k) => k,
            Err(e) => {
                if self.refs.layout.is_some() || req.layout_override.is_some() {
                    tracing::debug!("engine dialog: install layout not used ({e}); fallback");
                }
                match self.build_fallback(ui, req.kind) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("engine dialog not shown: {e}");
                        req.finish(DialogOutcome::Dismissed, None);
                        self.done.push(req);
                        return;
                    }
                }
            }
        };
        let pids = ui.vocab().pids;
        ui.mark_dialog(root);
        let mut mark = MARK_SHOWN;
        if req.cannot_be_overridden {
            mark |= MARK_CANNOT_OVERRIDE;
        }
        ui.set_property(root, MARK_PID, PropertyValue::Int(mark));
        // §18.4: the flag is read for confirmations and messages; a text input reacts to
        // its entry's commands; a wait dialog handles no keys.
        let ignore = match req.kind {
            DialogKind::Confirmation | DialogKind::Message => req.ignore_keyboard,
            DialogKind::TextInput => false,
            DialogKind::Wait => true,
        };
        ui.set_property(
            root,
            pids.dialog_ignore_keyboard,
            PropertyValue::Bool(ignore),
        );
        let container = if req.modal && !in_container(ui, root) {
            modal_container(ui)
        } else {
            None
        };
        let parts = self.fill(ui, root, &req, src);
        // §18.4: the core's key rules for this kind, and the modal key rule.
        let keys = match req.kind {
            DialogKind::Confirmation => crate::DialogKeys::Confirmation,
            DialogKind::Message => crate::DialogKeys::Message,
            DialogKind::TextInput => crate::DialogKeys::TextInput {
                entry: parts.entry,
                quit: parts.quit,
            },
            DialogKind::Wait => crate::DialogKeys::None,
        };
        ui.set_dialog_keys(root, keys);
        ui.set_dialog_modal(root, req.modal || in_container(ui, root));
        if req.start_inactive {
            if let Some(b) = before.filter(|b| ui.exists(*b) && ui.is_displayed(*b))
                && ui.active_panel() != Some(b)
            {
                ui.activate(b);
            }
        } else {
            if let Some(c) = container {
                to_front(ui, c);
            }
            match ui.parent(root).filter(|_| in_container(ui, root)) {
                Some(c) => to_front(ui, c),
                None => to_front(ui, root),
            }
            ui.request_activation(root);
        }
        if req.kind == DialogKind::TextInput && req.activate_and_take_focus {
            ui.request_activation(root);
            if let Some(e) = parts.entry {
                ui.set_focus(Some(e));
            }
        }
        let now = ui.time();
        let end = match req.kind {
            DialogKind::Confirmation | DialogKind::Message => {
                req.duration.filter(|d| d.is_finite()).map(|d| now + d)
            }
            _ => None,
        };
        let mut s = Shown {
            req,
            ids: self.refs.ids,
            root,
            container,
            parts,
            end,
            last_update: now,
            last_whole: None,
        };
        Self::countdown_text(ui, &mut s, src);
        self.shown = Some(s);
    }

    /// The install path: the element of the description's type (or its overrides) opened
    /// as a root panel (`ui-layout.md` §9.1).
    fn build_install(&self, ui: &mut Ui, req: &DialogRequest) -> Result<ElementKey, GameError> {
        let layout = req
            .layout_override
            .or(self.refs.layout)
            .ok_or(GameError::NoLayout("Dialog"))?;
        let element = req
            .element_override
            .unwrap_or_else(|| self.refs.element_of(req.kind));
        Ok(ui.open_panel(layout, element)?)
    }

    /// The fallback path: a code-made dialog with the §18.1 boxes, centred on the screen.
    fn build_fallback(&self, ui: &mut Ui, kind: DialogKind) -> Result<ElementKey, GameError> {
        let ids = self.refs.ids;
        let p = self.refs.pids;
        let art = &self.refs.art;
        let cp = p.text_font_color;
        let st = ui.vocab().states;
        let pids = ui.vocab().pids;
        let (sw, sh) = ui.screen_size();
        let button = |id: u32, x: i32, y: i32, w: i32| {
            FNode::button(
                id,
                x,
                y,
                w,
                20,
                (&art.button, &art.button_ghosted),
                cp,
                st.ghosted,
                "",
            )
        };
        let text = |id: u32, x: i32, y: i32, w: i32, h: i32| {
            FNode::text(id, x, y, w, h, cp, colours::BODY, "")
        };
        // The question, statement and wait texts: centred both ways (module docs).
        let centred = |id: u32, x: i32, y: i32, w: i32, h: i32| {
            text(id, x, y, w, h)
                .prop(p.text_horizontal, PropertyValue::Enum(JUSTIFY_CENTER))
                .prop(p.text_vertical, PropertyValue::Enum(JUSTIFY_CENTER))
        };
        let (h, node) = match kind {
            DialogKind::Wait => (
                200,
                FNode::field(ids.wait_field, 0, 0, 512, 200)
                    .image(&art.window)
                    .child(centred(ids.wait_text, 10, 22, 493, 121)),
            ),
            DialogKind::Confirmation => (
                183,
                FNode::field(ids.confirmation_field, 0, 0, 512, 183)
                    .image(&art.window)
                    .child(centred(ids.question, 6, 7, 500, 142))
                    .child(button(ids.yes, 144, 156, 106))
                    .child(button(ids.no, 263, 156, 106)),
            ),
            DialogKind::Message => (
                200,
                FNode::field(ids.message_field, 0, 0, 512, 200)
                    .image(&art.window)
                    .child(centred(ids.statement, 10, 22, 493, 121))
                    .child(button(ids.ok, 192, 170, 96)),
            ),
            DialogKind::TextInput => (
                180,
                FNode::field(ids.text_input_field, 0, 0, 512, 180)
                    .image(&art.window)
                    .child(text(ids.title, 13, 4, 489, 18).hidden())
                    .child(text(ids.input_question, 13, 25, 489, 68))
                    .child(
                        FNode::field(0, 10, 96, 492, 24).image(&art.entry).child(
                            FNode::text(ids.entry, 2, 2, 488, 20, cp, colours::WHITE, "")
                                .prop(pids.text_editable, PropertyValue::Bool(true))
                                .prop(pids.text_selectable, PropertyValue::Bool(true))
                                .prop(pids.text_max_chars, PropertyValue::Int(100))
                                .prop(p.text_one_line, PropertyValue::Bool(true)),
                        ),
                    )
                    .child(button(ids.done, 184, 151, 64))
                    .child(button(ids.quit, 264, 151, 64)),
            ),
        };
        let root = open_root(ui, &node)?;
        ui.move_to(
            root,
            sw.saturating_sub(512).max(0) / 2,
            sh.saturating_sub(h).max(0) / 2,
        );
        // Tessera's own button labels (the layout's labels are not available).
        let labels: [(u32, &str); 5] = [
            (ids.yes, "Yes"),
            (ids.no, "No"),
            (ids.ok, "OK"),
            (ids.done, "OK"),
            (ids.quit, "Cancel"),
        ];
        for (id, l) in labels {
            if let Some(b) = ui.find_descendant(root, id) {
                ui.set_text(b, Some(l.to_string()));
                // Centred labels (Tessera choice, module docs).
                for (pid, v) in [
                    (p.text_horizontal, JUSTIFY_CENTER),
                    (p.text_vertical, JUSTIFY_CENTER),
                ] {
                    ui.set_property(b, pid, PropertyValue::Enum(v));
                }
            }
        }
        Ok(root)
    }

    /// Fills the parts from the description (§18.3).
    fn fill(
        &self,
        ui: &mut Ui,
        root: ElementKey,
        req: &DialogRequest,
        src: &mut dyn TextSource,
    ) -> Parts {
        let ids = self.refs.ids;
        let find = |ui: &Ui, id: u32| crate::game::find_below(ui, root, id);
        let mut put = |ui: &mut Ui, k: Option<ElementKey>, t: &Option<DialogText>| {
            if let (Some(k), Some(t)) = (k, t) {
                let s = t.render(src, None);
                ui.set_text(k, Some(s));
            }
        };
        let mut parts = Parts::default();
        match req.kind {
            DialogKind::Wait => {
                let t = find(ui, ids.wait_text);
                put(ui, t, &req.wait_text);
            }
            DialogKind::Confirmation => {
                parts.text = find(ui, ids.question);
                parts.yes = find(ui, ids.yes);
                parts.no = find(ui, ids.no);
                put(ui, parts.text, &req.question);
                put(ui, parts.yes, &req.yes_text);
                put(ui, parts.no, &req.no_text);
            }
            DialogKind::Message => {
                parts.text = find(ui, ids.statement);
                parts.ok = find(ui, ids.ok);
                put(ui, parts.text, &req.statement);
                put(ui, parts.ok, &req.ok_text);
            }
            DialogKind::TextInput => {
                let title = find(ui, ids.title);
                put(ui, title, &req.title);
                if let (Some(t), Some(_)) = (title, &req.title) {
                    ui.set_shown(t, true);
                }
                let q = find(ui, ids.input_question);
                put(ui, q, &req.input_question);
                if let (Some(icon), Some(img)) = (find(ui, ids.icon_field), &req.icon) {
                    crate::game::set_image_ref(ui, icon, img, 4);
                }
                let it = find(ui, ids.icon_text);
                put(ui, it, &req.icon_text);
                parts.done = find(ui, ids.done);
                parts.quit = find(ui, ids.quit);
                parts.entry = find(ui, ids.entry);
                put(ui, parts.done, &req.done_text);
                put(ui, parts.quit, &req.cancel_text);
                if let Some(q) = parts.quit {
                    ui.set_shown(q, req.display_cancel);
                }
                if let Some(e) = parts.entry {
                    let text: String = req.response.chars().take(MAX_TEXT_CHARS).collect();
                    let caret = text.encode_utf16().count();
                    ui.set_text(e, Some(text));
                    // The caret after the initial text (typing appends).
                    if let Some(el) = ui.el_mut(e) {
                        el.text.caret = caret;
                    }
                    if req.max_chars > 0 {
                        let pid = ui.vocab().pids.text_max_chars;
                        ui.set_property(e, pid, PropertyValue::Int(req.max_chars));
                    }
                }
            }
        }
        parts
    }

    /// Renders the countdown text when the whole number of remaining seconds changed.
    fn countdown_text(ui: &mut Ui, s: &mut Shown, src: &mut dyn TextSource) {
        let Some(end) = s.end else { return };
        let remaining = end - ui.time();
        #[allow(clippy::cast_possible_truncation)]
        let whole = if remaining.is_finite() {
            remaining.trunc().clamp(-1e12, 1e12) as i64
        } else {
            0
        };
        if s.last_whole == Some(whole) {
            return;
        }
        s.last_whole = Some(whole);
        let text = match s.req.kind {
            DialogKind::Confirmation => s.req.question.as_ref(),
            DialogKind::Message => s.req.statement.as_ref(),
            _ => None,
        };
        if let (Some(k), Some(t)) = (s.parts.text, text) {
            let r = t.render(src, Some(whole.max(0)));
            ui.set_text(k, Some(r));
        }
    }

    /// Ends the shown dialog with `outcome` (§18.5) and shows the next request.
    fn finish(&mut self, ui: &mut Ui, outcome: DialogOutcome, src: &mut dyn TextSource) {
        let Some(mut s) = self.shown.take() else {
            return;
        };
        let text = s.parts.entry.map(|e| ui.text_of(e));
        self.remove_elements(ui, &s);
        s.req.finish(outcome, text);
        self.done.push(s.req);
        self.show_next(ui, src);
    }

    /// Handles one UI event (§18.4): button clicks and the core's dialog key results.
    /// Returns whether it was the shown dialog's.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent, src: &mut dyn TextSource) -> bool {
        let Some(s) = self.shown.as_ref() else {
            return false;
        };
        let (root, kind, parts, quit_shown) = (
            s.root,
            s.req.kind,
            s.parts,
            s.parts.quit.is_some_and(|q| ui.is_visible(q)),
        );
        match ev {
            UiEvent::DialogResult { dialog, yes } if *dialog == root => {
                let outcome = match (kind, *yes) {
                    (DialogKind::Confirmation, true) => Some(DialogOutcome::Yes),
                    (DialogKind::Confirmation, false) => Some(DialogOutcome::No),
                    (DialogKind::Message, _) => Some(DialogOutcome::Ok),
                    (DialogKind::TextInput, true) => Some(DialogOutcome::Done),
                    (DialogKind::TextInput, false) if quit_shown => Some(DialogOutcome::Quit),
                    _ => None,
                };
                match outcome {
                    Some(o) => self.finish(ui, o, src),
                    None => {
                        // No result (a wait dialog, or a quit result with the quit button
                        // hidden, which the core does not give): it is shown again.
                        ui.set_shown(root, true);
                        ui.request_activation(root);
                        if let Some(e) = parts.entry {
                            ui.set_focus(Some(e));
                        }
                    }
                }
                true
            }
            UiEvent::Game { message } if message.msg == ui.vocab().msgs.button_click => {
                let src_k = message.source;
                if !ui.exists(src_k) || !ui.is_ancestor_or_self(root, src_k) {
                    return false;
                }
                let hit =
                    |k: Option<ElementKey>| k.is_some_and(|k| ui.is_ancestor_or_self(k, src_k));
                let outcome = match kind {
                    DialogKind::Confirmation if hit(parts.yes) => Some(DialogOutcome::Yes),
                    DialogKind::Confirmation if hit(parts.no) => Some(DialogOutcome::No),
                    DialogKind::Message if hit(parts.ok) => Some(DialogOutcome::Ok),
                    DialogKind::TextInput if hit(parts.done) => Some(DialogOutcome::Done),
                    DialogKind::TextInput if hit(parts.quit) && quit_shown => {
                        Some(DialogOutcome::Quit)
                    }
                    _ => None,
                };
                if let Some(o) = outcome {
                    self.finish(ui, o, src);
                }
                true
            }
            _ => false,
        }
    }

    /// The per-frame work: a dialog taken down by another owner's `topmost` request goes
    /// back to the front of the queue, the countdown runs (§18.3), a timed-out dialog ends,
    /// and the next request is shown when no engine dialog is shown.
    pub fn update(&mut self, ui: &mut Ui, src: &mut dyn TextSource) {
        // (taken down or gone, hidden by the core without a result)
        let status = self.shown.as_ref().map(|s| {
            let gone = !ui.exists(s.root) || ui.prop(s.root, MARK_PID).int() & MARK_TAKEN_DOWN != 0;
            (gone, !gone && !ui.is_visible(s.root))
        });
        match status {
            Some((true, _)) => {
                if let Some(s) = self.shown.take() {
                    self.remove_elements(ui, &s);
                    // Back at the front; a full queue ends its newest request (module docs).
                    while self.queue.len() >= MAX_QUEUED {
                        let Some(mut r) = self.queue.pop_back() else {
                            break;
                        };
                        tracing::warn!("dialog queue full; request {} dismissed", r.tag);
                        r.finish(DialogOutcome::Dismissed, None);
                        self.done.push(r);
                    }
                    self.queue.push_front(s.req);
                }
            }
            Some((false, true)) => {
                if let Some(s) = self.shown.as_ref() {
                    // The core hid it on a key whose result event never arrived (module
                    // docs): shown again.
                    tracing::debug!(
                        "engine dialog {} hidden without a result; shown again",
                        s.req.tag
                    );
                    let (root, entry, inactive) = (s.root, s.parts.entry, s.req.start_inactive);
                    ui.set_shown(root, true);
                    if !inactive {
                        ui.request_activation(root);
                        if let Some(e) = entry {
                            ui.set_focus(Some(e));
                        }
                    }
                }
            }
            _ => {}
        }
        if !self.handlers.is_empty() || self.shown.is_some() {
            self.register(ui);
        }
        let now = ui.time();
        let mut timed_out = false;
        if let Some(s) = self.shown.as_mut()
            && let Some(end) = s.end
        {
            let gap = now - s.last_update;
            if gap > STALL_GAP && gap.is_finite() {
                s.end = Some(end + gap);
            }
            s.last_update = now;
            if s.end.is_some_and(|e| now >= e) {
                timed_out = true;
            } else {
                Self::countdown_text(ui, s, src);
            }
        }
        if timed_out {
            self.finish(ui, DialogOutcome::TimedOut, src);
        }
        self.show_next(ui, src);
    }
}

/// Whether `k` lies in a modal container (`ui-layout.md` §9.1).
fn in_container(ui: &Ui, k: ElementKey) -> bool {
    ui.parent(k)
        .and_then(|p| ui.el(p))
        .is_some_and(|e| e.dialog_container)
}

/// A screen-sized code-made container that blocks clicks behind a modal description's
/// dialog (module docs).
fn modal_container(ui: &mut Ui) -> Option<ElementKey> {
    let (sw, sh) = ui.screen_size();
    let pids = ui.vocab().pids;
    let node = FNode::field(0, 0, 0, sw.max(1), sh.max(1))
        .prop(pids.blocks_clicks, PropertyValue::Bool(true));
    match open_root(ui, &node) {
        Ok(k) => {
            ui.set_mouse_visible_flag(k, true);
            Some(k)
        }
        Err(e) => {
            tracing::debug!("modal container not made: {e}");
            None
        }
    }
}

/// Puts root `k` in front of every other root panel.
fn to_front(ui: &mut Ui, k: ElementKey) {
    let z = crate::game::popup::front_z(ui);
    ui.set_z(k, z);
    crate::game::move_to_front(ui, k);
}
