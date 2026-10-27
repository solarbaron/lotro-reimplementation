// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's built-in string table for official play (`docs/specs/official-entry.md` §5.2).
//!
//! The table is compiled into the binary: it is never read from the install, from a file next
//! to the binary, from a UI skin, or from any other file a user or program could replace. It
//! holds the warning's entries (the five that enter the digest of §5.3 and the four that do
//! not) and Tessera's other English texts for official play: the start screen (§4.1), the
//! not-available message (§4.4), the account form (§7.1), the login screens (§8.3, §8.4) and
//! the error rows (§8.6). Placeholders are written `{name}` and filled by [`fill`].

/// The language tag of the shipped entry set (§5.3 item 2).
pub const LANGUAGE: &str = "en";

/// The warning revision of the shipped text (§5.2). Raised with every intended change of the
/// five digest texts, in the same change as the pinned digest of the test OE-T4.
pub const WARNING_REVISION: u32 = 1;

/// The paragraph break of the warning body: two line feeds (§5.2).
pub const PARAGRAPH_BREAK: &str = "\n\n";

/// Every entry: (string ID, English text).
pub const TABLE: &[(&str, &str)] = &[
    // §5.2: the ban-risk warning.
    (
        "official.consent.title",
        "Before you play on the official servers",
    ),
    (
        "official.consent.body",
        "Tessera is an independent, third-party program. It is not made, approved or supported \
         by the operator of the official game service, and it is not the official game client.\n\
         \n\
         Using a third-party client may break the official service's terms of use. The operator \
         may notice that you are not using its own client and may suspend or permanently close \
         your account because of it, with every character, item and purchase on it. Nobody who \
         works on Tessera can prevent that, undo it or restore an account.\n\
         \n\
         Tessera may also behave differently from the official client in ways you cannot see, \
         and a fault in Tessera could make the service treat your account as misbehaving.\n\
         \n\
         Continue only if you accept this risk for the account you are going to use. If you are \
         not sure, choose Decline: you can still use Tessera offline.\n\
         \n\
         Tessera sends your account name and password only to the official service's login and \
         never stores them. It shows this warning again whenever its text changes.",
    ),
    (
        "official.consent.checkbox",
        "I have read this warning and I accept the risk to my account.",
    ),
    (
        "official.consent.accept",
        "Continue to the official servers",
    ),
    ("official.consent.decline", "Decline"),
    (
        "official.consent.changed",
        "This warning has changed since you last accepted it. Please read it again.",
    ),
    (
        "official.consent.saved_failed",
        "Your acceptance could not be saved. It counts for this run only, and the warning will \
         be shown again next time.",
    ),
    ("official.consent.withdraw", "Withdraw acceptance"),
    (
        "official.consent.not_stored",
        "On this system Tessera does not save your acceptance. It counts for this run only, \
         and the warning is shown every time you start Tessera.",
    ),
    // §5.5 "Failed or partial withdrawal": Withdraw could not delete a file of the record (the
    // log names it); shown on the start screen in place of the acceptance line (§4.1).
    (
        "official.consent.withdraw_failed",
        "Your acceptance could not be withdrawn completely: Tessera could not delete a file of \
         it. The log names the file.",
    ),
    (
        "official.consent.withdrawn",
        "Your acceptance was withdrawn. The warning will be shown again before you play on the \
         official servers.",
    ),
    // §4.1: the start screen.
    ("official.start.offline", "Explore offline"),
    ("official.start.official", "Official servers"),
    (
        "official.start.accepted",
        "You accepted the official-server warning on {date}.",
    ),
    ("official.start.quit", "Quit"),
    // §9 step 2.
    ("official.checking", "Checking…"),
    // §4.4, §8.6 `NotAvailable`.
    (
        "official.not_available",
        "Playing on the official servers is not available in this version of Tessera yet.",
    ),
    ("official.ok", "OK"),
    // §7.1: the account form.
    ("official.form.account", "Account name"),
    ("official.form.password", "Password"),
    ("official.form.login", "Log in"),
    ("official.form.back", "Back"),
    (
        "official.form.notice",
        "Tessera sends your account name and password only to the official service's login and \
         never stores them.",
    ),
    // §8.3: progress and the world list.
    ("official.login.authenticating", "Logging in…"),
    ("official.login.listing", "Getting the list of worlds…"),
    ("official.login.joining", "Joining {world}…"),
    ("official.login.cancel", "Cancel"),
    ("official.login.join", "Join"),
    ("official.login.back", "Back"),
    ("official.worlds.name", "Name"),
    ("official.worlds.status", "Status"),
    ("official.worlds.load", "Load"),
    ("official.worlds.note", "Note"),
    ("official.worlds.status.open", "Open"),
    ("official.worlds.status.busy", "Busy"),
    ("official.worlds.status.full", "Full (queue)"),
    ("official.worlds.status.closed", "Closed"),
    (
        "official.worlds.status.restricted",
        "Not available to this account",
    ),
    ("official.worlds.status.unknown", "Unknown"),
    ("official.worlds.load.low", "Low"),
    ("official.worlds.load.medium", "Medium"),
    ("official.worlds.load.high", "High"),
    // §8.4: the queue.
    ("official.queue.waiting", "Waiting for a place on {world}."),
    (
        "official.queue.position",
        "You are number {position} in the queue.",
    ),
    (
        "official.queue.position_of",
        "You are number {position} in the queue of {length}.",
    ),
    (
        "official.queue.wait",
        "Estimated wait: about {minutes} min.",
    ),
    (
        "official.queue.wait_short",
        "Estimated wait: less than a minute.",
    ),
    (
        "official.queue.unknown",
        "Your place in the queue is not known.",
    ),
    ("official.queue.leave", "Leave queue"),
    // §8.6: refusals and failures.
    (
        "official.error.bad_credentials",
        "The account name or password was not accepted.",
    ),
    (
        "official.error.account_not_allowed",
        "The official service did not let this account log in.",
    ),
    (
        "official.error.unsupported_step",
        "This login needs a step that Tessera does not support. Tessera cannot log in to this \
         account.",
    ),
    (
        "official.error.client_not_accepted",
        "The official service did not accept this client.",
    ),
    (
        "official.error.rate_limited",
        "Too many attempts. Please wait {seconds} s.",
    ),
    (
        "official.error.service_unavailable",
        "The official login service is not available right now.",
    ),
    ("official.error.other", "The login failed."),
    ("official.error.world_full", "{world} is full."),
    ("official.error.world_closed", "{world} is closed."),
    (
        "official.error.world_not_allowed",
        "This account cannot join {world}.",
    ),
    ("official.error.world_other", "{world} could not be joined."),
    (
        "official.error.no_answer",
        "No answer from the official service.",
    ),
    (
        "official.error.transport",
        "The connection to the official service failed.",
    ),
    (
        "official.error.server_closed",
        "The official service closed the connection.",
    ),
    ("official.error.detail", "The service said: {text}"),
    // §8.7: the official character placeholder of the spec. No screen of this build shows
    // it: on `Connected` the session goes to the character screens, and the interim screen
    // shows `official.characters.loading` until the window takes it.
    (
        "official.characters.unavailable",
        "Choosing a character on the official servers is not available in this version of \
         Tessera yet.",
    ),
    // §8.7: the interim screen while the window takes the session for the character screens.
    ("official.characters.loading", "Loading characters…"),
];

/// The text of `id`; the empty string for an ID the table does not hold (a test checks that
/// every ID the client uses is present).
#[must_use]
pub fn text(id: &str) -> &'static str {
    TABLE
        .iter()
        .find_map(|(k, v)| (*k == id).then_some(*v))
        .unwrap_or("")
}

/// Whether the table holds `id`.
#[must_use]
pub fn has(id: &str) -> bool {
    TABLE.iter().any(|(k, _)| *k == id)
}

/// `template` with every `{name}` of `values` replaced by its value. Values are inserted as
/// they are (the caller sanitises service text first); placeholders of no value stay.
#[must_use]
pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_owned();
    for (name, value) in values {
        let key = format!("{{{name}}}");
        if out.contains(&key) {
            out = out.replace(&key, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = TABLE.iter().map(|(k, _)| *k).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len());
    }

    #[test]
    fn body_paragraphs_are_two_line_feeds_without_other_whitespace() {
        let body = text("official.consent.body");
        assert_eq!(body.split(PARAGRAPH_BREAK).count(), 5);
        assert!(!body.contains("\n\n\n"));
        assert!(!body.contains(" \n") && !body.contains("\n "));
        assert_eq!(body.trim(), body);
        for p in body.split(PARAGRAPH_BREAK) {
            assert!(!p.contains('\n'), "single line feed in {p:?}");
            assert!(!p.contains("  "), "double space in {p:?}");
        }
    }

    #[test]
    fn fill_replaces_placeholders() {
        assert_eq!(
            fill(text("official.login.joining"), &[("world", "Alpha")]),
            "Joining Alpha…"
        );
        assert_eq!(fill("{a}{b}", &[("a", "1")]), "1{b}");
    }
}
