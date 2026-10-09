// SPDX-License-Identifier: GPL-3.0-or-later
//! The chat-window configuration `GOP_ChatWindows` (`chat-window.md` §3.3): windows by ID,
//! their visibility and their tabs (label and 64-bit type mask).

use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct, PropertyValue};
use tessera_formats::string_info::StringInfo;

use crate::props::{array_elements, as_bool, as_int, struct_field};

use super::super::GameError;
use super::super::vocab::GamePids;

/// Most windows (§3.2: nine window elements).
pub const MAX_WINDOWS: usize = 9;

/// Most tabs per window (Tessera cap; the primary tab list shows 2 × 9).
pub const MAX_TABS: usize = 64;

/// One tab (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TabConfig {
    /// `GOP_ChatWindow_TabName`.
    pub name: Option<StringInfo>,
    /// `GOP_ChatWindow_TabChatTextTypes`.
    pub mask: u64,
}

/// One window (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    /// `GOP_ChatWindow_WindowID`.
    pub id: i32,
    /// `GOP_ChatWindow_Visibility`.
    pub visible: bool,
    /// `GOP_ChatWindow_Tabs`, in tab-list order.
    pub tabs: Vec<TabConfig>,
}

/// The whole configuration, in creation order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatConfig {
    /// Windows.
    pub windows: Vec<WindowConfig>,
}

impl ChatConfig {
    /// Reads a `GOP_ChatWindows` array value (hostile input: bounded by [`MAX_WINDOWS`] and
    /// [`MAX_TABS`]; malformed entries are skipped, a window without an ID too).
    ///
    /// # Errors
    /// [`GameError::Malformed`] when the value is not an array.
    pub fn from_value(v: &PropertyValue, p: &GamePids) -> Result<Self, GameError> {
        if v.as_array().is_none() {
            return Err(GameError::Malformed(p.gop_chat_windows));
        }
        let mut c = Self::default();
        for w in array_elements(v) {
            if c.windows.len() >= MAX_WINDOWS {
                break;
            }
            let set = &w.value;
            let Some(id) = struct_field(set, p.gop_window_id).map(as_int) else {
                continue;
            };
            let visible = struct_field(set, p.gop_window_visible).is_some_and(as_bool);
            let mut tabs = Vec::new();
            if let Some(ts) = struct_field(set, p.gop_window_tabs) {
                for t in array_elements(ts).iter().take(MAX_TABS) {
                    let name = match struct_field(&t.value, p.gop_tab_name) {
                        Some(PropertyValue::StringInfo(si)) => Some((**si).clone()),
                        _ => None,
                    };
                    let mask = struct_field(&t.value, p.gop_tab_types)
                        .and_then(PropertyValue::as_bits)
                        .unwrap_or(0);
                    tabs.push(TabConfig { name, mask });
                }
            }
            if c.windows.iter().any(|x| x.id == id) {
                continue;
            }
            c.windows.push(WindowConfig { id, visible, tabs });
        }
        Ok(c)
    }

    /// Writes the configuration back as a `GOP_ChatWindows` value (§3.3 "Saving"): a window
    /// is written with its ID and tabs only if it has at least one tab. `window_pid` and
    /// `tab_pid` are the element PIDs of the two arrays (`GOP_ChatWindow`,
    /// `GOP_ChatWindow_Tab`).
    #[must_use]
    pub fn to_value(&self, p: &GamePids, window_pid: u32, tab_pid: u32) -> PropertyValue {
        let mut arr = PropertyArray::default();
        for w in self.windows.iter().filter(|w| !w.tabs.is_empty()) {
            let mut tabs = PropertyArray::default();
            for t in &w.tabs {
                let mut fields = Vec::new();
                if let Some(n) = &t.name {
                    fields.push((
                        p.gop_tab_name,
                        PropertyValue::StringInfo(Box::new(n.clone())),
                    ));
                }
                fields.push((p.gop_tab_types, PropertyValue::Bitfield64(t.mask)));
                tabs.elements.push(PropertyEntry {
                    pid: tab_pid,
                    value: strukt(fields),
                });
            }
            tabs.count = u32::try_from(tabs.elements.len()).unwrap_or(u32::MAX);
            let fields = vec![
                (p.gop_window_id, PropertyValue::Int(w.id)),
                (p.gop_window_visible, PropertyValue::Bool(w.visible)),
                (p.gop_window_tabs, PropertyValue::Array(tabs)),
            ];
            arr.elements.push(PropertyEntry {
                pid: window_pid,
                value: strukt(fields),
            });
        }
        arr.count = u32::try_from(arr.elements.len()).unwrap_or(u32::MAX);
        PropertyValue::Array(arr)
    }
}

fn strukt(fields: Vec<(u32, PropertyValue)>) -> PropertyValue {
    let mut s = PropertyStruct::default();
    for (pid, value) in fields {
        s.fields.push(PropertyEntry { pid, value });
    }
    s.size.count = u32::try_from(s.fields.len()).unwrap_or(u32::MAX);
    PropertyValue::Struct(s)
}
