// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's fallback crafting windows (`crafting-panels.md` §11, decision 0031 tier 1 and
//! runs without an install): the crafting panel (an 800 × 580 window with the tab row, the
//! profession pages with their trees and the recipe page) and the 349 × 375 trainer window,
//! built from code-made elements with the install's element IDs ([`P6Vocab`]) at the design
//! rects of §3–§5 and §8.3 (sizes and positions of parts the spec leaves out are Tessera's),
//! Tessera's colours (craftable rows green, `requirement_notmet` lines red, ghosted buttons
//! grey) and the client's [`P6Art`]. The controllers of [`super::panel`] and
//! [`super::trainer`] attach to these trees exactly as to the install's layouts; the parts
//! they create per row (tabs, pages, rows, tier marks) come from the node builders here when
//! the client names no install template.

use tessera_formats::did::Did;
use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::{Argb, DrawMode, MediaItem};

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::Vocab;
use crate::widgets::WidgetClass;

use super::super::GameError;
use super::super::npc::kit::{FNode, PAGE_NEXT_ID, PAGE_PREV_ID, colour_value, colours, open_root};
use super::super::npcvocab::NpcVocab;
use super::super::p6vocab::{P6Vocab, own};
use super::{CraftLabel, CraftTexts, P6Art, RowKind, TrainerKind, craft_label};

/// Height of a fallback title bar.
pub const TITLE_H: i32 = 24;

/// Size of a fallback recipe row (`Craft_Profession_RecipeTemplate`, 350 wide; Tessera:
/// 340 so that it fits the tree beside its scroll bar).
pub const ROW_SIZE: (i32, i32) = (340, 20);

/// Size of a fallback tier row (`CraftProfession_TierTemplate`, 340 × 20).
pub const TIER_SIZE: (i32, i32) = (340, 20);

/// Size of a fallback meter mark (`CraftProfession_ProfMeterTemplate`, 149 × 20).
pub const METER_SIZE: (i32, i32) = (149, 20);

/// The meter bar inside a meter mark (`CraftProfession_Meter`, (2, 0) 121 × 9; Tessera
/// moves it down to the row's middle and shortens it so that the value text fits beside
/// it in the 149-pixel mark).
pub const METER_BAR: (i32, i32, i32, i32) = (2, 6, 88, 9);

/// Size of a fallback icon mark (`CraftProfession_ProfIconTemplate`, 32 × 16).
pub const ICON_MARK_SIZE: (i32, i32) = (32, 16);

/// Size of a fallback tab.
pub const TAB_SIZE: (i32, i32) = (90, 24);

/// Tessera's colours of the crafting fallbacks (`0xAARRGGBB`); the install's values where
/// §4.2 and §5.1 name one.
pub mod craft_colours {
    /// A recipe row's text in the default state (Tessera).
    pub const ROW: u32 = 0xFFC0_C0C0;
    /// `RecipeGroup_Active` (§4.2: 0xFF29DF46).
    pub const ACTIVE: u32 = 0xFF29_DF46;
    /// A tier row's text (§4.2: 0xFFE2FB02).
    pub const TIER: u32 = 0xFFE2_FB02;
    /// The pack count (§4.2: 0xFF1FA935).
    pub const PACKS: u32 = 0xFF1F_A935;
    /// A cooldown row's timer (§4.2: 0xFFFFF600).
    pub const TIMER: u32 = 0xFFFF_F600;
    /// The experience line (§5.1: 0xFFF7E4A9).
    pub const EXPERIENCE: u32 = 0xFFF7_E4A9;
    /// The recipe page's cooldown timer (§5.1: 0xFFB80000).
    pub const COOLDOWN: u32 = 0xFFB8_0000;
    /// A met requirement line (§5.1: 0xFF59BF0E).
    pub const MET: u32 = 0xFF59_BF0E;
    /// A requirement line in `requirement_notmet` (Tessera).
    pub const NOT_MET: u32 = 0xFFFF_4040;
    /// The trainer's name text (§8.3: 0xFFECFF00).
    pub const TRAINER_NAME: u32 = 0xFFEC_FF00;
    /// A meter's back (Tessera).
    pub const METER_BACK: u32 = 0xFF20_2228;
    /// The proficiency fill (Tessera).
    pub const PROFICIENCY: u32 = 0xFF3C_8CE0;
    /// The mastery fill (Tessera).
    pub const MASTERY: u32 = 0xFFE0_B030;
    /// The master overlay (§5.1: 0xDD000000).
    pub const OVERLAY: u32 = 0xDD00_0000;
    /// The not-enough look of an ingredient's item display (Tessera).
    pub const NOT_ENOUGH: u32 = 0x80A0_2020;
}

/// `node` with a solid colour fill (a Coloured image item without an image, drawn with
/// Alphablend) in front of its own media.
pub(crate) fn fill(mut node: FNode, argb: u32) -> FNode {
    node.desc.block.media.push(MediaItem::ColouredImage {
        did: Did(0),
        draw_mode: DrawMode(4),
        colour: Argb(argb),
    });
    node
}

/// Shared builders of the fallback parts.
pub(crate) struct Kit<'a> {
    pub v: Vocab,
    pub nv: &'a NpcVocab,
    pub p6: &'a P6Vocab,
    pub art: &'a P6Art,
}

impl<'a> Kit<'a> {
    /// A kit over `ui`'s vocabulary.
    pub fn new(ui: &Ui, nv: &'a NpcVocab, p6: &'a P6Vocab, art: &'a P6Art) -> Self {
        Self {
            v: ui.vocab().clone(),
            nv,
            p6,
            art,
        }
    }

    /// The text-colour property.
    pub fn cp(&self) -> u32 {
        self.nv.pids.text_font_color
    }

    /// A text.
    pub fn text(&self, id: u32, r: (i32, i32, i32, i32), colour: u32, t: &str) -> FNode {
        FNode::text(id, r.0, r.1, r.2, r.3, self.cp(), colour, t)
    }

    /// A button with the label colour in `normal` (so that unghosting restores it).
    pub fn button(&self, id: u32, r: (i32, i32, i32, i32), t: &str) -> FNode {
        let base = &self.art.base;
        FNode::button(
            id,
            r.0,
            r.1,
            r.2,
            r.3,
            (&base.button, &base.button_ghosted),
            self.cp(),
            self.v.states.ghosted,
            t,
        )
        .state(
            self.v.states.normal,
            base.button.as_ref(),
            vec![(self.cp(), colour_value(colours::TITLE))],
        )
    }

    /// A check box: a boolean Button (`UICore_Button_boolean_button`, `ui-widgets.md` §7)
    /// whose `highlight` state shows the checked art.
    pub fn checkbox(&self, id: u32, x: i32, y: i32) -> FNode {
        let st = self.v.states;
        FNode::new(WidgetClass::Button, id, x, y, 14, 14)
            .image(&self.art.check_off)
            .prop(self.v.pids.button_boolean, PropertyValue::Bool(true))
            .state(st.normal, self.art.check_off.as_ref(), vec![])
            .state(st.highlight, self.art.check_on.as_ref(), vec![])
            .state(st.ghosted, self.art.base.button_ghosted.as_ref(), vec![])
    }

    /// A close button.
    pub fn close(&self, id: u32, x: i32, y: i32) -> FNode {
        let base = &self.art.base;
        FNode::button(
            id,
            x,
            y,
            16,
            16,
            (&base.close, &base.close),
            self.cp(),
            self.v.states.ghosted,
            "",
        )
    }

    /// A title bar with its text.
    pub fn title_bar(&self, w: i32, id: u32, t: &str) -> FNode {
        FNode::field(0, 0, 0, w, TITLE_H)
            .image(&self.art.base.title_bar)
            .child(self.text(id, (10, 4, w - 40, 16), colours::TITLE, t))
    }

    /// An editable one-line entry of at most `max` characters.
    pub fn entry(&self, id: u32, r: (i32, i32, i32, i32), max: i32) -> FNode {
        let p = self.v.pids;
        FNode::new(WidgetClass::Text, id, r.0, r.1, r.2, r.3)
            .image(&self.art.base.entry)
            .prop(self.cp(), colour_value(colours::WHITE))
            .prop(p.text_editable, PropertyValue::Bool(true))
            .prop(p.text_max_chars, PropertyValue::Int(max))
            .prop(self.nv.pids.text_one_line, PropertyValue::Bool(true))
    }

    /// A requirement line with its `normal` and `requirement_notmet` colours.
    pub fn requirement(&self, id: u32, r: (i32, i32, i32, i32)) -> FNode {
        let cp = self.cp();
        self.text(id, r, craft_colours::MET, "")
            .state(
                self.v.states.normal,
                None,
                vec![(cp, colour_value(craft_colours::MET))],
            )
            .state(
                self.p6.states.requirement_not_met,
                None,
                vec![(cp, colour_value(craft_colours::NOT_MET))],
            )
    }

    /// An item display: a slot frame with the icon (`ItemDisplay_Icon`), the quantity
    /// overlay (hidden) and the not-enough overlay (hidden), `w` × 32 with the name at the
    /// right when `name`.
    pub fn item_display(&self, id: u32, x: i32, y: i32, name: bool) -> FNode {
        let ids = self.nv.ids;
        let mut n = FNode::field(id, x, y, if name { 343 } else { 32 }, 36).child(
            FNode::field(0, 0, 2, 32, 32)
                .image(&self.art.base.slot)
                .child(FNode::field(ids.item_icon, 0, 0, 32, 32))
                .child(
                    self.text(ids.item_quantity, (0, 18, 30, 14), colours::WHITE, "")
                        .hidden(),
                )
                .child(
                    fill(
                        FNode::field(own::CRAFT_NOT_ENOUGH, 0, 0, 32, 32),
                        craft_colours::NOT_ENOUGH,
                    )
                    .hidden(),
                ),
        );
        if name {
            n = n.child(self.text(ids.item_name, (40, 9, 300, 18), colours::WHITE, ""));
        }
        n
    }

    /// An ingredient slot (`CraftIngredient`, 175 × 36): the item display and the info text.
    pub fn ingredient(&self, id: u32, x: i32, y: i32, w: i32) -> FNode {
        let ids = self.p6.ids;
        FNode::field(id, x, y, w, 36).children([
            self.item_display(ids.ingredient_item, 0, 0, false),
            self.text(ids.ingredient_info, (36, 9, w - 38, 18), colours::WHITE, ""),
        ])
    }

    /// A page control with Tessera's previous and next buttons and a page text.
    pub fn pager(&self, id: u32, r: (i32, i32, i32, i32)) -> FNode {
        let w = 22;
        FNode::field(id, r.0, r.1, r.2, r.3).children([
            self.button(PAGE_PREV_ID, (0, 0, w, r.3), "<"),
            self.text(
                own::CRAFT_PAGER_TEXT,
                (w + 2, 2, r.2 - 2 * w - 4, r.3 - 4),
                colours::WHITE,
                "",
            ),
            self.button(PAGE_NEXT_ID, (r.2 - w, 0, w, r.3), ">"),
        ])
    }

    /// A tab (`Craft_ProfTabTemplate`): a button with the `normal` and `highlight` (chosen)
    /// art.
    pub fn tab(&self, text: &str) -> FNode {
        let st = self.v.states;
        let base = &self.art.base;
        FNode::new(
            WidgetClass::Field,
            own::CRAFT_TAB,
            0,
            0,
            TAB_SIZE.0,
            TAB_SIZE.1,
        )
        .image(&base.tab)
        .state(st.normal, base.tab.as_ref(), vec![])
        .state(st.highlight, base.tab_selected.as_ref(), vec![])
        .child(self.text(0, (6, 4, TAB_SIZE.0 - 12, 16), colours::TITLE, text))
    }

    /// A profession page (`Craft_Profession_Window`, 375 × 516): the search, the expand and
    /// collapse buttons and the recipe tree.
    pub fn profession_page(&self, texts: &dyn CraftTexts) -> FNode {
        let ids = self.p6.ids;
        let p = self.v.pids;
        FNode::field(ids.prof_window, 0, 0, 375, 516).children([
            self.button(ids.expand_all, (4, 6, 16, 16), "+"),
            self.button(ids.collapse_all, (23, 6, 16, 16), "-"),
            self.text(
                ids.search_label,
                (51, 6, 48, 18),
                colours::TITLE,
                &craft_label(texts, CraftLabel::Search),
            ),
            self.entry(
                ids.search_entry,
                (101, 4, 142, 20),
                i32::try_from(super::SEARCH_CHARS).unwrap_or(80),
            ),
            FNode::new(WidgetClass::Tree, ids.recipe_tree, 5, 30, 360, 480)
                .image(&self.art.base.panel)
                .prop(p.lb_click_select, PropertyValue::Bool(true))
                .prop(p.tree_auto_expand, PropertyValue::Bool(true))
                .prop(p.tree_indent, PropertyValue::Int(8)),
        ])
    }

    /// The "train more" page (`Craft_TrainProfPageTemplate`): its centred text.
    pub fn train_page(&self, text: &str) -> FNode {
        FNode::field(0, 0, 0, 375, 516).child(self.text(
            self.p6.ids.train_page_text,
            (40, 200, 295, 60),
            colours::WHITE,
            text,
        ))
    }

    /// A recipe row of `kind` (§4.2 templates): the row text with the `RecipeGroup_Active`
    /// colour, the kind's icon at (2, 2) (text at x 20) and, for a cooldown row, the timer;
    /// the pack field (hidden).
    pub fn recipe_row(&self, kind: RowKind) -> FNode {
        let ids = self.p6.ids;
        let st = self.v.states;
        let cp = self.cp();
        let (id, icon) = match kind {
            RowKind::Plain => (ids.row_plain, None),
            RowKind::OneUse => (ids.row_one_use, Some(&self.art.one_use)),
            RowKind::MultiOutput => (ids.row_multi, Some(&self.art.multi_output)),
            RowKind::Cooldown => (ids.row_cooldown, Some(&self.art.cooldown)),
        };
        let tx = if icon.is_some() { 20 } else { 6 };
        let text = self
            .text(ids.row_text, (tx, 2, 270 - tx, 16), craft_colours::ROW, "")
            .state(
                st.normal,
                None,
                vec![(cp, colour_value(craft_colours::ROW))],
            )
            .state(
                self.p6.states.recipe_active,
                None,
                vec![(cp, colour_value(craft_colours::ACTIVE))],
            );
        let base = &self.art.base;
        let mut row = FNode::field(id, 0, 0, ROW_SIZE.0, ROW_SIZE.1)
            .state(st.normal, None, vec![])
            .state(st.highlight, base.row_highlight.as_ref(), vec![])
            .child(text)
            .child(
                FNode::field(ids.row_packs, 280, 2, 58, 16)
                    .children([
                        FNode::field(0, 0, 0, 16, 16).image(&self.art.pack),
                        self.text(
                            own::CRAFT_PACK_COUNT,
                            (18, 0, 40, 16),
                            craft_colours::PACKS,
                            "",
                        ),
                    ])
                    .hidden(),
            );
        if let Some(icon) = icon {
            let mut i = FNode::field(
                if kind == RowKind::Cooldown {
                    ids.row_cooldown_icon
                } else {
                    0
                },
                2,
                2,
                16,
                16,
            )
            .image(icon);
            if kind == RowKind::Cooldown {
                i = i
                    .state(st.normal, self.art.cooldown.as_ref(), vec![])
                    .state(
                        self.p6.states.cooldown_ready,
                        self.art.cooldown_ready.as_ref(),
                        vec![],
                    );
            }
            row = row.child(i);
        }
        if kind == RowKind::Cooldown {
            row = row.child(
                self.text(ids.row_timer, (200, 2, 136, 16), craft_colours::TIMER, "")
                    .hidden(),
            );
        }
        row
    }

    /// A tier row (`CraftProfession_TierTemplate`, 340 × 20): the text and a horizontal list
    /// of up to four marks at x 150.
    pub fn tier_row(&self) -> FNode {
        let ids = self.p6.ids;
        let st = self.v.states;
        let cp = self.cp();
        let p = self.v.pids;
        FNode::field(ids.tier_row, 0, 0, TIER_SIZE.0, TIER_SIZE.1)
            .state(st.normal, None, vec![])
            .state(st.highlight, self.art.base.row_highlight.as_ref(), vec![])
            .children([
                self.text(ids.tier_text, (2, 2, 148, 16), craft_colours::TIER, "")
                    .state(
                        st.normal,
                        None,
                        vec![(cp, colour_value(craft_colours::TIER))],
                    )
                    .state(
                        self.p6.states.recipe_active,
                        None,
                        vec![(cp, colour_value(craft_colours::ACTIVE))],
                    ),
                FNode::new(WidgetClass::ListBox, ids.tier_marks, 150, 0, 190, 20)
                    .prop(p.lb_horizontal, PropertyValue::Bool(true))
                    .prop(p.lb_max_columns, PropertyValue::Int(4))
                    .prop(p.lb_max_rows, PropertyValue::Int(1)),
            ])
    }

    /// A meter mark (`CraftProfession_ProfMeterTemplate` or `…MasteryMeterTemplate`): the
    /// meter (`CraftProfession_Meter`, a back with a fill whose width is the position) and
    /// its text.
    pub fn meter_mark(&self, mastery: bool) -> FNode {
        let ids = self.p6.ids;
        let (id, colour) = if mastery {
            (ids.mastery_meter, craft_colours::MASTERY)
        } else {
            (ids.prof_meter, craft_colours::PROFICIENCY)
        };
        let (x, y, w, h) = METER_BAR;
        FNode::field(id, 0, 0, METER_SIZE.0, METER_SIZE.1).children([
            fill(
                FNode::field(ids.meter, x, y, w, h),
                craft_colours::METER_BACK,
            )
            .child(fill(
                FNode::field(own::CRAFT_METER_FILL, 0, 0, 0, h),
                colour,
            )),
            self.text(ids.meter_text, (93, 2, 56, 16), colours::WHITE, ""),
        ])
    }

    /// An icon mark (`CraftProfession_ProfIconTemplate` or `…MasteryIconTemplate`).
    pub fn icon_mark(&self, mastery: bool) -> FNode {
        let ids = self.p6.ids;
        let (id, art) = if mastery {
            (ids.mastery_icon, &self.art.mastery)
        } else {
            (ids.prof_icon, &self.art.proficiency)
        };
        FNode::field(id, 0, 2, ICON_MARK_SIZE.0, ICON_MARK_SIZE.1).image(art)
    }

    /// An optional slot (`CraftIngredient_Optional`, 360 × 36): the ingredient, the check
    /// box and the bonus text.
    fn optional_slot(&self, id: u32) -> FNode {
        let ids = self.p6.ids;
        FNode::field(id, 49, 32, 360, 36)
            .children([
                self.ingredient(0, 0, 0, 260),
                self.checkbox(ids.optional_button, 272, 4),
                self.text(
                    ids.optional_crit_text,
                    (292, 9, 66, 18),
                    colours::YELLOW,
                    "",
                ),
            ])
            .hidden()
    }

    /// The recipe page (`Craft_Recipe_Window`, 415 × 522, §5.1).
    pub fn recipe_page(&self, texts: &dyn CraftTexts) -> FNode {
        let ids = self.p6.ids;
        let nv = self.nv;
        let l = |x: CraftLabel| craft_label(texts, x);
        let output = FNode::field(ids.multi_output, 312, 6, 99, 41)
            .children([
                self.text(0, (0, 0, 99, 16), colours::TITLE, &l(CraftLabel::Outputs)),
                self.pager(ids.multi_output_pager, (0, 18, 99, 22)),
            ])
            .hidden();
        let header = FNode::field(0, 0, 0, 413, 93)
            .image(&self.art.base.panel)
            .children([
                self.item_display(ids.recipe_item, 10, 8, false),
                FNode::field(ids.non_item_result, 10, 10, 32, 32).hidden(),
                self.text(ids.recipe_name, (50, 14, 261, 20), colours::TITLE, ""),
                output,
                self.text(
                    ids.earned_points,
                    (10, 54, 250, 18),
                    craft_colours::EXPERIENCE,
                    "",
                ),
                self.text(
                    ids.one_use,
                    (254, 54, 153, 18),
                    colours::YELLOW,
                    &l(CraftLabel::OneUse),
                )
                .hidden(),
                self.text(ids.cooldown_line, (10, 73, 235, 18), colours::WHITE, "")
                    .hidden(),
                self.text(
                    ids.cooldown_timer,
                    (240, 73, 169, 18),
                    craft_colours::COOLDOWN,
                    "",
                )
                .hidden(),
            ]);
        let requirements = FNode::field(0, 0, 94, 413, 52)
            .image(&self.art.base.panel)
            .children([
                self.requirement(ids.tool_line, (3, 8, 410, 18)),
                self.requirement(ids.facility_line, (3, 28, 410, 18)),
            ]);
        let slot_at = [
            (29, 30),
            (212, 30),
            (29, 76),
            (212, 76),
            (29, 121),
            (212, 121),
        ];
        let mut main = FNode::field(ids.main_ingredients, 0, 147, 413, 197)
            .image(&self.art.base.panel)
            .child(self.text(
                0,
                (10, 6, 200, 18),
                colours::TITLE,
                &l(CraftLabel::Ingredients),
            ));
        for (i, id) in self.p6.ingredient_ids().into_iter().enumerate() {
            let (x, y) = slot_at[i];
            main = main.child(self.ingredient(id, x, y, 175).hidden());
        }
        let packs = FNode::field(ids.packs_field, 0, 147, 413, 197)
            .image(&self.art.base.panel)
            .children([
                self.text(
                    0,
                    (10, 6, 300, 18),
                    colours::TITLE,
                    &l(CraftLabel::Ingredients),
                ),
                self.ingredient(ids.pack_display, 29, 30, 300),
            ])
            .hidden();
        let use_pack = FNode::field(ids.use_pack, 210, 150, 200, 16)
            .children([
                self.checkbox(0, 0, 1),
                self.text(ids.use_pack_label, (18, 0, 182, 16), colours::WHITE, ""),
            ])
            .hidden();
        let mut master = FNode::field(ids.master_page, 0, 345, 413, 126)
            .image(&self.art.base.panel)
            .children([
                self.text(0, (10, 7, 88, 18), colours::TITLE, &l(CraftLabel::Optional)),
                self.pager(ids.optional_pager, (100, 5, 92, 20)).hidden(),
                self.text(
                    0,
                    (235, 7, 80, 18),
                    colours::TITLE,
                    &l(CraftLabel::Critical),
                ),
                self.text(ids.critical_chance, (315, 7, 90, 18), colours::YELLOW, ""),
                self.text(
                    0,
                    (9, 69, 183, 18),
                    colours::TITLE,
                    &l(CraftLabel::CriticalOutput),
                ),
                self.item_display(ids.critical_item, 9, 88, true),
                FNode::field(ids.critical_image, 10, 90, 32, 32).hidden(),
                self.text(ids.critical_name, (48, 97, 300, 18), colours::WHITE, "")
                    .hidden(),
            ])
            .hidden();
        for id in self.p6.optional_ids() {
            master = master.child(self.optional_slot(id));
        }
        let overlay = fill(
            FNode::field(ids.master_overlay, 0, 350, 413, 120),
            craft_colours::OVERLAY,
        )
        .child(self.text(
            0,
            (20, 50, 373, 20),
            colours::WHITE,
            &l(CraftLabel::MasterOverlay),
        ))
        .hidden();
        let make = FNode::field(0, 0, 472, 413, 50)
            .image(&self.art.base.panel)
            .children([
                self.text(
                    0,
                    (120, 18, 76, 18),
                    colours::TITLE,
                    &l(CraftLabel::Quantity),
                ),
                self.button(ids.make_decrease, (198, 16, 19, 20), "-"),
                self.entry(ids.quantity_entry, (217, 16, 26, 20), 3),
                self.button(ids.make_increase, (243, 16, 19, 20), "+"),
                self.button(ids.make, (266, 16, 70, 20), &l(CraftLabel::Make)),
                self.button(ids.make_all, (339, 16, 70, 20), &l(CraftLabel::MakeAll)),
            ]);
        let _ = nv;
        FNode::field(ids.recipe_window, 0, 0, 415, 522).children([
            header,
            requirements,
            main,
            packs,
            use_pack,
            master,
            overlay,
            make,
        ])
    }
}

/// The crafting panel (`Craft_Window`, 800 × 580, §11): title, close, the tab row and page
/// parent of the dynamic panel, the recipe page, the "hide when crafting" check box and the
/// no-profession field. Pages and tabs are added by [`super::CraftPanel`].
///
/// # Errors
/// The element cap or an attachment error.
pub fn panel(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
    texts: &dyn CraftTexts,
) -> Result<ElementKey, GameError> {
    let k = Kit::new(ui, nv, p6, art);
    let ids = p6.ids;
    let l = |x: CraftLabel| craft_label(texts, x);
    let dynamic = FNode::field(ids.craft_dynamic_panel, 3, 28, 378, 520).children([
        FNode::field(ids.dynamic_tabs, 0, 0, 378, TAB_SIZE.1),
        FNode::field(ids.dynamic_pages, 0, 28, 375, 492).image(&art.base.panel),
    ]);
    let root = FNode::field(ids.craft_window, 0, 0, 800, 580)
        .image(&art.base.window)
        .children([
            k.title_bar(800, ids.craft_title, &l(CraftLabel::Title)),
            k.close(ids.craft_close, 776, 4),
            dynamic,
            FNode::field(ids.craft_recipe_page, 381, 28, 415, 522)
                .child(k.recipe_page(texts))
                .hidden(),
            k.checkbox(ids.hide_on_execute, 386, 557),
            k.text(
                ids.hide_on_execute_text,
                (404, 555, 200, 18),
                colours::WHITE,
                &l(CraftLabel::HideOnExecute),
            ),
            FNode::field(ids.no_profession, 0, 24, 800, 556)
                .image(&art.base.window)
                .child(k.text(
                    ids.no_profession_text,
                    (150, 236, 500, 40),
                    colours::WHITE,
                    &l(CraftLabel::NoProfession),
                ))
                .hidden(),
        ]);
    open_root(ui, &root)
}

/// A crafting-trainer window (§11: 349 × 375): the title, the NPC's name, the display text,
/// the profession trainer's label and selector (hidden), Train, Cancel, a close button and
/// Tessera's confirmation field (hidden).
///
/// # Errors
/// The element cap or an attachment error.
pub fn trainer(
    ui: &mut Ui,
    nv: &NpcVocab,
    p6: &P6Vocab,
    art: &P6Art,
    kind: TrainerKind,
    texts: &dyn CraftTexts,
) -> Result<ElementKey, GameError> {
    let k = Kit::new(ui, nv, p6, art);
    let ids = p6.ids;
    let l = |x: CraftLabel| craft_label(texts, x);
    let (root_id, name, text, train, cancel) = match kind {
        TrainerKind::Vocation => (
            ids.vocation_window,
            ids.vocation_name,
            ids.vocation_text,
            ids.vocation_train,
            ids.vocation_cancel,
        ),
        TrainerKind::Profession => (
            ids.profession_window,
            ids.profession_name,
            ids.profession_text,
            ids.profession_train,
            ids.profession_cancel,
        ),
        TrainerKind::Guild => (
            ids.guild_window,
            ids.guild_name,
            ids.guild_text,
            ids.guild_train,
            ids.guild_cancel,
        ),
    };
    let title = if kind == TrainerKind::Guild {
        CraftLabel::GuildTrainerTitle
    } else {
        CraftLabel::TrainerTitle
    };
    let text_h = if kind == TrainerKind::Profession {
        198
    } else {
        252
    };
    let mut children = vec![
        k.title_bar(349, 0, &l(title)),
        k.close(own::CRAFT_TRAINER_CLOSE, 323, 4),
        k.text(name, (18, 37, 301, 30), craft_colours::TRAINER_NAME, ""),
        FNode::field(0, 12, 71, 325, text_h)
            .image(&art.base.panel)
            .child(k.text(text, (4, 4, 317, text_h - 8), colours::BODY, "")),
    ];
    if kind == TrainerKind::Profession {
        children.push(
            k.text(
                ids.lose_profession,
                (55, 281, 239, 20),
                colours::TITLE,
                &l(CraftLabel::LoseProfession),
            )
            .hidden(),
        );
        children.push(
            k.button(ids.profession_selector, (53, 302, 239, 25), "")
                .hidden(),
        );
    }
    children.push(k.button(train, (51, 337, 158, 22), &l(CraftLabel::Train)));
    children.push(k.button(cancel, (222, 337, 79, 22), &l(CraftLabel::Cancel)));
    children.push(confirm(&k, texts));
    let root = FNode::field(root_id, 0, 0, 349, 375)
        .image(&art.base.window)
        .children(children);
    open_root(ui, &root)
}

/// Tessera's two-button confirmation of a trainer window (§8.3 Tessera rule; hidden): the
/// text and the train and cancel buttons, 289 × 130 at (30, 110). The trainer controller
/// adds it to an install window that has none.
pub(crate) fn confirm(k: &Kit<'_>, texts: &dyn CraftTexts) -> FNode {
    let l = |x: CraftLabel| craft_label(texts, x);
    FNode::field(own::CRAFT_CONFIRM, 30, 110, 289, 130)
        .image(&k.art.base.window)
        .children([
            k.text(
                own::CRAFT_CONFIRM_TEXT,
                (10, 10, 269, 70),
                colours::WHITE,
                "",
            ),
            k.button(
                own::CRAFT_CONFIRM_YES,
                (30, 92, 100, 24),
                &l(CraftLabel::Train),
            ),
            k.button(
                own::CRAFT_CONFIRM_NO,
                (159, 92, 100, 24),
                &l(CraftLabel::Cancel),
            ),
        ])
        .hidden()
}
