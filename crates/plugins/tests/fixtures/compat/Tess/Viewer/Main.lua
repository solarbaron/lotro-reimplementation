-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: an image-DID browser (shape of plugin-api-usage.md §9.2 #14):
-- a grid of game images in a list box, stretch modes, an import done on demand, a large
-- data table.
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Tess.Viewer.Data"

viewer = { page = 0, extra = false, picked = nil }
viewer.window = Turbine.UI.Lotro.Window()
viewer.window:SetText("Viewer")
viewer.window:SetSize(420, 380)
viewer.window:SetPosition(60, 420)
viewer.grid = Turbine.UI.ListBox()
viewer.grid:SetParent(viewer.window)
viewer.grid:SetPosition(15, 40)
viewer.grid:SetSize(390, 280)
viewer.grid:SetOrientation(Turbine.UI.Orientation.Horizontal)
viewer.grid:SetMaxItemsPerLine(6)
viewer.grid:SetVisible(true)
viewer.label = Turbine.UI.Label()
viewer.label:SetParent(viewer.window)
viewer.label:SetPosition(15, 325)
viewer.label:SetSize(250, 20)
viewer.label:SetMarkupEnabled(true)
viewer.label:SetVisible(true)

function ShowPage(page)
  viewer.page = page
  viewer.grid:ClearItems()
  for i = 1, 24 do
    local did = ImageIds[page * 24 + i]
    if did == nil then break end
    local cell = Turbine.UI.Control()
    cell:SetSize(64, 64)
    cell:SetBackground(did)
    cell:SetStretchMode(i % 2 == 0 and 1 or 2)
    cell:SetVisible(true)
    cell.did = did
    viewer.grid:AddItem(cell)
  end
  viewer.label:SetText(string.format("<rgb=#C0C0FF>Page %d</rgb>", page + 1))
end
viewer.grid.SelectedIndexChanged = function(sender)
  local item = sender:GetSelectedItem()
  viewer.picked = item and item.did
end
viewer.next = Turbine.UI.Lotro.Button()
viewer.next:SetParent(viewer.window)
viewer.next:SetPosition(330, 325)
viewer.next:SetSize(75, 20)
viewer.next:SetText("Next")
viewer.next:SetVisible(true)
viewer.next.Click = function()
  if not viewer.extra then import "Tess.Viewer.Extra" viewer.extra = true end
  ShowPage(viewer.page + 1)
end
ShowPage(0)
viewer.window:SetVisible(true)
