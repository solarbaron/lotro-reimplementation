// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own texts of the character screens (`character-screens.md` §2.3, §3, §5.3,
//! §7.1, §7.2: "Tessera's own text"), and the lines of `CharGen_ErrorTextList`.
//!
//! These are project-made English texts, never text from the install.

use tessera_net::domain::Text;

/// A Tessera text the screens show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OwnText {
    /// Continue was clicked while the race, gender and class choice is not valid (§2.3).
    ChoiceIncomplete,
    /// Create was clicked while the choice (with nationality) is not valid (§7.1).
    ChoiceInvalid,
    /// The typed name fails the name check (§5.3); the detail says why.
    NameInvalid(String),
    /// The random-name generator gave no name (`player-character.md` §8.5).
    NoRandomName,
    /// A generated name did not pass the name check in the generator's attempts (§8.5).
    RandomNameUnchecked,
    /// The session refused `CreateCharacter` itself (`NetError::InvalidCommand`, §7.1).
    CreateRefused(String),
    /// No answer to `CreateCharacter` within the time-out (§7.1).
    CreateTimedOut,
    /// The session refused `DeleteCharacter` (§7.2).
    DeleteRefused,
    /// The session refused `EnterWorld` (§7.3).
    EnterRefused(String),
    /// The session refused `EnterWorld` while a character creation is still waiting for
    /// the server's answer (§7.3, §7.1.1 step 6).
    CreationStillOutstanding,
    /// The session ended before world entry.
    Disconnected(String),
    /// The ghosted New button's tooltip (§7.1).
    CreationNotAllowed,
    /// The ghosted Delete button's tooltip (§7.2).
    DeletionNotSupported,
    /// The confirmation dialog's question before a deletion (§7.2).
    ConfirmDelete(String),
    /// The character list is full (`scenario-feed.md` §5.1: 64 names).
    ListFull,
}

impl std::fmt::Display for OwnText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChoiceIncomplete => f.write_str("Choose a race, a gender and a class first."),
            Self::ChoiceInvalid => f.write_str(
                "This combination of race, gender, nationality and class is not offered.",
            ),
            Self::NameInvalid(why) => write!(f, "That name cannot be used: {why}."),
            Self::NoRandomName => f.write_str("No name could be generated; type one instead."),
            Self::RandomNameUnchecked => f.write_str(
                "The generated name did not pass the name check; change it before creating.",
            ),
            Self::CreateRefused(why) => write!(f, "The character could not be created ({why})."),
            Self::CreateTimedOut => {
                f.write_str("No answer to the request to create the character; try again.")
            }
            Self::DeleteRefused => f.write_str("This server cannot delete characters."),
            Self::EnterRefused(why) => write!(f, "Could not enter the world ({why})."),
            Self::CreationStillOutstanding => f.write_str(
                "A character creation is still waiting for the server's answer; try again \
                 once it has arrived.",
            ),
            Self::Disconnected(why) => write!(f, "The connection ended ({why})."),
            Self::CreationNotAllowed => {
                f.write_str("This server does not allow creating characters.")
            }
            Self::DeletionNotSupported => {
                f.write_str("This server does not support deleting characters.")
            }
            Self::ConfirmDelete(name) => write!(
                f,
                "Delete the character {name}? Click Delete again to confirm."
            ),
            Self::ListFull => f.write_str("The character list is full."),
        }
    }
}

/// One line of `CharGen_ErrorTextList`.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorLine {
    /// The session's reason of a `CharacterRefused` (`character-screens.md` §8: shown as
    /// `Text`, `item-tooltips.md` §11 for the text kinds).
    Reason(Text),
    /// A Tessera text.
    Own(OwnText),
}

impl ErrorLine {
    /// The line as plain text: a literal reason or a Tessera text as is; another reason
    /// kind through `resolve` (the install's texts), else a neutral placeholder.
    #[must_use]
    pub fn text(&self, resolve: &dyn Fn(&Text) -> Option<String>) -> String {
        match self {
            Self::Own(t) => t.to_string(),
            Self::Reason(Text::Literal(s)) => s.clone(),
            Self::Reason(t) => resolve(t).unwrap_or_else(|| "The request was refused.".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_render() {
        assert!(
            ErrorLine::Own(OwnText::ChoiceIncomplete)
                .text(&|_| None)
                .starts_with("Choose")
        );
        assert_eq!(
            ErrorLine::Reason(Text::Literal("taken".into())).text(&|_| None),
            "taken"
        );
        let other = ErrorLine::Reason(Text::Name(tessera_net::DataId(1)));
        assert_eq!(other.text(&|_| Some("x".into())), "x");
        assert!(!other.text(&|_| None).is_empty());
        assert!(
            OwnText::ConfirmDelete("Cora".into())
                .to_string()
                .contains("Cora")
        );
    }
}
