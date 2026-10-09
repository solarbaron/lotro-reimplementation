-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a helper loaded by AtlasMain into its own apartment.
import "Turbine.UI"
import "Turbine.UI.Lotro"

found = 0
queue = {}
win = Turbine.UI.Lotro.GoldWindow()
win:SetText("Search")
win:SetSize(260, 120)
win:SetPosition(600, 50)
input = Turbine.UI.Lotro.TextBox()
input:SetParent(win)
input:SetPosition(20, 40)
input:SetSize(150, 20)
input:SetVisible(true)
go = Turbine.UI.Lotro.Button()
go:SetParent(win)
go:SetPosition(180, 40)
go:SetSize(60, 20)
go:SetText("Go")
go:SetVisible(true)
go.Click = function()
  for i = 1, 5 do queue[#queue + 1] = input:GetText() .. i end
  win:SetWantsUpdates(true)
end
win.Update = function()
  local item = table.remove(queue, 1)
  if item then found = found + 1 else win:SetWantsUpdates(false) end
end
win:SetVisible(true)
