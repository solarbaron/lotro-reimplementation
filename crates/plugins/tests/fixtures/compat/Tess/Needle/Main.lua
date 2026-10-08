-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a heading dial (shape of plugin-api-usage.md §9.2 #12):
-- computed imports, rotation about x and z, an alias quickslot, a handler-table helper.
local parts = { "Tess", "Needle", "Lang" }
import(table.concat(parts, "."))

needle = { heading = 0, updates = 0, callbacks = 0 }
function AddCallback(object, event, f)
  if object[event] == nil then object[event] = {} end
  if type(object[event]) == "table" then table.insert(object[event], f) else object[event] = { object[event], f } end
  return f
end
function RemoveCallback(object, event, f)
  if type(object[event]) == "table" then
    for i, v in ipairs(object[event]) do if v == f then table.remove(object[event], i) break end end
  end
end

needle.window = Turbine.UI.Window()
needle.window:SetSize(128, 128)
needle.window:SetPosition(1400, 100)
needle.window:SetBackground(ImagePath("dial"))
needle.window:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend)
needle.window:SetVisible(true)
needle.window:SetRotation({ x = 30, y = 0, z = 0 })
needle.window:SetRotation({ x = 0, y = 0, z = 90 })
needle.slot = Turbine.UI.Lotro.Quickslot()
needle.slot:SetParent(needle.window)
needle.slot:SetSize(32, 32)
needle.slot:SetPosition(48, 96)
needle.slot:SetShortcut(Turbine.UI.Lotro.Shortcut(Turbine.UI.Lotro.ShortcutType.Alias, "/needle refresh"))
needle.slot:SetVisible(true)
AddCallback(needle.window, "Update", function() needle.updates = needle.updates + 1 end)
AddCallback(needle.window, "Update", function() needle.callbacks = needle.callbacks + 1 end)
needle.window:SetWantsUpdates(true)
local cmd = Turbine.ShellCommand()
function cmd:Execute() needle.heading = (needle.heading + 90) % 360 needle.window:SetRotation({ x = 0, y = 0, z = needle.heading }) end
Turbine.Shell.AddCommand("needle", cmd)
Turbine.Plugin.Unload = function(self) Turbine.PluginData.Save(Turbine.DataScope.Character, "Needle", { heading = needle.heading }) end
