// SPDX-License-Identifier: GPL-3.0-or-later
//! Which backend the screens talk to, and the per-backend button rules of
//! `character-screens.md` §7.1 (New) and §7.2 (Delete), and where the creation choices are
//! kept (§7.1 Tessera rule; decision 0025 D2).

use tessera_net::Session;
use tessera_net::sandbox::SandboxBackend;

use super::texts::OwnText;

/// The server key of a plain sandbox in the settings file.
pub const SANDBOX_KEY: &str = "sandbox";
/// The key of the scenario feed: its own, so looks kept from plain-sandbox runs never
/// reach a scenario's characters (its looks are scripted and never kept).
pub const SCENARIO_KEY: &str = "scenario";

/// The session's backend as the screens' rules see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendKind {
    /// The sandbox playing a scenario: the feed answers the character commands
    /// (`scenario-feed.md` §7.5).
    ScenarioFeed,
    /// The sandbox without a scenario (`scenario-feed.md` §9.6).
    PlainSandbox,
    /// Tessera's test server (`remote` backend, also the server half of an overlay run).
    TestServer {
        /// The server advertised `CHARACTER_CREATION` (`None`: not greeted yet).
        creation: Option<bool>,
        /// The negotiated minor protocol version (`None`: not greeted yet); 1 and higher
        /// carry looks (`scenario-feed.md` §9.7).
        minor: Option<u16>,
        /// The operator's display name, once greeted.
        server_name: Option<String>,
    },
    /// An official session (`tessera-compat`, `official-entry.md` §8.7): New and Delete
    /// are ghosted with their reason, since the creation and deletion messages need install
    /// enum mappers no spec assigns yet (`official-messages-world.md` §3.2.5); choices are
    /// never kept in the settings.
    Official,
    /// Any other backend (named): no rule of §7 names it; the screens enable New and
    /// Delete and fall back to §7.2's refusal rule.
    Other(String),
}

/// The backend name of the compatibility backend (`tessera-compat`).
pub const OFFICIAL_BACKEND: &str = "compat";

/// Whether a button is enabled, or ghosted with its tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonRule {
    /// Enabled.
    Enabled,
    /// Ghosted (disabled look); the tooltip says why.
    Ghosted(Option<OwnText>),
    /// Not shown.
    Hidden,
}

impl ButtonRule {
    /// True when enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }
}

impl BackendKind {
    /// The backend of `session`.
    #[must_use]
    pub fn of(session: &Session) -> Self {
        if let Some(r) = session.remote_backend() {
            let info = r.server_info();
            return Self::TestServer {
                creation: r.server_caps().map(|_| r.can_create_characters()),
                minor: r.negotiated_minor(),
                server_name: info.map(|i| i.name.clone()),
            };
        }
        if let Some(s) = session.backend::<SandboxBackend>() {
            return if s.scenario_status().is_some() {
                Self::ScenarioFeed
            } else {
                Self::PlainSandbox
            };
        }
        if session.backend_name() == OFFICIAL_BACKEND {
            return Self::Official;
        }
        Self::Other(session.backend_name().to_owned())
    }

    /// The New button (§3, §7.1): enabled on the sandbox (with or without a scenario);
    /// on the test server only when it advertised `CHARACTER_CREATION`, else ghosted with
    /// its tooltip (also while not yet greeted).
    #[must_use]
    pub fn new_button(&self) -> ButtonRule {
        match self {
            Self::ScenarioFeed | Self::PlainSandbox | Self::Other(_) => ButtonRule::Enabled,
            Self::TestServer {
                creation: Some(true),
                ..
            } => ButtonRule::Enabled,
            Self::TestServer { .. } => ButtonRule::Ghosted(Some(OwnText::CreationNotAllowed)),
            Self::Official => ButtonRule::Ghosted(Some(OwnText::OfficialCreationUnavailable)),
        }
    }

    /// The Delete button (§7.2): enabled on the scenario feed while a character is
    /// selected; ghosted (never sends) on the plain sandbox and the test server; on another
    /// backend enabled while selected until a refusal (`refused`) ghosts it for the rest of
    /// the session.
    #[must_use]
    pub fn delete_button(&self, selected: bool, refused: bool) -> ButtonRule {
        let ghost = ButtonRule::Ghosted(Some(OwnText::DeletionNotSupported));
        match self {
            Self::Official => ButtonRule::Ghosted(Some(OwnText::OfficialDeletionUnavailable)),
            Self::PlainSandbox | Self::TestServer { .. } => ghost,
            _ if refused => ghost,
            Self::ScenarioFeed | Self::Other(_) if selected => ButtonRule::Enabled,
            Self::ScenarioFeed | Self::Other(_) => ButtonRule::Ghosted(None),
        }
    }

    /// Whether the session carries a character's look itself, so the screens need not keep
    /// the choices in the settings (§7.1 Tessera rule, decision 0025 D2): the test server
    /// at minor 1 and higher, and the scenario feed (its looks are scripted). The plain
    /// sandbox keeps looks for its lifetime only, so its creations are kept in the settings
    /// for later runs.
    #[must_use]
    pub fn carries_look(&self) -> bool {
        match self {
            Self::ScenarioFeed => true,
            Self::TestServer { minor, .. } => minor.is_some_and(|m| m >= 1),
            Self::PlainSandbox | Self::Official | Self::Other(_) => false,
        }
    }

    /// The key the settings file stores creation choices under: `explicit` when the caller
    /// knows the server (a test server's connect string, which the client's start-up
    /// passes as `ScreenOptions::server_key`), else [`SANDBOX_KEY`] for the plain sandbox
    /// and [`SCENARIO_KEY`] for the scenario feed. A test server without an explicit key
    /// has none (`None`: its choices are neither kept nor applied), since its display name
    /// (by default the same for every server) does not tell servers apart. An official
    /// session has none, whatever the caller passes.
    #[must_use]
    pub fn settings_key(&self, explicit: Option<&str>) -> Option<String> {
        if *self == Self::Official {
            // Creation is not offered there; nothing is kept for an official server.
            return None;
        }
        if let Some(k) = explicit.filter(|k| !k.is_empty()) {
            return Some(k.to_owned());
        }
        match self {
            Self::TestServer { .. } | Self::Official => None,
            Self::Other(n) => Some(format!("backend:{n}")),
            Self::ScenarioFeed => Some(SCENARIO_KEY.to_owned()),
            Self::PlainSandbox => Some(SANDBOX_KEY.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::sandbox::SandboxConfig;

    fn server(creation: Option<bool>, minor: Option<u16>) -> BackendKind {
        BackendKind::TestServer {
            creation,
            minor,
            server_name: None,
        }
    }

    #[test]
    fn new_button_rule() {
        assert!(BackendKind::ScenarioFeed.new_button().is_enabled());
        assert!(BackendKind::PlainSandbox.new_button().is_enabled());
        assert!(server(Some(true), Some(1)).new_button().is_enabled());
        assert_eq!(
            server(Some(false), Some(1)).new_button(),
            ButtonRule::Ghosted(Some(OwnText::CreationNotAllowed))
        );
        assert!(!server(None, None).new_button().is_enabled());
    }

    #[test]
    fn delete_button_rule() {
        assert!(
            BackendKind::ScenarioFeed
                .delete_button(true, false)
                .is_enabled()
        );
        assert!(
            !BackendKind::ScenarioFeed
                .delete_button(false, false)
                .is_enabled()
        );
        assert!(
            !BackendKind::ScenarioFeed
                .delete_button(true, true)
                .is_enabled()
        );
        assert_eq!(
            BackendKind::PlainSandbox.delete_button(true, false),
            ButtonRule::Ghosted(Some(OwnText::DeletionNotSupported))
        );
        assert!(
            !server(Some(true), Some(1))
                .delete_button(true, false)
                .is_enabled()
        );
        let other = BackendKind::Other("x".into());
        assert!(other.delete_button(true, false).is_enabled());
        assert!(!other.delete_button(true, true).is_enabled());
    }

    #[test]
    fn looks_and_keys() {
        assert!(server(Some(true), Some(1)).carries_look());
        assert!(!server(Some(true), Some(0)).carries_look());
        assert!(!BackendKind::PlainSandbox.carries_look());
        assert_eq!(
            BackendKind::PlainSandbox.settings_key(None).as_deref(),
            Some(SANDBOX_KEY)
        );
        assert_eq!(
            BackendKind::ScenarioFeed.settings_key(None).as_deref(),
            Some(SCENARIO_KEY),
            "the scenario feed never shares the plain sandbox's key"
        );
        assert_eq!(
            BackendKind::PlainSandbox
                .settings_key(Some("tessera://h:1"))
                .as_deref(),
            Some("tessera://h:1")
        );
        let named = BackendKind::TestServer {
            creation: None,
            minor: None,
            server_name: Some("Test".into()),
        };
        assert_eq!(
            named.settings_key(None),
            None,
            "a display name does not identify a server"
        );
        assert_eq!(
            named.settings_key(Some("tessera://h:1")).as_deref(),
            Some("tessera://h:1")
        );
    }

    #[test]
    fn official_sessions_ghost_creation_and_deletion() {
        let o = BackendKind::Official;
        assert_eq!(
            o.new_button(),
            ButtonRule::Ghosted(Some(OwnText::OfficialCreationUnavailable))
        );
        assert_eq!(
            o.delete_button(true, false),
            ButtonRule::Ghosted(Some(OwnText::OfficialDeletionUnavailable))
        );
        assert!(!o.carries_look());
        assert_eq!(o.settings_key(None), None, "choices are never kept");
        assert_eq!(o.settings_key(Some("x")), None);
    }

    #[test]
    fn detects_the_plain_sandbox() {
        let s = Session::new(SandboxBackend::new(SandboxConfig::default()).unwrap());
        assert_eq!(BackendKind::of(&s), BackendKind::PlainSandbox);
    }
}
