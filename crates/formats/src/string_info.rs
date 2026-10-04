// SPDX-License-Identifier: GPL-3.0-or-later
//! StringInfo: a localizable text value (spec §4).

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::reader::{Reader, Utf16String};

/// Maximum nesting depth of StringInfo arguments (spec §4.2 suggests 16).
pub const STRING_INFO_MAX_DEPTH: usize = 16;

/// Where the text of a [`StringInfo`] comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringSource {
    /// Inline text.
    Literal(Utf16String),
    /// Reference to an entry of a string table.
    Table {
        /// Entry token.
        token: u32,
        /// String-table DID (0x25…).
        table: Did,
    },
}

/// Value of one StringInfo argument (spec §4.2).
#[derive(Debug, Clone, PartialEq)]
pub enum StringArgValue {
    /// Kind 1: nested StringInfo.
    Nested(Box<StringInfo>),
    /// Kind 2: float.
    Float(f32),
    /// Kind 4: integer (stored as a VLE).
    Integer(u32),
}

/// One argument of a StringInfo: the variable it fills and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct StringArg {
    /// Name hash of the variable this value fills.
    pub var_id: u32,
    /// The value.
    pub value: StringArgValue,
}

/// Argument block of a StringInfo (present when `has_args` = 1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StringArgs {
    /// Three auxiliary strings of unknown meaning (spec §4.1, OPEN).
    pub labels: [String; 3],
    /// Argument values.
    pub values: Vec<StringArg>,
}

/// A localizable text value (spec §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct StringInfo {
    /// Literal text or string-table reference.
    pub source: StringSource,
    /// Argument block, if present. Boxed because most values have none, which keeps a
    /// StringInfo small.
    pub args: Option<Box<StringArgs>>,
}

impl StringInfo {
    /// Decodes a StringInfo at the reader's position.
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        Self::read_depth(r, 0)
    }

    fn read_depth(r: &mut Reader<'_>, depth: usize) -> Result<Self> {
        if depth >= STRING_INFO_MAX_DEPTH {
            return Err(FormatError::DepthLimit {
                offset: r.position(),
                limit: STRING_INFO_MAX_DEPTH,
            });
        }
        let source = if r.bool8()? {
            StringSource::Literal(r.utf16_string()?)
        } else {
            let token = r.u32()?;
            let table = r.did()?;
            StringSource::Table { token, table }
        };
        let args = if r.bool8()? {
            let labels = [r.pascal_string()?, r.pascal_string()?, r.pascal_string()?];
            let count = r.vle()?;
            // Smallest argument is kind + var_id + marker + 1-byte VLE = 7 bytes.
            let cap = r.ensure_elements(count, 7)?;
            let mut values = Vec::with_capacity(cap);
            for _ in 0..count {
                values.push(read_arg(r, depth)?);
            }
            Some(Box::new(StringArgs { labels, values }))
        } else {
            let offset = r.position();
            let term = r.array::<2>()?;
            if term != [0x01, 0x00] {
                return Err(FormatError::malformed(
                    "StringInfo",
                    offset,
                    format!("terminator {:02X} {:02X}, expected 01 00", term[0], term[1]),
                ));
            }
            None
        };
        Ok(Self { source, args })
    }

    /// Decodes a buffer that holds exactly one StringInfo.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let v = Self::read(&mut r)?;
        r.expect_end("StringInfo")?;
        Ok(v)
    }
}

fn read_arg(r: &mut Reader<'_>, depth: usize) -> Result<StringArg> {
    let offset = r.position();
    let kind = r.u8()?;
    let var_id = r.u32()?;
    if kind == 1 {
        let nested = StringInfo::read_depth(r, depth + 1)?;
        return Ok(StringArg {
            var_id,
            value: StringArgValue::Nested(Box::new(nested)),
        });
    }
    if kind != 2 && kind != 4 {
        return Err(FormatError::malformed(
            "StringInfo argument",
            offset,
            format!("argument kind {kind}, expected 1, 2 or 4"),
        ));
    }
    let marker_at = r.position();
    let marker = r.u8()?;
    if marker != 1 {
        return Err(FormatError::malformed(
            "StringInfo argument",
            marker_at,
            format!("marker {marker}, expected 1"),
        ));
    }
    let value = if kind == 2 {
        StringArgValue::Float(r.f32()?)
    } else {
        StringArgValue::Integer(r.vle()?)
    };
    Ok(StringArg { var_id, value })
}
