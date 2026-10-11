// SPDX-License-Identifier: GPL-3.0-or-later
//! The labels game code sets in the NPC windows, rendered from the install's `UI` string
//! table (`npc-windows.md` §8.3 Loot All labels, §9.2 coin counts and coin tooltips); every
//! other label keeps the layout's own text, and without an install the fallback widgets
//! show Tessera's own short texts ([`NpcLabel::fallback`]).

use tessera_formats::StringInfo;
use tessera_formats::string_info::StringSource;
use tessera_gamedata::npc::tokens;
use tessera_gamedata::tooltip::TextRef;
use tessera_text::Values;
use tessera_ui::game::npc::{CoinKind, NpcLabel, NpcTexts};

use super::data::NpcData;

/// [`NpcTexts`] over the install (module docs); with `None` every default.
#[derive(Debug, Clone, Copy)]
pub struct InstallTexts<'a> {
    data: Option<&'a NpcData>,
}

impl<'a> InstallTexts<'a> {
    /// The texts of `data` (Tessera's defaults without it).
    #[must_use]
    pub fn new(data: Option<&'a NpcData>) -> Self {
        Self { data }
    }

    /// Renders `token` of the `UI` table; `None` when it does not render.
    fn ui_token(&self, token: u32) -> Option<String> {
        let d = self.data?;
        let table = d.data().table(tessera_gamedata::npc::UI_TABLE)?;
        let info = StringInfo {
            source: StringSource::Table { token, table },
            args: None,
        };
        d.data()
            .render(TextRef::Info(&info), &Values::new())
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
    }
}

impl NpcTexts for InstallTexts<'_> {
    fn label(&self, label: NpcLabel) -> Option<String> {
        match label {
            NpcLabel::LootAll => self.ui_token(tokens::LOOT_ALL),
            NpcLabel::LootAllEmpty => self.ui_token(tokens::LOOT_EMPTY),
            _ => None,
        }
    }

    fn coin_count(&self, count: u64) -> String {
        self.data
            .and_then(|d| d.presenter().coin_text(count).ok().flatten())
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| count.to_string())
    }

    fn coin_tooltip(&self, kind: CoinKind) -> Option<String> {
        let i = match kind {
            CoinKind::Gold => 0,
            CoinKind::Silver => 1,
            CoinKind::Copper => 2,
        };
        self.ui_token(tokens::COIN_TOOLTIPS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_an_install() {
        let t = InstallTexts::new(None);
        assert_eq!(t.label(NpcLabel::LootAll), None);
        assert_eq!(t.coin_count(12), "12");
        assert_eq!(t.coin_tooltip(CoinKind::Gold), None);
    }
}
