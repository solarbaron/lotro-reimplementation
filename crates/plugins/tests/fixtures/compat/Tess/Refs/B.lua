-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: reference window B in the same apartment as A (shape of #7).
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Tess.Refs.Common"

refB = { lines = 0 }
refB.window = MakeWindow("Notes", 340)
local filter = Turbine.UI.Lotro.TextBox()
filter:SetParent(refB.window)
filter:SetPosition(15, 20)
filter:SetSize(200, 18)
filter:SetVisible(true)
refB.filter = filter
-- A handler table shared with A through the same singleton (plugin-api-usage.md §6.1).
local handlers = Turbine.Chat.Received
if type(handlers) ~= "table" then handlers = {} Turbine.Chat.Received = handlers end
handlers[#handlers + 1] = function(sender, args) refB.lines = refB.lines + 1 end
plugin.GetOptionsPanel = function(self) return OptionsPanel("notes") end
plugin.Load = function(self) refB.window:SetVisible(true) end
