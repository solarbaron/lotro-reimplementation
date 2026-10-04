// SPDX-License-Identifier: GPL-3.0-or-later
//! The property registry: PID → name, type and definition, from the master property
//! dictionary (`property-system.md` §2).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::{
    Did, DidKind, PropertyDefinition, PropertyDictionary, PropertySchema, PropertyType, Record,
};

use crate::error::{PropsError, Result};
use crate::value::Value;

/// DID of the master property dictionary (`property-system.md` §2).
pub const PROPERTY_DICTIONARY_DID: Did = Did(0x3400_0000);

/// Where the labels of an enum or bitfield property come from (`property-system.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumSource {
    /// An enum mapper (prefix 0x23): keys are looked up in it and up its base chain.
    /// Used by enum values (type 8) and bitfields (types 14, 18, 22).
    Mapper(Did),
    /// A DID map (prefix 0x28): an enum value (type 8) is a key of the map, whose label
    /// is the display name and whose DID is the target (§7.1).
    DidMap(Did),
}

impl EnumSource {
    /// The DID of the mapper or map.
    #[must_use]
    pub fn did(self) -> Did {
        match self {
            Self::Mapper(d) | Self::DidMap(d) => d,
        }
    }
}

/// The property registry: every PID's name, value type and definition.
///
/// Wraps the decoded master property dictionary (DID 0x34000000). It is the
/// [`PropertySchema`] for decoding property sets, and it answers name and metadata
/// queries for display.
#[derive(Debug, Clone)]
pub struct PropertyRegistry {
    dict: PropertyDictionary,
    /// Shared copies of the names (first name of each PID), so labels and resolved trees
    /// hold a pointer instead of a fresh copy per value.
    shared_names: HashMap<u32, Arc<str>>,
}

impl PropertyRegistry {
    /// Wraps an already decoded dictionary.
    #[must_use]
    pub fn from_dictionary(dict: PropertyDictionary) -> Self {
        let mut shared_names = HashMap::with_capacity(dict.names().len());
        for (pid, name) in dict.names() {
            shared_names
                .entry(*pid)
                .or_insert_with(|| Arc::<str>::from(name.as_str()));
        }
        Self { dict, shared_names }
    }

    /// Decodes the dictionary record payload (DID 0x34000000).
    pub fn parse(data: &[u8]) -> Result<Self> {
        let dict = PropertyDictionary::parse(data).map_err(|e| {
            PropsError::decode(PROPERTY_DICTIONARY_DID, PropertyDictionary::NAME, e)
        })?;
        Ok(Self::from_dictionary(dict))
    }

    /// The underlying dictionary.
    #[must_use]
    pub fn dictionary(&self) -> &PropertyDictionary {
        &self.dict
    }

    /// Number of definitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dict.definitions().len()
    }

    /// Whether the registry defines no property.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dict.definitions().is_empty()
    }

    /// The internal name of a PID (Latin-1 identifiers, not shown to players).
    #[must_use]
    pub fn name(&self, pid: u32) -> Option<&str> {
        self.dict.name(pid)
    }

    /// The same name as [`PropertyRegistry::name`], as a shared string (no copy of the
    /// text is made).
    #[must_use]
    pub fn shared_name(&self, pid: u32) -> Option<Arc<str>> {
        self.shared_names.get(&pid).cloned()
    }

    /// The PID with the given name (the first stored one if several share it).
    #[must_use]
    pub fn pid(&self, name: &str) -> Option<u32> {
        self.dict.pid(name)
    }

    /// The definition of a PID.
    #[must_use]
    pub fn definition(&self, pid: u32) -> Option<&PropertyDefinition> {
        self.dict.definition(pid)
    }

    /// The value type of a PID.
    #[must_use]
    pub fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.definition(pid).map(|d| d.ty)
    }

    /// Where the labels of an enum (type 8) or bitfield (types 14, 18, 22) property come
    /// from; `None` for other types or when the definition's `data` field names neither
    /// an enum mapper nor a DID map.
    #[must_use]
    pub fn enum_source(&self, pid: u32) -> Option<EnumSource> {
        let def = self.definition(pid)?;
        let kind = def.data.kind();
        match def.ty {
            PropertyType::Enum => match kind {
                DidKind::EnumMapper => Some(EnumSource::Mapper(def.data)),
                DidKind::DataIdMap => Some(EnumSource::DidMap(def.data)),
                _ => None,
            },
            PropertyType::Bitfield64 | PropertyType::Bitfield32 | PropertyType::BitfieldAny => {
                (kind == DidKind::EnumMapper).then_some(EnumSource::Mapper(def.data))
            }
            _ => None,
        }
    }

    /// For a DID-reference property (type 20), the DB-object-type code its values refer
    /// to (§9). Its label is in the "DBOType" enum mapper
    /// ([`crate::GameData::dbo_type_name`]).
    #[must_use]
    pub fn dbo_type(&self, pid: u32) -> Option<u8> {
        self.definition(pid).and_then(PropertyDefinition::dbo_type)
    }

    /// The child PIDs of a struct or array definition (§2.4); empty for an "open"
    /// definition or an unknown PID.
    #[must_use]
    pub fn children(&self, pid: u32) -> &[u32] {
        self.definition(pid).map_or(&[], |d| d.children.as_slice())
    }

    /// Whether `child` may appear in a struct or array value of `parent` (§5.6): true for
    /// listed children and for every PID of an open definition (no children).
    #[must_use]
    pub fn allows_child(&self, parent: u32, child: u32) -> bool {
        let kids = self.children(parent);
        kids.is_empty() || kids.contains(&child)
    }

    /// The default value stored in the definition, if any.
    #[must_use]
    pub fn default_value(&self, pid: u32) -> Option<Value> {
        let raw = self.definition(pid)?.default.clone()?;
        Value::from_raw_pid(raw, pid).ok()
    }

    /// Iterates the definitions with their names, in stored order.
    pub fn iter(&self) -> impl Iterator<Item = (&PropertyDefinition, Option<&str>)> + '_ {
        self.dict
            .definitions()
            .iter()
            .map(|d| (d, self.dict.name(d.pid)))
    }
}

impl PropertySchema for PropertyRegistry {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        PropertyRegistry::property_type(self, pid)
    }
}

impl From<PropertyDictionary> for PropertyRegistry {
    fn from(dict: PropertyDictionary) -> Self {
        Self::from_dictionary(dict)
    }
}
