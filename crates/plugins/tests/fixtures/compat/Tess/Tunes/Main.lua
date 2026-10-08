-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a song list (shape of plugin-api-usage.md §9.2 #3), in the
-- default apartment.
import "Turbine.UI"
import "Turbine.UI.Lotro"

tunes = { wheel = 0, left = 0, played = {} }
local songs = Turbine.PluginData.Load(Turbine.DataScope.Account, "TunesLibrary")
if type(songs) ~= "table" then songs = { "Morning", "Rain", "Harbour", "Lanterns", "Ferry", "Embers", "Orchard" } end
local prefs = Turbine.PluginData.Load(Turbine.DataScope.Character, "TunesPrefs") or {}

tunes.window = Turbine.UI.Window()
tunes.window:SetSize(260, 200)
tunes.window:SetPosition(700, 400)
tunes.window:SetBackColor(Turbine.UI.Color(0.9, 0.05, 0.05, 0.1))
tunes.list = Turbine.UI.ListBox()
tunes.list:SetParent(tunes.window)
tunes.list:SetPosition(5, 5)
tunes.list:SetSize(250, 150)
tunes.list:SetVisible(true)
tunes.bar = Turbine.UI.Lotro.ScrollBar()
tunes.bar:SetOrientation(Turbine.UI.Orientation.Vertical)
tunes.bar:SetParent(tunes.window)
tunes.bar:SetPosition(245, 5)
tunes.bar:SetSize(10, 150)
tunes.bar:SetVisible(true)
tunes.list:SetVerticalScrollBar(tunes.bar)
for i, name in ipairs(songs) do
  local row = Turbine.UI.Label()
  row:SetSize(240, 30)
  row:SetText(i .. ". " .. name)
  row:SetVisible(true)
  row.MouseWheel = function(sender, args) tunes.wheel = tunes.wheel + args.Direction end
  tunes.list:AddItem(row)
end
tunes.list.SelectedIndexChanged = function(sender)
  local i = sender:GetSelectedIndex()
  tunes.selected = i
  tunes.slot:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, "/play " .. songs[i]))
end
tunes.slot = Turbine.UI.Lotro.Quickslot()
tunes.slot:SetParent(tunes.window)
tunes.slot:SetPosition(5, 160)
tunes.slot:SetSize(32, 32)
tunes.slot:SetVisible(true)
tunes.slot.DragLeave = function() tunes.left = tunes.left + 1 end
tunes.slot.DragDrop = function(sender, args) args.DragDropInfo:SetSuccessful(true) end
tunes.window:SetVisible(prefs.hidden ~= true)
local cmd = Turbine.ShellCommand()
function cmd:Execute(c, args) tunes.played[#tunes.played + 1] = args end
Turbine.Shell.AddCommand("play", cmd)
