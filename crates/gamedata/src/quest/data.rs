// SPDX-License-Identifier: GPL-3.0-or-later
//! What the journal and NPC-window models read: the [`PresentationData`] trait (the
//! tooltip model's [`TooltipData`] plus named string tables, the argument IDs of a text and
//! enum label names) and its implementation over an install ([`InstallPresentationData`]).
//! The models never touch archives, so tests can supply the same facts from memory.

use std::sync::Arc;

use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{
    Did, ElementDesc, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource, UiLayout,
};
use tessera_props::{PropertyRegistry, RecordSource};
use tessera_text::{NamedTables, STRING_TABLE_MAP_LABEL, Strings, Value, Values};

use crate::store::{GameDataStore, InstallSource};
use crate::tooltip::{InstallTooltipData, TextRef, TextResult, TooltipData, TooltipError};

/// The facts the presentation models of `quest-journal.md` and `npc-windows.md` read. Every
/// method is a lookup; none may panic on missing or malformed data.
pub trait PresentationData: TooltipData {
    /// The DID of the string table the `STRINGTABLE` map names `label`
    /// (`string-table.md` §9.1).
    fn table(&self, label: &str) -> Option<Did>;
    /// The argument IDs a text needs from its caller: the IDs of the message's slots that
    /// the StringInfo's own arguments do not fill, each once, in first-use order. Empty for
    /// a literal. `Err` when the text cannot be found.
    fn text_args(&self, text: TextRef<'_>) -> Result<Vec<u32>, String>;
    /// The label (internal name) of `key` in the enum mapper of `pid` and its base chain.
    fn enum_name(&self, pid: u32, key: u32) -> Option<String>;
}

/// [`PresentationData`] over an install: [`InstallTooltipData`] plus the named string
/// tables and the strings of one locale.
pub struct InstallPresentationData<'a, S: RecordSource = InstallSource> {
    inner: InstallTooltipData<'a, S>,
    store: &'a GameDataStore<S>,
    strings: Arc<Strings>,
    registry: Arc<PropertyRegistry>,
    named: NamedTables,
}

impl<S: RecordSource> std::fmt::Debug for InstallPresentationData<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallPresentationData")
            .field("named_tables", &self.named.tables().len())
            .finish_non_exhaustive()
    }
}

impl<'a, S: RecordSource> InstallPresentationData<'a, S> {
    /// Reads the dictionary, the UI labels, the layout map and the named string tables of
    /// the install behind `store`; `strings` gives the locale's texts.
    ///
    /// # Errors
    ///
    /// [`TooltipError::DataUnavailable`] when the property dictionary cannot be read.
    pub fn new(store: &'a GameDataStore<S>, strings: Arc<Strings>) -> Result<Self, TooltipError> {
        let inner = InstallTooltipData::new(store, Arc::clone(&strings))?;
        let gd = store.object_store().game_data();
        let registry = gd
            .registry()
            .map_err(|e| TooltipError::DataUnavailable(format!("property dictionary: {e}")))?
            .clone();
        let named = if strings.named_tables().tables().is_empty() {
            gd.did_map_by_label(STRING_TABLE_MAP_LABEL)
                .ok()
                .flatten()
                .map(|m| NamedTables::from_did_map(&m))
                .unwrap_or_default()
        } else {
            strings.named_tables().clone()
        };
        Ok(Self {
            inner,
            store,
            strings,
            registry,
            named,
        })
    }

    /// The strings in use.
    #[must_use]
    pub fn strings(&self) -> &Arc<Strings> {
        &self.strings
    }
}

impl<S: RecordSource> TooltipData for InstallPresentationData<'_, S> {
    fn pid(&self, name: &str) -> Option<u32> {
        self.inner.pid(name)
    }
    fn property_name(&self, pid: u32) -> Option<String> {
        self.inner.property_name(pid)
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.inner.property_type(pid)
    }
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.inner.default_value(pid)
    }
    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        self.inner.enum_text(pid, key)
    }
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        self.inner.enum_key(pid, name)
    }
    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        self.inner.record(did)
    }
    fn weenie(&self, label: &str) -> Option<Did> {
        self.inner.weenie(label)
    }
    fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String> {
        self.inner.layout(did)
    }
    fn layout_by_label(&self, label: &str) -> Option<Did> {
        self.inner.layout_by_label(label)
    }
    fn ui_label(&self, which: UiEnum, key: u32) -> Option<String> {
        self.inner.ui_label(which, key)
    }
    fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32> {
        self.inner.ui_key(which, name)
    }
    fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
        self.inner.text_value(info)
    }
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        self.inner.render(text, values)
    }
    fn set_of(&self, item: Did) -> Option<Did> {
        self.inner.set_of(item)
    }
}

impl<S: RecordSource> PresentationData for InstallPresentationData<'_, S> {
    fn table(&self, label: &str) -> Option<Did> {
        self.named.by_label(label).map(Did)
    }

    fn text_args(&self, text: TextRef<'_>) -> Result<Vec<u32>, String> {
        let (msg, own) = match text {
            TextRef::Info(info) => match &info.source {
                StringSource::Literal(_) => return Ok(Vec::new()),
                StringSource::Table { token, table } => (
                    self.strings
                        .message(table.0, *token)
                        .map_err(|m| m.to_string())?,
                    own_args(info),
                ),
            },
            TextRef::Token { table_key, token } => {
                let table = self
                    .named
                    .by_key(table_key)
                    .ok_or_else(|| format!("no named string table with key {table_key:#010x}"))?;
                (
                    self.strings
                        .message(table, token)
                        .map_err(|m| m.to_string())?,
                    Vec::new(),
                )
            }
        };
        let mut out: Vec<u32> = Vec::new();
        for id in msg.arg_ids() {
            if !own.contains(&id) && !out.contains(&id) {
                out.push(id);
            }
        }
        Ok(out)
    }

    fn enum_name(&self, pid: u32, key: u32) -> Option<String> {
        let mapper = self.registry.enum_source(pid)?.did();
        let label = self
            .store
            .object_store()
            .game_data()
            .enum_label(mapper, key)
            .ok()?;
        label.name.map(|n| n.to_string())
    }
}

/// The argument IDs a StringInfo fills itself.
pub(crate) fn own_args(info: &StringInfo) -> Vec<u32> {
    info.args
        .as_ref()
        .map(|a| a.values.iter().map(|v| v.var_id).collect())
        .unwrap_or_default()
}

/// A StringInfo naming `token` of `table`.
#[must_use]
pub(crate) fn token_info(table: Did, token: u32) -> StringInfo {
    StringInfo {
        source: StringSource::Table { token, table },
        args: None,
    }
}

/// Renders `token` of `table` (`None`: the table is not in the install).
pub(crate) fn render_token(
    data: &dyn PresentationData,
    table: Option<Did>,
    token: u32,
    values: &Values,
) -> TextResult {
    let table = table.ok_or_else(|| format!("string table of token {token:#010x} not found"))?;
    data.render(TextRef::Info(&token_info(table, token)), values)
}

/// A StringInfo as a nested message value with `extra` added to its own values (its own
/// arguments win). Literal texts become plain text values.
pub(crate) fn info_value(
    data: &dyn PresentationData,
    info: &StringInfo,
    extra: &Values,
) -> Result<Value, String> {
    Ok(match data.text_value(info)? {
        Value::Message(mut mv) => {
            for (id, v) in extra.iter() {
                if mv.values.get(id).is_none() {
                    mv.values.insert(id, v.clone());
                }
            }
            Value::Message(mv)
        }
        other => other,
    })
}

/// Most elements visited while searching one layout (a Tessera bound).
pub const MAX_LAYOUT_WALK: usize = 65_536;

/// The element with ID `element` anywhere in `layout` (top level first, then depth first),
/// within [`MAX_LAYOUT_WALK`] elements.
#[must_use]
pub fn find_element(layout: &UiLayout, element: u32) -> Option<&ElementDesc> {
    if let Some(e) = layout.elements.iter().find(|e| e.element_id == element) {
        return Some(e);
    }
    let mut stack: Vec<&ElementDesc> = layout.elements.iter().rev().collect();
    let mut seen = 0usize;
    while let Some(e) = stack.pop() {
        seen += 1;
        if seen > MAX_LAYOUT_WALK {
            tracing::warn!(layout = %layout.did, "layout walk limit reached");
            return None;
        }
        if e.element_id == element {
            return Some(e);
        }
        stack.extend(e.children.iter().rev());
    }
    None
}

/// Why a layout element cannot be found: the failure categories `LayoutMissing` and
/// `ElementMissing` of `npc-windows.md` §11 (and `quest-journal.md` §9), or a read error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The `UILAYOUT` label is absent, or names no layout record.
    #[error("LayoutMissing: {layout}")]
    LayoutMissing {
        /// The layout label.
        layout: String,
    },
    /// The element label is unknown, or the layout has no such element (at the place
    /// asked for).
    #[error("ElementMissing: {layout}/{element}{detail}")]
    ElementMissing {
        /// The layout label.
        layout: String,
        /// The element label.
        element: String,
        /// What exactly is missing (empty, " (no label)" or " is not top-level").
        detail: &'static str,
    },
    /// The layout record could not be read.
    #[error("layout {layout}: {message}")]
    Read {
        /// The layout label.
        layout: String,
        /// The reader's message.
        message: String,
    },
}

impl From<LayoutError> for String {
    fn from(e: LayoutError) -> Self {
        e.to_string()
    }
}

/// The element labelled `element` in the layout the `UILAYOUT` map names `layout`; `Err`
/// names what is missing (`LayoutMissing` / `ElementMissing` of `npc-windows.md` §11).
pub fn layout_element(
    data: &dyn PresentationData,
    layout: &str,
    element: &str,
) -> Result<(Arc<UiLayout>, u32), LayoutError> {
    let missing = || LayoutError::LayoutMissing {
        layout: layout.to_owned(),
    };
    let element_missing = |detail| LayoutError::ElementMissing {
        layout: layout.to_owned(),
        element: element.to_owned(),
        detail,
    };
    let did = data.layout_by_label(layout).ok_or_else(missing)?;
    let l = data
        .layout(did)
        .map_err(|message| LayoutError::Read {
            layout: layout.to_owned(),
            message,
        })?
        .ok_or_else(missing)?;
    let id = data
        .ui_key(UiEnum::ElementId, element)
        .ok_or_else(|| element_missing(" (no label)"))?;
    if find_element(&l, id).is_none() {
        return Err(element_missing(""));
    }
    Ok((l, id))
}

/// Whether the element labelled `root` is a top-level element of the layout `layout`.
pub fn top_level_root(
    data: &dyn PresentationData,
    layout: &str,
    root: &str,
) -> Result<(), LayoutError> {
    let (l, id) = layout_element(data, layout, root)?;
    if l.elements.iter().any(|e| e.element_id == id) {
        Ok(())
    } else {
        Err(LayoutError::ElementMissing {
            layout: layout.to_owned(),
            element: root.to_owned(),
            detail: " is not top-level",
        })
    }
}

/// The value of property `pid` on element `element` (default state block first, then the
/// other state blocks).
#[must_use]
pub fn element_property(element: &ElementDesc, pid: u32) -> Option<&PropertyValue> {
    element
        .block
        .properties
        .get(pid)
        .or_else(|| element.states.iter().find_map(|s| s.properties.get(pid)))
}
