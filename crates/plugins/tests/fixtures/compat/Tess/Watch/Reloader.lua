-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: unloads and reloads the Watch apartment (a script state cannot
-- unload itself while it runs, lua-plugin-api.md §6.3).
import "Turbine"
reloader = Turbine.UI.Control()
reloader:SetWantsUpdates(true)
local step = 0
reloader.Update = function(sender)
  step = step + 1
  if step == 1 then
    Turbine.PluginManager.UnloadScriptState("Watch")
  elseif step == 2 then
    Turbine.PluginManager.LoadPlugin("Watch")
  elseif step == 3 then
    sender:SetWantsUpdates(false)
  end
end
