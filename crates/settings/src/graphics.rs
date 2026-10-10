// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own answers for client graphics options that the data reads as properties,
//! kept in the `[Graphics]` section of the settings file.
//!
//! The section is stored in [`Settings::extra`] under [`GRAPHICS_SECTION`], so files written
//! by this version round-trip unchanged through older ones, and unknown keys inside the
//! section are kept. Every key is optional; a missing key takes its default and a malformed
//! one is reported by [`GraphicsPrefs::from_settings`].
//!
//! ```toml
//! [Graphics]
//! avatar_update_visible = true  # appearance.md §3.3: the updated-avatar option (default on)
//! avatar_face_compositing = true  # appearance.md §3.5: the face-compositing option (default on)
//! ```

use crate::Settings;

/// Key of the section in [`Settings::extra`].
pub const GRAPHICS_SECTION: &str = "Graphics";
/// Key of the updated-avatar option (`appearance.md` §3.3: `GOP_AvatarUpdateVisible`).
pub const AVATAR_UPDATE_VISIBLE_KEY: &str = "avatar_update_visible";
/// Key of the face-compositing option (`appearance.md` §3.5: `GOP_AvatarFaceCompositing`).
pub const AVATAR_FACE_COMPOSITING_KEY: &str = "avatar_face_compositing";

/// The `[Graphics]` preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsPrefs {
    /// The updated-avatar option (`appearance.md` §3.3): with it on (the default), figure
    /// appearance elements that name a toggle file use it.
    pub avatar_update_visible: bool,
    /// The face-compositing option (`appearance.md` §3.5): with it on (the default), the
    /// species tables compose the face texture from the character's choices.
    pub avatar_face_compositing: bool,
}

impl Default for GraphicsPrefs {
    fn default() -> Self {
        Self {
            avatar_update_visible: true,
            avatar_face_compositing: true,
        }
    }
}

fn boolean(v: &toml::Value) -> Option<bool> {
    match v {
        toml::Value::Boolean(b) => Some(*b),
        toml::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "on" | "true" | "yes" => Some(true),
            "off" | "false" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

impl GraphicsPrefs {
    /// Reads the `[Graphics]` section. Never fails: a malformed value keeps its default and
    /// adds a warning.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut p = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(GRAPHICS_SECTION) else {
            return (p, warn);
        };
        let Some(t) = section.as_table() else {
            warn.push(format!(
                "[{GRAPHICS_SECTION}] is not a table; using defaults"
            ));
            return (p, warn);
        };
        for (key, slot) in [
            (AVATAR_UPDATE_VISIBLE_KEY, &mut p.avatar_update_visible),
            (AVATAR_FACE_COMPOSITING_KEY, &mut p.avatar_face_compositing),
        ] {
            if let Some(v) = t.get(key) {
                match boolean(v) {
                    Some(b) => *slot = b,
                    None => warn.push(format!("[{GRAPHICS_SECTION}] {key}: invalid value ignored")),
                }
            }
        }
        (p, warn)
    }

    /// Writes the section into `settings`, replacing only the keys this type knows; other
    /// keys of the section are kept.
    pub fn store(&self, settings: &mut Settings) {
        let entry = settings
            .extra
            .entry(GRAPHICS_SECTION.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        if let Some(t) = entry.as_table_mut() {
            t.insert(
                AVATAR_UPDATE_VISIBLE_KEY.into(),
                toml::Value::Boolean(self.avatar_update_visible),
            );
            t.insert(
                AVATAR_FACE_COMPOSITING_KEY.into(),
                toml::Value::Boolean(self.avatar_face_compositing),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_bad_values() {
        let s = Settings::default();
        let (p, w) = GraphicsPrefs::from_settings(&s);
        assert!(p.avatar_update_visible && w.is_empty());
        let mut s = Settings::default();
        GraphicsPrefs {
            avatar_update_visible: false,
            avatar_face_compositing: false,
        }
        .store(&mut s);
        let (p, w) = GraphicsPrefs::from_settings(&s);
        assert!(!p.avatar_update_visible && !p.avatar_face_compositing && w.is_empty());
        let mut s = Settings::default();
        let mut t = toml::Table::new();
        t.insert(AVATAR_UPDATE_VISIBLE_KEY.into(), toml::Value::Integer(3));
        t.insert("other".into(), toml::Value::Integer(1));
        s.extra
            .insert(GRAPHICS_SECTION.into(), toml::Value::Table(t.clone()));
        let (p, w) = GraphicsPrefs::from_settings(&s);
        assert!(p.avatar_update_visible);
        assert_eq!(w.len(), 1);
        GraphicsPrefs::default().store(&mut s);
        let kept = s.extra[GRAPHICS_SECTION].as_table().unwrap();
        assert_eq!(kept.get("other"), Some(&toml::Value::Integer(1)));
        s.extra
            .insert(GRAPHICS_SECTION.into(), toml::Value::Integer(1));
        let (p, w) = GraphicsPrefs::from_settings(&s);
        assert!(p.avatar_update_visible);
        assert_eq!(w.len(), 1);
    }
}
