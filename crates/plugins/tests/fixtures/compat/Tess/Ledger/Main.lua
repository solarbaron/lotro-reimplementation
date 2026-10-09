-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a log window (shape of plugin-api-usage.md §9.2 #5).
import "Turbine.UI"
import "Turbine.UI.Lotro"

ledger = { entries = 0 }
ledger.data = Turbine.PluginData.Load(Turbine.DataScope.Server, "Ledger") or { totals = {} }
ledger.window = Turbine.UI.Lotro.Window()
ledger.window:SetText("Ledger")
ledger.window:SetSize(360, 240)
ledger.window:SetPosition(400, 500)
ledger.log = Turbine.UI.Label()
ledger.log:SetParent(ledger.window)
ledger.log:SetPosition(15, 40)
ledger.log:SetSize(330, 150)
ledger.log:SetMarkupEnabled(true)
ledger.log:SetFont(Turbine.UI.Lotro.Font.Verdana12)
ledger.log:SetForeColor(Turbine.UI.Color.Wheat)
ledger.log:SetVisible(true)
ledger.bg = Turbine.UI.Control()
ledger.bg:SetParent(ledger.window)
ledger.bg:SetPosition(15, 40)
ledger.bg:SetSize(32, 32)
ledger.bg:SetBackground("Tess/Ledger/back.tga")
ledger.bg:SetStretchMode(1)
ledger.bg:SetSize(330, 150)
ledger.bg:SetZOrder(-1)
ledger.bg:SetVisible(true)
ledger.clear = Turbine.UI.Lotro.Button()
ledger.clear:SetParent(ledger.window)
ledger.clear:SetPosition(15, 200)
ledger.clear:SetSize(80, 20)
ledger.clear:SetText("Clear")
ledger.clear:SetVisible(true)
ledger.clear.Click = function()
  ledger.log:SetText("")
  ledger.entries = 0
end
ledger.slot = Turbine.UI.Lotro.Quickslot()
ledger.slot:SetParent(ledger.window)
ledger.slot:SetPosition(110, 195)
ledger.slot:SetSize(32, 32)
ledger.slot:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, "/ledger add"))
ledger.slot:SetVisible(true)
local cmd = Turbine.ShellCommand()
function cmd:Execute(c, args)
  ledger.entries = ledger.entries + 1
  ledger.log:AppendText("<rgb=#FFD700>" .. ledger.entries .. "</rgb> " .. (args or "") .. "\n")
end
Turbine.Shell.AddCommand("ledger", cmd)
ledger.window:SetVisible(true)
plugin.Unload = function()
  Turbine.PluginData.Save(Turbine.DataScope.Account, "LedgerAccount", { last = ledger.entries })
  Turbine.PluginData.Save(Turbine.DataScope.Server, "Ledger", ledger.data)
end
