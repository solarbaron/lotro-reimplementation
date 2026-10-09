-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: alerts (shape of plugin-api-usage.md §9.2 #4).
import "Turbine.UI"
import "Turbine.UI.Lotro"
import "Turbine.Gameplay"

watch = { alerts = 0, combat = false, menu = {} }
local player = Turbine.Gameplay.LocalPlayer.GetInstance()
if player then watch.combat = player:IsInCombat() end

watch.window = Turbine.UI.Lotro.Window()
watch.window:SetText("Watch")
watch.window:SetSize(320, 260)
watch.window:SetPosition(900, 50)

-- Three stretch modes and a game image DID.
for mode = 1, 3 do
  local c = Turbine.UI.Control()
  c:SetParent(watch.window)
  c:SetPosition(15 + (mode - 1) * 70, 40)
  c:SetSize(64, 64)
  c:SetBackground(0x41000000 + mode)
  c:SetStretchMode(mode)
  c:SetVisible(true)
end

watch.tree = Turbine.UI.TreeView()
watch.tree:SetParent(watch.window)
watch.tree:SetPosition(15, 110)
watch.tree:SetSize(290, 130)
watch.tree:SetVisible(true)
local groups = { Combat = { "Low morale", "Interrupted" }, Chat = { "Tell", "Say" } }
for group, entries in pairs(groups) do
  local node = Turbine.UI.TreeNode()
  node:SetSize(280, 20)
  node:SetVisible(true)
  node.group = group
  watch.tree:GetNodes():Add(node)
  for _, e in ipairs(entries) do
    local leaf = Turbine.UI.TreeNode()
    leaf:SetSize(260, 20)
    leaf:SetVisible(true)
    leaf.entry = e
    node:GetChildNodes():Add(leaf)
  end
end
watch.tree:SetSortMethod(function(a, b) return (a.group or a.entry) < (b.group or b.entry) end)
watch.tree.SelectedNodeChanged = function(sender) watch.selected = sender:GetSelectedNode() end

watch.menu = Turbine.UI.ContextMenu()
local options = Turbine.UI.MenuItem("Options")
local sound = Turbine.UI.MenuItem("Sound", true, true)
sound.Click = function(sender) sender:SetChecked(not sender:IsChecked()) watch.sound = sender:IsChecked() end
options:GetItems():Add(sound)
watch.menu:GetItems():Add(options)
local reload = Turbine.UI.MenuItem("Reload")
reload.Click = function() Turbine.PluginManager.LoadPlugin("WatchReloader") end
watch.menu:GetItems():Add(reload)
watch.window.MouseClick = function(sender, args)
  if args.Button == Turbine.UI.MouseButton.Right then watch.menu:ShowMenuAt(args.X + 900, args.Y + 50) end
end

watch.slot = Turbine.UI.Lotro.Quickslot()
watch.slot:SetParent(watch.window)
watch.slot:SetPosition(240, 40)
watch.slot:SetSize(32, 32)
watch.slot:SetVisible(true)
watch.slot.DragDrop = function(sender, args)
  if args.DragDropInfo:IsA(Turbine.UI.Lotro.DragDropInfo) then watch.dropped = true end
end

Turbine.Chat.Received = function(sender, args)
  watch.alerts = watch.alerts + 1
end
watch.window:SetVisible(true)
