// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin manager window (`ui-layout.md` §17.2 P12, `lua-plugin-api.md` §6.4–§6.7):
//! the user's plugins, their state, and load and unload through [`PluginControl`], which
//! the client's plugin host implements.
//!
//! Two views:
//!
//! - **Layout**: the game's `pluginmanager` panel (P12) opened from its layout; Tessera puts
//!   one row per plugin into the panel's plugin Tree (the first Tree below the root).
//!   Which elements of the layout are the load, unload and refresh buttons is not named by
//!   the specs (`ui-layout.md` §17.5 OPEN); until it is, a row is selected by a click and
//!   its load state toggled by a double-click, and the client may bind its own keys or
//!   commands to [`PluginManagerPanel::load_selected`] and
//!   [`PluginManagerPanel::unload_selected`]. The Tessera rule of `ui-widgets.md` §13
//!   OPEN (P12) (the about tab highlighted, the options pane hidden) is **not applied**: the
//!   specs do not name the two info-tab Buttons or the panes (no element-ID labels), so
//!   they cannot be found; recorded as implementation OPEN (e) of `ui-layout.md` §17.5.
//! - **Own**: when the install has no such layout, Tessera's own window built from plugin
//!   controls (text in Tessera's fallback font) with a row list and Load, Unload, Refresh
//!   and Close buttons.
//!
//! The rows are Tessera controls of the reserved apartment [`TESSERA_APARTMENT`], so a
//! plugin's unload never destroys them.

use tessera_ui::events::ControlEvent;
use tessera_ui::{ControlKind, ElementKey, Ui, UiEvent, WidgetClass};

use super::PanelsError;

/// The apartment number of Tessera's own controls (never a plugin apartment).
pub const TESSERA_APARTMENT: u32 = u32::MAX;

/// The apartment of a plugin whose descriptor names none (`lua-plugin-api.md` §5.2, P11;
/// apartment names compare case-insensitively).
pub const DEFAULT_APARTMENT: &str = "Default";

/// Height of one row in pixels.
pub const ROW_HEIGHT: i32 = 18;

/// Most rows shown (Tessera cap).
pub const MAX_ROWS: usize = 512;

/// One plugin as the manager lists it (`lua-plugin-api.md` §6.7).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginRow {
    /// `Name`: the key for load.
    pub name: String,
    /// `Version`.
    pub version: String,
    /// `Author`.
    pub author: String,
    /// `Description` (available rows only).
    pub description: String,
    /// `ScriptState`: the apartment it loads (or loaded) into. Empty means
    /// [`DEFAULT_APARTMENT`].
    pub apartment: String,
    /// Whether it is loaded.
    pub loaded: bool,
}

/// Plugin management as the client's plugin host provides it.
pub trait PluginControl {
    /// The available plugins, each marked loaded or not, in a stable order.
    fn rows(&self) -> Vec<PluginRow>;

    /// Queues a load of the plugin named `name` (`lua-plugin-api.md` §6.2).
    ///
    /// # Errors
    /// A message for the user when the request is refused.
    fn load(&mut self, name: &str) -> Result<(), String>;

    /// Unloads apartment `apartment`, or every apartment for `None` (§6.3).
    ///
    /// # Errors
    /// A message for the user when the request is refused.
    fn unload(&mut self, apartment: Option<&str>) -> Result<(), String>;

    /// Rediscovers the available plugins (§4.3).
    fn refresh(&mut self) {}
}

/// A plugin host with no plugins.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPlugins;

impl PluginControl for NoPlugins {
    fn rows(&self) -> Vec<PluginRow> {
        Vec::new()
    }

    fn load(&mut self, name: &str) -> Result<(), String> {
        Err(format!("no plugin named {name}"))
    }

    fn unload(&mut self, _apartment: Option<&str>) -> Result<(), String> {
        Ok(())
    }
}

/// The buttons of Tessera's own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnButtons {
    load: ElementKey,
    unload: ElementKey,
    refresh: ElementKey,
    close: ElementKey,
    details: ElementKey,
}

/// Which window shows the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum View {
    #[default]
    Closed,
    /// The game layout: root and the plugin tree (if found).
    Layout {
        root: ElementKey,
        list: Option<ElementKey>,
    },
    /// Tessera's own window.
    Own {
        window: ElementKey,
        list: ElementKey,
        buttons: OwnButtons,
    },
}

/// The plugin manager window.
#[derive(Debug, Clone, Default)]
pub struct PluginManagerPanel {
    view: View,
    rows: Vec<(ElementKey, PluginRow)>,
    selected: Option<usize>,
}

impl PluginManagerPanel {
    /// The window's root, when open.
    #[must_use]
    pub fn root(&self) -> Option<ElementKey> {
        match self.view {
            View::Closed => None,
            View::Layout { root, .. } => Some(root),
            View::Own { window, .. } => Some(window),
        }
    }

    /// Whether the view is Tessera's own window.
    #[must_use]
    pub fn is_own(&self) -> bool {
        matches!(self.view, View::Own { .. })
    }

    /// The rows shown, in order.
    pub fn rows(&self) -> impl Iterator<Item = &PluginRow> {
        self.rows.iter().map(|(_, r)| r)
    }

    /// The row elements, in order.
    #[must_use]
    pub fn row_elements(&self) -> Vec<ElementKey> {
        self.rows.iter().map(|(k, _)| *k).collect()
    }

    /// The selected row.
    #[must_use]
    pub fn selected(&self) -> Option<&PluginRow> {
        self.selected.and_then(|i| self.rows.get(i)).map(|(_, r)| r)
    }

    /// Uses the game's P12 panel opened at `root`: finds its plugin Tree and fills it.
    pub fn attach_layout(&mut self, ui: &mut Ui, root: ElementKey, plugins: &dyn PluginControl) {
        self.clear_rows(ui);
        let list = tessera_ui::game::subtree(ui, root)
            .into_iter()
            .find(|k| ui.class(*k) == Some(WidgetClass::Tree));
        if list.is_none() {
            tracing::debug!("plugin manager: no Tree in the layout; rows not shown");
        }
        self.view = View::Layout { root, list };
        self.rebuild(ui, plugins);
    }

    /// Opens Tessera's own window (the fallback when the install has no plugin-manager
    /// layout), centred on the screen, and fills it.
    ///
    /// # Errors
    /// [`PanelsError::Ui`] when the element cap is reached.
    pub fn open_own(
        &mut self,
        ui: &mut Ui,
        plugins: &dyn PluginControl,
    ) -> Result<ElementKey, PanelsError> {
        if let View::Own { window, .. } = self.view
            && ui.exists(window)
        {
            ui.control_set_visible(window, true);
            ui.activate(window);
            return Ok(window);
        }
        self.clear_rows(ui);
        let (sw, sh) = ui.screen_size();
        let (w, h) = (420, 340);
        let window = ui.create_control(ControlKind::Window, TESSERA_APARTMENT)?;
        // On any failure the window (and the children already under it) is destroyed, so a
        // failed open leaves no element behind.
        let built = Self::build_own(ui, window, (w, h));
        let (list, buttons) = match built {
            Ok(v) => v,
            Err(e) => {
                ui.destroy_control(window);
                return Err(e);
            }
        };
        ui.control_set_position(window, ((sw - w) / 2).max(0), ((sh - h) / 2).max(0));
        ui.control_set_visible(window, true);
        ui.activate(window);
        self.view = View::Own {
            window,
            list,
            buttons,
        };
        self.rebuild(ui, plugins);
        Ok(window)
    }

    /// Creates the own window's children under `window`.
    fn build_own(
        ui: &mut Ui,
        window: ElementKey,
        (w, h): (i32, i32),
    ) -> Result<(ElementKey, OwnButtons), PanelsError> {
        ui.control_set_size(window, w, h);
        ui.control_set_back_colour(window, Some([0.0, 0.0, 0.0, 0.85]));
        let child = |ui: &mut Ui,
                     kind: ControlKind,
                     (x, y, cw, ch): (i32, i32, i32, i32),
                     text: Option<&str>|
         -> Result<ElementKey, PanelsError> {
            let k = ui.create_control(kind, TESSERA_APARTMENT)?;
            if let Err(e) = ui.control_set_parent(k, Some(window)) {
                ui.destroy_control(k);
                return Err(e.into());
            }
            ui.control_set_position(k, x, y);
            ui.control_set_size(k, cw, ch);
            if let Some(t) = text {
                ui.set_text(k, Some(t.to_string()));
            }
            ui.control_set_visible(k, true);
            Ok(k)
        };
        child(ui, ControlKind::Label, (10, 6, 400, 20), Some("Plugins"))?;
        let list = child(ui, ControlKind::Control, (10, 30, 400, 200), None)?;
        ui.control_set_back_colour(list, Some([0.1, 0.1, 0.1, 0.9]));
        let details = child(ui, ControlKind::Label, (10, 236, 400, 54), Some(""))?;
        let load = child(ui, ControlKind::Button, (10, 300, 92, 26), Some("Load"))?;
        let unload = child(ui, ControlKind::Button, (112, 300, 92, 26), Some("Unload"))?;
        let refresh = child(ui, ControlKind::Button, (214, 300, 92, 26), Some("Refresh"))?;
        let close = child(ui, ControlKind::Button, (318, 300, 92, 26), Some("Close"))?;
        Ok((
            list,
            OwnButtons {
                load,
                unload,
                refresh,
                close,
                details,
            },
        ))
    }

    /// Whether the window is open and shown.
    #[must_use]
    pub fn is_shown(&self, ui: &Ui) -> bool {
        self.root()
            .is_some_and(|k| ui.exists(k) && ui.is_visible(k))
    }

    /// Hides the window.
    pub fn hide(&mut self, ui: &mut Ui) {
        match self.view {
            View::Closed => {}
            View::Layout { root, .. } => ui.set_shown(root, false),
            View::Own { window, .. } => ui.control_set_visible(window, false),
        }
    }

    fn clear_rows(&mut self, ui: &mut Ui) {
        for (k, _) in self.rows.drain(..) {
            if let View::Layout {
                list: Some(list), ..
            } = self.view
                && ui.exists(list)
            {
                ui.list_remove(list, k);
            }
            ui.destroy_control(k);
        }
        self.selected = None;
    }

    /// Rebuilds the rows from the plugin host (after a load, unload or refresh).
    pub fn rebuild(&mut self, ui: &mut Ui, plugins: &dyn PluginControl) {
        let keep = self.selected().map(|r| r.name.clone());
        self.clear_rows(ui);
        let (list, width, own) = match self.view {
            View::Closed => return,
            View::Layout { list: None, .. } => return,
            View::Layout {
                list: Some(list), ..
            } => (list, ui.layout_box(list).map_or(200, |b| b.w()), false),
            View::Own { list, .. } => (list, ui.control_size(list).0, true),
        };
        if !ui.exists(list) {
            return;
        }
        for (i, row) in plugins.rows().into_iter().take(MAX_ROWS).enumerate() {
            let Ok(k) = ui.create_control(ControlKind::Label, TESSERA_APARTMENT) else {
                break;
            };
            ui.set_text(k, Some(row_text(&row)));
            let placed = if own {
                ui.control_set_parent(k, Some(list)).is_ok()
            } else {
                ui.list_add(list, k, None).is_ok()
            };
            if !placed {
                ui.destroy_control(k);
                break;
            }
            let y = i32::try_from(i)
                .unwrap_or(i32::MAX)
                .saturating_mul(ROW_HEIGHT);
            if own {
                ui.control_set_position(k, 0, y);
            }
            ui.control_set_size(k, width, ROW_HEIGHT);
            ui.control_set_visible(k, true);
            self.rows.push((k, row));
        }
        if let Some(name) = keep {
            let i = self.rows.iter().position(|(_, r)| r.name == name);
            self.select(ui, i);
        } else {
            self.update_details(ui);
        }
    }

    /// Selects row `i` (or none) and shows it highlighted.
    pub fn select(&mut self, ui: &mut Ui, i: Option<usize>) {
        self.selected = i.filter(|i| *i < self.rows.len());
        for (j, (k, _)) in self.rows.iter().enumerate() {
            let c = (Some(j) == self.selected).then_some([0.25, 0.35, 0.6, 0.8]);
            ui.control_set_back_colour(*k, c);
        }
        self.update_details(ui);
    }

    fn update_details(&self, ui: &mut Ui) {
        if let View::Own { buttons, .. } = self.view {
            let text = self.selected().map_or_else(String::new, |r| {
                format!(
                    "{} {} by {}\n{}",
                    r.name, r.version, r.author, r.description
                )
            });
            ui.set_text(buttons.details, Some(text));
        }
    }

    /// Loads the selected plugin (`lua-plugin-api.md` §6.2) and refreshes the rows.
    ///
    /// # Errors
    /// The plugin host's message, or "no plugin selected".
    pub fn load_selected(
        &mut self,
        ui: &mut Ui,
        plugins: &mut dyn PluginControl,
    ) -> Result<(), String> {
        let name = self
            .selected()
            .map(|r| r.name.clone())
            .ok_or_else(|| "no plugin selected".to_string())?;
        plugins.load(&name)?;
        self.rebuild(ui, plugins);
        Ok(())
    }

    /// Unloads the selected plugin's apartment (§6.3) and refreshes the rows.
    ///
    /// # Errors
    /// The plugin host's message, or "no plugin selected".
    pub fn unload_selected(
        &mut self,
        ui: &mut Ui,
        plugins: &mut dyn PluginControl,
    ) -> Result<(), String> {
        let row = self
            .selected()
            .cloned()
            .ok_or_else(|| "no plugin selected".to_string())?;
        // LPA §5.2: a descriptor without an apartment loads into `Default`.
        let apt = if row.apartment.is_empty() {
            DEFAULT_APARTMENT.to_string()
        } else {
            row.apartment.clone()
        };
        plugins.unload(Some(&apt))?;
        self.rebuild(ui, plugins);
        Ok(())
    }

    /// Routes a UI event: row clicks and double-clicks, and the own window's buttons.
    /// Returns whether the event was the manager's.
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        plugins: &mut dyn PluginControl,
    ) -> bool {
        let UiEvent::Control { control, event } = ev else {
            return false;
        };
        let row = self.rows.iter().position(|(k, _)| k == control);
        match (event, row) {
            (ControlEvent::MouseClick { .. }, Some(i)) => {
                self.select(ui, Some(i));
                true
            }
            (ControlEvent::MouseDoubleClick { .. }, Some(i)) => {
                self.select(ui, Some(i));
                let loaded = self.rows[i].1.loaded;
                let r = if loaded {
                    self.unload_selected(ui, plugins)
                } else {
                    self.load_selected(ui, plugins)
                };
                if let Err(e) = r {
                    tracing::info!("plugin manager: {e}");
                }
                true
            }
            (ControlEvent::MouseClick { .. }, None) => {
                let View::Own { buttons, .. } = self.view else {
                    return false;
                };
                let r = if *control == buttons.load {
                    self.load_selected(ui, plugins)
                } else if *control == buttons.unload {
                    self.unload_selected(ui, plugins)
                } else if *control == buttons.refresh {
                    plugins.refresh();
                    self.rebuild(ui, plugins);
                    Ok(())
                } else if *control == buttons.close {
                    self.hide(ui);
                    Ok(())
                } else {
                    return false;
                };
                if let Err(e) = r {
                    tracing::info!("plugin manager: {e}");
                }
                true
            }
            _ => false,
        }
    }
}

/// A row's text (Tessera's wording).
fn row_text(r: &PluginRow) -> String {
    let mark = if r.loaded { "[x]" } else { "[ ]" };
    if r.version.is_empty() {
        format!("{mark} {}", r.name)
    } else {
        format!("{mark} {} {}", r.name, r.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_text_marks_loaded_plugins() {
        let mut r = PluginRow {
            name: "Clock".into(),
            version: "1.2".into(),
            ..PluginRow::default()
        };
        assert_eq!(row_text(&r), "[ ] Clock 1.2");
        r.loaded = true;
        r.version.clear();
        assert_eq!(row_text(&r), "[x] Clock");
    }

    #[test]
    fn no_plugins_refuses_loads() {
        let mut p = NoPlugins;
        assert!(p.rows().is_empty());
        assert!(p.load("x").is_err());
        assert!(p.unload(None).is_ok());
    }
}
