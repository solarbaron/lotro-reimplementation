-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a map-style window (shape of plugin-api-usage.md §9.2 #1).
import "Turbine"
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Tess.Atlas.Util"

state = { ticks = 0, menu = {}, keys = 0, chat = 0, centered = 0 }
settings = { visible = true, zoom = 1 }

window = Turbine.UI.Lotro.Window()
window:SetText("Atlas")
window:SetSize(500, 400)
window:SetPosition(50, 50)
window:SetResizable(true)

map = Turbine.UI.Control()
map:SetParent(window)
map:SetPosition(10, 40)
map:SetSize(256, 256)
map:SetBackground("Tess/Atlas/Resources/map.tga")
map:SetStretchMode(1)
map:SetSize(300, 300)
map:SetVisible(true)

pattern = Turbine.UI.Control()
pattern:SetParent(window)
pattern:SetPosition(320, 40)
pattern:SetSize(64, 64)
pattern:SetBackground("Tess/Atlas/Resources/tile.jpg")
pattern:SetStretchMode(2)
pattern:SetVisible(true)

caption = Turbine.UI.Label()
caption:SetParent(window)
caption:SetPosition(320, 120)
caption:SetSize(170, 40)
caption:SetMarkupEnabled(true)
caption:SetText("<rgb=#FFCC00>Atlas</rgb> ready")
caption:SetVisible(true)

regions = Turbine.UI.TreeView()
regions:SetParent(window)
regions:SetPosition(320, 170)
regions:SetSize(170, 150)
regions:SetVisible(true)
for _, name in ipairs({ "North", "South" }) do
  local node = Turbine.UI.TreeNode()
  node:SetSize(160, 18)
  node:SetVisible(true)
  local text = Turbine.UI.Label()
  text:SetParent(node)
  text:SetSize(160, 18)
  text:SetText(name)
  text:SetMouseVisible(false)
  text:SetVisible(true)
  regions:GetNodes():Add(node)
  for i = 1, 3 do
    local child = Turbine.UI.TreeNode()
    child:SetSize(140, 18)
    child:SetVisible(true)
    node:GetChildNodes():Add(child)
  end
end

menu = Turbine.UI.ContextMenu()
local center = Turbine.UI.MenuItem("Center")
center.Click = function() state.menu[#state.menu + 1] = "Center" end
local zoom = Turbine.UI.MenuItem("Zoom")
for _, z in ipairs({ 1, 2, 4 }) do
  local item = Turbine.UI.MenuItem("x" .. z, true, z == settings.zoom)
  item.Click = function() settings.zoom = z state.menu[#state.menu + 1] = "x" .. z end
  zoom:GetItems():Add(item)
end
menu:GetItems():Add(center)
menu:GetItems():Add(zoom)
map.MouseClick = function(sender, args)
  if args.Button == Turbine.UI.MouseButton.Right then
    menu:ShowMenu()
  end
end

slot = Turbine.UI.Lotro.Quickslot()
slot:SetParent(window)
slot:SetPosition(10, 350)
slot:SetSize(32, 32)
slot:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, "/atlas center"))
slot:SetVisible(true)

searchButton = Turbine.UI.Lotro.Button()
searchButton:SetParent(window)
searchButton:SetPosition(60, 350)
searchButton:SetSize(100, 20)
searchButton:SetText("Search")
searchButton:SetVisible(true)
searchButton.Click = function()
  Turbine.PluginManager.LoadPlugin("AtlasSearch")
end

timer = Turbine.UI.Control()
timer:SetWantsUpdates(true)
timer.Update = function() state.ticks = state.ticks + 1 end

hotkeys = Turbine.UI.Control()
hotkeys:SetWantsKeyEvents(true)
hotkeys.KeyDown = function(sender, args)
  if args.Action == Turbine.UI.Lotro.Action.ToggleBags then
    state.keys = state.keys + 1
    window:SetVisible(not window:IsVisible())
  end
end

Turbine.Chat.Received = function(sender, args)
  if args.ChatType == Turbine.ChatType.Standard then
    state.chat = state.chat + 1
  end
end

local command = Turbine.ShellCommand()
function command:Execute(cmd, args)
  if args == "center" then
    state.centered = state.centered + 1
  else
    window:SetVisible(not window:IsVisible())
  end
end
function command:GetHelp() return "Atlas window" end
Turbine.Shell.AddCommand("atlas", command)

plugin.Load = function(self, args)
  local saved = Turbine.PluginData.Load(Turbine.DataScope.Account, "AtlasSettings")
  if type(saved) == "table" and saved.zoom then settings.zoom = saved.zoom end
  window:SetVisible(settings.visible)
  Log("loaded")
end
plugin.Unload = function(self, args)
  Turbine.PluginData.Save(Turbine.DataScope.Account, "AtlasSettings", settings)
end
