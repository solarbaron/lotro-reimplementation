// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared rig of the client's game-UI tests: a [`UiHost`] (with or without an install), the
//! client's plugin host over a synthetic `Plugins/` tree written into a temporary folder,
//! and the walker's input mapper, driven through the frame-loop steps 1, 7, 8 and the
//! frame clock exactly as the client drives them. Everything synthetic; no game data.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tessera_client::plugins::Plugins;
use tessera_client::ui_host::cursor::NoWindow;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_input::{
    InputMapper, KeyCode, MouseButton, RawEvent, StandardActions, default_keymap, default_registry,
};

/// The default screen of the rig.
pub const SCREEN: (u32, u32) = (1600, 900);

/// Writes a plugin `author/name.plugin` with `Main.lua` = `source` below `plugins`.
pub fn write_plugin(
    plugins: &Path,
    author: &str,
    name: &str,
    apartment: Option<&str>,
    source: &str,
) {
    let conf = apartment.map_or(String::new(), |a| {
        format!("<Configuration Apartment=\"{a}\"/>")
    });
    let dir = plugins.join(author);
    std::fs::create_dir_all(dir.join(name)).expect("create plugin folder");
    std::fs::write(
        dir.join(format!("{name}.plugin")),
        format!(
            "<?xml version=\"1.0\"?><Plugin><Information><Name>{name}</Name>\
             <Author>{author}</Author><Version>1.0</Version></Information>\
             <Package>{author}.{name}.Main</Package>{conf}</Plugin>"
        ),
    )
    .expect("write descriptor");
    std::fs::write(dir.join(name).join("Main.lua"), source).expect("write Main.lua");
}

/// The rig.
pub struct Rig {
    /// The host.
    pub host: UiHost,
    /// The client's plugin host.
    pub plugins: Plugins,
    /// The walker's input mapper.
    pub mapper: InputMapper,
    start: Instant,
    /// The temporary folder (Plugins/, plugin data).
    pub dir: tempfile::TempDir,
    /// A headless renderer; when set, step 8 applies its GPU operations to it.
    pub renderer: Option<tessera_render::Renderer<'static>>,
}

impl Rig {
    /// A rig without an install whose `Plugins/` folder holds what `setup` writes.
    pub fn new(setup: impl FnOnce(&Path)) -> Self {
        Self::with_host(setup, UiHost::without_install)
    }

    /// A rig with a host made by `make`.
    pub fn with_host(setup: impl FnOnce(&Path), make: impl FnOnce(&UiOptions) -> UiHost) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path().join("Plugins");
        std::fs::create_dir_all(&plugins_dir).expect("Plugins folder");
        setup(&plugins_dir);
        let mut plugins = Plugins::start(&plugins_dir, dir.path().join("data"), &[]);
        // No install: the command words are Tessera's own, and plugins may load
        // (`chat-window.md` §10 item 3).
        plugins.data_unavailable();
        let options = UiOptions {
            screen: SCREEN,
            record: true,
            ..UiOptions::default()
        };
        let host = make(&options);
        let (mut mapper, warnings) = InputMapper::new(default_registry(), &default_keymap());
        assert!(warnings.is_empty(), "{warnings:?}");
        // The walker's contexts in walk mode (camera, movement), as `Controls` sets them.
        StandardActions::new(mapper.registry())
            .expect("built-in actions")
            .enter_walk_mode(&mut mapper);
        let mut rig = Self {
            host,
            plugins,
            mapper,
            start: Instant::now(),
            dir,
            renderer: None,
        };
        rig.host.attach_mapper(&mut rig.mapper);
        rig
    }

    /// The Plugins folder.
    #[must_use]
    pub fn plugins_dir(&self) -> PathBuf {
        self.dir.path().join("Plugins")
    }

    /// Attaches the plugin host and queues `load`.
    pub fn attach(&mut self, load: &[&str]) {
        let load: Vec<String> = load.iter().map(|s| (*s).to_owned()).collect();
        self.host.attach_plugins(&mut self.plugins, &load);
    }

    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    /// One frame: pending offers, the snapshot, step 7, step 8 (no GPU), the clock.
    pub fn frame(&mut self) {
        let now = self.now();
        self.host
            .offer_pending(&mut self.mapper, &mut self.plugins, &mut NoWindow);
        self.mapper.update(now);
        self.host.step7(&mut self.plugins, Some(&mut self.mapper));
        // Lines typed in the chat window go to the command dispatcher (no session here).
        self.host
            .dispatch_entry_lines(&mut self.plugins, None, Instant::now());
        self.host.step8(self.renderer.as_mut(), &mut self.plugins);
        self.host.apply_band_changes(&mut self.mapper);
        self.host.end_frame(&self.plugins, 0.0);
    }

    /// Runs frames until `done` holds or `max` frames ran; returns whether it holds.
    pub fn frames_until(&mut self, max: usize, mut done: impl FnMut(&Self) -> bool) -> bool {
        for _ in 0..max {
            if done(self) {
                return true;
            }
            self.frame();
            std::thread::sleep(Duration::from_millis(1));
        }
        done(self)
    }

    /// Loads queued plugins: frames until `n` plugins are loaded.
    pub fn wait_loaded(&mut self, n: usize) {
        let ok = self.frames_until(5000, |r| r.plugins.manager().loaded_plugins().len() >= n);
        assert!(
            ok,
            "plugins loaded: {:?}; chat: {:?}",
            self.plugins.manager().loaded_plugins(),
            self.plugins.host().chat_lines()
        );
    }

    /// Feeds one raw event (step 1).
    pub fn raw(&mut self, ev: RawEvent) {
        let now = self.now();
        self.host
            .handle_raw(&mut self.mapper, &ev, now, &mut self.plugins, &mut NoWindow);
    }

    /// Moves the pointer.
    pub fn move_to(&mut self, x: f64, y: f64) {
        self.raw(RawEvent::CursorMoved { x, y });
    }

    /// A left click at (x, y), then a frame.
    pub fn click(&mut self, x: f64, y: f64) {
        self.move_to(x, y);
        self.raw(RawEvent::MouseButton {
            button: MouseButton::Left,
            pressed: true,
        });
        self.raw(RawEvent::MouseButton {
            button: MouseButton::Left,
            pressed: false,
        });
        self.frame();
    }

    /// Presses and releases a key, then a frame.
    pub fn key(&mut self, key: KeyCode) {
        self.raw(RawEvent::Key {
            key,
            pressed: true,
            repeat: false,
        });
        self.raw(RawEvent::Key {
            key,
            pressed: false,
            repeat: false,
        });
        self.frame();
    }

    /// Types text (one text event per character), then a frame.
    pub fn type_text(&mut self, s: &str) {
        for c in s.chars() {
            self.raw(RawEvent::Text(c));
        }
        self.frame();
    }

    /// The chat lines the plugin host wrote.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.plugins.host().chat_lines()
    }

    /// Whether a chat line equals `s`.
    #[must_use]
    pub fn said(&self, s: &str) -> bool {
        self.lines().iter().any(|l| l == s)
    }

    /// Shuts the UI down as the client does at exit.
    pub fn shutdown(&mut self) {
        self.host
            .shutdown(&mut self.plugins, self.renderer.as_mut());
    }
}
