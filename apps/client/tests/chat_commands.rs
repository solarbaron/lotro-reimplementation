// SPDX-License-Identifier: GPL-3.0-or-later
//! Commands typed in the walker's chat line (decision 0020), headless and without a GPU:
//! the overlay's submitted lines go through the client's command dispatcher (the same
//! `walker::chat::dispatch` the walker calls every frame), never to the session unless they
//! are chat.
//!
//! - `/play` typed in the chat starts the player music. With `TESSERA_GAME_DIR` (the
//!   instruments come from the install) the performance is scheduled on the offline
//!   backend; without it the music reports clearly that there is no instrument data. In
//!   both cases nothing is sent to the session.
//! - An unknown command posts the unknown-command line and sends nothing; plain text is
//!   said on the Say channel.
//!
//! Prints counts only, never game text:
//!
//! ```text
//! export TESSERA_GAME_DIR="$(cargo run -q -p tessera-dattool -- detect --path)"
//! cargo test -p tessera-client --release --test chat_commands -- --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::audio::backend::BackendChoice;
use tessera_client::audio::player_music::{MusicConfig, PlayerMusic};
use tessera_client::net::PlayerLocation;
use tessera_client::plugins::Plugins;
use tessera_client::wait::{LoadWait, ProgressWait, wait_until};
use tessera_client::walker::WalkerSession;
use tessera_client::walker::chat::{ChatOverlay, dispatch};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_input::{KeyCode, RawEvent};
use tessera_net::ChatChannel;

/// An original test tune.
const TUNE: &str = "X:1\nT:Tessera Chat Air\nM:3/4\nL:1/8\nQ:1/4=100\nK:D\n\
                    D2 F2 A2|d4 A2|[D2F2A2] B2 c2-|c4 z2|\n";

/// A session double that records what it is asked to send.
#[derive(Debug, Default)]
struct Recorder {
    sent: Vec<(ChatChannel, String)>,
}

impl WalkerSession for Recorder {
    fn player_location(&self) -> Option<PlayerLocation> {
        None
    }
    fn send_move(&mut self, _: Instant, _: tessera_net::Location, _: f32, _: tessera_net::Vec3) {}
    fn send_chat(&mut self, _: Instant, channel: &ChatChannel, text: &str) -> bool {
        self.sent.push((channel.clone(), text.to_owned()));
        true
    }
}

fn key(key: KeyCode) -> RawEvent {
    RawEvent::Key {
        key,
        pressed: true,
        repeat: false,
    }
}

/// Types `line` into the overlay and presses Enter.
fn type_line(chat: &mut ChatOverlay, line: &str) {
    chat.handle(&key(KeyCode::Enter));
    for c in line.chars() {
        chat.handle(&RawEvent::Text(c));
    }
    chat.handle(&key(KeyCode::Enter));
}

/// One walker frame of the chat: dispatch, then the plugin host's per-frame work.
fn frame(chat: &mut ChatOverlay, plugins: &mut Plugins, session: &mut Recorder) {
    let mut s: Option<&mut dyn WalkerSession> = Some(session);
    dispatch(chat, plugins, &mut s, Instant::now());
    plugins.pump();
    dispatch(chat, plugins, &mut s, Instant::now());
}

fn shown(chat: &ChatOverlay) -> Vec<String> {
    chat.shown(Instant::now(), 400)
        .into_iter()
        .map(|(t, _)| t)
        .collect()
}

/// How long the player-music waits last: as long as start-up, jobs and queued commands
/// progress ([`PlayerMusic::wait_settled`]: polls about 2 ms apart; start-up and each
/// command job are single steps, so the stall limit covers the longest of them).
const MUSIC_WAIT: LoadWait = if cfg!(debug_assertions) {
    LoadWait::Progress(ProgressWait::new(900_000, 7_200_000))
} else {
    LoadWait::Progress(ProgressWait::new(150_000, 1_800_000))
};

/// How long the wait for the install's command words lasts (polls 20 ms apart; the words
/// are read in one step, so only the stall limit and the cap apply).
const WORDS_WAIT: LoadWait = if cfg!(debug_assertions) {
    LoadWait::Progress(ProgressWait::new(90_000, 720_000))
} else {
    LoadWait::Progress(ProgressWait::new(15_000, 180_000))
};

fn wait_music(plugins: &mut Plugins) -> bool {
    plugins
        .music_mut()
        .is_some_and(|m| m.wait_settled(MUSIC_WAIT))
}

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os(tessera_install::GAME_DIR_ENV) {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => None,
    }
}

#[test]
fn a_play_line_from_the_chat_starts_the_music_or_says_why_not() {
    let scratch = tempfile::tempdir().expect("scratch");
    let music_dir = scratch.path().join("Music");
    std::fs::create_dir_all(&music_dir).expect("music folder");
    std::fs::write(music_dir.join("tessera-chat.abc"), TUNE).expect("tune");
    let mut plugins = Plugins::start(
        &scratch.path().join("no-plugins"),
        scratch.path().join("data"),
        &[],
    );
    let dir = game_dir();
    plugins.set_music(PlayerMusic::new(MusicConfig {
        backend: BackendChoice::Offline,
        game_dir: dir.clone(),
        folders: vec![music_dir],
        ..MusicConfig::default()
    }));
    if let Some(dir) = &dir {
        let set = Arc::new(ArchiveSet::open(dir, SetOptions::default()).expect("open archives"));
        if let Some(m) = plugins.music_mut() {
            m.attach_data(Arc::clone(&set), dir);
        }
        plugins.attach_data(set);
    } else {
        plugins.data_unavailable();
    }
    let mut chat = ChatOverlay::default();
    let mut session = Recorder::default();

    match &dir {
        Some(_) => {
            // Start-up, then the debug instrument choice (no item model in the sandbox):
            // the first named type other than 0. Both typed in the chat.
            type_line(&mut chat, "/instrument");
            frame(&mut chat, &mut plugins, &mut session);
            assert!(wait_music(&mut plugins), "start-up finished");
            let instrument = plugins
                .music_mut()
                .and_then(|m| m.data())
                .and_then(|d| d.table.as_ref().ok())
                .and_then(|t| t.instruments().iter().copied().find(|&k| k != 0))
                .expect("an instrument type in the install");
            type_line(&mut chat, &format!("/instrument {instrument}"));
            type_line(&mut chat, "/PLAY tessera-chat");
            frame(&mut chat, &mut plugins, &mut session);
            assert!(wait_music(&mut plugins), "the song was read");
            frame(&mut chat, &mut plugins, &mut session);
            let m = plugins.music_mut().expect("music");
            let scheduled = m.schedule_log().last().map_or(0, |p| p.note_on_times.len());
            eprintln!(
                "real data: instrument {instrument}, {scheduled} note-ons scheduled, \
                 performing {}, {} chat rows",
                m.is_performing(),
                shown(&chat).len()
            );
            assert!(scheduled > 0, "the performance is scheduled");
            assert!(m.is_performing(), "the local player performs");
            assert!(m.stats().commands >= 3);
            // The install's command words (chat-window.md §10 items 3, 4; CW-I6, CW-I7):
            // counts only, no game text.
            let _ = wait_until(WORDS_WAIT, Duration::from_millis(20), || {
                if !plugins.dispatcher().text().modes.is_empty() {
                    return None;
                }
                plugins.pump();
                let t = plugins.dispatcher().text();
                Some((
                    t.modes.len(),
                    t.help_names.is_some(),
                    t.plugins_names.is_some(),
                    t.default_word.is_some(),
                    t.unknown_line.is_some(),
                ))
            });
            let d = plugins.dispatcher();
            let t = d.text();
            eprintln!(
                "real data: {} of {} channel rows read, help names {}, plugin names {}, \
                 default word {}, unknown line {}, prefix {} chars, {} names registered",
                t.modes.len(),
                tessera_client::commands::text::MODE_TABLE.len(),
                t.help_names.is_some(),
                t.plugins_names.is_some(),
                t.default_word.is_some(),
                t.unknown_line.is_some(),
                t.prefix.chars().count(),
                d.registry().len()
            );
            assert_eq!(
                t.modes.len(),
                tessera_client::commands::text::MODE_TABLE.len()
            );
            assert!(t.help_names.is_some() && t.plugins_names.is_some());
            assert!(t.default_word.is_some() && t.unknown_line.is_some());
            assert!(!t.prefix.is_empty());
        }
        None => {
            eprintln!("TESSERA_GAME_DIR not set: checking the report without game data");
            type_line(&mut chat, "/play tessera-chat");
            frame(&mut chat, &mut plugins, &mut session);
            assert!(wait_music(&mut plugins), "the song was read");
            frame(&mut chat, &mut plugins, &mut session);
            let rows = shown(&chat);
            assert!(
                rows.iter().any(|r| r.contains("no instrument data")),
                "the chat says why nothing plays: {rows:?}"
            );
            let m = plugins.music_mut().expect("music");
            assert!(!m.is_performing());
            assert_eq!(m.stats().songs_accepted, 1, "the song was read and checked");
        }
    }
    assert!(
        session.sent.is_empty(),
        "music commands never reach the session: {:?}",
        session.sent
    );

    // Unknown commands and a prefix-only line send nothing; plain text is said.
    type_line(&mut chat, "/nosuchcommand x");
    type_line(&mut chat, "/");
    frame(&mut chat, &mut plugins, &mut session);
    assert!(session.sent.is_empty(), "{:?}", session.sent);
    // The line is the install's when it has one, else Tessera's; not printed (game text).
    let unknown = plugins.dispatcher().unknown_line();
    if dir.is_none() {
        assert_eq!(unknown, "Unknown command. Type /help for a list.");
    }
    assert_eq!(
        shown(&chat).iter().filter(|r| **r == unknown).count(),
        1,
        "the unknown-command line is shown once (the prefix-only line shows nothing)"
    );
    type_line(&mut chat, "hello there");
    type_line(&mut chat, "/say /x");
    frame(&mut chat, &mut plugins, &mut session);
    assert_eq!(
        session.sent,
        [
            (ChatChannel::Say, "hello there".to_owned()),
            (ChatChannel::Say, "/x".to_owned())
        ]
    );
    plugins.shutdown();
}
