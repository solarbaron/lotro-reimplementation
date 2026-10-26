// SPDX-License-Identifier: GPL-3.0-or-later
//! Official play: the entry points, the ban-risk consent, the account form and the login
//! screens (`docs/specs/official-entry.md`, decision 0034 §1 item 4).
//!
//! - [`flow`]: the §9 state machine from the start screen to the character screens, with
//!   the record directory, the backends and the plugin host injected.
//! - [`input`]: device input, made only by the window-event handler (§5.4 rule 1).
//! - [`warning`]: the warning screen and its input rules (§5.4–§5.5), with the Continue
//!   handler that alone creates the user-confirmation value (§6.3).
//! - [`form`]: the account form with its secret buffers (§7).
//! - [`login`]: the world list, queue, progress and error presentation (§8.3–§8.6).
//! - [`backend`]: the one construction site of the compatibility backend (§6.2), over the
//!   live connector.
//! - [`install_data`]: the install's records for the live connector.
//! - [`plugin_host`]: the plugin host of a start-screen run (§4.5).
//! - [`view`]: the screens' drawing with the bundled font.
//! - [`window`]: the window and its event handler of a start-screen run (§4).
//!
//! The gate (§6): without a consent proof no compatibility backend can be built, and
//! without one no network operation of `tessera-compat` exists. The proof comes only from a
//! record that passed every check or from an acceptance on the warning screen in this run.
//!
//! Rules of revision 3 that shape the code:
//!
//! - The screens are painted by Tessera's own code with the bundled font, not built from
//!   `tessera-ui` elements (§4.6, [`view`]).
//! - §8.7: on `Connected` the session is handed over to the character screens of
//!   `--character-select` (`crate::pre_world`) in a client over that session
//!   (`crate::app::App::with_session`, [`window`]), whose runtime starts the plugin host
//!   (§4.5). Creation and deletion are ghosted there with their reason (their creation
//!   properties need install enum mappers no spec assigns yet). Leaving the screens, closing
//!   the window or the session's end shuts that client down and reopens the empty account
//!   form. The §8.7 screen with `official.characters.unavailable` and Back is left only as
//!   the fallback while the window has not taken the session (or could not open the
//!   screens).
//! - There is no way back from the offline walker to the start screen: quitting it ends the
//!   client (§4.1, §4.5).
//! - On Windows no acceptance record is stored (§5.6 "Windows"): an acceptance lasts for the
//!   run and `official.consent.not_stored` is shown.

pub mod backend;
pub mod flow;
pub mod form;
pub mod input;
pub mod install_data;
pub mod login;
pub mod plugin_host;
pub mod view;
pub mod warning;
pub mod window;

#[cfg(test)]
mod tests;
