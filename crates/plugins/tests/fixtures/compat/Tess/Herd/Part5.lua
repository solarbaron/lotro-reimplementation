-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: part 5 of five descriptors sharing the default apartment
-- (shape of plugin-api-usage.md §9.2 #11).
import "Turbine.UI"
import "Turbine.UI.Lotro"

herd = herd or { windows = {}, drops = 0 }
local w = Turbine.UI.Lotro.GoldWindow()
w:SetText("Herd 5")
w:SetSize(180, 120)
w:SetPosition(20 + 5 * 30, 600 + 5 * 20)
local q = Turbine.UI.Lotro.Quickslot()
q:SetParent(w)
q:SetPosition(20, 40)
q:SetSize(36, 36)
q:SetVisible(true)
q.DragDrop = function(sender, args)
  local s = args.DragDropInfo:GetShortcut()
  if s == nil or s:GetType() == Turbine.UI.Lotro.ShortcutType.Alias then herd.drops = herd.drops + 1 end
end
w:SetVisible(true)
herd.windows[5] = w
