-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin helper shared by RefA and RefB (one apartment).
function MakeWindow(title, x)
  local w = Turbine.UI.Lotro.Window()
  w:SetText(title)
  w:SetSize(300, 220)
  w:SetPosition(x, 300)
  local list = Turbine.UI.ListBox()
  list:SetParent(w)
  list:SetPosition(15, 40)
  list:SetSize(270, 160)
  list:SetOrientation(Turbine.UI.Orientation.Horizontal)
  list:SetMaxItemsPerLine(1)
  list:SetVisible(true)
  w.list = list
  return w
end

function OptionsPanel(name)
  local panel = Turbine.UI.Control()
  panel:SetSize(300, 100)
  local check = Turbine.UI.Lotro.CheckBox()
  check:SetParent(panel)
  check:SetSize(200, 20)
  check:SetText("Show " .. name)
  check:SetChecked(true)
  check:SetVisible(true)
  panel.check = check
  return panel
end
