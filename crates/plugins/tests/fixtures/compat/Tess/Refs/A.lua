-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: reference window A (shape of plugin-api-usage.md §9.2 #6).
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Tess.Refs.Common"

refA = { scale = nil, dropped = 0, loaded = nil }
refA.window = MakeWindow("Places", 20)
for i = 1, 12 do
  local row = Turbine.UI.Control()
  row:SetSize(270, 24)
  row:SetBackground("Tess/Refs/Art/row.tga")
  row:SetStretchMode(1)
  row:SetVisible(true)
  refA.window.list:AddItem(row)
end
refA.banner = Turbine.UI.Control()
refA.banner:SetParent(refA.window)
refA.banner:SetPosition(15, 20)
refA.banner:SetSize(64, 16)
refA.banner:SetBackground("Tess/Refs/Art/row.tga")
refA.banner:SetStretchMode(2)
refA.banner:SetVisible(true)
refA.window.UIScaleChanged = function(sender, args) refA.scale = args.Value end
refA.window:RegisterForGlobalScaling()

refA.slot = Turbine.UI.Lotro.Quickslot()
refA.slot:SetParent(refA.window)
refA.slot:SetPosition(250, 5)
refA.slot:SetSize(32, 32)
refA.slot:SetAllowDrop(true)
refA.slot:SetVisible(true)
refA.slot.DragDrop = function(sender, args)
  refA.dropped = refA.dropped + 1
  local s = args.DragDropInfo and args.DragDropInfo:GetShortcut()
  if s then sender:SetShortcut(s) end
end

plugin.GetOptionsPanel = function(self) return OptionsPanel("places") end
plugin.Load = function(self)
  Turbine.PluginData.Load(Turbine.DataScope.Character, "RefA", function(data)
    refA.loaded = data or false
  end)
  refA.window:SetVisible(true)
end
plugin.Unload = function(self)
  Turbine.PluginData.Save(Turbine.DataScope.Server, "RefA", { opened = true }, function(ok) end)
end
