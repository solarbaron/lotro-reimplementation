// SPDX-License-Identifier: GPL-3.0-or-later
//! The scripted UI measurement of decision 0015 §6 (`--ui-script`).
//!
//! The run opens the HUD set, P7, P9 and three plugin windows of a synthetic plugin that
//! this module writes itself ([`BUDGET_PLUGIN`]: a window with a stretch-mode child and
//! `MouseMove` handlers, a window rotated by `SetRotation` (*z* = 90) holding such a child,
//! and a screen-sized window with a full-size background), then plays the phases of §6 as
//! raw window events through the input mapper and as host actions, at one step per frame:
//!
//! | Phase | What |
//! |---|---|
//! | `walk` | 20 s forward (W) with key turns, mouse-look (right button held, raw motion) and the `MouseLeft+MouseRight` run chord |
//! | `sweep` | the pointer across the toolbar and over the `MouseMove` window, one move per frame |
//! | `sweep1000` | the same with 1000 Hz moves (16–17 cursor events per frame, coalesced by the mapper) |
//! | `wheel` | wheel notches over P7's options area |
//! | `chat` | 10 chat lines per second |
//! | `panels` | every panel of `ui-layout.md` §17 opened and closed |
//! | `drag10`, `drag15` | a drag from the bottom-right corner of P10, then P15, for 3 s each |
//! | `pluginresize` | `SetSize` on the stretch-child window on every frame for 3 s, a second stretch child shown during it |
//! | `scale` | the native panels scaled 1.0 → 1.5 → 1.0 |
//! | `screen` | the screen resized and restored, a stretch child shown in the screen-sized window during the resize |
//!
//! Recorded differences from §6 (the run is indicative; decision 0010): the resize bars of
//! P10 and P15 are approximated by a drag from each root's bottom-right corner (the specs
//! do not name the bar elements by ID); `UIScale_Global` is approximated by the native
//! panels' scalers (`tessera-ui` has no setter for it); chat lines are Tessera system
//! notices.

use std::path::Path;

use tessera_input::{KeyCode, MouseButton, RawEvent};

use crate::panels::PanelId;

/// The synthetic plugin of the scripted run (written into the run's own Plugins folder,
/// never into the install).
pub const BUDGET_PLUGIN: &str = r#"
import "Turbine.UI"
local W, H = Turbine.UI.Display.GetSize()
local function colour(a, r, g, b) return Turbine.UI.Color(a, r, g, b) end

local a = Turbine.UI.Window()
a:SetPosition(60, 200)
a:SetSize(360, 260)
a:SetBackColor(colour(0.85, 0.15, 0.15, 0.2))
a:SetVisible(true)
local moves = 0
a.MouseMove = function(sender, args) moves = moves + 1 end
local child = Turbine.UI.Control()
child:SetParent(a)
child:SetPosition(20, 20)
child:SetSize(64, 64)
child:SetBackColor(colour(1, 0.9, 0.6, 0.1))
child:SetStretchMode(1)
child:SetSize(128, 96)
child:SetVisible(true)
local child2 = Turbine.UI.Control()
child2:SetParent(a)
child2:SetPosition(180, 20)
child2:SetSize(32, 32)
child2:SetBackColor(colour(1, 0.2, 0.8, 0.3))
child2:SetStretchMode(1)
child2:SetSize(96, 96)
child2:SetVisible(false)

local b = Turbine.UI.Window()
b:SetPosition(520, 220)
b:SetSize(300, 200)
b:SetBackColor(colour(0.85, 0.1, 0.2, 0.4))
b:SetVisible(true)
b:SetRotation({ x = 0, y = 0, z = 90 })
local bchild = Turbine.UI.Control()
bchild:SetParent(b)
bchild:SetPosition(30, 30)
bchild:SetSize(40, 40)
bchild:SetBackColor(colour(1, 0.3, 0.9, 0.9))
bchild:SetStretchMode(1)
bchild:SetSize(120, 80)
bchild:SetVisible(true)

local c = Turbine.UI.Window()
c:SetPosition(0, 0)
c:SetSize(W, H)
c:SetBackground("Tessera/Budget/bg.tga")
c:SetOpacity(0.25)
c:SetMouseVisible(false)
c:SetZOrder(-100)
c:SetVisible(true)
local cchild = Turbine.UI.Control()
cchild:SetParent(c)
cchild:SetPosition(W / 2, H / 2)
cchild:SetSize(50, 50)
cchild:SetBackColor(colour(1, 1, 1, 0))
cchild:SetStretchMode(1)
cchild:SetSize(150, 150)
cchild:SetVisible(false)

local resizing, n = false, 0
a.Update = function(sender, args)
  if resizing then
    n = n + 1
    a:SetSize(360 + (n % 40), 260 + (n % 30))
  end
end
a:SetWantsUpdates(true)

local cmd = Turbine.ShellCommand()
function cmd:Execute(command, args)
  if args == "resize on" then resizing = true
  elseif args == "resize off" then resizing = false a:SetSize(360, 260)
  elseif args == "child" then child2:SetVisible(true)
  elseif args == "bigchild" then cchild:SetVisible(true)
  elseif args == "moves" then Turbine.Shell.WriteLine("budget: moves " .. moves)
  end
end
Turbine.Shell.AddCommand("budget", cmd)
Turbine.Shell.WriteLine("budget: ready")
"#;

/// Writes the synthetic plugin (descriptor, script and a 64 × 64 TGA background) below
/// `plugins` (`<data dir>/ui-script/Plugins`).
///
/// # Errors
///
/// I/O errors.
pub fn write_budget_plugin(plugins: &Path) -> std::io::Result<()> {
    let dir = plugins.join("Tessera");
    std::fs::create_dir_all(dir.join("Budget"))?;
    std::fs::write(
        dir.join("Budget.plugin"),
        "<?xml version=\"1.0\"?><Plugin><Information><Name>Budget</Name>\
         <Author>Tessera</Author><Version>1.0</Version></Information>\
         <Package>Tessera.Budget.Main</Package><Configuration Apartment=\"Budget\"/></Plugin>",
    )?;
    std::fs::write(dir.join("Budget").join("Main.lua"), BUDGET_PLUGIN)?;
    let mut tga = vec![0u8; 18];
    tga[2] = 2;
    tga[12] = 64;
    tga[14] = 64;
    tga[16] = 32;
    tga[17] = 0x28;
    for y in 0..64u32 {
        for x in 0..64u32 {
            let v = if (x / 8 + y / 8) % 2 == 0 { 0x60 } else { 0xA0 };
            tga.extend_from_slice(&[v, v / 2, 0x30, 0xFF]);
        }
    }
    std::fs::write(dir.join("Budget").join("bg.tga"), tga)
}

/// One thing the script asks for in a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Raw window events (step 1).
    Raw(RawEvent),
    /// Open a panel.
    Open(PanelId),
    /// Hide a panel.
    Hide(PanelId),
    /// A plugin command line.
    Command(String),
    /// A chat line.
    Chat(String),
    /// Scale the native panels.
    Scale(f32),
    /// Resize the screen.
    Resize(u32, u32),
    /// Mark the frame as an event frame.
    Event,
    /// Drag from the bottom-right corner of a panel's root by this many pixels so far
    /// (0 starts it, `None` ends it).
    Drag(PanelId, Option<i32>),
}

#[derive(Debug, Clone, Copy)]
struct Phase {
    name: &'static str,
    frames: u64,
}

const PHASES: [Phase; 13] = [
    Phase {
        name: "prepare",
        frames: 2,
    },
    Phase {
        name: "settle",
        frames: 180,
    },
    Phase {
        name: "walk",
        frames: 1200,
    },
    Phase {
        name: "sweep",
        frames: 180,
    },
    Phase {
        name: "sweep1000",
        frames: 180,
    },
    Phase {
        name: "wheel",
        frames: 180,
    },
    Phase {
        name: "chat",
        frames: 300,
    },
    Phase {
        name: "panels",
        frames: 300,
    },
    Phase {
        name: "drag10",
        frames: 200,
    },
    Phase {
        name: "drag15",
        frames: 200,
    },
    Phase {
        name: "pluginresize",
        frames: 200,
    },
    Phase {
        name: "scale",
        frames: 240,
    },
    Phase {
        name: "screen",
        frames: 240,
    },
];

/// The script's state.
#[derive(Debug, Clone)]
pub struct UiScript {
    screen: (u32, u32),
    phase: usize,
    frame: u64,
    done: bool,
}

fn key(k: KeyCode, pressed: bool) -> Action {
    Action::Raw(RawEvent::Key {
        key: k,
        pressed,
        repeat: false,
    })
}

fn mouse(b: MouseButton, pressed: bool) -> Action {
    Action::Raw(RawEvent::MouseButton { button: b, pressed })
}

fn moved(x: f64, y: f64) -> Action {
    Action::Raw(RawEvent::CursorMoved { x, y })
}

impl UiScript {
    /// A script for a screen of `screen` pixels.
    #[must_use]
    pub fn new(screen: (u32, u32)) -> Self {
        Self {
            screen,
            phase: 0,
            frame: 0,
            done: false,
        }
    }

    /// Sets the screen the phases work from: the UI's current screen when the phases start
    /// (in a windowed run, the size the compositor gave, not the requested one).
    pub fn set_screen(&mut self, screen: (u32, u32)) {
        self.screen = (screen.0.max(1), screen.1.max(1));
    }

    /// The screen the phases work from.
    #[must_use]
    pub fn screen(&self) -> (u32, u32) {
        self.screen
    }

    /// Whether every phase ran.
    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    /// The current phase's label.
    #[must_use]
    pub fn phase(&self) -> &'static str {
        PHASES.get(self.phase).map_or("done", |p| p.name)
    }

    /// Total frames of the script.
    #[must_use]
    pub fn total_frames() -> u64 {
        PHASES.iter().map(|p| p.frames).sum()
    }

    /// The pointer path of the sweeps: across the toolbar (bottom of the screen), then over
    /// the plugin window with `MouseMove` handlers.
    fn sweep_point(&self, t: f64) -> (f64, f64) {
        let (w, h) = (f64::from(self.screen.0), f64::from(self.screen.1));
        if t < 0.5 {
            let u = t * 2.0;
            (w * 0.25 + u * w * 0.5, h - 60.0)
        } else {
            let u = (t - 0.5) * 2.0;
            (70.0 + u * 340.0, 210.0 + u * 240.0)
        }
    }

    /// The actions of this frame, then advances.
    pub fn tick(&mut self) -> Vec<Action> {
        if self.done {
            return Vec::new();
        }
        let Some(p) = PHASES.get(self.phase).copied() else {
            self.done = true;
            return Vec::new();
        };
        let f = self.frame;
        let (w, h) = (f64::from(self.screen.0), f64::from(self.screen.1));
        let mut out = Vec::new();
        match p.name {
            "prepare" if f == 0 => {
                out.push(Action::Open(
                    PanelId::from_number(7).unwrap_or(PanelId::ALL[0]),
                ));
                out.push(Action::Open(
                    PanelId::from_number(9).unwrap_or(PanelId::ALL[0]),
                ));
                out.push(Action::Event);
            }
            "walk" => {
                if f == 0 {
                    out.push(key(KeyCode::W, true));
                }
                match f {
                    120 => out.push(key(KeyCode::D, true)),
                    180 => out.push(key(KeyCode::D, false)),
                    240 => out.push(key(KeyCode::A, true)),
                    300 => out.push(key(KeyCode::A, false)),
                    360 => {
                        out.push(moved(w * 0.6, h * 0.45));
                        out.push(mouse(MouseButton::Right, true));
                    }
                    540 => out.push(mouse(MouseButton::Left, true)),
                    720 => out.push(mouse(MouseButton::Left, false)),
                    780 => out.push(mouse(MouseButton::Right, false)),
                    _ => {}
                }
                if (360..780).contains(&f) {
                    out.push(Action::Raw(RawEvent::MouseMotion { dx: 6.0, dy: 0.0 }));
                }
                if f + 1 == p.frames {
                    out.push(key(KeyCode::W, false));
                }
            }
            "sweep" => {
                let (x, y) = self.sweep_point(f as f64 / p.frames as f64);
                out.push(moved(x, y));
            }
            "sweep1000" => {
                for i in 0..17u32 {
                    let t = (f as f64 + f64::from(i) / 17.0) / p.frames as f64;
                    let (x, y) = self.sweep_point(t);
                    out.push(moved(x, y));
                }
            }
            "wheel" => {
                if f == 0 {
                    out.push(moved(224.0 + 288.0, 120.0 + 300.0));
                }
                if f % 3 == 1 {
                    out.push(Action::Raw(RawEvent::Wheel { dx: 0.0, dy: -1.0 }));
                }
            }
            "chat" => {
                if f.is_multiple_of(6) {
                    out.push(Action::Chat(format!("Tessera budget line {}", f / 6)));
                }
            }
            "panels" => {
                let i = (f / 20) as usize;
                if let Some(id) = PanelId::ALL.get(i).copied() {
                    if f.is_multiple_of(20) {
                        out.push(Action::Open(id));
                    } else if f % 20 == 10 {
                        out.push(Action::Hide(id));
                    }
                }
            }
            "drag10" | "drag15" => {
                let id = PanelId::from_number(if p.name == "drag10" { 10 } else { 15 })
                    .unwrap_or(PanelId::ALL[0]);
                if f == 0 {
                    out.push(Action::Open(id));
                } else if f < 190 {
                    out.push(Action::Drag(id, Some(i32::try_from(f - 1).unwrap_or(0))));
                } else if f == 190 {
                    out.push(Action::Drag(id, None));
                    out.push(Action::Hide(id));
                }
                out.push(Action::Event);
            }
            "pluginresize" => {
                if f == 0 {
                    out.push(Action::Command("/budget resize on".into()));
                } else if f == 30 {
                    out.push(Action::Command("/budget child".into()));
                } else if f == 180 {
                    out.push(Action::Command("/budget resize off".into()));
                }
                out.push(Action::Event);
            }
            "scale" => {
                if f == 0 {
                    out.push(Action::Scale(1.5));
                } else if f == 120 {
                    out.push(Action::Scale(1.0));
                }
            }
            "screen" => {
                if f == 0 {
                    out.push(Action::Resize(self.screen.0 * 3 / 4, self.screen.1 * 3 / 4));
                } else if f == 5 {
                    out.push(Action::Command("/budget bigchild".into()));
                } else if f == 120 {
                    out.push(Action::Resize(self.screen.0, self.screen.1));
                }
            }
            _ => {}
        }
        self.frame += 1;
        if self.frame >= p.frames {
            self.frame = 0;
            self.phase += 1;
            if self.phase >= PHASES.len() {
                self.done = true;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_runs_through_every_phase() {
        let mut s = UiScript::new((1920, 1080));
        let mut frames = 0u64;
        let mut phases = Vec::new();
        let mut raws = 0usize;
        while !s.done() {
            let p = s.phase();
            if phases.last() != Some(&p) {
                phases.push(p);
            }
            raws += s
                .tick()
                .iter()
                .filter(|a| matches!(a, Action::Raw(_)))
                .count();
            frames += 1;
            assert!(frames <= UiScript::total_frames());
        }
        assert_eq!(frames, UiScript::total_frames());
        assert_eq!(phases.len(), PHASES.len());
        assert!(raws > 3000, "{raws}");
        assert!(s.tick().is_empty());
    }

    #[test]
    fn budget_plugin_is_written() {
        let d = tempfile::tempdir().expect("tempdir");
        write_budget_plugin(d.path()).expect("write");
        assert!(d.path().join("Tessera/Budget.plugin").is_file());
        let bg = std::fs::read(d.path().join("Tessera/Budget/bg.tga")).expect("bg");
        assert_eq!(bg.len(), 18 + 64 * 64 * 4);
    }
}
