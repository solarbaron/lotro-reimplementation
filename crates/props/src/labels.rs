// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable forms of property values: enum and bit labels, property names, DID-reference
//! types (`property-system.md` §7).

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use tessera_formats::{Did, PropertyType, StringInfo};

use crate::value::Value;

/// The label of one enum key, found in an enum mapper or its base chain (§7.1).
///
/// The symbolic name and the display text are looked up independently: each comes from
/// the first mapper in the chain that has one for the key. Both are shared with the
/// cached mapper, so a label costs the same whatever the length of its texts.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumLabel {
    /// The key.
    pub key: u32,
    /// Internal symbolic name, if any mapper in the chain names the key.
    pub name: Option<Arc<str>>,
    /// Localized display text (a StringInfo to resolve against the string tables), if
    /// any mapper in the chain has one for the key.
    pub display: Option<Arc<StringInfo>>,
}

impl EnumLabel {
    /// Whether a name or display text was found.
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.name.is_some() || self.display.is_some()
    }
}

/// The label of one set bit of a bitfield (§7.3).
#[derive(Debug, Clone, PartialEq)]
pub struct BitLabel {
    /// Bit index, from 0 at the least significant bit (byte 0 bit 0 for type 22).
    pub bit: u32,
    /// The enum key that labels it: `bit + 1`.
    pub label: EnumLabel,
}

/// What a value means beyond its raw number (§7). Produced by
/// [`crate::GameData::label`].
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum Label {
    /// The value's type has nothing to label, or its definition names no label source.
    #[default]
    None,
    /// An enum value (type 8) labelled by an enum mapper.
    Enum {
        /// The mapper named by the definition.
        mapper: Did,
        /// The key's label.
        label: EnumLabel,
    },
    /// An enum value (type 8) that is a key of a DID map (§7.1).
    DidMapKey {
        /// The DID map named by the definition.
        map: Did,
        /// The key.
        key: u32,
        /// The map's label for the key, if present.
        label: Option<Arc<str>>,
        /// The map's target DID for the key, if present.
        target: Option<Did>,
    },
    /// A bitfield (types 14, 18, 22): one label per set bit, for at most
    /// [`crate::game_data::MAX_LABELLED_BITS`] bits.
    Bits {
        /// The mapper named by the definition.
        mapper: Did,
        /// Labels of the lowest set bits, ascending.
        bits: Vec<BitLabel>,
        /// Number of further set bits that were not labelled (0 when all were).
        omitted: u64,
    },
    /// A property-ID value (type 10), named by the registry (§7.4).
    Property {
        /// The PID value (0 = none).
        pid: u32,
        /// Its name; always `None` for PID 0, which means "none" here although the
        /// dictionary names it "invalid" (§2.5 MP3).
        name: Option<Arc<str>>,
    },
    /// A DID reference (type 20) with the DB-object type its definition declares (§7.6).
    DidRef {
        /// The referenced DID (null = none).
        did: Did,
        /// The declared DB-object-type code.
        dbo_type: Option<u8>,
        /// The code's label from the "DBOType" enum mapper.
        dbo_name: Option<Arc<str>>,
    },
}

impl Label {
    /// Whether everything that could be labelled was: `None` for [`Label::None`]
    /// (nothing to resolve), otherwise whether the lookup found a label. A null DID or
    /// PID 0 needs no label; any other DID reference counts as resolved when its declared
    /// type has a name (or no type is declared). Bits left out of a long bitfield
    /// (`omitted`) count as unresolved.
    #[must_use]
    pub fn is_resolved(&self) -> Option<bool> {
        Some(match self {
            Self::None => return None,
            Self::Enum { label, .. } => label.is_resolved(),
            Self::DidMapKey { label, .. } => label.is_some(),
            Self::Bits { bits, omitted, .. } => {
                *omitted == 0 && bits.iter().all(|b| b.label.is_resolved())
            }
            Self::Property { pid, name } => *pid == 0 || name.is_some(),
            Self::DidRef {
                did,
                dbo_type,
                dbo_name,
            } => did.is_null() || dbo_type.is_none() || dbo_name.is_some(),
        })
    }
}

impl fmt::Display for Label {
    /// A short text form: symbolic names where known, `#key` otherwise. Display texts
    /// (StringInfo) are not resolved here.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Enum { label, .. } => match &label.name {
                Some(n) => f.write_str(n),
                None => write!(f, "#{}", label.key),
            },
            Self::DidMapKey {
                key, label, target, ..
            } => {
                match label {
                    Some(l) => f.write_str(l)?,
                    None => write!(f, "#{key}")?,
                }
                if let Some(t) = target {
                    write!(f, " -> {t}")?;
                }
                Ok(())
            }
            Self::Bits { bits, omitted, .. } => {
                for (i, b) in bits.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" | ")?;
                    }
                    match &b.label.name {
                        Some(n) => f.write_str(n)?,
                        None => write!(f, "bit{}", b.bit)?,
                    }
                }
                if *omitted > 0 {
                    if !bits.is_empty() {
                        f.write_str(" | ")?;
                    }
                    write!(f, "+{omitted} more")?;
                }
                Ok(())
            }
            Self::Property { pid, name } => match name {
                Some(n) => f.write_str(n),
                None if *pid == 0 => f.write_str("none"),
                None => write!(f, "{pid:#x}"),
            },
            Self::DidRef { did, dbo_name, .. } => {
                write!(f, "{did}")?;
                if let Some(n) = dbo_name {
                    write!(f, " ({n})")?;
                }
                Ok(())
            }
        }
    }
}

/// One property of a resolved tree (see [`crate::GameData::resolve`]): the raw value
/// together with its name, declared type and label.
///
/// A tree from `resolve` borrows the values of the resolved [`crate::PropertySet`] and
/// nothing else; [`ResolvedProperty::into_owned`] detaches it, so it can be kept next
/// to (or without) the set or sent to another thread.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedProperty<'a> {
    /// Property ID.
    pub pid: u32,
    /// Registry name, if any (shared with the registry).
    pub name: Option<Arc<str>>,
    /// Declared type from the registry (`None` for an undefined PID).
    pub declared: Option<PropertyType>,
    /// The value. Borrowed from the set in a tree from `resolve`; see
    /// [`ResolvedProperty::into_owned`] for the owned form.
    pub value: Cow<'a, Value>,
    /// Its label.
    pub label: Label,
    /// Resolved struct fields or array elements, in stored order. Empty for scalars, and
    /// for containers below [`crate::game_data::RESOLVE_MAX_DEPTH`].
    pub children: Vec<ResolvedProperty<'a>>,
}

impl<'a> ResolvedProperty<'a> {
    /// Visits this property and all its descendants depth-first in stored order, with
    /// their depth below this property (0 for this property), like
    /// [`crate::PropertySet::walk`].
    pub fn walk<'s>(&'s self, mut f: impl FnMut(usize, &'s Self)) {
        let mut stack = vec![(0usize, self)];
        while let Some((depth, p)) = stack.pop() {
            f(depth, p);
            stack.extend(p.children.iter().rev().map(|c| (depth + 1, c)));
        }
    }

    /// Detaches the tree from the property set it was resolved from.
    ///
    /// To avoid copying every subtree once per ancestor, a struct or array whose
    /// children were resolved keeps an empty container of the same type as its `value`;
    /// its contents are in `children`. Scalars, and containers whose children were not
    /// resolved (below [`crate::game_data::RESOLVE_MAX_DEPTH`]), keep a full copy.
    #[must_use]
    pub fn into_owned(self) -> ResolvedProperty<'static> {
        let value = if self.children.is_empty() {
            Cow::Owned(self.value.into_owned())
        } else {
            Cow::Owned(match &*self.value {
                Value::Struct(_) => Value::from(crate::PropertySet::new()),
                Value::Array(_) => Value::Array(Vec::new()),
                other => other.clone(),
            })
        };
        ResolvedProperty {
            pid: self.pid,
            name: self.name,
            declared: self.declared,
            value,
            label: self.label,
            children: self
                .children
                .into_iter()
                .map(ResolvedProperty::into_owned)
                .collect(),
        }
    }
}
