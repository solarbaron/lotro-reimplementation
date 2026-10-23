// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI vocabulary: property IDs, UI message values, state IDs, engine element types,
//! engine element IDs and input-command names.
//!
//! The specs give every value next to the label it has in the user's install
//! (`ui-layout.md` §3.3, `ui-widgets.md` §2.2–§2.4, `ui-behaviour.md` §3.1, §12.2) and say
//! that implementations look the values up **by label** at run time. [`Vocab::default`]
//! holds the values the specs list (enough for synthetic trees and for runs without an
//! install); [`Vocab::resolve_with`] replaces every value whose label the caller can resolve
//! (the property dictionary for PIDs, the enum mapper chains for the rest). Labels are
//! interoperability identifiers read from the user's data; they are kept here only to look
//! the values up.

use std::collections::BTreeMap;

use tessera_formats::property::PropertyValue;

/// Declares a struct of `u32` values, each with its label and the value the specs list.
macro_rules! vocab_table {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident = ($label:literal, $value:expr)),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fm])* pub $field: u32,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $($field: $value,)* }
            }
        }

        impl $name {
            /// Every `(label, value)` pair of the table.
            #[must_use]
            pub fn entries(&self) -> Vec<(&'static str, u32)> {
                vec![$(($label, self.$field),)*]
            }

            /// Replaces each value whose label `lookup` resolves.
            pub fn resolve_with(&mut self, lookup: &mut dyn FnMut(&str) -> Option<u32>) {
                $(if let Some(v) = lookup($label) { self.$field = v; })*
            }
        }
    };
}

vocab_table! {
    /// Property IDs the UI core reads (`ui-layout.md` §5.9, `ui-widgets.md`, `ui-behaviour.md`
    /// §15). Labels are property-dictionary names. Where the specs give no value, the default
    /// is a Tessera placeholder in 0xFFFE_0001.. that no dictionary uses, so an unresolved
    /// name never matches data (synthetic trees use the placeholders directly).
    Pids {
        /// `UICore_Element_ObjectOpacity`.
        object_opacity = ("UICore_Element_ObjectOpacity", 0x00C0),
        /// `UICore_Element_alphablendmod`.
        alphablendmod = ("UICore_Element_alphablendmod", 0x06E5),
        /// `UICore_Element_ObjectMode`.
        object_mode = ("UICore_Element_ObjectMode", 0x0203),
        /// `UICore_Element_notify_on_media_done`.
        notify_on_media_done = ("UICore_Element_notify_on_media_done", 0x024F),
        /// `UICore_Element_IgnoreParentStateChange`.
        ignore_parent_state = ("UICore_Element_IgnoreParentStateChange", 0x0B58),
        /// `UICore_Text_fonts`.
        text_fonts = ("UICore_Text_fonts", 0x0025),
        /// `UICore_Text_entry`.
        text_entry = ("UICore_Text_entry", 0x0070),
        /// `UICore_Dialog_modal`.
        dialog_modal = ("UICore_Dialog_modal", 0x011C),
        /// `UICore_Dialog_UIElement_Override`.
        dialog_element_override = ("UICore_Dialog_UIElement_Override", 0x011B),
        /// `UICore_Dialog_UILayout_Override`.
        dialog_layout_override = ("UICore_Dialog_UILayout_Override", 0x055E),
        /// `UICore_Button_ghosted`.
        button_ghosted = ("UICore_Button_ghosted", 0x051F),
        /// `UICore_Button_highlighted`.
        button_highlighted = ("UICore_Button_highlighted", 0x042A),
        /// `UICore_Button_rollover`.
        button_rollover = ("UICore_Button_rollover", 0x02F8),
        /// `UICore_Button_supports_ghosted_highlight`.
        button_supports_ghosted_highlight = ("UICore_Button_supports_ghosted_highlight", 0x07A8),
        /// `UICore_Button_hot_button`.
        button_hot = ("UICore_Button_hot_button", 0x0071),
        /// `UICore_Button_hot_click_first_interval`.
        button_hot_first = ("UICore_Button_hot_click_first_interval", 0x05AE),
        /// `UICore_Button_hot_click_interval`.
        button_hot_interval = ("UICore_Button_hot_click_interval", 0x00C8),
        /// `UICore_Button_broadcast_when_ghosted`.
        button_broadcast_when_ghosted = ("UICore_Button_broadcast_when_ghosted", 0x0072),
        /// `UICore_Button_click_when_ghosted`.
        button_click_when_ghosted = ("UICore_Button_click_when_ghosted", 0x0B15),
        /// `UICore_Button_boolean_button`.
        button_boolean = ("UICore_Button_boolean_button", 0x0026),
        /// `UICore_Element_DrawAfterChildren`.
        draw_after_children = ("UICore_Element_DrawAfterChildren", 0x03CD),
        /// `UICore_Element_clip_to_parent`.
        clip_to_parent = ("UICore_Element_clip_to_parent", 0x0BD0),
        /// `UICore_Element_save_loc`.
        save_loc = ("UICore_Element_save_loc", 0x0695),
        /// `UICore_Element_save_size`.
        save_size = ("UICore_Element_save_size", 0x0476),
        /// `UICore_Element_save_visibility`.
        save_visibility = ("UICore_Element_save_visibility", 0x0475),
        /// `UICore_Element_hide`.
        hide = ("UICore_Element_hide", 0x0381),
        /// `UICore_Element_blocks_clicks`.
        blocks_clicks = ("UICore_Element_blocks_clicks", 0xFFFE_0001),
        /// `UICore_Element_context_menu`.
        context_menu = ("UICore_Element_context_menu", 0xFFFE_0002),
        /// `UICore_Element_tooltips_on`.
        tooltips_on = ("UICore_Element_tooltips_on", 0xFFFE_0003),
        /// `UICore_Element_tooltip_entry`.
        tooltip_entry = ("UICore_Element_tooltip_entry", 0xFFFE_0004),
        /// `UICore_Element_tooltip_ID`.
        tooltip_id = ("UICore_Element_tooltip_ID", 0xFFFE_0005),
        /// `UICore_Element_tooltip_layout`.
        tooltip_layout = ("UICore_Element_tooltip_layout", 0xFFFE_0006),
        /// `UICore_Element_tooltip_text_ID`.
        tooltip_text_id = ("UICore_Element_tooltip_text_ID", 0xFFFE_0007),
        /// `UICore_Element_HoverDelay`.
        hover_delay = ("UICore_Element_HoverDelay", 0xFFFE_0008),
        /// `UICore_Element_container`.
        container = ("UICore_Element_container", 0xFFFE_0009),
        /// `UICore_Element_disallow_drag_in`.
        disallow_drag_in = ("UICore_Element_disallow_drag_in", 0xFFFE_000A),
        /// `UICore_Element_disallow_drag_out`.
        disallow_drag_out = ("UICore_Element_disallow_drag_out", 0xFFFE_000B),
        /// `UICore_Element_dragable`.
        dragable = ("UICore_Element_dragable", 0xFFFE_000C),
        /// `UICore_Element_drag_rollover`.
        drag_rollover = ("UICore_Element_drag_rollover", 0xFFFE_000D),
        /// `UICore_Element_DragAcceptedState`.
        drag_accepted_state = ("UICore_Element_DragAcceptedState", 0xFFFE_000E),
        /// `UICore_Element_AllowStretchOnDrag`.
        allow_stretch_on_drag = ("UICore_Element_AllowStretchOnDrag", 0xFFFE_000F),
        /// `UICore_Element_activatable`.
        activatable = ("UICore_Element_activatable", 0xFFFE_0010),
        /// `UICore_Element_activate_on_show`.
        activate_on_show = ("UICore_Element_activate_on_show", 0xFFFE_0011),
        /// `UICore_Element_focus_on_show`.
        focus_on_show = ("UICore_Element_focus_on_show", 0xFFFE_0012),
        /// `UICore_Element_InputMapID`.
        input_map_id = ("UICore_Element_InputMapID", 0xFFFE_0013),
        /// `UICore_Element_mouse_visible_alpha_threshold`.
        alpha_threshold = ("UICore_Element_mouse_visible_alpha_threshold", 0xFFFE_0014),
        /// `UICore_Element_min_width`.
        min_width = ("UICore_Element_min_width", 0xFFFE_0015),
        /// `UICore_Element_min_height`.
        min_height = ("UICore_Element_min_height", 0xFFFE_0016),
        /// `UICore_Element_max_width`.
        max_width = ("UICore_Element_max_width", 0xFFFE_0017),
        /// `UICore_Element_max_height`.
        max_height = ("UICore_Element_max_height", 0xFFFE_0018),
        /// `UICore_Element_resize_aspect_ratio`.
        resize_aspect_ratio = ("UICore_Element_resize_aspect_ratio", 0xFFFE_0019),
        /// `UICore_Element_IgnoreResizeWidthCap`.
        ignore_resize_width_cap = ("UICore_Element_IgnoreResizeWidthCap", 0xFFFE_001A),
        /// `UICore_Element_IgnoreResizeHeightCap`.
        ignore_resize_height_cap = ("UICore_Element_IgnoreResizeHeightCap", 0xFFFE_001B),
        /// `UICore_Element_force_match_parent_scale`.
        force_match_parent_scale = ("UICore_Element_force_match_parent_scale", 0xFFFE_001C),
        /// `UICore_Element_AutoFade`.
        auto_fade = ("UICore_Element_AutoFade", 0xFFFE_001D),
        /// `UICore_Element_KeepObjectWhenHidden`.
        keep_object_when_hidden = ("UICore_Element_KeepObjectWhenHidden", 0xFFFE_001E),
        /// `UICore_Element_enable_visibility_broadcast`.
        enable_visibility_broadcast = ("UICore_Element_enable_visibility_broadcast", 0xFFFE_001F),
        /// `UICore_Element_BroadcastOnTooltipStop`.
        broadcast_on_tooltip_stop = ("UICore_Element_BroadcastOnTooltipStop", 0xFFFE_0020),
        /// `UICoreDragbar_Ancestor_UIElementID`.
        dragbar_ancestor = ("UICoreDragbar_Ancestor_UIElementID", 0xFFFE_0021),
        /// `UICore_Resizebar_border_right`.
        resize_border_right = ("UICore_Resizebar_border_right", 0xFFFE_0022),
        /// `UICore_Resizebar_border_left`.
        resize_border_left = ("UICore_Resizebar_border_left", 0xFFFE_0023),
        /// `UICore_Resizebar_border_top`.
        resize_border_top = ("UICore_Resizebar_border_top", 0xFFFE_0024),
        /// `UICore_Resizebar_border_bottom`.
        resize_border_bottom = ("UICore_Resizebar_border_bottom", 0xFFFE_0025),
        /// `UICore_Resizebar_Target_Element`.
        resize_target = ("UICore_Resizebar_Target_Element", 0xFFFE_0026),
        /// `UIComponent_UIScaler_Origin_X`.
        scaler_origin_x = ("UIComponent_UIScaler_Origin_X", 0xFFFE_0027),
        /// `UIComponent_UIScaler_Origin_Y`.
        scaler_origin_y = ("UIComponent_UIScaler_Origin_Y", 0xFFFE_0028),
        /// `UICore_Text_editable`.
        text_editable = ("UICore_Text_editable", 0xFFFE_0029),
        /// `UICore_Text_selectable`.
        text_selectable = ("UICore_Text_selectable", 0xFFFE_002A),
        /// `UICore_Text_auto_tooltip_truncated_text`.
        text_auto_tooltip = ("UICore_Text_auto_tooltip_truncated_text", 0xFFFE_002B),
        /// `UICore_Text_lose_focus_on_escape`.
        text_lose_focus_on_escape = ("UICore_Text_lose_focus_on_escape", 0xFFFE_002C),
        /// `UICore_Text_lose_focus_on_acceptinput`.
        text_lose_focus_on_accept = ("UICore_Text_lose_focus_on_acceptinput", 0xFFFE_002D),
        /// `UICore_Text_max_chars`.
        text_max_chars = ("UICore_Text_max_chars", 0xFFFE_0100),
        /// `UICore_Text_left_margin` (`fonts.md` §7, `ui-layout.md` §5.9).
        text_left_margin = ("UICore_Text_left_margin", 0xFFFE_002F),
        /// `UICore_Text_right_margin`.
        text_right_margin = ("UICore_Text_right_margin", 0xFFFE_0030),
        /// `UICore_Text_top_margin`.
        text_top_margin = ("UICore_Text_top_margin", 0xFFFE_0031),
        /// `UICore_Text_bottom_margin`.
        text_bottom_margin = ("UICore_Text_bottom_margin", 0xFFFE_0032),
        /// `UICore_Dialog_ignorekeyboardinput`.
        dialog_ignore_keyboard = ("UICore_Dialog_ignorekeyboardinput", 0xFFFE_002E),
        /// `UICore_Meter_child_meter`.
        meter_child = ("UICore_Meter_child_meter", 0x0342),
        /// `UICore_Meter_frame_meter`.
        meter_frame = ("UICore_Meter_frame_meter", 0x05FF),
        /// `UICore_Meter_alphatest_meter`.
        meter_alphatest = ("UICore_Meter_alphatest_meter", 0x0AD5),
        /// `UICore_Meter_position`.
        meter_position = ("UICore_Meter_position", 0x02F3),
        /// `UICore_Meter_goal_position`.
        meter_goal = ("UICore_Meter_goal_position", 0x0020),
        /// `UICore_Meter_smooth_movement`.
        meter_smooth = ("UICore_Meter_smooth_movement", 0x024D),
        /// `UICore_Meter_smooth_movement_duration`.
        meter_smooth_duration = ("UICore_Meter_smooth_movement_duration", 0x001F),
        /// `UICore_Meter_notify_on_new_position`.
        meter_notify = ("UICore_Meter_notify_on_new_position", 0x03CB),
        /// `UICore_Meter_child_direction`.
        meter_direction = ("UICore_Meter_child_direction", 0x06E2),
        /// `UICore_Meter_child_image`.
        meter_child_image = ("UICore_Meter_child_image", 0x011F),
        /// `UICore_Meter_child_drawmode`.
        meter_child_drawmode = ("UICore_Meter_child_drawmode", 0x05FE),
        /// `UICore_Meter_move_fill`.
        meter_move_fill = ("UICore_Meter_move_fill", 0x05A7),
        /// `UICore_Meter_offset`.
        meter_offset = ("UICore_Meter_offset", 0x024C),
        /// `UICore_Meter_frame_array`.
        meter_frame_array = ("UICore_Meter_frame_array", 0x0422),
        /// `UICore_Meter_draw_mode`.
        meter_draw_mode = ("UICore_Meter_draw_mode", 0x0341),
        /// `UICore_Scrollable_vertical_scrollbar`.
        scrollable_vbar = ("UICore_Scrollable_vertical_scrollbar", 0x01B7),
        /// `UICore_Scrollable_horizontal_scrollbar`.
        scrollable_hbar = ("UICore_Scrollable_horizontal_scrollbar", 0x051A),
        /// `UICore_Scrollable_keep_in_bounds`.
        scrollable_keep_in_bounds = ("UICore_Scrollable_keep_in_bounds", 0x0170),
        /// `UICore_Scrollable_preserve_percentage_on_resize`.
        scrollable_preserve = ("UICore_Scrollable_preserve_percentage_on_resize", 0x0561),
        /// `UICore_Scrollable_notify_on_size_change`.
        scrollable_notify_size = ("UICore_Scrollable_notify_on_size_change", 0x071F),
        /// `UICore_Scrollbar_horizontal`.
        sb_horizontal = ("UICore_Scrollbar_horizontal", 0x006B),
        /// `UICore_Scrollbar_proportional`.
        sb_proportional = ("UICore_Scrollbar_proportional", 0x006A),
        /// `UICore_Scrollbar_widget_size`.
        sb_widget_size = ("UICore_Scrollbar_widget_size", 0x06DF),
        /// `UICore_Scrollbar_min_widget_size`.
        sb_min_widget_size = ("UICore_Scrollbar_min_widget_size", 0x02F0),
        /// `UICore_Scrollbar_position`.
        sb_position = ("UICore_Scrollbar_position", 0x06E0),
        /// `UICore_Scrollbar_goal_position`.
        sb_goal = ("UICore_Scrollbar_goal_position", 0x0421),
        /// `UICore_Scrollbar_smooth_movement`.
        sb_smooth = ("UICore_Scrollbar_smooth_movement", 0x05A6),
        /// `UICore_Scrollbar_smooth_movement_duration`.
        sb_smooth_duration = ("UICore_Scrollbar_smooth_movement_duration", 0x001E),
        /// `UICore_Scrollbar_increment_button`.
        sb_inc_button = ("UICore_Scrollbar_increment_button", 0x011E),
        /// `UICore_Scrollbar_decrement_button`.
        sb_dec_button = ("UICore_Scrollbar_decrement_button", 0x02F2),
        /// `UICore_Scrollbar_disabled`.
        sb_disabled = ("UICore_Scrollbar_disabled", 0x0694),
        /// `UICore_Scrollbar_hide_when_disabled`.
        sb_hide_when_disabled = ("UICore_Scrollbar_hide_when_disabled", 0x01B6),
        /// `UICore_Scrollbar_ghost_when_disabled`.
        sb_ghost_when_disabled = ("UICore_Scrollbar_ghost_when_disabled", 0x0560),
        /// `UICore_Scrollbar_disallow_updating`.
        sb_disallow_updating = ("UICore_Scrollbar_disallow_updating", 0x0340),
        /// `UICore_Scrollbar_num_stop_locations`.
        sb_num_stops = ("UICore_Scrollbar_num_stop_locations", 0x024B),
        /// `UICore_Scrollbar_current_stop`.
        sb_current_stop = ("UICore_Scrollbar_current_stop", 0x02A0),
        /// `UICore_Scrollbar_stops_dont_include_endpoints`.
        sb_no_endpoints = ("UICore_Scrollbar_stops_dont_include_endpoints", 0x03C9),
        /// `UICore_Scrollbar_move_to_touched_position`.
        sb_slider = ("UICore_Scrollbar_move_to_touched_position", 0x03CA),
        /// `UICore_Scrollbar_page_interval`.
        sb_page_interval = ("UICore_Scrollbar_page_interval", 0x04C6),
        /// `UICore_Scrollbar_page_first_interval`.
        sb_page_first_interval = ("UICore_Scrollbar_page_first_interval", 0x06E1),
        /// `UICore_Scrollbar_horizontal_allow_mouse_wheel`.
        sb_hwheel = ("UICore_Scrollbar_horizontal_allow_mouse_wheel", 0x0519),
        /// `UICore_ListBox_horizontal`.
        lb_horizontal = ("UICore_ListBox_horizontal", 0x01B9),
        /// `UICore_ListBox_max_columns`.
        lb_max_columns = ("UICore_ListBox_max_columns", 0x0202),
        /// `UICore_ListBox_max_rows`.
        lb_max_rows = ("UICore_ListBox_max_rows", 0x024E),
        /// `UICore_ListBox_layout_only_visible`.
        lb_only_visible = ("UICore_ListBox_layout_only_visible", 0x0343),
        /// `UICore_ListBox_fill_in_reverse`.
        lb_reverse = ("UICore_ListBox_fill_in_reverse", 0x0423),
        /// `UICore_ListBox_flipped_layout`.
        lb_flipped = ("UICore_ListBox_flipped_layout", 0x0B22),
        /// `UICore_ListBox_wrap_indentation_amount`.
        lb_wrap_indent = ("UICore_ListBox_wrap_indentation_amount", 0x0B23),
        /// `UICore_ListBox_ignore_cell_alignment`.
        lb_ignore_cell = ("UICore_ListBox_ignore_cell_alignment", 0x0B24),
        /// `UICore_ListBox_click_select`.
        lb_click_select = ("UICore_ListBox_click_select", 0x06E3),
        /// `UICore_ListBox_selected_item_state_change`.
        lb_state_change = ("UICore_ListBox_selected_item_state_change", 0x0600),
        /// `UICore_ListBox_item_normal_state`.
        lb_normal_state = ("UICore_ListBox_item_normal_state", 0x00BE),
        /// `UICore_ListBox_item_selected_state`.
        lb_selected_state = ("UICore_ListBox_item_selected_state", 0x0120),
        /// `UICore_ListBox_drag_select`.
        lb_drag_select = ("UICore_ListBox_drag_select", 0x04C8),
        /// `UICore_ListBox_entry_templates`.
        lb_templates = ("UICore_ListBox_entry_templates", 0x051C),
        /// `UICore_ListBox_entry_template_layout`.
        lb_template_layout = ("UICore_ListBox_entry_template_layout", 0x01B8),
        /// `UICore_ListBox_entry_template_element`.
        lb_template_element = ("UICore_ListBox_entry_template_element", 0x04C7),
        /// `UICore_ListBox_entry_templates_autofill`.
        lb_autofill = ("UICore_ListBox_entry_templates_autofill", 0x0171),
        /// `UICore_Tree_indent_amount`.
        tree_indent = ("UICore_Tree_indent_amount", 0x00B9),
        /// `UICore_Tree_icon_layout`.
        tree_icon_layout = ("UICore_Tree_icon_layout", 0x055D),
        /// `UICore_Tree_icon_ID`.
        tree_icon_id = ("UICore_Tree_icon_ID", 0x016D),
        /// `UICore_Tree_indentelement_ID`.
        tree_indent_element = ("UICore_Tree_indentelement_ID", 0x0A6F),
        /// `UICore_Tree_indent_elements_for_icon`.
        tree_indent_for_icon = ("UICore_Tree_indent_elements_for_icon", 0x0759),
        /// `UICore_Tree_auto_expand`.
        tree_auto_expand = ("UICore_Tree_auto_expand", 0x0643),
        /// `UICore_Panel_pages`.
        panel_pages = ("UICore_Panel_pages", 0x0649),
        /// `UICore_Panel_tab_element`.
        panel_tab = ("UICore_Panel_tab_element", 0x0250),
        /// `UICore_Panel_page_element`.
        panel_page = ("UICore_Panel_page_element", 0x02A1),
        /// `UICore_Panel_page_open`.
        panel_page_open = ("UICore_Panel_page_open", 0x06E7),
        /// `UICore_Menu_popup_layout`.
        menu_popup_layout = ("UICore_Menu_popup_layout", 0x0205),
        /// `UICore_Menu_popup`.
        menu_popup = ("UICore_Menu_popup", 0x0383),
        /// `UICore_Menu_listbox`.
        menu_listbox = ("UICore_Menu_listbox", 0x02A5),
        /// `UICore_Menu_selection_display`.
        menu_selection_display = ("UICore_Menu_selection_display", 0x04CC),
        /// `UICore_Menu_default_no_selection_text`.
        menu_no_selection_text = ("UICore_Menu_default_no_selection_text", 0x0829),
        /// `UICore_Menu_default_selected_item`.
        menu_default_item = ("UICore_Menu_default_selected_item", 0x0607),
        /// `UICore_Menu_open_up`.
        menu_open_up = ("UICore_Menu_open_up", 0x0606),
        /// `UICore_Menu_open_center`.
        menu_open_center = ("UICore_Menu_open_center", 0x06EB),
        /// `UICore_Menu_auto_close_on_selection`.
        menu_auto_close = ("UICore_Menu_auto_close_on_selection", 0x047B),
        /// `UICore_GroupBox_selected_button`.
        gb_selected = ("UICore_GroupBox_selected_button", 0x02EE),
        /// `UICore_GroupBox_default_button`.
        gb_default = ("UICore_GroupBox_default_button", 0x0248),
        /// `UICore_GroupBox_allow_button_click_on_selected_button`.
        gb_allow_click_selected = ("UICore_GroupBox_allow_button_click_on_selected_button", 0x01B4),
        /// `UI_Element_CreateAsRootElement` (game level).
        create_as_root = ("UI_Element_CreateAsRootElement", 0x1000_0F07),
    }
}

vocab_table! {
    /// UI message values (`UIElementMessage` base mapper; `ui-widgets.md` §2.4,
    /// `ui-behaviour.md` §12.2, `ui-layout.md` §9.2, §9.5, §9.9).
    Msgs {
        /// `Scrollable_scrollablesize_changed`.
        scrollable_size_changed = ("Scrollable_scrollablesize_changed", 0x01),
        /// `ListBox_same_selection`.
        listbox_same_selection = ("ListBox_same_selection", 0x04),
        /// `Element_drag_global_end`.
        drag_global_end = ("Element_drag_global_end", 0x06),
        /// `Element_drag_over`.
        drag_over = ("Element_drag_over", 0x07),
        /// `Element_set_focus`.
        set_focus = ("Element_set_focus", 0x09),
        /// `Element_mouse_over_top`.
        mouse_over_top = ("Element_mouse_over_top", 0x0A),
        /// `Menu_closed`.
        menu_closed = ("Menu_closed", 0x0C),
        /// `Element_mouse_release`.
        mouse_release = ("Element_mouse_release", 0x0E),
        /// `Element_stop_drag_item`.
        stop_drag_item = ("Element_stop_drag_item", 0x0F),
        /// `Meter_new_position`.
        meter_new_position = ("Meter_new_position", 0x12),
        /// `Element_drag_item_has_been_removed`.
        drag_item_removed = ("Element_drag_item_has_been_removed", 0x14),
        /// `Menu_opened`.
        menu_opened = ("Menu_opened", 0x15),
        /// `Element_mouse_tap`.
        mouse_tap = ("Element_mouse_tap", 0x18),
        /// `Element_mouse_hover`.
        mouse_hover = ("Element_mouse_hover", 0x19),
        /// `Element_mouse_over`.
        mouse_over = ("Element_mouse_over", 0x1A),
        /// `Element_media_done`.
        media_done = ("Element_media_done", 0x1B),
        /// `Element_mouse_dbl_click`.
        mouse_dbl_click = ("Element_mouse_dbl_click", 0x1D),
        /// `Element_mouse_click`.
        mouse_click = ("Element_mouse_click", 0x1E),
        /// `Element_not_dragable`.
        not_dragable = ("Element_not_dragable", 0x20),
        /// `Scrollbar_page_decrement`.
        sb_page_decrement = ("Scrollbar_page_decrement", 0x22),
        /// `Element_auto_fade_stop`.
        auto_fade_stop = ("Element_auto_fade_stop", 0x24),
        /// `Element_mouse_move`.
        mouse_move = ("Element_mouse_move", 0x25),
        /// `Button_hot_click`.
        button_hot_click = ("Button_hot_click", 0x29),
        /// `Text_character_input`.
        character_input = ("Text_character_input", 0x28),
        /// `Element_auto_fade_start`.
        auto_fade_start = ("Element_auto_fade_start", 0x2A),
        /// `Element_open_context_menu`.
        open_context_menu = ("Element_open_context_menu", 0x2B),
        /// `Element_mouse_scroll`.
        mouse_scroll = ("Element_mouse_scroll", 0x2C),
        /// `Element_key_release`.
        key_release = ("Element_key_release", 0x2D),
        /// `Element_saved_location_applied`.
        saved_location_applied = ("Element_saved_location_applied", 0x2E),
        /// `Element_deactivated`.
        deactivated = ("Element_deactivated", 0x2F),
        /// `Element_drag_global_begin`.
        drag_global_begin = ("Element_drag_global_begin", 0x30),
        /// `Element_mouse_press`.
        mouse_press = ("Element_mouse_press", 0x32),
        /// `Scrollbar_line_increment`.
        sb_line_increment = ("Scrollbar_line_increment", 0x33),
        /// `Scrollbar_new_position`.
        sb_new_position = ("Scrollbar_new_position", 0x34),
        /// `Button_click`.
        button_click = ("Button_click", 0x35),
        /// `Meter_goal_position_reached`.
        meter_goal_reached = ("Meter_goal_position_reached", 0x36),
        /// `Panel_NewPage`.
        panel_new_page = ("Panel_NewPage", 0x37),
        /// `Scrollbar_page_increment`.
        sb_page_increment = ("Scrollbar_page_increment", 0x3A),
        /// `Element_set_show_state`.
        set_show_state = ("Element_set_show_state", 0x3B),
        /// `ListBox_new_selection`.
        listbox_new_selection = ("ListBox_new_selection", 0x3C),
        /// `Element_activated`.
        activated = ("Element_activated", 0x3D),
        /// `Scrollbar_line_decrement`.
        sb_line_decrement = ("Scrollbar_line_decrement", 0x3E),
        /// `Element_key_press`.
        key_press = ("Element_key_press", 0x3F),
        /// `Element_tooltip_stopped`.
        tooltip_stopped = ("Element_tooltip_stopped", 0x44),
        /// `Element_is_visible_changed`.
        is_visible_changed = ("Element_is_visible_changed", 0x45),
    }
}

/// Tessera-only notification values: the plugin API's `SizeChanged`, `PositionChanged` and
/// `VisibleChanged` travel as notifications (`lua-plugin-api.md` §9.6–§9.7) but the specs
/// name no `UIElementMessage` for them. They use values outside the mapper's range.
pub mod tessera_msg {
    /// The element's size changed.
    pub const SIZE_CHANGED: u32 = 0xF000_0001;
    /// The element's position changed.
    pub const POSITION_CHANGED: u32 = 0xF000_0002;
    /// The element's own visibility flag changed.
    pub const VISIBLE_CHANGED: u32 = 0xF000_0003;
    /// A plugin button was activated (`Button.Click`, `lua-plugin-api.md` §13.6).
    pub const CONTROL_CLICK: u32 = 0xF000_0004;
    /// A one-shot Impulse mouse-button notification (`lua-plugin-api.md` §13.12.1).
    pub const IMPULSE: u32 = 0xF000_0005;
    /// A text box's text changed (`TextBox.TextChanged`).
    pub const TEXT_CHANGED: u32 = 0xF000_0006;
}

vocab_table! {
    /// State IDs (`UIStateID` base mapper; `ui-layout.md` §9.7, `ui-widgets.md` §2.4,
    /// `ui-behaviour.md` §5.1–§5.2, §7.3).
    States {
        /// `highlight_pressed`.
        highlight_pressed = ("highlight_pressed", 0x01),
        /// `normal_pressed`.
        normal_pressed = ("normal_pressed", 0x03),
        /// `open`.
        open = ("open", 0x06),
        /// `highlight`.
        highlight = ("highlight", 0x07),
        /// `normal_rollover`.
        normal_rollover = ("normal_rollover", 0x08),
        /// `closed`.
        closed = ("closed", 0x09),
        /// `normal`.
        normal = ("normal", 0x0B),
        /// `highlight_rollover`.
        highlight_rollover = ("highlight_rollover", 0x11),
        /// `ghosted`.
        ghosted = ("ghosted", 0x12),
        /// `ghosted_highlight`.
        ghosted_highlight = ("ghosted_highlight", 0x13),
        /// `ghosted_rollover`.
        ghosted_rollover = ("ghosted_rollover", 0x14),
        /// `ghosted_pressed`.
        ghosted_pressed = ("ghosted_pressed", 0x15),
        /// `normal_activated` (not keyed in the specs; resolved by label at run time).
        normal_activated = ("normal_activated", 0xFFFF_FF01),
        /// `normal_focussed` (resolved by label at run time).
        normal_focussed = ("normal_focussed", 0xFFFF_FF02),
        /// `drag_rollover_accept` (resolved by label at run time).
        drag_rollover_accept = ("drag_rollover_accept", 0xFFFF_FF03),
        /// `drag_rollover_reject` (resolved by label at run time).
        drag_rollover_reject = ("drag_rollover_reject", 0xFFFF_FF04),
    }
}

vocab_table! {
    /// Engine element types (`UIElementType` base mapper, keys 1–19; `ui-layout.md` §3.3,
    /// `ui-widgets.md` §3.1).
    Types {
        /// GroupBox.
        group_box = ("GroupBox", 1),
        /// ColorPicker.
        color_picker = ("ColorPicker", 2),
        /// Scrollable (no factory).
        scrollable = ("Scrollable", 3),
        /// Menu.
        menu = ("Menu", 4),
        /// Scrollbar.
        scrollbar = ("Scrollbar", 5),
        /// ComboBox (no factory).
        combo_box = ("ComboBox", 6),
        /// ListBox.
        list_box = ("ListBox", 7),
        /// Panel.
        panel = ("Panel", 8),
        /// Button.
        button = ("Button", 9),
        /// Tree.
        tree = ("Tree", 10),
        /// Viewport.
        viewport = ("Viewport", 11),
        /// Browser.
        browser = ("Browser", 12),
        /// Frame (no factory).
        frame = ("Frame", 13),
        /// Field.
        field = ("Field", 14),
        /// Dragbar.
        dragbar = ("Dragbar", 15),
        /// Text.
        text = ("Text", 16),
        /// Meter.
        meter = ("Meter", 17),
        /// ResizeBar.
        resize_bar = ("ResizeBar", 18),
        /// Flash (no factory).
        flash = ("Flash", 19),
    }
}

vocab_table! {
    /// Engine element IDs the engine looks up by ID (`ui-widgets.md` §4.3, §6.2,
    /// `ui-behaviour.md` §5.5, `ui-layout.md` §8.5).
    ElementIds {
        /// `meter_fill_image`.
        meter_fill_image = ("meter_fill_image", 0x23),
        /// `meter_fill`.
        meter_fill = ("meter_fill", 0x26),
        /// `widget`: a scroll bar's thumb.
        widget = ("widget", 0x16),
        /// `scrollbar_left_fill`.
        sb_left_fill = ("scrollbar_left_fill", 0x1D),
        /// `scrollbar_right_fill`.
        sb_right_fill = ("scrollbar_right_fill", 0x20),
        /// `scrollbar_top_fill`.
        sb_top_fill = ("scrollbar_top_fill", 0x03),
        /// `scrollbar_bottom_fill`.
        sb_bottom_fill = ("scrollbar_bottom_fill", 0x09),
        /// `DialogConfirmationYesButton`.
        dialog_yes = ("DialogConfirmationYesButton", 0x0F),
        /// `DialogConfirmationNoButton`.
        dialog_no = ("DialogConfirmationNoButton", 0x21),
        /// `Hollow`: the screen root.
        screen_root = ("Hollow", 0x13),
    }
}

/// The InputActions command names the UI routes by class (`ui-behaviour.md` §3.1,
/// `ui-widgets.md` §2.4, decision 0014 D1). These are interoperability identifiers of the
/// key map.
pub mod commands {
    /// The nine engine mouse-button commands (`ui-behaviour.md` §3.1).
    pub const MOUSE_BUTTONS: [&str; 9] = [
        SELECT_LEFT,
        SELECT_RIGHT,
        SELECT_MID,
        SELECT_DBL_LEFT,
        SELECT_DBL_RIGHT,
        SELECT_DBL_MID,
        "TapLeft",
        "TapRight",
        "TapMid",
    ];
    /// Left select.
    pub const SELECT_LEFT: &str = "SelectLeft";
    /// Right select.
    pub const SELECT_RIGHT: &str = "SelectRight";
    /// Middle select.
    pub const SELECT_MID: &str = "SelectMid";
    /// Left double select.
    pub const SELECT_DBL_LEFT: &str = "SelectDblLeft";
    /// Right double select.
    pub const SELECT_DBL_RIGHT: &str = "SelectDblRight";
    /// Middle double select.
    pub const SELECT_DBL_MID: &str = "SelectDblMid";
    /// Wheel up.
    pub const SCROLL_UP: &str = "ScrollUp";
    /// Wheel down.
    pub const SCROLL_DOWN: &str = "ScrollDown";
    /// Analog wheel.
    pub const SCROLL: &str = "Scroll";
    /// Escape.
    pub const ESCAPE: &str = "EscapeKey";
    /// Accept input (Return).
    pub const ACCEPT_INPUT: &str = "AcceptInput";
    /// Accept key.
    pub const ACCEPT_KEY: &str = "AcceptKey";
    /// Next focus.
    pub const FOCUS_NEXT: &str = "FocusNext";
    /// Previous focus.
    pub const FOCUS_PREVIOUS: &str = "FocusPrevious";
    /// Mouse-look (camera steer).
    pub const MOUSELOOK: &str = "CameraInstantMouseSteer";
    /// Backspace.
    pub const BACKSPACE: &str = "BackspaceKey";
    /// Delete.
    pub const DELETE: &str = "DeleteKey";
    /// Caret left.
    pub const CURSOR_CHAR_LEFT: &str = "CursorCharLeft";
    /// Caret right.
    pub const CURSOR_CHAR_RIGHT: &str = "CursorCharRight";
    /// Caret to line start.
    pub const CURSOR_START_OF_LINE: &str = "CursorStartOfLine";
    /// Caret to line end.
    pub const CURSOR_END_OF_LINE: &str = "CursorEndOfLine";
    /// Caret to text start.
    pub const CURSOR_START_OF_DOCUMENT: &str = "CursorStartOfDocument";
    /// Caret to text end.
    pub const CURSOR_END_OF_DOCUMENT: &str = "CursorEndOfDocument";
    /// Copy.
    pub const COPY: &str = "CopyText";
    /// Cut.
    pub const CUT: &str = "CutText";
    /// Paste.
    pub const PASTE: &str = "PasteText";
}

/// InputMap names the UI pushes (`ui-behaviour.md` §5.2, decision 0014 D5), plus the Tessera
/// context `ui.chat_log`.
pub mod input_maps {
    /// The scroll-wheel map pushed on hover (`ui-widgets.md` §5.7, §6.9).
    pub const SCROLLABLE_MOUSE: &str = "ScrollableControls_Mouse";
    /// Text editing.
    pub const EDIT: &str = "EditControls";
    /// Copy and paste.
    pub const CLIPBOARD: &str = "CopyAndPasteControls";
    /// Dialog keys.
    pub const DIALOG: &str = "DialogBoxes";
    /// The chat window's Tessera-only context (decision 0014 D5).
    pub const CHAT_LOG: &str = "ui.chat_log";
}

/// Everything the UI core looks up by label, plus the dictionary defaults of the PIDs it
/// reads (`ui-widgets.md` §2.2 step 2).
#[derive(Debug, Clone, PartialEq)]
pub struct Vocab {
    /// Property IDs.
    pub pids: Pids,
    /// UI message values.
    pub msgs: Msgs,
    /// State IDs.
    pub states: States,
    /// Engine element types.
    pub types: Types,
    /// Engine element IDs.
    pub ids: ElementIds,
    /// Dictionary defaults by PID (`ui-widgets.md` §2.2).
    pub defaults: BTreeMap<u32, PropertyValue>,
}

impl Default for Vocab {
    fn default() -> Self {
        let pids = Pids::default();
        let mut v = Self {
            pids,
            msgs: Msgs::default(),
            states: States::default(),
            types: Types::default(),
            ids: ElementIds::default(),
            defaults: BTreeMap::new(),
        };
        v.set_spec_defaults();
        v
    }
}

impl Vocab {
    /// Sets the dictionary defaults that `ui-widgets.md` §2.2 lists, plus the documented
    /// defaults of `UICore_Element_ObjectOpacity` and `UICore_Element_alphablendmod` (1.0,
    /// `ui-layout.md` §5.9, `lua-plugin-api.md` §13.19) and `UICore_Element_AllowStretchOnDrag`
    /// (true, `ui-behaviour.md` §7.2).
    pub fn set_spec_defaults(&mut self) {
        let p = self.pids;
        let s = self.states;
        let d = &mut self.defaults;
        d.insert(p.sb_widget_size, PropertyValue::Float(1.0));
        d.insert(p.scrollable_keep_in_bounds, PropertyValue::Bool(true));
        d.insert(p.lb_normal_state, PropertyValue::Enum(s.normal));
        d.insert(p.lb_selected_state, PropertyValue::Enum(s.highlight));
        d.insert(p.lb_horizontal, PropertyValue::Bool(false));
        d.insert(p.lb_only_visible, PropertyValue::Bool(false));
        d.insert(p.tree_indent_for_icon, PropertyValue::Bool(true));
        d.insert(p.menu_auto_close, PropertyValue::Bool(true));
        d.insert(p.button_hot_interval, PropertyValue::Float(0.05));
        d.insert(p.button_hot_first, PropertyValue::Float(0.2));
        d.insert(p.meter_notify, PropertyValue::Bool(false));
        d.insert(p.sb_hwheel, PropertyValue::Bool(false));
        d.insert(p.object_opacity, PropertyValue::Float(1.0));
        d.insert(p.alphablendmod, PropertyValue::Float(1.0));
        if p.allow_stretch_on_drag != 0 {
            d.insert(p.allow_stretch_on_drag, PropertyValue::Bool(true));
        }
        d.remove(&0);
    }

    /// Replaces every value whose label resolves: `pid` through the property dictionary,
    /// `message`, `state`, `element_type` and `element` through the `UIElementMessage`,
    /// `UIStateID`, `UIElementType` and `UIElementID` mapper chains (`ui-layout.md` §3.3).
    /// The dictionary defaults are then rebuilt from the spec table, with `default` (the
    /// dictionary's own default for a PID, when it has one) taking precedence.
    pub fn resolve_with(
        &mut self,
        pid: &mut dyn FnMut(&str) -> Option<u32>,
        message: &mut dyn FnMut(&str) -> Option<u32>,
        state: &mut dyn FnMut(&str) -> Option<u32>,
        element_type: &mut dyn FnMut(&str) -> Option<u32>,
        element: &mut dyn FnMut(&str) -> Option<u32>,
        default: &mut dyn FnMut(u32) -> Option<PropertyValue>,
    ) {
        self.pids.resolve_with(pid);
        self.msgs.resolve_with(message);
        self.states.resolve_with(state);
        self.types.resolve_with(element_type);
        self.ids.resolve_with(element);
        self.defaults.clear();
        self.set_spec_defaults();
        for (_, p) in self.pids.entries() {
            if p != 0
                && let Some(v) = default(p)
            {
                self.defaults.insert(p, v);
            }
        }
    }

    /// The engine class of an element type (`ui-widgets.md` §3.1).
    #[must_use]
    pub fn class_of(&self, element_type: u32) -> Option<crate::widgets::WidgetClass> {
        use crate::widgets::WidgetClass as C;
        let t = &self.types;
        Some(match element_type {
            x if x == t.group_box => C::GroupBox,
            x if x == t.color_picker => C::ColorPicker,
            x if x == t.menu => C::Menu,
            x if x == t.scrollbar => C::Scrollbar,
            x if x == t.list_box => C::ListBox,
            x if x == t.panel => C::Panel,
            x if x == t.button => C::Button,
            x if x == t.tree => C::Tree,
            x if x == t.viewport => C::Viewport,
            x if x == t.browser => C::Browser,
            x if x == t.field => C::Field,
            x if x == t.dragbar => C::Dragbar,
            x if x == t.text => C::Text,
            x if x == t.meter => C::Meter,
            x if x == t.resize_bar => C::ResizeBar,
            _ => return None,
        })
    }
}
