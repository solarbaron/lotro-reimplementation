-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: subclasses native classes with the class helper Tessera
-- provides (decision 0016) and imports one of its on-demand packages.
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Turbine.UI.Extensions"

classy = { clicks = 0 }
Panel = class(Turbine.UI.Lotro.Window)
function Panel:Constructor(title)
  Turbine.UI.Lotro.Window.Constructor(self)
  self:SetText(title)
  self:SetSize(240, 160)
  self:SetPosition(1300, 300)
  self.button = CountButton(self)
  self:SetVisible(true)
end
function Panel:Closing(args) args.Cancel = classy.keepOpen == true end

CountButton = class(Turbine.UI.Lotro.Button)
function CountButton:Constructor(parent)
  Turbine.UI.Lotro.Button.Constructor(self)
  self:SetParent(parent)
  self:SetPosition(20, 60)
  self:SetSize(120, 20)
  self:SetText("Count")
  self:SetVisible(true)
end
function CountButton:Click() classy.clicks = classy.clicks + 1 self:SetText("Count " .. classy.clicks) end

classy.panel = Panel("Classy")
