// SPDX-License-Identifier: GPL-3.0-or-later
//! String tables, message grammar and StringInfo resolution.
//!
//! This crate implements `docs/specs/string-table.md` on top of the string-table and
//! StringInfo byte layouts decoded by `tessera-formats` (`docs/specs/records.md` §3, §4).
//!
//! # Overview
//!
//! * [`Strings`] gives access to one locale: tables by DID with a bounded cache
//!   ([`TableCache`]), messages by (table DID, token), rendering, StringInfo resolution
//!   ([`Strings::resolve`]) and named tables ([`NamedTables`]).
//! * [`Message`] is a parsed entry: pieces, slots, slot labels (`#N:`), selectors
//!   (`#N:{…}` and bare `{…}`), the `{{keepspaces}}` directive. [`Message::issues`]
//!   reports grammar-invariant violations ([`GrammarIssue`]).
//! * [`render`] turns a message plus a [`Values`] map into [`RichText`] (styled
//!   [`Span`]s with colour, emphasis, list and inert link annotations). Rendering never
//!   fails; misses and limits are reported as [`Diagnostic`]s. [`RenderMode::Explorer`]
//!   shows `${NAME}` placeholders for missing values (selectors still see them as
//!   absent, also inside nested messages).
//! * The `tessera-formats` record types used in this API ([`StringInfo`], [`StringTable`],
//!   [`StringTableEntry`], [`DataIdMap`], ...) are re-exported.
//! * Grammar tags: values carry a trailing tag group (`Name[fn]`); selectors pick an
//!   option by the value's [`TagSet`] using [`select_option`] (spec §6).
//! * [`format_string`] / [`Message::from_format_string`] / [`render_template`]: the
//!   `${VAR}` presentation and substitution with a caller-provided variable map.
//! * [`select_language`] / [`open_locale`]: locale selection; the archive's subset word is
//!   reported as a [`LanguageId`].
//!
//! ```
//! use tessera_text::{Message, Values, render_plain};
//!
//! let msg = Message::from_format_string("#1:${PLAYER} lights #1:{her[f]|his[m]|a} lamp.");
//! let values = Values::new().with("PLAYER", "Wren Hollow[fn]");
//! assert_eq!(render_plain(&msg, &values), "Wren Hollow lights her lamp.");
//! ```

#![warn(missing_docs)]

mod error;
mod format;
mod locale;
mod markup;
mod message;
mod render;
mod source;
mod strings;
mod tags;
mod value;

pub use error::{Result, TextError};
pub use format::{Template, arg_id_for_name, format_string, parse_format_string, render_template};
pub use locale::{LanguageId, Locale, available_languages, open_locale, select_language};
pub use markup::{
    Link, MAX_LINK_FIELDS, MAX_MARKUP_DEPTH, Rgba, RichText, Span, Style, decode_escapes,
    parse_markup,
};
pub use message::{GrammarIssue, Label, Message, Part, Piece, Segment, Selector, Slot};
pub use render::{
    DEFAULT_MAX_DEPTH, DEFAULT_MAX_OUTPUT, Diagnostic, Miss, MissReason, RenderMode, RenderOptions,
    Rendered, display_text, render, render_plain,
};
pub use source::{
    CacheLimits, CacheStats, MemorySource, STRING_TABLE_DIDS, TableCache, TableSource,
    is_string_table_did,
};
pub use strings::{NamedTable, NamedTables, ROOT_DID_MAP, STRING_TABLE_MAP_LABEL, Strings};
pub use tags::{
    SelectorOption, TAG_BLANK, TAG_NON_EMPTY, Tag, TagAtom, TagSet, parse_options, parse_tag_list,
    select_option, split_tag_group,
};
pub use value::{MessageValue, NameDictionary, Value, Values, f32_to_f64_shortest, format_float};

/// Record types of `tessera-formats` that appear in this crate's public API, re-exported so
/// consumers do not need a direct dependency to name them.
pub use tessera_formats::{
    DataIdMap, Did, FormatError, StringArg, StringArgValue, StringArgs, StringInfo, StringSource,
    StringTable, StringTableEntry, Utf16String, name_hash,
};
