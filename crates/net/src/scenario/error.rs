// SPDX-License-Identifier: GPL-3.0-or-later
//! Error types of the scenario feed (`docs/specs/scenario-feed.md` §11).

use std::fmt;

/// Which limit of `scenario-feed.md` §11.1 a file exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Limit {
    /// More than 65,536 lines.
    Lines,
    /// More than 64 tokens on one line.
    Tokens,
    /// More than 256 selector bindings.
    Selectors,
    /// More than 1,024 actors.
    Actors,
    /// More than 4,096 items.
    Items,
    /// More than 1,024 pages.
    Pages,
    /// More than 1,024 marker bindings.
    Markers,
    /// More than 16 options on a page.
    Options,
    /// More than 20,000 step lines, or more than 16 in a pre-world trigger body.
    Steps,
    /// More than 256 triggers.
    Triggers,
    /// More than 10,000 input lines.
    Input,
    /// A repeat count above 10,000, or more than 200,000 expanded step lines.
    Repeat,
    /// `n=` above 65,536.
    Selector,
    /// More than 256 DIDs in a `did` list.
    Did,
    /// Too many window, fellowship, friend, ignore or trade entries.
    Entries,
    /// More than 64 characters in the character list.
    Characters,
    /// More than 64 look bindings (`scenario-feed.md` §5.8).
    Looks,
    /// More than 48 property values in one look (§5.8).
    Look,
}

impl Limit {
    /// The limit's name as the spec writes it (`lines`, `tokens`, ...).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::Tokens => "tokens",
            Self::Selectors => "selectors",
            Self::Actors => "actors",
            Self::Items => "items",
            Self::Pages => "pages",
            Self::Markers => "markers",
            Self::Options => "options",
            Self::Steps => "steps",
            Self::Triggers => "triggers",
            Self::Input => "input",
            Self::Repeat => "repeat",
            Self::Selector => "selector",
            Self::Did => "did",
            Self::Entries => "entries",
            Self::Characters => "characters",
            Self::Looks => "looks",
            Self::Look => "look",
        }
    }
}

/// The kind of a static scenario error (`scenario-feed.md` §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScenarioErrorKind {
    /// Invalid UTF-8, an overlong or surrogate encoding, or a leading byte-order mark.
    Encoding,
    /// A control character other than TAB (or a CR not before LF).
    ControlCharacter,
    /// The file is larger than 1,048,576 bytes.
    FileTooLarge,
    /// A line is longer than 4,096 bytes.
    LineTooLong,
    /// The header line is missing or malformed.
    BadHeader,
    /// The header names another format version.
    UnsupportedVersion,
    /// An unknown first keyword of a statement.
    UnknownKeyword,
    /// A token that fits no production at its place.
    UnexpectedToken,
    /// An unknown attribute of a selector family.
    UnknownAttribute,
    /// An unknown selector family.
    UnknownFamily,
    /// A malformed number (or a numeric component longer than 32 characters).
    BadNumber,
    /// A malformed or negative duration.
    BadDuration,
    /// A malformed position.
    BadPosition,
    /// A malformed fraction.
    BadFraction,
    /// An invalid escape in a string.
    BadEscape,
    /// A string without its closing quote.
    UnterminatedString,
    /// A string, keyword, binding or label is too long.
    TooLong,
    /// A binding used before its declaration.
    UndefinedBinding,
    /// A binding (or character name, or the player) declared twice.
    DuplicateBinding,
    /// A binding of the wrong kind for its place.
    WrongBindingKind,
    /// `enter` names no character of the file.
    UnknownCharacter,
    /// A selector without a count.
    MissingCount,
    /// A missing required argument or option.
    MissingArgument,
    /// A setting given twice.
    DuplicateSetting,
    /// A setting after the first step line or block opener.
    SettingAfterStep,
    /// A block left open at end of file.
    UnclosedBlock,
    /// `end` without an open block.
    UnexpectedEnd,
    /// A block opener at depth 3 or more.
    NestingTooDeep,
    /// A statement where its context does not allow it.
    NotAllowedHere,
    /// A number outside the range of its position.
    OutOfRange,
    /// A limit of `scenario-feed.md` §11.1.
    LimitExceeded(Limit),
    /// A property value whose form does not fit its written kind, or that a look does not
    /// allow (§5.8).
    BadValue,
    /// A setting, declaration, step, trigger pattern or input line outside the overlay
    /// subset, in an overlay run (§12.5.1).
    NotAllowedInOverlay,
}

impl fmt::Display for ScenarioErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Encoding => "Encoding",
            Self::ControlCharacter => "ControlCharacter",
            Self::FileTooLarge => "FileTooLarge",
            Self::LineTooLong => "LineTooLong",
            Self::BadHeader => "BadHeader",
            Self::UnsupportedVersion => "UnsupportedVersion",
            Self::UnknownKeyword => "UnknownKeyword",
            Self::UnexpectedToken => "UnexpectedToken",
            Self::UnknownAttribute => "UnknownAttribute",
            Self::UnknownFamily => "UnknownFamily",
            Self::BadNumber => "BadNumber",
            Self::BadDuration => "BadDuration",
            Self::BadPosition => "BadPosition",
            Self::BadFraction => "BadFraction",
            Self::BadEscape => "BadEscape",
            Self::UnterminatedString => "UnterminatedString",
            Self::TooLong => "TooLong",
            Self::UndefinedBinding => "UndefinedBinding",
            Self::DuplicateBinding => "DuplicateBinding",
            Self::WrongBindingKind => "WrongBindingKind",
            Self::UnknownCharacter => "UnknownCharacter",
            Self::MissingCount => "MissingCount",
            Self::MissingArgument => "MissingArgument",
            Self::DuplicateSetting => "DuplicateSetting",
            Self::SettingAfterStep => "SettingAfterStep",
            Self::UnclosedBlock => "UnclosedBlock",
            Self::UnexpectedEnd => "UnexpectedEnd",
            Self::NestingTooDeep => "NestingTooDeep",
            Self::NotAllowedHere => "NotAllowedHere",
            Self::OutOfRange => "OutOfRange",
            Self::BadValue => "BadValue",
            Self::NotAllowedInOverlay => "NotAllowedInOverlay",
            Self::LimitExceeded(l) => return write!(f, "LimitExceeded({})", l.name()),
        };
        f.write_str(s)
    }
}

/// A static error of a scenario file: its kind and position (`scenario-feed.md` §11.2).
///
/// Lines and columns count from 1; the column counts Unicode scalar values. Displayed as
/// `LINE:COLUMN: KIND`; [`with_file`](Self::with_file) adds the file name in front.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{line}:{column}: {kind}")]
pub struct ScenarioError {
    /// What went wrong.
    pub kind: ScenarioErrorKind,
    /// The line, from 1.
    pub line: u32,
    /// The column in Unicode scalar values, from 1.
    pub column: u32,
}

impl ScenarioError {
    pub(crate) fn new(kind: ScenarioErrorKind, line: u32, column: u32) -> Self {
        Self { kind, line, column }
    }

    /// The error displayed as `FILE:LINE:COLUMN: KIND`.
    #[must_use]
    pub fn with_file<'a>(&'a self, file: &'a str) -> impl fmt::Display + 'a {
        struct WithFile<'a>(&'a ScenarioError, &'a str);
        impl fmt::Display for WithFile<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.1, self.0)
            }
        }
        WithFile(self, file)
    }
}

/// An error of selector resolution (`scenario-feed.md` §8.6, §11.2): the client exits 4.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ResolveError {
    /// The install as a whole could not be opened or read.
    #[error("{line}: InstallUnreadable: {reason}")]
    InstallUnreadable {
        /// The `select` line concerned.
        line: u32,
        /// What the resolver reported.
        reason: String,
    },
    /// The scenario says `requires install` and there is none.
    #[error("0: InstallRequired")]
    InstallRequired,
    /// The resolved records of all selectors exceed 1,048,576.
    #[error("{line}: ResolveLimit")]
    ResolveLimit {
        /// The `select` line that passed the limit.
        line: u32,
    },
}

impl ResolveError {
    /// The error's kind as the spec names it.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::InstallUnreadable { .. } => "InstallUnreadable",
            Self::InstallRequired => "InstallRequired",
            Self::ResolveLimit { .. } => "ResolveLimit",
        }
    }

    /// The scenario line concerned (0 for [`InstallRequired`](Self::InstallRequired)).
    #[must_use]
    pub fn line(&self) -> u32 {
        match self {
            Self::InstallUnreadable { line, .. } | Self::ResolveLimit { line } => *line,
            Self::InstallRequired => 0,
        }
    }
}

/// The kind of a run-time error of the feed (`scenario-feed.md` §11.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeErrorKind {
    /// The output queue would pass 1,048,576 events or 2,097,152 weight units.
    QueueOverflow,
    /// The run would emit more than 16,777,216 events.
    EventOverflow,
    /// The agenda would pass 262,144 entries.
    AgendaOverflow,
    /// More than 65,536 run-time items.
    ItemOverflow,
    /// An identifier counter would pass its type's maximum.
    IdOverflow,
}

impl fmt::Display for RuntimeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::QueueOverflow => "QueueOverflow",
            Self::EventOverflow => "EventOverflow",
            Self::AgendaOverflow => "AgendaOverflow",
            Self::ItemOverflow => "ItemOverflow",
            Self::IdOverflow => "IdOverflow",
        })
    }
}

/// A run-time error that stopped the feed: its kind and the scenario line of the step that
/// caused it (0 for a generic answer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind} at line {line}")]
pub struct ScenarioRuntimeError {
    /// What overflowed.
    pub kind: RuntimeErrorKind,
    /// The step's line, 0 when no scenario line caused it.
    pub line: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays() {
        let e = ScenarioError::new(ScenarioErrorKind::LimitExceeded(Limit::Selector), 2, 18);
        assert_eq!(e.to_string(), "2:18: LimitExceeded(selector)");
        assert_eq!(
            e.with_file("a.scenario").to_string(),
            "a.scenario:2:18: LimitExceeded(selector)"
        );
        let r = ScenarioRuntimeError {
            kind: RuntimeErrorKind::QueueOverflow,
            line: 7,
        };
        assert_eq!(r.to_string(), "QueueOverflow at line 7");
    }
}
