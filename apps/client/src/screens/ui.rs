// SPDX-License-Identifier: GPL-3.0-or-later
//! The chargen layouts in the game UI (`character-screens.md` §2–§5): the base layout
//! `chargen` with the full-screen `chargen_viewport` and the shared controls, and the three
//! screen layouts, opened from the install by their `UILAYOUT` labels; their elements
//! found by `UIElementID` label; their state set from the [`ScreenModel`] every frame
//! ([`ScreenUi::sync`]); clicks, selections and typing turned into [`Action`]s
//! ([`ScreenUi::handle_event`]).
//!
//! What the data does not name is not bound (OPEN in `character-screens.md` §3, §5): the
//! selection screen's quit, options and plugin-manager buttons and its slots label, the
//! randomise button, the group headers of the control list, the racial-trait rows. The
//! client's start-up maps Escape to [`Action::Quit`]. The viewport element hosts external
//! content ([`VIEWPORT_CONTENT_ID`], `render.md` §6.10): the walker renders the screens'
//! figure into a target of its size ([`ScreenUi::portrait`]), what it shows is
//! [`super::viewport`].
//!
//! **Tessera rules** (OPEN in `character-screens.md` §4.2): a character row's description
//! is Tessera's own line "Level *n* *race* *class*" with the parts the session told (the
//! install's three-variable format of CS11 is Probable only); the race and class
//! information panels show their texts in the `Text` elements below the panel whose layout
//! gives them no text of their own, in tree order (race: the description; class: the name,
//! then the description).

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use tessera_formats::property::PropertyValue;
use tessera_formats::{Did, StringInfo};
use tessera_net::domain::Text;
use tessera_ui::game::creation::{
    Choice, ChoiceParts, Slider, Stepper, SwatchGrid, fill_choices_with, select_choice,
    selected_choice,
};
use tessera_ui::game::examine::ExamTemplate;
use tessera_ui::game::{find_below, subtree};
use tessera_ui::{ElementKey, IBox, ImageRef, Ui, UiEvent, WidgetClass};

use super::backend::ButtonRule;
use super::controls::{ControlChoices, ControlValue};
use super::data::ScreenData;
use super::model::{Action, CharacterRow, ScreenModel, Stage};
use super::turntable::Turn;

/// Declares a table of labels resolved at run time.
macro_rules! label_table {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident = $label:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fm])* pub $field: Option<u32>,)*
        }

        impl $name {
            /// Every label of the table.
            pub const LABELS: &'static [&'static str] = &[$($label),*];

            /// Resolves every label with `lookup`.
            pub fn resolve(lookup: &mut dyn FnMut(&str) -> Option<u32>) -> Self {
                Self { $($field: lookup($label),)* }
            }

            /// Labels `lookup` did not resolve.
            #[must_use]
            pub fn missing(&self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $(if self.$field.is_none() { out.push($label); })*
                out
            }
        }
    };
}

label_table! {
    /// The `UILAYOUT` labels of the four layouts (§1 OBS).
    ChargenLayouts {
        /// The base: viewport, stage field, shared controls, error list.
        base = "chargen",
        /// Character selection.
        select = "chargen_characterselect",
        /// Race and class.
        race_class = "chargen_raceclass",
        /// Appearance and name.
        appearance = "chargen_appearance",
    }
}

label_table! {
    /// The `UIElementID` labels the screens use (§2–§5).
    ChargenIds {
        /// Root of `chargen`.
        base_root = "CharGen_Base_Field",
        /// Root of the selection screen.
        select_root = "CharGen_CharacterSelect_Base",
        /// Root of the race-and-class screen.
        race_class_root = "CharGen_RaceClass_Base",
        /// Root of the appearance screen.
        appearance_root = "CharGen_Appearance_Base",
        /// The full-screen viewport.
        viewport = "chargen_viewport",
        /// The error list and its line template.
        error_list = "CharGen_ErrorTextList",
        /// The error line template.
        error_template = "CharGenErrorTextTemplate",
        /// Shared: back.
        back = "CharGen_Back_Button",
        /// Shared: exit (hidden in P1).
        exit = "CharGen_Exit_Button",
        /// Shared: continue.
        continue_button = "CharGen_Continue_Button",
        /// Shared: turntable group.
        turntable = "Chargen_Controls",
        /// Turn left.
        turn_left = "CharGen_Turntable_Left_Button",
        /// Turn right.
        turn_right = "CharGen_Turntable_Right_Button",
        /// Zoom in.
        zoom_in = "CharGen_Turntable_Zoom_In_Button",
        /// Zoom out.
        zoom_out = "CharGen_Turntable_Zoom_Out_Button",
        /// The avatar-update toggle field.
        avatar_toggle = "CharGen_AvatarUpdate_Toggle",
        /// Its check box.
        avatar_toggle_button = "CharGen_AvatarUpdate_Toggle_Button",
        /// Examination tooltip anchor (never used, §2.1).
        exam_anchor = "GamePlay_ExaminationTooltip_Activatable",
        /// Selection: the character list.
        character_list = "CharGen_CharacterSelect_Listbox",
        /// Selection: the row template.
        character_template = "CharGen_CharacterSelect_Character_Template",
        /// Row: name.
        character_name = "CharGen_CharacterSelect_Character_Name",
        /// Row: description.
        character_description = "CharGen_CharacterSelect_Character_Description",
        /// Row: class icon.
        character_class = "CharGen_CharacterSelect_Character_Class",
        /// Selection: new.
        new = "CharGen_CharacterSelect_New_Button",
        /// Selection: buy a slot (hidden).
        new_slot = "NewCharacterSlot_Button",
        /// Selection: delete.
        delete = "CharGen_CharacterSelect_Delete_Button",
        /// Selection: race change (hidden).
        race_change = "CharGen_CharacterSelect_RaceChange_Button",
        /// Selection: rename (hidden).
        rename = "CharGen_RenameCharacter_Button",
        /// Selection: rename store (hidden).
        rename_buy = "CharGen_RenameCharacter_BuyNow",
        /// Selection: enter.
        enter = "CharGen_CharacterSelect_Enter_Button",
        /// Selection: PvP lobby (hidden).
        pvp = "CharGen_PVP_Lobby_HideWhenField",
        /// Selection: logout notice.
        notice_logout = "CharGen_Notice_FinishingLogout",
        /// Selection: population notice.
        notice_players = "CharGen_Temp_TooManyPlayers",
        /// Race: male.
        male = "CharGen_Male_Button",
        /// Race: female.
        female = "CharGen_Female_Button",
        /// Race list.
        race_list = "CharGen_Race_Select_Listbox",
        /// Race entry template.
        race_template = "CharGen_Race_Select_Race_Template",
        /// Race entry name (review probe, §4.2 OPEN).
        race_name = "CharGen_Race_Select_Race_Name",
        /// Race entry icon.
        race_icon = "CharGen_Race_Select_Race_Icon",
        /// Race information panel.
        race_info = "Chargen_Race_Info_Panel",
        /// Class list.
        class_list = "CharGen_Class_Select_Listbox",
        /// Class entry template.
        class_template = "CharGen_Class_Select_Class_Template",
        /// Class entry name.
        class_name = "CharGen_Class_Select_Class_Name",
        /// Class entry icon.
        class_icon = "CharGen_Class_Select_Class_Icon",
        /// Class information panel.
        class_info = "CharGen_Class_Info_Panel",
        /// Locked-continue store button (never shown).
        continue_locked = "CharGen_ContinueLocked_BuyNowButton",
        /// Loading notice.
        loading = "CharGen_Avatar_Loading_CenterScreen",
        /// Race-list blocker while loading.
        race_blocker = "CharGen_Avatar_Loading_RaceBlocker",
        /// Nationality panel.
        nationality_panel = "CharGen_Nationality_Select_Panel",
        /// Nationality entry template.
        nationality_template = "CharGen_Nationality_Entry_Template",
        /// Appearance controls list.
        controls_list = "CharGen_Appearance_Controls_Listbox",
        /// Style control template.
        apr_template = "CharGen_APRAppearanceChoice_BaseField",
        /// Face-layer control template.
        compositor_template = "CharGen_CompositorAppearanceChoice_BaseField",
        /// Colour control template.
        dyemap_template = "CharGen_DyeMapAppearanceChoice_BaseField",
        /// Body-scale control template.
        bone_scaling_template = "CharGen_BoneScalingAppearanceChoice_BaseField",
        /// Slider control template.
        bone_slider_template = "CharGen_BoneSliderSystemChoice_BaseField",
        /// Presets control template.
        presets_template = "CharGen_PresetsAppearanceChoice_BaseField",
        /// The nationality text above the controls.
        nationality_text = "CharGen_Appearance_Nationality_Text",
        /// The name field.
        name_field = "CharGen_Name_Entry_Field",
        /// Random name.
        random_name = "CharGen_Name_RandomNameGenerator",
        /// Tutorial choice (hidden).
        tutorial = "CharGen_Name_ChangeTutorial",
        /// Admin button (hidden by its data).
        name_admin = "CharGen_Name_Admin_Button",
        /// Create.
        create = "CharGen_Name_Create_Button",
        /// Advanced start (hidden).
        advanced_start = "CharGen_Name_AdvancedStart_Panel",
        /// Race change in place of create (hidden).
        name_race_change = "CharGen_Name_RaceChange_Button",
    }
}

/// Where the four layouts and their elements are in the install.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChargenRefs {
    /// Layout DIDs.
    pub layouts: ChargenLayouts,
    /// Element IDs.
    pub ids: ChargenIds,
}

impl ChargenRefs {
    /// Resolves the layouts through the `UILAYOUT` map (`layout`) and the elements through
    /// the `UIElementID` mapper chain (`element`).
    pub fn resolve(
        layout: &mut dyn FnMut(&str) -> Option<u32>,
        element: &mut dyn FnMut(&str) -> Option<u32>,
    ) -> Self {
        Self {
            layouts: ChargenLayouts::resolve(layout),
            ids: ChargenIds::resolve(element),
        }
    }
}

/// Texts the UI needs from the install.
pub trait ScreenTexts {
    /// The plain text of a string reference.
    fn text(&self, info: &StringInfo) -> Option<String>;

    /// The plain text of a session text that is not a literal (a refusal reason, §8:
    /// `Text::Name`, `Text::Description`, …); `None` shows Tessera's placeholder. The
    /// default resolves nothing; [`super::data::InstallTexts`] resolves record names.
    fn reason(&self, text: &Text) -> Option<String> {
        let _ = text;
        None
    }
}

impl<F: Fn(&StringInfo) -> Option<String>> ScreenTexts for F {
    fn text(&self, info: &StringInfo) -> Option<String> {
        self(info)
    }
}

/// One appearance control's widgets.
#[derive(Debug)]
enum ControlWidget {
    Stepper(Stepper),
    Slider(Slider),
    Swatch(SwatchGrid),
    Sliders { which: Stepper, value: Slider },
}

/// A filled choice list: the keys of its entries (for clicks), the entries, and a stamp of
/// everything the entries show, so the list is refilled when a shown value changes.
#[derive(Debug)]
struct ListCache<K> {
    keys: Vec<K>,
    entries: Vec<Option<ElementKey>>,
    stamp: Option<u64>,
}

impl<K> Default for ListCache<K> {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            entries: Vec::new(),
            stamp: None,
        }
    }
}

fn stamp_of<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// What the swatch grids and the other control widgets were built for: the body and the
/// nationality (its dye-map overrides replace palettes, §5.1).
type ControlsFor = (u32, u32, Option<u32>);

/// The external-content id of the creation viewport `chargen_viewport` (the client's id
/// space, beside the paperdoll's `crate::character::panel::PORTRAIT_CONTENT_ID`): the
/// walker renders the screens' figure into it (`render.md` §6.10).
pub const VIEWPORT_CONTENT_ID: u64 = 0x0051_0000;

/// The chargen layouts opened in a [`Ui`].
#[derive(Debug, Default)]
pub struct ScreenUi {
    refs: ChargenRefs,
    roots: BTreeMap<&'static str, ElementKey>,
    elements: BTreeMap<u32, ElementKey>,
    rows: ListCache<String>,
    races: ListCache<u32>,
    classes: ListCache<u32>,
    nationalities: ListCache<u32>,
    errors: Vec<String>,
    controls_for: Option<ControlsFor>,
    controls: Vec<(usize, ElementKey, ControlWidget)>,
    name_shown: String,
    race_info: (Vec<ElementKey>, Option<u64>),
    class_info: (Vec<ElementKey>, Option<u64>),
    /// Layouts or elements that could not be opened or found.
    pub missing: Vec<String>,
}

/// The `Text` elements below `panel` that the layout gives no text (where the screens
/// write their texts), tree order, at most `n`.
fn free_texts(ui: &mut Ui, panel: Option<ElementKey>, n: usize) -> Vec<ElementKey> {
    let Some(panel) = panel else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for k in subtree(ui, panel).into_iter().skip(1) {
        if out.len() >= n {
            break;
        }
        if ui.class(k) == Some(WidgetClass::Text) && ui.text_of(k).is_empty() {
            out.push(k);
        }
    }
    out
}

/// The run-time tooltip a shown button gets under `rule`: the ghosted button's Tessera
/// text (§7.1, §7.2), else none (so a tooltip set while ghosted goes).
#[must_use]
pub fn button_tooltip(rule: &ButtonRule) -> Option<String> {
    match rule {
        ButtonRule::Ghosted(Some(t)) => Some(t.to_string()),
        _ => None,
    }
}

/// A character row's description (Tessera rule, module docs): "Level *n*", the race name
/// of its body and its class name, each when known.
#[must_use]
pub fn row_description(row: &CharacterRow, data: &ScreenData, texts: &dyn ScreenTexts) -> String {
    let mut parts = Vec::new();
    if let Some(l) = row.level {
        parts.push(format!("Level {l}"));
    }
    if let Some((species, sex)) = row.character_body()
        && let Some(r) = data.chargen.race(species)
        && let Some(name) = r
            .gender(sex)
            .or_else(|| r.genders.first())
            .and_then(|g| g.race_name.as_ref())
            .and_then(|i| texts.text(i))
    {
        parts.push(name);
    }
    if let Some(name) = row
        .class
        .and_then(|c| data.chargen.classes.get(&c))
        .and_then(|c| c.name.as_ref())
        .and_then(|i| texts.text(i))
    {
        parts.push(name);
    }
    parts.join(" ")
}

fn template(layout: Option<u32>, element: Option<u32>) -> Option<ExamTemplate> {
    Some(ExamTemplate {
        layout: layout?,
        element: element?,
    })
}

impl ScreenUi {
    /// Opens the four layouts (`chargen` first, so the screens lie over the viewport) and
    /// finds the named elements below their roots. A layout that cannot be opened is
    /// listed in [`ScreenUi::missing`]; the screens then run without it.
    pub fn open(ui: &mut Ui, refs: ChargenRefs) -> Self {
        let mut s = Self {
            refs,
            ..Self::default()
        };
        for l in refs.layouts.missing() {
            s.missing.push(format!("layout {l}"));
        }
        let ids = refs.ids;
        let pairs = [
            ("base", refs.layouts.base, ids.base_root),
            ("select", refs.layouts.select, ids.select_root),
            ("race_class", refs.layouts.race_class, ids.race_class_root),
            ("appearance", refs.layouts.appearance, ids.appearance_root),
        ];
        for (name, layout, root) in pairs {
            let (Some(layout), Some(root)) = (layout, root) else {
                continue;
            };
            match ui.open_panel(Did(layout), root) {
                Ok(k) => {
                    s.roots.insert(name, k);
                }
                Err(e) => s.missing.push(format!("{name}: {e}")),
            }
        }
        let wanted: Vec<u32> = [
            ids.viewport,
            ids.error_list,
            ids.back,
            ids.exit,
            ids.continue_button,
            ids.turntable,
            ids.turn_left,
            ids.turn_right,
            ids.zoom_in,
            ids.zoom_out,
            ids.avatar_toggle,
            ids.avatar_toggle_button,
            ids.exam_anchor,
            ids.character_list,
            ids.new,
            ids.new_slot,
            ids.delete,
            ids.race_change,
            ids.rename,
            ids.rename_buy,
            ids.enter,
            ids.pvp,
            ids.notice_logout,
            ids.notice_players,
            ids.male,
            ids.female,
            ids.race_list,
            ids.race_info,
            ids.class_list,
            ids.class_info,
            ids.continue_locked,
            ids.loading,
            ids.race_blocker,
            ids.nationality_panel,
            ids.controls_list,
            ids.nationality_text,
            ids.name_field,
            ids.random_name,
            ids.tutorial,
            ids.name_admin,
            ids.create,
            ids.advanced_start,
            ids.name_race_change,
        ]
        .into_iter()
        .flatten()
        .collect();
        let roots: Vec<ElementKey> = s.roots.values().copied().collect();
        for id in wanted {
            if let Some(k) = roots.iter().find_map(|r| {
                if ui.element_id(*r) == Some(id) {
                    Some(*r)
                } else {
                    find_below(ui, *r, id)
                }
            }) {
                s.elements.insert(id, k);
            }
        }
        // Listen to the buttons the screens act on.
        let click = ui.vocab().msgs.button_click;
        let press = ui.vocab().msgs.mouse_press;
        let release = ui.vocab().msgs.mouse_release;
        let select = ui.vocab().msgs.listbox_new_selection;
        let dbl = ui.vocab().msgs.mouse_dbl_click;
        let input = ui.vocab().msgs.character_input;
        for id in [
            ids.back,
            ids.continue_button,
            ids.zoom_in,
            ids.zoom_out,
            ids.avatar_toggle_button,
            ids.new,
            ids.delete,
            ids.enter,
            ids.male,
            ids.female,
            ids.random_name,
            ids.create,
        ]
        .into_iter()
        .flatten()
        {
            ui.register_handler(id, click);
        }
        for id in [ids.turn_left, ids.turn_right].into_iter().flatten() {
            ui.register_handler(id, press);
            ui.register_handler(id, release);
        }
        for id in [
            ids.character_list,
            ids.race_list,
            ids.class_list,
            ids.controls_list,
        ]
        .into_iter()
        .flatten()
        {
            ui.register_handler(id, select);
        }
        if let Some(id) = ids.character_list {
            ui.register_handler(id, dbl);
        }
        if let Some(id) = ids.name_field {
            ui.register_handler(id, input);
        }
        // §2.1, §3–§5: never shown in P1.
        for id in [
            ids.exit,
            ids.exam_anchor,
            ids.new_slot,
            ids.race_change,
            ids.rename,
            ids.rename_buy,
            ids.pvp,
            ids.notice_logout,
            ids.notice_players,
            ids.continue_locked,
            ids.tutorial,
            ids.name_admin,
            ids.advanced_start,
            ids.name_race_change,
        ]
        .into_iter()
        .flatten()
        {
            if let Some(k) = s.elements.get(&id).copied() {
                ui.set_shown(k, false);
            }
        }
        // The viewport hosts the figure the walker renders (`render.md` §6.10, §2.1).
        if let Some(k) = s.el(ids.viewport) {
            ui.set_external_content(k, Some(VIEWPORT_CONTENT_ID));
        }
        // Where the information panels' texts go (module docs), found before any is set.
        s.race_info.0 = free_texts(ui, s.el(ids.race_info), 1);
        s.class_info.0 = free_texts(ui, s.el(ids.class_info), 2);
        s
    }

    fn el(&self, id: Option<u32>) -> Option<ElementKey> {
        self.elements.get(&id?).copied()
    }

    /// The viewport element's box on screen (the renderer draws the scene there).
    #[must_use]
    pub fn viewport_box(&self, ui: &Ui) -> Option<IBox> {
        self.el(self.refs.ids.viewport)
            .map(|k| ui.absolute_layout_box(k))
    }

    /// What the walker renders into the viewport this frame: its content id, its layout
    /// box's size, the turntable's turn and zoom (`view`); `None` while the viewport is
    /// not displayed or has no size.
    #[must_use]
    pub fn portrait(
        &self,
        ui: &Ui,
        view: &super::viewport::ViewportState,
    ) -> Option<crate::character::panel::Portrait> {
        let k = self.el(self.refs.ids.viewport)?;
        if !ui.is_displayed(k) {
            return None;
        }
        let b = ui.layout_box(k)?;
        let w = u32::try_from(b.w()).ok().filter(|w| *w > 0)?;
        let h = u32::try_from(b.h()).ok().filter(|h| *h > 0)?;
        Some(crate::character::panel::Portrait {
            id: VIEWPORT_CONTENT_ID,
            size: (w, h),
            yaw: view.yaw_degrees,
            figure: crate::character::panel::PortraitFigure::Screens {
                zoomed: view.zoomed,
            },
        })
    }

    fn apply_button(&self, ui: &mut Ui, id: Option<u32>, rule: &ButtonRule) {
        let Some(k) = self.el(id) else {
            return;
        };
        let ghost = ui.vocab().pids.button_ghosted;
        match rule {
            ButtonRule::Hidden => ui.set_shown(k, false),
            ButtonRule::Enabled | ButtonRule::Ghosted(_) => {
                ui.set_shown(k, true);
                ui.set_property(k, ghost, PropertyValue::Bool(!rule.is_enabled()));
                // A tooltip set while ghosted goes when the rule has none; the layout's
                // own tooltip stays.
                ui.set_tooltip_text(k, button_tooltip(rule));
            }
        }
    }

    fn show(&self, ui: &mut Ui, id: Option<u32>, on: bool) {
        if let Some(k) = self.el(id) {
            ui.set_shown(k, on);
        }
    }

    /// Fills `list` unless it already shows `keys` with the same `stamp` (a hash of every
    /// value the entries show); returns whether it was filled.
    #[allow(clippy::too_many_arguments)]
    fn fill<K: PartialEq + Clone>(
        ui: &mut Ui,
        list: Option<ElementKey>,
        tpl: Option<ExamTemplate>,
        parts: ChoiceParts,
        cache: &mut ListCache<K>,
        keys: Vec<K>,
        stamp: u64,
        choices: impl FnOnce() -> Vec<Choice>,
    ) -> bool {
        let (Some(list), Some(tpl)) = (list, tpl) else {
            return false;
        };
        if cache.keys == keys && cache.stamp == Some(stamp) && !cache.entries.is_empty() {
            return false;
        }
        cache.entries = fill_choices_with(ui, list, tpl, parts, &choices());
        cache.keys = keys;
        cache.stamp = Some(stamp);
        true
    }

    fn set_texts(ui: &mut Ui, at: &[ElementKey], texts: &[String]) {
        for (k, t) in at.iter().zip(texts) {
            ui.set_text(*k, Some(t.clone()));
        }
    }

    /// Sets every element from `model` (call once per frame after the model changed).
    pub fn sync(&mut self, ui: &mut Ui, model: &ScreenModel, texts: &dyn ScreenTexts) {
        let ids = self.refs.ids;
        let stage = model.stage();
        for (name, k) in &self.roots {
            let on = match *name {
                "base" => true,
                "select" => stage == Stage::Select,
                "race_class" => stage == Stage::RaceClass,
                _ => stage == Stage::Appearance,
            };
            ui.set_shown(*k, on);
        }
        self.apply_button(ui, ids.back, &model.back_button());
        self.apply_button(ui, ids.continue_button, &model.continue_button());
        self.apply_button(ui, ids.create, &model.create_button());
        self.apply_button(ui, ids.new, &model.new_button());
        self.apply_button(ui, ids.delete, &model.delete_button());
        self.apply_button(ui, ids.enter, &model.enter_button());
        self.show(ui, ids.turntable, true);
        self.show(ui, ids.avatar_toggle, model.shows_avatar_update_toggle());
        if let Some(k) = self.el(ids.avatar_toggle_button) {
            let st = ui.vocab().states;
            ui.switch_state(
                k,
                if model.avatar_update_visible() {
                    st.highlight
                } else {
                    st.normal
                },
            );
        }
        let loading = model.figure_loading() && stage != Stage::Select;
        self.show(ui, ids.loading, loading);
        self.show(ui, ids.race_blocker, loading && stage == Stage::RaceClass);

        let data = model.data();
        let image = |d: Option<Did>| d.map(|d| ImageRef::Did(d.0));
        let text = |i: Option<&StringInfo>| i.and_then(|i| texts.text(i)).unwrap_or_default();

        // Selection list (§3).
        let row_parts = ChoiceParts {
            name: ids.character_name,
            icon: ids.character_class,
        };
        let names: Vec<String> = model.rows().iter().map(|r| r.name.clone()).collect();
        let rows = model.rows().to_vec();
        let row_stamp = stamp_of(
            &rows
                .iter()
                .map(|r| (&r.name, r.class, r.level, r.body))
                .collect::<Vec<_>>(),
        );
        let classes = data.chargen.classes.clone();
        let refilled = Self::fill(
            ui,
            self.el(ids.character_list),
            template(self.refs.layouts.select, ids.character_template),
            row_parts,
            &mut self.rows,
            names,
            row_stamp,
            || {
                rows.iter()
                    .map(|r| Choice {
                        label: r.name.clone(),
                        icon: r
                            .class
                            .and_then(|c| classes.get(&c))
                            .and_then(|c| c.icon)
                            .map(|d| ImageRef::Did(d.0)),
                        disabled: false,
                    })
                    .collect()
            },
        );
        // The row descriptions (§3, §8: level, race from the body, class).
        if refilled && let Some(desc_id) = ids.character_description {
            for (row, entry) in rows.iter().zip(&self.rows.entries) {
                if let Some(k) = entry.and_then(|e| find_below(ui, e, desc_id)) {
                    ui.set_text(k, Some(row_description(row, data, texts)));
                }
            }
        }
        if let (Some(list), Some(i)) = (self.el(ids.character_list), model.selected()) {
            select_choice(ui, list, &self.rows.entries, i);
        }

        // Race and class (§4).
        if let Some(species) = model.creation().species {
            let sex = model.creation().sex;
            let races: Vec<u32> = data.chargen.races.iter().map(|r| r.species()).collect();
            let race_choices = || {
                data.chargen
                    .races
                    .iter()
                    .map(|r| {
                        let g = sex.and_then(|x| r.gender(x)).or_else(|| r.genders.first());
                        Choice {
                            label: text(g.and_then(|g| g.race_name.as_ref())),
                            icon: image(g.and_then(|g| g.icon)),
                            disabled: false,
                        }
                    })
                    .collect()
            };
            // The names and icons come from the selected gender's entries (§4.1).
            Self::fill(
                ui,
                self.el(ids.race_list),
                template(self.refs.layouts.race_class, ids.race_template),
                ChoiceParts {
                    name: ids.race_name,
                    icon: ids.race_icon,
                },
                &mut self.races,
                races.clone(),
                stamp_of(&sex),
                race_choices,
            );
            if let (Some(list), Some(i)) = (
                self.el(ids.race_list),
                races.iter().position(|s| *s == species),
            ) {
                select_choice(ui, list, &self.races.entries, i);
            }
            // Race information (§4.1): the description (traits: OPEN, module docs).
            let race_stamp = stamp_of(&(species, sex));
            if self.race_info.1 != Some(race_stamp) {
                self.race_info.1 = Some(race_stamp);
                let d = data
                    .chargen
                    .race(species)
                    .and_then(|r| r.record.description.as_ref());
                Self::set_texts(ui, &self.race_info.0, &[text(d)]);
            }
            let class_list = data.chargen.classes_for(species);
            let class_keys: Vec<u32> = class_list.iter().map(|c| c.class).collect();
            Self::fill(
                ui,
                self.el(ids.class_list),
                template(self.refs.layouts.race_class, ids.class_template),
                ChoiceParts {
                    name: ids.class_name,
                    icon: ids.class_icon,
                },
                &mut self.classes,
                class_keys.clone(),
                0,
                || {
                    class_list
                        .iter()
                        .map(|c| Choice {
                            label: text(c.name.as_ref()),
                            icon: image(c.icon),
                            disabled: false,
                        })
                        .collect()
                },
            );
            if let (Some(list), Some(i)) = (
                self.el(ids.class_list),
                model
                    .creation()
                    .class
                    .and_then(|c| class_keys.iter().position(|k| *k == c)),
            ) {
                select_choice(ui, list, &self.classes.entries, i);
            }
            // Class information (§4.2): name, then description.
            let class = model.creation().class;
            let class_stamp = stamp_of(&class);
            if self.class_info.1 != Some(class_stamp) {
                self.class_info.1 = Some(class_stamp);
                let c = class.and_then(|c| data.chargen.classes.get(&c));
                Self::set_texts(
                    ui,
                    &self.class_info.0,
                    &[
                        text(c.and_then(|c| c.name.as_ref())),
                        text(c.and_then(|c| c.description.as_ref())),
                    ],
                );
            }
            let race = data.chargen.race(species);
            let has = |x: u32| race.is_some_and(|r| r.gender(x).is_some());
            let male = tessera_gamedata::GENDER_MALE;
            let female = tessera_gamedata::GENDER_FEMALE;
            self.apply_button(
                ui,
                ids.male,
                &if has(male) {
                    ButtonRule::Enabled
                } else {
                    ButtonRule::Ghosted(None)
                },
            );
            self.apply_button(
                ui,
                ids.female,
                &if has(female) {
                    ButtonRule::Enabled
                } else {
                    ButtonRule::Ghosted(None)
                },
            );

            // Nationality and appearance (§5).
            let nats = data.chargen.nationalities_for(species);
            let nat_keys: Vec<u32> = nats.iter().map(|(k, _)| *k).collect();
            Self::fill(
                ui,
                self.el(ids.nationality_panel),
                template(self.refs.layouts.appearance, ids.nationality_template),
                ChoiceParts::default(),
                &mut self.nationalities,
                nat_keys.clone(),
                0,
                || {
                    nats.iter()
                        .map(|(_, n)| Choice {
                            label: text(n.and_then(|n| n.name.as_ref())),
                            icon: image(n.and_then(|n| n.icon)),
                            disabled: false,
                        })
                        .collect()
                },
            );
            if let Some(k) = self.el(ids.nationality_text) {
                let name = model
                    .creation()
                    .nationality
                    .and_then(|n| data.chargen.nationalities.get(&n))
                    .and_then(|(_, n)| n.name.as_ref());
                ui.set_text(k, Some(text(name)));
            }
            self.sync_controls(ui, model);
        }

        // Name field (§5.3): shown text follows the model unless the player is typing.
        if let Some(k) = self.el(ids.name_field)
            && model.creation().name != self.name_shown
        {
            self.name_shown.clone_from(&model.creation().name);
            ui.set_text(k, Some(self.name_shown.clone()));
        }

        // Error lines.
        let lines: Vec<String> = model
            .errors()
            .map(|e| e.text(&|t| texts.reason(t)))
            .chain(
                model
                    .confirming()
                    .map(|n| super::texts::OwnText::ConfirmDelete(n.to_owned()).to_string()),
            )
            .collect();
        if lines != self.errors
            && let (Some(list), Some(tpl)) = (
                self.el(ids.error_list),
                template(self.refs.layouts.base, ids.error_template),
            )
        {
            let choices: Vec<Choice> = lines
                .iter()
                .map(|l| Choice {
                    label: l.clone(),
                    icon: None,
                    disabled: false,
                })
                .collect();
            fill_choices_with(ui, list, tpl, ChoiceParts::default(), &choices);
            self.errors = lines;
        }
    }

    fn sync_controls(&mut self, ui: &mut Ui, model: &ScreenModel) {
        let ids = self.refs.ids;
        let body = model
            .creation()
            .species
            .zip(model.creation().sex)
            .map(|(s, x)| (s, x, model.creation().nationality));
        let Some(list) = self.el(ids.controls_list) else {
            return;
        };
        if body != self.controls_for {
            for (_, _, w) in self.controls.drain(..) {
                match w {
                    ControlWidget::Stepper(s) => s.release(ui),
                    ControlWidget::Slider(s) => s.release(ui),
                    ControlWidget::Swatch(s) => s.release(ui),
                    ControlWidget::Sliders { which, value } => {
                        which.release(ui);
                        value.release(ui);
                    }
                }
            }
            ui.list_clear(list);
            self.controls_for = body;
            let Some(detail) = model.current_avatar() else {
                return;
            };
            let layout = self.refs.layouts.appearance;
            let nationality = model.creation().nationality.unwrap_or(0);
            for c in &detail.controls {
                let tpl_id = match c.choices {
                    ControlChoices::Styles { .. } => ids.apr_template,
                    ControlChoices::Options(_) => ids.compositor_template,
                    ControlChoices::Palette(_) => ids.dyemap_template,
                    ControlChoices::Range { .. } => ids.bone_scaling_template,
                    ControlChoices::Sliders { .. } => ids.bone_slider_template,
                    ControlChoices::Presets(_) => ids.presets_template,
                    ControlChoices::Nothing => None,
                };
                let Some(tpl) = template(layout, tpl_id) else {
                    continue;
                };
                let Ok(host) = ui.create_element(None, Did(tpl.layout), tpl.element) else {
                    continue;
                };
                if ui.list_add(list, host, None).is_err() {
                    ui.destroy(host);
                    continue;
                }
                let label = format!("{}", c.control.index + 1);
                let w = match &c.choices {
                    ControlChoices::Styles { variants, .. } => {
                        Stepper::build(ui, host, &label, variants.len(), 0)
                            .map(ControlWidget::Stepper)
                    }
                    ControlChoices::Options(o) => {
                        Stepper::build(ui, host, &label, o.len(), 0).map(ControlWidget::Stepper)
                    }
                    ControlChoices::Presets(p) => {
                        Stepper::build(ui, host, &label, p.len(), 0).map(ControlWidget::Stepper)
                    }
                    ControlChoices::Range { min, max } => {
                        Slider::build(ui, host, *min, *max, *min).map(ControlWidget::Slider)
                    }
                    ControlChoices::Palette(_) => model
                        .data()
                        .chargen
                        .palette_for(&c.control, nationality)
                        .and_then(|p| SwatchGrid::build(ui, host, ImageRef::Did(p.0), (64, 64)))
                        .map(ControlWidget::Swatch),
                    ControlChoices::Sliders { count } => {
                        Stepper::build(ui, host, &label, *count, 0).and_then(|which| {
                            Slider::build(ui, host, 0.0, 255.0, 128.0)
                                .map(|value| ControlWidget::Sliders { which, value })
                        })
                    }
                    ControlChoices::Nothing => None,
                };
                if let Some(w) = w {
                    self.controls.push((c.control.index, host, w));
                }
            }
        }
        // Values from the model.
        let Some(detail) = model.current_avatar() else {
            return;
        };
        for (index, _, w) in &mut self.controls {
            let v = model.creation().appearance.value(detail, *index);
            let c = detail.control(*index);
            match (w, v, c.map(|c| &c.choices)) {
                (ControlWidget::Stepper(s), Some(ControlValue::Style(i)), _) => {
                    let n = s.count();
                    s.set(ui, *i as usize, n);
                }
                (
                    ControlWidget::Stepper(s),
                    Some(ControlValue::Option(o)),
                    Some(ControlChoices::Options(opts)),
                ) => {
                    let n = s.count();
                    let i = opts.iter().position(|x| x == o).unwrap_or(0);
                    s.set(ui, i, n);
                }
                (ControlWidget::Stepper(s), Some(ControlValue::Preset(i)), _) => {
                    let n = s.count();
                    s.set(ui, *i as usize, n);
                }
                (ControlWidget::Slider(s), Some(ControlValue::Float(f)), _) => {
                    if (s.value() - f).abs() > f32::EPSILON {
                        s.set_value(ui, *f);
                    }
                }
                (ControlWidget::Sliders { which, value }, Some(ControlValue::Sliders(b)), _) => {
                    if let Some(byte) = b.get(which.index()) {
                        let f = f32::from(*byte);
                        if (value.value() - f).abs() > 0.5 {
                            value.set_value(ui, f);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Turns one UI event into the player's actions (`seed` seeds random names and the
    /// randomise button, normally from the clock).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        model: &ScreenModel,
        seed: u64,
    ) -> Vec<Action> {
        let mut out = Vec::new();
        let ids = self.refs.ids;
        // Control widgets first (they listen on their own code elements).
        if let Some(detail) = model.current_avatar() {
            for (index, _, w) in &mut self.controls {
                let Some(c) = detail.control(*index) else {
                    continue;
                };
                let v = match (w, &c.choices) {
                    (ControlWidget::Stepper(s), ControlChoices::Styles { .. }) => s
                        .handle_event(ui, ev)
                        .map(|i| ControlValue::Style(u32::try_from(i).unwrap_or(0))),
                    (ControlWidget::Stepper(s), ControlChoices::Options(o)) => s
                        .handle_event(ui, ev)
                        .and_then(|i| o.get(i).copied())
                        .map(ControlValue::Option),
                    (ControlWidget::Stepper(s), ControlChoices::Presets(_)) => s
                        .handle_event(ui, ev)
                        .map(|i| ControlValue::Preset(u32::try_from(i).unwrap_or(0))),
                    (ControlWidget::Slider(s), _) => s
                        .handle_event(ui, ev)
                        .or_else(|| s.update(ui))
                        .map(ControlValue::Float),
                    (ControlWidget::Swatch(s), _) => {
                        if s.handle_event(ui, ev).is_some() {
                            s.colour(ui).map(|argb| {
                                let [a, r, g, b] = argb.to_be_bytes();
                                ControlValue::Colour([r, g, b, a])
                            })
                        } else {
                            None
                        }
                    }
                    (ControlWidget::Sliders { which, value }, _) => {
                        if which.handle_event(ui, ev).is_some() {
                            None
                        } else {
                            value
                                .handle_event(ui, ev)
                                .or_else(|| value.update(ui))
                                .and_then(|f| {
                                    match model.creation().appearance.value(detail, *index) {
                                        Some(ControlValue::Sliders(b)) => {
                                            let mut b = b.clone();
                                            let byte = f.round().clamp(0.0, 255.0) as u8;
                                            *b.get_mut(which.index())? = byte;
                                            Some(ControlValue::Sliders(b))
                                        }
                                        _ => None,
                                    }
                                })
                        }
                    }
                    _ => None,
                };
                if let Some(v) = v {
                    out.push(Action::Control(*index, v));
                }
            }
        }
        let UiEvent::Game { message } = ev else {
            return out;
        };
        let msgs = ui.vocab().msgs;
        let src = Some(message.source_id);
        if message.msg == msgs.button_click {
            let a = if src == ids.back {
                Some(Action::Back)
            } else if src == ids.continue_button {
                Some(Action::Continue)
            } else if src == ids.zoom_in {
                Some(Action::ZoomIn)
            } else if src == ids.zoom_out {
                Some(Action::ZoomOut)
            } else if src == ids.avatar_toggle_button {
                Some(Action::AvatarUpdateVisible(!model.avatar_update_visible()))
            } else if src == ids.new {
                Some(Action::New)
            } else if src == ids.delete {
                Some(Action::Delete)
            } else if src == ids.enter {
                Some(Action::Enter)
            } else if src == ids.male {
                Some(Action::Gender(tessera_gamedata::GENDER_MALE))
            } else if src == ids.female {
                Some(Action::Gender(tessera_gamedata::GENDER_FEMALE))
            } else if src == ids.random_name {
                Some(Action::RandomName(seed))
            } else if src == ids.create {
                Some(Action::Create)
            } else {
                None
            };
            out.extend(a);
        } else if message.msg == msgs.mouse_press || message.msg == msgs.mouse_release {
            let turn = if src == ids.turn_left {
                Some(Turn::Left)
            } else if src == ids.turn_right {
                Some(Turn::Right)
            } else {
                None
            };
            if let Some(t) = turn {
                out.push(if message.msg == msgs.mouse_press {
                    Action::TurnPress(t)
                } else {
                    Action::TurnRelease(t)
                });
            }
        } else if message.msg == msgs.listbox_new_selection {
            let pick = |ui: &Ui, cache: &ListCache<u32>| {
                selected_choice(ui, message.source, &cache.entries)
                    .and_then(|i| cache.keys.get(i).copied())
            };
            if src == ids.character_list {
                if let Some(i) = selected_choice(ui, message.source, &self.rows.entries) {
                    out.push(Action::Select(i));
                }
            } else if src == ids.race_list {
                out.extend(pick(ui, &self.races).map(Action::Race));
            } else if src == ids.class_list {
                out.extend(pick(ui, &self.classes).map(Action::Class));
            }
        } else if message.msg == msgs.mouse_dbl_click && src == ids.character_list {
            out.push(Action::Enter);
        } else if message.msg == msgs.character_input && src == ids.name_field {
            let t = ui.text_of(message.source);
            self.name_shown.clone_from(&t);
            out.push(Action::Name(t));
        }
        // Nationality entries are buttons of a list without its own label (§5.1 OPEN): a
        // click on one selects it.
        if message.msg == msgs.button_click
            && let Some(i) = self
                .nationalities
                .entries
                .iter()
                .position(|e| *e == Some(message.source))
            && let Some(n) = self.nationalities.keys.get(i)
        {
            out.push(Action::Nationality(*n));
        }
        out
    }

    /// Closes the four layouts.
    pub fn close(self, ui: &mut Ui) {
        if let Some(k) = self.el(self.refs.ids.viewport) {
            ui.set_external_content(k, None);
        }
        for (_, w) in self.controls.into_iter().map(|(_, k, w)| (k, w)) {
            match w {
                ControlWidget::Stepper(s) => s.release(ui),
                ControlWidget::Slider(s) => s.release(ui),
                ControlWidget::Swatch(s) => s.release(ui),
                ControlWidget::Sliders { which, value } => {
                    which.release(ui);
                    value.release(ui);
                }
            }
        }
        for k in self.roots.into_values() {
            ui.destroy(k);
        }
    }
}
