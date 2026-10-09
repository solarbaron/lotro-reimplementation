// SPDX-License-Identifier: GPL-3.0-or-later
//! Test harness for the `Turbine.UI` bindings: a recording host, a synthetic `Plugins/`
//! tree in a temporary folder, a `tessera-ui` UI without install data, and synthetic
//! input offered the way the client offers it (decision 0014 D7). All values invented.
#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_plugins::api::NativeApi;
use tessera_plugins::class_helper::ClassHelper;
use tessera_plugins::ui::{PluginUi, UiBinding, install};
use tessera_plugins::{
    CoreHost, DataScope, HostApi, Limits, ManagerConfig, PluginDataLocation, PluginManager,
    ScriptErrorReport, mlua,
};
use tessera_ui::{Command, CommandOffer, MemoryEnv, Phase, Ui, UiEvent, UiInput, Vocab};

/// Screen size of the synthetic UI.
pub const SCREEN: (i32, i32) = (1600, 900);

#[derive(Debug, Default)]
pub struct Rec {
    pub lines: Vec<String>,
    pub chat: Vec<(u32, String)>,
    pub errors: Vec<ScriptErrorReport>,
    pub logs: Vec<String>,
}

#[derive(Debug)]
pub struct Host {
    /// The host's command registry (the plugin host keeps none).
    commands: tessera_plugins::CommandTable,
    pub rec: Mutex<Rec>,
    pub data: PluginDataLocation,
    pub binding: Mutex<Option<UiBinding>>,
}

impl CoreHost for Host {
    fn script_log(&self, _state: &str, message: &str) {
        self.rec.lock().unwrap().logs.push(message.to_owned());
    }
    fn report_error(&self, report: &ScriptErrorReport) {
        self.rec.lock().unwrap().errors.push(report.clone());
    }
}

impl HostApi for Host {
    fn command_table(&self) -> Option<&tessera_plugins::CommandTable> {
        Some(&self.commands)
    }

    fn write_line(&self, text: &str) {
        self.rec.lock().unwrap().lines.push(text.to_owned());
    }
    fn write_chat(&self, chat_type: u32, text: &str) {
        self.rec
            .lock()
            .unwrap()
            .chat
            .push((chat_type, text.to_owned()));
    }
    fn plugin_data_dir(&self, scope: DataScope) -> Option<PathBuf> {
        self.data.dir_for(scope)
    }
    fn install_natives(&self, api: &NativeApi<'_>) -> mlua::Result<()> {
        let b = *self.binding.lock().unwrap();
        match b {
            Some(b) => install(api, b),
            None => Ok(()),
        }
    }
}

/// An 18-byte TGA header (w × h, 32 bits) followed by pixels.
pub fn tga(w: u16, h: u16) -> Vec<u8> {
    let mut v = vec![0u8; 18];
    v[2] = 2;
    v[12..14].copy_from_slice(&w.to_le_bytes());
    v[14..16].copy_from_slice(&h.to_le_bytes());
    v[16] = 32;
    v.extend(std::iter::repeat_n(
        0x80u8,
        usize::from(w.min(64)) * usize::from(h.min(64)) * 4,
    ));
    v
}

/// The start of a JPEG file: SOI and an SOF0 frame of w × h (enough for the header probe).
pub fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08];
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&[0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    v.extend_from_slice(&[0xFF, 0xD9]);
    v
}

/// A fixture: temporary folders, the host, the UI, the binding layer and the manager.
pub struct Fx {
    pub dir: tempfile::TempDir,
    pub plugins: PathBuf,
    pub host: Arc<Host>,
    pub pui: PluginUi,
    pub manager: PluginManager,
    pub client: Vec<UiEvent>,
    hold: u64,
    /// Apartment → namespace path of the plugins written by [`Fx::plugin`].
    namespaces: RefCell<Vec<(String, String)>>,
}

impl Fx {
    /// An empty fixture (no plugins yet; add them with [`Fx::plugin`] and [`Fx::file`],
    /// then call [`Fx::refresh`]).
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    /// A fixture with the given limits.
    pub fn with_limits(limits: Limits) -> Self {
        Self::build(limits, None)
    }

    /// A fixture over an existing `Plugins/` folder, read-only (real-data runs); plugin data
    /// goes to a temporary folder.
    pub fn over(plugins: &Path) -> Self {
        Self::build(Limits::default(), Some(plugins.to_path_buf()))
    }

    fn build(limits: Limits, external: Option<PathBuf>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let plugins = match external {
            Some(p) => p,
            None => {
                let p = dir.path().join("Plugins");
                std::fs::create_dir_all(&p).unwrap();
                p
            }
        };
        let data = dir.path().join("data/plugindata");
        let host = Arc::new(Host {
            commands: tessera_plugins::CommandTable::new(),
            rec: Mutex::new(Rec::default()),
            data: PluginDataLocation {
                root: data,
                account: "acct".into(),
                server: Some("srv".into()),
                character: Some("char".into()),
            },
            binding: Mutex::new(None),
        });
        let ui = Rc::new(RefCell::new(Ui::new(
            Box::new(MemoryEnv::default()),
            Vocab::default(),
            SCREEN,
        )));
        let pui = PluginUi::new(ui);
        *host.binding.lock().unwrap() = Some(pui.binding());
        let mut config = ManagerConfig::new(&plugins).with_limits(limits);
        config.index_on_worker = false;
        config.writes_on_worker = false;
        config.deferred_slice = Duration::from_secs(60);
        config.builtins = Some(ClassHelper::provider());
        let host_dyn: Arc<dyn HostApi> = host.clone();
        let manager = PluginManager::new(config, host_dyn);
        Self {
            dir,
            plugins,
            host,
            pui,
            manager,
            client: Vec::new(),
            hold: 1,
            namespaces: RefCell::new(Vec::new()),
        }
    }

    /// Write a file below `Plugins/`.
    pub fn file(&self, rel: &str, bytes: impl AsRef<[u8]>) {
        let p = self.plugins.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    /// A plugin `Author/Name.plugin` with package `Author.Name.Main` in `apartment`
    /// (`None`: the default apartment) whose main file is `source`.
    pub fn plugin(&self, author: &str, name: &str, apartment: Option<&str>, source: &str) {
        self.namespaces.borrow_mut().push((
            apartment.unwrap_or("Default").to_owned(),
            format!("{author}.{name}"),
        ));
        let conf = apartment.map_or(String::new(), |a| {
            format!("<Configuration Apartment=\"{a}\"/>")
        });
        self.file(
            &format!("{author}/{name}.plugin"),
            format!(
                "<?xml version=\"1.0\"?><Plugin><Information><Name>{name}</Name>\
                 <Author>{author}</Author><Version>1.0</Version></Information>\
                 <Package>{author}.{name}.Main</Package>{conf}</Plugin>"
            ),
        );
        self.file(&format!("{author}/{name}/Main.lua"), source);
    }

    /// Let [`Fx::eval`] in `apartment` see the globals of package namespace `ns`.
    pub fn namespace(&self, apartment: &str, ns: &str) {
        self.namespaces
            .borrow_mut()
            .push((apartment.to_owned(), ns.to_owned()));
    }

    /// Copy a fixture tree (relative to `tests/fixtures/`) into `Plugins/`.
    pub fn copy_fixture(&self, rel: &str) {
        let src = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(rel);
        copy_dir(&src, &self.plugins);
    }

    /// Rescan the plugin folder and rebuild the file index.
    pub fn refresh(&mut self) {
        self.manager.refresh();
        assert!(self.manager.wait_for_index(Duration::from_secs(5)));
    }

    /// Load a plugin now and deliver what it caused.
    pub fn load(&mut self, name: &str) {
        self.manager
            .load_plugin(name)
            .unwrap_or_else(|e| panic!("loading {name}: {e}; errors: {:?}", self.errors()));
        self.deliver();
    }

    /// Run plugin code in `apartment` as one host call and return its value. The code sees
    /// the globals of the last plugin [`Fx::plugin`] wrote for that apartment (its
    /// package namespace) before the real globals.
    pub fn eval<R: mlua::FromLuaMulti>(&self, apartment: &str, code: &str) -> R {
        self.try_eval(apartment, code)
            .unwrap_or_else(|e| panic!("eval in {apartment}: {e}"))
    }

    /// Run plugin code in `apartment`; the Lua result (errors included).
    pub fn try_eval<R: mlua::FromLuaMulti>(&self, apartment: &str, code: &str) -> mlua::Result<R> {
        self.manager
            .with_lua(apartment, |lua| {
                self.chunk(lua, apartment, code)?.eval::<R>()
            })
            .expect("apartment exists")
    }

    /// A chunk of `code` whose environment sees the namespace of the apartment's plugin.
    fn chunk<'l>(
        &self,
        lua: &'l mlua::Lua,
        apartment: &str,
        code: &str,
    ) -> mlua::Result<mlua::chunk::Chunk<'l>> {
        let ns = self
            .namespaces
            .borrow()
            .iter()
            .rev()
            .find(|(a, _)| a == apartment)
            .map(|(_, n)| n.clone());
        let chunk = lua.load(code.to_owned());
        Ok(match &ns {
            Some(ns) => {
                let env: mlua::Table = lua
                    .load(format!(
                        "local g = ... local ns = g \
                         for part in string.gmatch('{ns}', '[^.]+') do ns = ns and ns[part] end \
                         ns = ns or g \
                         return setmetatable({{}}, {{ __index = function(_, k) \
                           local v = rawget(ns, k) if v ~= nil then return v end return g[k] end, \
                           __newindex = ns }})"
                    ))
                    .call(lua.globals())?;
                chunk.set_environment(env)
            }
            None => chunk,
        })
    }

    /// Deliver the UI outbox; returns the client events.
    pub fn deliver(&mut self) -> Vec<UiEvent> {
        let ev = self.pui.deliver(&self.manager);
        self.client.extend(ev.clone());
        ev
    }

    /// Frame-loop step 7: hover tick, `Update` broadcast, frame work, delivery, deferred
    /// work and the collector step.
    pub fn frame(&mut self, dt: f64) {
        {
            let mut ui = self.pui.ui().borrow_mut();
            let t = ui.time() + dt;
            ui.set_time(t);
            ui.tick_hover();
        }
        self.deliver();
        self.pui.update_tick(&self.manager);
        self.pui.ui().borrow_mut().frame_work();
        self.deliver();
        self.manager.pump();
        self.deliver();
    }

    fn offer(&mut self, input: UiInput) -> bool {
        // Plugin code run by `eval` since the last delivery may have left list layouts
        // pending (the Wiring rule of `tessera_plugins::ui`).
        self.pui.flush_layouts();
        let r = self.pui.ui().borrow_mut().offer(&input);
        self.deliver();
        r
    }

    /// Move the pointer.
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.offer(UiInput::PointerMoved {
            x: f64::from(x),
            y: f64::from(y),
        });
    }

    fn command(&mut self, phase: Phase, hold: u64, name: &str, id: u32) -> bool {
        self.offer(UiInput::Command(CommandOffer {
            phase,
            hold,
            command: Command {
                name: name.into(),
                id: Some(id),
            },
            value: 1.0,
            position: None,
        }))
    }

    /// Press and release a mouse button at (x, y): `1` left, `2` right, `4` middle.
    pub fn click(&mut self, x: i32, y: i32, button: u32) {
        self.move_to(x, y);
        let (name, id) = match button {
            2 => ("SelectRight", 0x4C),
            4 => ("SelectMid", 0xA4),
            _ => ("SelectLeft", 0x56),
        };
        let h = self.next_hold();
        self.command(Phase::Start, h, name, id);
        self.command(Phase::End, h, name, id);
    }

    /// Press a mouse button at (x, y) without releasing it; returns the hold.
    pub fn press(&mut self, x: i32, y: i32) -> u64 {
        self.move_to(x, y);
        let h = self.next_hold();
        self.command(Phase::Start, h, "SelectLeft", 0x56);
        h
    }

    /// Release the hold of [`Fx::press`] at (x, y).
    pub fn release(&mut self, hold: u64, x: i32, y: i32) {
        self.move_to(x, y);
        self.command(Phase::End, hold, "SelectLeft", 0x56);
    }

    /// Turn the wheel at (x, y): `up` = the scroll-up action.
    pub fn wheel(&mut self, x: i32, y: i32, up: bool) {
        self.move_to(x, y);
        let h = self.next_hold();
        let (name, id) = if up {
            ("ScrollUp", 0x82)
        } else {
            ("ScrollDown", 0x10)
        };
        self.command(Phase::Start, h, name, id);
        self.command(Phase::End, h, name, id);
    }

    /// Press and release a key action with an ID.
    pub fn key(&mut self, name: &str, id: u32) -> (bool, bool) {
        let h = self.next_hold();
        let a = self.command(Phase::Start, h, name, id);
        let b = self.command(Phase::End, h, name, id);
        (a, b)
    }

    /// Type a character.
    pub fn type_char(&mut self, c: char) {
        self.offer(UiInput::Char(c));
    }

    fn next_hold(&mut self) -> u64 {
        self.hold += 1;
        self.hold
    }

    /// The screen box (left, top, width, height) of a plugin object given by a Lua
    /// expression evaluated in `apartment`.
    pub fn screen_box(&self, apartment: &str, expr: &str) -> (i32, i32, i32, i32) {
        let key = self
            .manager
            .with_lua(apartment, |lua| {
                let t: mlua::Table = self
                    .chunk(lua, apartment, &format!("return {expr}"))
                    .and_then(|c| c.eval())
                    .unwrap_or_else(|e| panic!("{expr}: {e}"));
                self.pui.element_of(lua, &t)
            })
            .flatten()
            .unwrap_or_else(|| panic!("{expr} is not a control"));
        let ui = self.pui.ui().borrow();
        let b = ui.displayed_box(key);
        (b.l, b.t, b.w(), b.h())
    }

    /// The centre of a plugin object on the screen.
    pub fn centre(&self, apartment: &str, expr: &str) -> (i32, i32) {
        let (l, t, w, h) = self.screen_box(apartment, expr);
        (l + w / 2, t + h / 2)
    }

    /// Lines written to chat by `Shell.WriteLine` and the host.
    pub fn chat(&self) -> Vec<String> {
        let r = self.host.rec.lock().unwrap();
        r.chat
            .iter()
            .map(|c| c.1.clone())
            .chain(r.lines.iter().cloned())
            .collect()
    }

    /// Plugin errors reported so far.
    pub fn errors(&self) -> Vec<String> {
        self.host
            .rec
            .lock()
            .unwrap()
            .errors
            .iter()
            .map(|e| format!("[{}] {}: {}", e.state, e.context, e.message))
            .collect()
    }

    /// Assert that no plugin error was reported.
    pub fn assert_no_errors(&self) {
        let e = self.errors();
        assert!(e.is_empty(), "plugin errors: {e:#?}");
    }
}

impl Default for Fx {
    fn default() -> Self {
        Self::new()
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}
