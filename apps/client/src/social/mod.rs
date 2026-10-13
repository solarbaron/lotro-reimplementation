// SPDX-License-Identifier: GPL-3.0-or-later
//! The social panels and social chat in the client (decision 0024 P4): the friend and ignore
//! lists, the fellowship display with shared vitals, the fellowship invitation, the trade
//! window, tells, user chat channels and the ignore filter, fed by the world session
//! (`social-panels.md`, `chat-window.md` §15, `scenario-feed.md` §9.2 and "P4 social").
//!
//! | Module | What | Spec |
//! |---|---|---|
//! | [`state`] | [`SocialState`]: the session-fed models (chat targets and slots, fellowship, lists, invitation, trade) and the chat lines in the original's forms | `social-panels.md` §8–§9; `chat-window.md` §15.10 |
//! | [`texts`] | [`InstallText`]: lines and labels from the install's string tables, session values inserted literally | `chat-window.md` §15.10 CT12 |
//! | [`ui`] | [`SocialUi`]: the Social panel, the fellowship panels, the invitation and trade windows on the game UI; user actions as session commands | `social-panels.md` §3–§7 |
//! | [`party`] | [`party::PartyBridge`]: the plugins' `Party` views fed from the fellowship | `lua-plugin-api.md` §15.4; `social-panels.md` §10 |
//!
//! The command dispatcher's P4 built-ins (`tell`, `reply`, `join`, `friend`, ...) live in
//! [`crate::commands`]; the synchronised `/play` in [`crate::audio::player_music`].
//! Tessera's own server runs no gameplay rules (LEGAL.md §11.1 rule 5): on the test server
//! only tells and named channels work, and the panels stay empty (TR13).

pub mod party;
pub mod state;
pub mod texts;
pub mod ui;

use std::sync::Arc;

pub use state::{ItemResolver, ResolvedEntry, SocialState};
pub use texts::InstallText;
pub use ui::SocialUi;

use tessera_net::domain::Ref;
use tessera_ui::game::social::trade::CoinValues;

use crate::character::CharacterData;

/// The coin values of the install (`economy.md` §7: largest first, gold, silver, copper).
#[must_use]
pub fn coin_values(data: &CharacterData) -> Option<CoinValues> {
    data.coin_values().map(|[gold, silver, copper]| CoinValues {
        gold,
        silver,
        copper,
    })
}

/// The trade entries' resolver over the install's item facts (`social-panels.md` §6.2, TR9):
/// a record's rendered name, composed icon and `Item_Quality` key; a literal ref's text
/// without icon (TR9: no item image).
#[must_use]
pub fn item_resolver(data: Arc<CharacterData>) -> Arc<ItemResolver> {
    Arc::new(move |what: &Ref, _item: Option<u64>| match what {
        Ref::Literal(t) => ResolvedEntry {
            name: t.clone(),
            ..ResolvedEntry::default()
        },
        Ref::Record(did) => match data.item(tessera_formats::Did(did.0)) {
            Some(f) => ResolvedEntry {
                name: f.info.name.clone(),
                icon: f.icon.clone(),
                quality: Some(f.info.quality).filter(|q| *q != 0),
            },
            None => ResolvedEntry::default(),
        },
    })
}

/// Hands the install's facts to a social state: the string tables (CT12), the trade
/// entries' names and icons, and the coin values.
pub fn attach_data(state: &mut SocialState, data: &Arc<CharacterData>) {
    state.set_text(Some(InstallText::new(Arc::clone(data.strings()))));
    state.set_items(Some(item_resolver(Arc::clone(data))));
    if let Some(v) = coin_values(data) {
        state.set_coin_values(v);
    }
}

/// Sends the player music's requests to the world session (`scenario-feed.md` §6.7); a
/// `StartPerformance` the session refuses (the test server) gets the local start
/// (`instruments.md` §12 P4 Decision 3), once per refused request.
pub fn send_music_requests(
    world: &mut crate::net::World,
    music: &mut crate::audio::player_music::PlayerMusic,
    now: std::time::Instant,
) {
    use crate::audio::player_music::MusicRequest;
    for r in music.take_requests() {
        if let Err(e) = world.send_command(now, r.command()) {
            tracing::info!("player music request not sent: {e}");
            if r == MusicRequest::Start {
                music.start_refused();
            }
        }
    }
}
