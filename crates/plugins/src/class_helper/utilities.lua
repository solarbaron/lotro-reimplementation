-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Tessera's built-in class helper, part 2: the utilities plugins depend on (fading
-- windows, the quickslot replacement, the table dump, the interpolator, rounding, table
-- comparison, the log) and the install step of each built-in package name.
--
-- Written for Tessera from docs/specs/plugin-class-helper.md §5 and §7 (decision 0016).
-- Compiled once per apartment with an empty environment; it receives the primitives
-- table and the table returned by class.lua, and returns the install functions, keyed by
-- the names the host's package table uses. Each install function takes the namespace
-- table of the package being installed.

local P, core = ...

local type, error, tostring, pairs, rawset, rawequal =
  P.type, P.error, P.tostring, P.pairs, P.rawset, P.rawequal
local floor, sort = P.floor, P.sort
local byteLess, clock = P.byte_less, P.clock
local G = P.G
local declare = core.declare

-- "The game time" (§7.1): looked up in the apartment's global table at each call.
local function gameTime()
  return G.Turbine.Engine.GetGameTime()
end

-- One chat line through the plugin API's own WriteLine, looked up at each call.
local function writeLine(text)
  G.Turbine.Shell.WriteLine(text)
end

---------------------------------------------------------------------------------------
-- Fading windows (§7.1)
---------------------------------------------------------------------------------------

local function fadingWindow(B, name)
  local W = declare(B, name)

  rawset(W, "Constructor", function(self)
    B.Constructor(self)
    self:SetFadeSpeed(0.2)
    self:SetOpacity(1)
  end)

  rawset(W, "SetFadeSpeed", function(self, seconds)
    self.fadeSpeed = 1 / seconds
  end)

  -- The writes happen in this order, so an error in the game-time read or in B's
  -- GetOpacity leaves the later fields as they were.
  rawset(W, "SetOpacity", function(self, value)
    self.realOpacity = value
    self.currentTime = gameTime()
    self.currentOpacity = B.GetOpacity(self)
    self.targetOpacity = value
    if self.targetOpacity ~= self.currentOpacity then
      self:SetWantsUpdates(true)
    end
  end)

  rawset(W, "SetVisible", function(self, visible)
    if rawequal(visible, true) then
      B.SetOpacity(self, 0)
      B.SetVisible(self, true)
      self:SetOpacity(self.realOpacity)
    else
      self.hideOnClose = true
      local keep = self.realOpacity
      self:SetOpacity(0)
      self.realOpacity = keep
    end
  end)

  rawset(W, "Update", function(self)
    local now = gameTime()
    local step = (now - self.currentTime) * self.fadeSpeed
    self.currentTime = now
    local current, target = self.currentOpacity, self.targetOpacity
    local opacity
    if current < target then
      opacity = current + step
      if opacity > target then
        self:SetWantsUpdates(false)
        opacity = target
      end
    else
      opacity = current - step
      if opacity < target then
        self:SetWantsUpdates(false)
        opacity = target
        if self.hideOnClose == true then
          B.SetVisible(self, false)
          self.hideOnClose = false
        end
      end
    end
    self.currentOpacity = opacity
    B.SetOpacity(self, opacity)
  end)

  return W
end

-- A native class the built-in package builds on, read from the apartment's globals.
local function nativeClass(value, path)
  if type(value) ~= "table" then
    error("class helper: the native class " .. path .. " is not available", 0)
  end
  return value
end

---------------------------------------------------------------------------------------
-- Turbine.Debug.Table.Dump (§7.3)
---------------------------------------------------------------------------------------

local function byText(a, b)
  return byteLess(a.text, b.text)
end

local function dumpTable(t, indent, seen)
  if t == nil then
    writeLine(indent .. "(nil)")
    return
  end
  seen[t] = true
  local entries = {}
  for k, v in pairs(t) do
    entries[#entries + 1] = { key = k, value = v }
  end
  for i = 1, #entries do
    entries[i].text = tostring(entries[i].key)
  end
  sort(entries, byText)
  for i = 1, #entries do
    local e = entries[i]
    writeLine(indent .. i .. ": " .. e.text)
    local v = e.value
    if type(v) == "table" and not seen[v] then
      dumpTable(v, indent .. "  ", seen)
    end
  end
end

local function Dump(t, indent)
  if indent == nil or indent == false then
    indent = "  "
  end
  dumpTable(t, indent, {})
end

---------------------------------------------------------------------------------------
-- Turbine.Animation.LerpValue (§7.4)
---------------------------------------------------------------------------------------

local function lerpValue(namespace)
  local L = declare(nil, "Turbine.Animation.LerpValue")
  rawset(L, "Constructor", function(self)
    self.value = 0
    self.target = 0
    self.rate = 1
  end)
  rawset(L, "GetValue", function(self)
    return self.value
  end)
  rawset(L, "GetTargetValue", function(self)
    return self.target
  end)
  rawset(L, "SetValue", function(self, x)
    self.target = x
  end)
  rawset(L, "SetValueImmediate", function(self, x)
    self.value = x
    self.target = x
  end)
  rawset(L, "SetRate", function(self, r)
    self.rate = r
  end)
  -- Reads `rate` of the package's namespace, not of the instance (the original's
  -- behaviour, §7.4).
  rawset(L, "GetRate", function()
    return namespace.rate
  end)
  rawset(L, "NeedsUpdate", function(self)
    return self.value ~= self.target
  end)
  rawset(L, "Update", function(self, d)
    local value, target = self.value, self.target
    if value < target then
      value = value + self.rate * d
      if value > target then
        value = target
      end
      self.value = value
    elseif value > target then
      value = value - self.rate * d
      if value < target then
        value = target
      end
      self.value = value
    end
  end)
  return L
end

---------------------------------------------------------------------------------------
-- math.round, Turbine.Table.AreEqual, Turbine.Log (§7.5–§7.7)
---------------------------------------------------------------------------------------

local function round(x)
  return floor(x + 0.5)
end

-- A plain table: lookup finds neither a GetType function nor a Constructor function. A
-- GetType function found is called (a native one raises without an object, §7.6).
local function isPlain(x)
  if type(x) ~= "table" then
    return false
  end
  local getType = x.GetType
  if type(getType) == "function" then
    getType()
    return false
  end
  if type(x.Constructor) == "function" then
    return false
  end
  return true
end

local function AreEqual(a, b)
  local plainA = isPlain(a)
  local plainB = isPlain(b)
  return plainA and plainB
end

local function logLine(label, message)
  writeLine(clock() .. " " .. label .. ": " .. tostring(message))
end

local function logDebug(_, message)
  logLine("debug", message)
end

local function logInformation(_, message)
  logLine("info", message)
end

---------------------------------------------------------------------------------------
-- Installs (§5.3): what each built-in name stores once its imports have run.
---------------------------------------------------------------------------------------

local installs = {}

function installs.class()
  G.class = core.class
end

function installs.type(ns)
  rawset(ns, "Type", core.TypeTable)
end

function installs.math_round()
  G.math.round = round
end

function installs.table(ns)
  rawset(ns, "Table", { AreEqual = AreEqual })
end

function installs.log(ns)
  rawset(ns, "Log", { Debug = logDebug, Information = logInformation })
end

function installs.debug_stack(ns)
  rawset(ns, "Stack", {})
end

function installs.debug_table(ns)
  rawset(ns, "Table", { Dump = Dump })
end

function installs.lerp_value(ns)
  rawset(ns, "LerpValue", lerpValue(ns))
end

function installs.simple_window(ns)
  local B = nativeClass(G.Turbine.UI.Window, "Turbine.UI.Window")
  rawset(ns, "SimpleWindow", fadingWindow(B, "Turbine.UI.Extensions.SimpleWindow"))
end

function installs.window(ns)
  local B = nativeClass(G.Turbine.UI.Lotro.GoldWindow, "Turbine.UI.Lotro.GoldWindow")
  rawset(ns, "Window", fadingWindow(B, "Turbine.UI.Extensions.Window"))
end

-- §7.2: every quickslot of the apartment checks its visibility, sets the shortcut with
-- the earlier function, then passes the same value to SetVisible.
function installs.quickslot()
  local Q = nativeClass(G.Turbine.UI.Lotro.Quickslot, "Turbine.UI.Lotro.Quickslot")
  local earlier = Q.SetShortcut
  Q.SetShortcut = function(q, shortcut)
    q:IsVisible()
    earlier(q, shortcut)
    q:SetVisible(shortcut)
  end
end

return installs
