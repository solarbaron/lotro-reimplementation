-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin: a digest browser (shape of plugin-api-usage.md §9.2 #13): all
-- three data scopes, label markup, image DIDs, .tga and .jpg files, a tree.
import "Turbine.UI"
import "Turbine.UI.Lotro"

digest = { opened = 0 }
digest.account = Turbine.PluginData.Load(Turbine.DataScope.Account, "DigestA") or {}
digest.server = Turbine.PluginData.Load(Turbine.DataScope.Server, "DigestS") or {}
digest.character = Turbine.PluginData.Load(Turbine.DataScope.Character, "DigestC") or { seen = 0 }
digest.character.seen = (digest.character.seen or 0) + 1

digest.window = Turbine.UI.Lotro.Window()
digest.window:SetText("Digest")
digest.window:SetSize(420, 320)
digest.window:SetPosition(500, 100)
digest.header = Turbine.UI.Control()
digest.header:SetParent(digest.window)
digest.header:SetPosition(15, 35)
digest.header:SetSize(390, 40)
digest.header:SetBackground("Tess/Digest/header.jpg")
digest.header:SetVisible(true)
digest.icon = Turbine.UI.Control()
digest.icon:SetParent(digest.window)
digest.icon:SetPosition(15, 80)
digest.icon:SetSize(32, 32)
digest.icon:SetBackground(0x41007E7B)
digest.icon:SetBlendMode(Turbine.UI.BlendMode.AlphaBlend)
digest.icon:SetVisible(true)
digest.text = Turbine.UI.Label()
digest.text:SetParent(digest.window)
digest.text:SetPosition(55, 80)
digest.text:SetSize(350, 40)
digest.text:SetMarkupEnabled(true)
digest.text:SetText("<rgb=#FF8000>Week</rgb> <u>summary</u>")
digest.text:SetVisible(true)
digest.tree = Turbine.UI.TreeView()
digest.tree:SetParent(digest.window)
digest.tree:SetPosition(15, 125)
digest.tree:SetSize(390, 180)
digest.tree:SetIndentationWidth(16)
digest.tree:SetVisible(true)
for d = 1, 3 do
  local node = Turbine.UI.TreeNode()
  node:SetSize(380, 20)
  node:SetBackground("Tess/Digest/row.tga")
  node:SetVisible(true)
  digest.tree:GetNodes():Add(node)
  for e = 1, 4 do
    local leaf = Turbine.UI.TreeNode()
    leaf:SetSize(360, 18)
    leaf:SetVisible(true)
    leaf.MouseDoubleClick = function() digest.opened = digest.opened + 1 end
    node:GetChildNodes():Add(leaf)
  end
end
digest.window:SetVisible(true)
plugin.Unload = function()
  Turbine.PluginData.Save(Turbine.DataScope.Character, "DigestC", digest.character)
  Turbine.PluginData.Save(Turbine.DataScope.Server, "DigestS", digest.server)
  Turbine.PluginData.Save(Turbine.DataScope.Account, "DigestA", digest.account)
end
