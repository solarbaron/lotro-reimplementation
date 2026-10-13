// SPDX-License-Identifier: GPL-3.0-or-later
//! An in-memory [`TooltipData`] for the P2 synthetic tests: our own layouts, records and
//! texts with invented labels, IDs and wording. No game data.
#![allow(dead_code, clippy::cast_possible_truncation)]

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyStruct};
use tessera_formats::reader::Utf16String;
use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{
    Did, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource, UiLayout,
};
use tessera_gamedata::tooltip::{TextRef, TextResult, TooltipData};
use tessera_text::{RichText, Value, Values};

/// A string table DID for formats stored in layouts and records.
pub const T_FORMATS: Did = Did(0x2500_0001);

/// The in-memory install.
#[derive(Default)]
pub struct Mem {
    pub names: Vec<(String, PropertyType)>,
    pub defaults: HashMap<u32, PropertyValue>,
    pub enum_texts: HashMap<(u32, u32), String>,
    pub enum_labels: HashMap<(u32, u32), String>,
    pub enum_keys: HashMap<(u32, String), u32>,
    pub records: HashMap<Did, Arc<PropertySet>>,
    pub classes: HashMap<Did, u32>,
    pub parents: HashMap<u32, u32>,
    pub weenies: HashMap<String, Did>,
    pub uniques: HashMap<String, Did>,
    pub layouts: HashMap<Did, Arc<UiLayout>>,
    pub layout_labels: HashMap<String, Did>,
    pub ui: Vec<(UiEnum, u32, String)>,
    /// (table key or table DID, token) → template with `{0x…}` slots.
    pub texts: HashMap<(u32, u32), String>,
}

impl Mem {
    /// The PID of `name`, defining it with `ty` when new (PIDs are 1-based positions).
    pub fn p(&mut self, name: &str, ty: PropertyType) -> u32 {
        if let Some(i) = self.names.iter().position(|(n, _)| n == name) {
            return i as u32 + 1;
        }
        self.names.push((name.to_owned(), ty));
        self.names.len() as u32
    }

    pub fn ui_label(&mut self, which: UiEnum, key: u32, label: &str) {
        self.ui.push((which, key, label.to_owned()));
    }

    pub fn text(&mut self, table: u32, token: u32, t: &str) {
        self.texts.insert((table, token), t.to_owned());
    }

    pub fn record(&mut self, did: Did, props: Vec<(u32, PropertyValue)>) {
        self.records.insert(did, Arc::new(set(props)));
    }

    /// Substitutes `{0x…}` slots; `None` when a slot has no value (G25).
    fn fill(template: &str, values: &Values) -> Option<String> {
        let mut out = String::new();
        let mut rest = template;
        while let Some(i) = rest.find("{0x") {
            out.push_str(&rest[..i]);
            let end = rest[i..].find('}').expect("closed slot") + i;
            let id = u32::from_str_radix(&rest[i + 3..end], 16).expect("hex");
            match values.get(id)? {
                Value::Text(s) => out.push_str(s),
                Value::Integer(n) => out.push_str(&n.to_string()),
                Value::Float(x) => out.push_str(&x.to_string()),
                _ => return None,
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        Some(out)
    }
}

impl TooltipData for Mem {
    fn pid(&self, name: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|(n, _)| n == name)
            .map(|i| i as u32 + 1)
    }
    fn property_name(&self, pid: u32) -> Option<String> {
        self.names
            .get((pid as usize).wrapping_sub(1))
            .map(|(n, _)| n.clone())
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.names
            .get((pid as usize).wrapping_sub(1))
            .map(|(_, t)| *t)
    }
    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.defaults.get(&pid).cloned()
    }
    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        self.enum_texts.get(&(pid, key)).cloned()
    }
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        self.enum_keys.get(&(pid, name.to_owned())).copied()
    }
    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        Ok(self.records.get(&did).cloned())
    }
    fn weenie(&self, label: &str) -> Option<Did> {
        self.weenies.get(label).copied()
    }
    fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String> {
        Ok(self.layouts.get(&did).cloned())
    }
    fn layout_by_label(&self, label: &str) -> Option<Did> {
        self.layout_labels.get(label).copied()
    }
    fn ui_label(&self, which: UiEnum, key: u32) -> Option<String> {
        self.ui
            .iter()
            .find(|(w, k, _)| *w == which && *k == key)
            .map(|(_, _, l)| l.clone())
    }
    fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32> {
        self.ui
            .iter()
            .find(|(w, _, l)| *w == which && l == name)
            .map(|(_, k, _)| *k)
    }
    fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
        match &info.source {
            StringSource::Literal(s) => Ok(Value::Text(String::from_utf16_lossy(&s.0))),
            StringSource::Table { token, table } => self
                .texts
                .get(&(table.0, *token))
                .map(|t| Value::Text(t.clone()))
                .ok_or_else(|| format!("miss {token:#x}")),
        }
    }
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        let template = match text {
            TextRef::Info(info) => match &info.source {
                StringSource::Literal(s) => String::from_utf16_lossy(&s.0),
                StringSource::Table { token, table } => self
                    .texts
                    .get(&(table.0, *token))
                    .cloned()
                    .ok_or_else(|| format!("miss {token:#x}"))?,
            },
            TextRef::Token { table_key, token } => self
                .texts
                .get(&(table_key, token))
                .cloned()
                .ok_or_else(|| format!("miss {token:#x}"))?,
        };
        Ok(Self::fill(&template, values)
            .filter(|t| !t.trim().is_empty())
            .map(|t| RichText::plain(&t)))
    }
    fn set_of(&self, _item: Did) -> Option<Did> {
        None
    }
    fn root_class(&self, did: Did) -> Option<u32> {
        self.classes.get(&did).copied()
    }
    fn class_lineage(&self, class: u32) -> Vec<u32> {
        let mut out = vec![class];
        let mut c = class;
        while let Some(&p) = self.parents.get(&c) {
            if out.contains(&p) || out.len() > 16 {
                break;
            }
            out.push(p);
            c = p;
        }
        out
    }
    fn enum_label(&self, pid: u32, key: u32) -> Option<String> {
        self.enum_labels.get(&(pid, key)).cloned()
    }
    fn unique(&self, label: &str) -> Option<Did> {
        self.uniques.get(label).copied()
    }
    fn properties_with_prefix(&self, prefix: &str) -> Vec<(u32, String)> {
        self.names
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| n.starts_with(prefix))
            .map(|(i, (n, _))| (i as u32 + 1, n.clone()))
            .collect()
    }
}

pub fn literal(s: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(Utf16String(s.encode_utf16().collect())),
        args: None,
    }
}

pub fn table_ref(token: u32) -> StringInfo {
    StringInfo {
        source: StringSource::Table {
            token,
            table: T_FORMATS,
        },
        args: None,
    }
}

pub fn info(i: StringInfo) -> PropertyValue {
    PropertyValue::StringInfo(Box::new(i))
}

pub fn set(props: Vec<(u32, PropertyValue)>) -> PropertySet {
    PropertySet {
        entries: props
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
        ..PropertySet::default()
    }
}

pub fn colour(argb: u32) -> PropertyValue {
    PropertyValue::Colour(argb.to_le_bytes())
}

/// An array of values with element PID `pid`.
pub fn array(pid: u32, items: Vec<PropertyValue>) -> PropertyValue {
    PropertyValue::Array(PropertyArray {
        count: items.len() as u32,
        elements: items
            .into_iter()
            .map(|value| PropertyEntry { pid, value })
            .collect(),
    })
}

/// A struct value.
pub fn strukt(fields: Vec<(u32, PropertyValue)>) -> PropertyValue {
    PropertyValue::Struct(PropertyStruct {
        fields: fields
            .into_iter()
            .map(|(pid, value)| PropertyEntry { pid, value })
            .collect(),
        ..PropertyStruct::default()
    })
}

/// An array of structs with element PID `pid`.
pub fn structs(pid: u32, items: Vec<Vec<(u32, PropertyValue)>>) -> PropertyValue {
    array(pid, items.into_iter().map(strukt).collect())
}
