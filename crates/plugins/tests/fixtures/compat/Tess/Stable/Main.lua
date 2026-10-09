-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a travel panel whose alias quickslots run a command another
-- plugin of the archive registers (shape of plugin-api-usage.md §9.2 #8).
import "Turbine.UI"
import "Turbine.UI.Lotro"

stable = { slots = {} }
stable.window = Turbine.UI.Window()
stable.window:SetSize(200, 120)
stable.window:SetPosition(1200, 600)
stable.window:SetBackground("Tess/Stable/compass.tga")
stable.window:SetStretchMode(2)
stable.arrow = Turbine.UI.Window()
stable.arrow:SetParent(stable.window)
stable.arrow:SetSize(32, 32)
stable.arrow:SetPosition(160, 10)
stable.arrow:SetBackground("Tess/Stable/compass.tga")
for i, dest in ipairs({ "harbour", "mill", "bridge" }) do
  local q = Turbine.UI.Lotro.Quickslot()
  q:SetParent(stable.window)
  q:SetPosition(10 + (i - 1) * 40, 60)
  q:SetSize(36, 36)
  q:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, "/waypoint " .. dest))
  q:SetVisible(true)
  stable.slots[i] = q
end
plugin.Load = function(self)
  stable.window:SetVisible(true)
  stable.arrow:SetVisible(true)
  stable.window:SetRotation({ x = 0, y = 0, z = 45 })
end
