-- SPDX-License-Identifier: GPL-3.0-or-later
--
-- Tessera's built-in class helper, part 1: the `class` function, the classes it makes,
-- their type objects, `IsA`, `ToString` and `GetUniqueID`.
--
-- Written for Tessera from docs/specs/plugin-class-helper.md §6 (decision 0016). The
-- host compiles this chunk once per apartment with an empty environment and passes it
-- one table of primitives; every name used below is a local, so the chunk never reads
-- the plugins' globals. The registry tables are locals of this chunk: no plugin can reach
-- them (the plugin sandbox has no `debug` library).
--
-- Returns the table the second part (utilities.lua) and the host's installs use.

local P = ...

local type, select, error, rawequal, rawset, pairs, setmetatable, getfenv =
  P.type, P.select, P.error, P.rawequal, P.rawset, P.pairs, P.setmetatable, P.getfenv
local find, sub = P.find, P.sub
local uid = P.uid

-- The registry (§6.13). Lua 5.1 has no ephemerons: it marks every value of a weak-keyed
-- table, so an entry whose value can reach its own key is never cleared. Hence no entry
-- keyed by a class holds anything but a string, and a class's base lives only in its own
-- closures and metatable, never in the registry, so a base or instance that refers back to
-- its subclass does not keep either alive.
--
-- The lifetime rules are those of §6.13.1. A type object keeps its record (class, base,
-- mixin list) alive through `infoOf` (rule 1), so the §6.8 answers stay correct while a
-- plugin holds only the type object. A class keeps its base alive through its metatable
-- and its `GetType` closure, but not its type object (rule 2): an unheld type object is
-- collected and the next `GetType` call makes a new one. Otherwise nothing here keeps a
-- class alive (rule 3). When the class, its base or the mixin list refer to the type
-- object, the record reaches its key and may stay until the apartment closes (rule 4).
local function weak(mode)
  return setmetatable({}, { __mode = mode })
end
local nameOf = weak("k")    -- Tessera class -> its full name ("" when unnamed)
local ownerOf = weak("kv")  -- a class's own GetType function -> that class
local typeOf = weak("kv")   -- Tessera class -> its type object, while one is alive
local infoOf = weak("k")    -- Tessera type object -> { class, base, hasBase, mixins }
local idOf = weak("k")      -- table -> cached unique ID text

-- Unique ID: the address text of the table, computed once (§6.9). The metatable is
-- never touched.
local function idText(v)
  local id = idOf[v]
  if id == nil then
    id = uid(v)
    idOf[v] = id
  end
  return id
end

local function describe(v)
  return type(v)
end

-- A query of a type object (§6.8): look the method up on it and call it with the type
-- object alone. `lvl` is the error level of the caller to blame.
local function query(t, method, lvl)
  if type(t) ~= "table" then
    error("class helper: a type object must be a table, got a " .. describe(t), lvl)
  end
  local f = t[method]
  if type(f) ~= "function" then
    error("class helper: the type object does not answer " .. method, lvl)
  end
  return f(t)
end

---------------------------------------------------------------------------------------
-- Type objects and the `Turbine.Type` table (§6.8)
---------------------------------------------------------------------------------------

local TypeTable = {}
local typeMeta = { __index = TypeTable }

-- The registry record of a Tessera type object; raises for any other value.
local function infoOfType(t)
  local info = infoOf[t]
  if info == nil then
    error("class helper: not a type object of this class helper", 3)
  end
  return info
end

local function classOfType(t)
  return infoOfType(t).class
end

function TypeTable.IsClass(t)
  infoOfType(t)
  return true
end

function TypeTable.IsFinal(t)
  infoOfType(t)
  return false
end

function TypeTable.IsMixin(t)
  infoOfType(t)
  return false
end

function TypeTable.GetBaseClass(t)
  local info = infoOfType(t)
  if info.hasBase then
    return info.base
  end
  return nil
end

function TypeTable.GetMixins(t)
  local info = infoOfType(t)
  local m = info.mixins
  if m == nil then
    m = {}
    info.mixins = m
  end
  return m
end

function TypeTable.GetClass(t)
  return classOfType(t)
end

function TypeTable.GetName(t)
  local name = nameOf[classOfType(t)]
  local last, from = nil, 1
  while true do
    local at = find(name, ".", from, true)
    if at == nil then
      break
    end
    last, from = at, at + 1
  end
  if last == nil then
    return nil
  end
  return sub(name, last + 1)
end

---------------------------------------------------------------------------------------
-- Members stored on new classes (§6.3, §6.7, §6.9)
---------------------------------------------------------------------------------------

local function GetUniqueID(v)
  if type(v) ~= "table" then
    error("GetUniqueID: expected a table, got a " .. describe(v), 2)
  end
  return idText(v)
end

-- `<named> <name> (<id>)`, or `<unnamed> (<id>)` when the name is empty.
local function named(namedKind, unnamedKind, name, id)
  if name ~= "" then
    return namedKind .. " " .. name .. " (" .. id .. ")"
  end
  return unnamedKind .. " (" .. id .. ")"
end

local function ToString(v)
  if type(v) ~= "table" then
    error("ToString: expected a table, got a " .. describe(v), 2)
  end
  local id = idText(v)
  local name = nameOf[v]
  if name ~= nil then
    return named("class", "class", name, id)
  end
  local getType = v.GetType
  if type(getType) == "function" then
    local owner = ownerOf[getType]
    if owner ~= nil then
      name = nameOf[owner]
      if name ~= nil then
        return named("instance of", "instance", name, id)
      end
    end
  end
  return "object (" .. id .. ")"
end

local function IsA(x, target)
  -- Step 1: the start type, by lookup on `x` (a string looks in its metatable's
  -- `__index`, a userdata in its own; other values have nothing to look up).
  local getType
  local kind = type(x)
  if kind == "table" or kind == "string" or kind == "userdata" then
    getType = x.GetType
  end
  if type(getType) ~= "function" then
    error("IsA: the object has no GetType function", 2)
  end
  local start = getType()

  -- Step 2: the target type.
  if type(target) ~= "table" then
    error("IsA: the target must be a class or an instance, got a " .. describe(target), 2)
  end
  local targetType
  local targetGetType = target.GetType
  if type(targetGetType) == "function" then
    targetType = targetGetType()
  elseif type(target.Constructor) == "function" then
    return false
  else
    error("IsA: the target is neither a class nor an instance", 2)
  end

  -- Step 3: a class target, by walking the bases.
  if query(targetType, "IsClass", 3) then
    local current, seen = start, {}
    while true do
      if rawequal(current, targetType) then
        return true
      end
      local base = query(current, "GetBaseClass", 3)
      seen[current] = true
      if base == nil or type(base) ~= "table" then
        return false
      end
      local baseGetType = base.GetType
      if type(baseGetType) ~= "function" then
        return false
      end
      local nextType = baseGetType()
      if type(nextType) == "table" and seen[nextType] then
        return false
      end
      current = nextType
    end
  end

  -- Step 4: a mixin target, through the start type's own mixin list.
  if query(targetType, "IsMixin", 3) then
    local mixins = query(start, "GetMixins", 3)
    if type(mixins) == "table" then
      for _, m in pairs(mixins) do
        if m == target then
          return true
        end
        if m:IsA(target) then
          return true
        end
      end
    end
    return false
  end

  -- Step 5.
  error("IsA: the target's type reports neither a class nor a mixin", 2)
end

---------------------------------------------------------------------------------------
-- Declaring classes (§6.1–§6.5, §6.10)
---------------------------------------------------------------------------------------

-- `tostring` of a class or an instance goes through its ToString method (§6.4).
local function viaToString(v)
  return v:ToString()
end

-- Classify the arguments of `class` (§6.1). Returns the base and whether there is one.
-- `lvl` is the error level of the code that called `class`.
local function classify(lvl, ...)
  local base, hasBase = nil, false
  for i = 1, select("#", ...) do
    local arg = (select(i, ...))
    if arg ~= nil then
      local isBase = false
      if type(arg) == "table" then
        local getType = arg.GetType
        if type(getType) == "function" then
          local t = getType()
          if not query(t, "IsClass", lvl + 1) then
            error("class: argument " .. i .. " is not a class", lvl)
          end
          if query(t, "IsFinal", lvl + 1) then
            error("class: argument " .. i .. " is a final class", lvl)
          end
          isBase = true
        elseif type(arg.Constructor) == "function" then
          isBase = true
        end
      end
      if not isBase then
        error("class: argument " .. i .. " (a " .. describe(arg)
          .. ") is neither a class nor a table with a Constructor", lvl)
      end
      if hasBase then
        error("class: more than one base class given", lvl)
      end
      base, hasBase = arg, true
    end
  end
  return base, hasBase
end

-- Make a class table with the given base (if any) and full name (§6.3–§6.5).
local function newClass(base, hasBase, name)
  local C = {}
  local instanceMeta = { __index = C, __tostring = viaToString }
  local classMeta = {
    __call = function(_, ...)
      local i = setmetatable({}, instanceMeta)
      local ctor = i.Constructor
      if type(ctor) == "function" then
        ctor(i, ...)
      end
      return i
    end,
    __tostring = viaToString,
  }
  if hasBase then
    classMeta.__index = base
  end
  setmetatable(C, classMeta)

  -- The base is an upvalue here (and the metatable's __index), never a registry value.
  local function GetType()
    local t = typeOf[C]
    if t == nil then
      t = setmetatable({}, typeMeta)
      infoOf[t] = { class = C, base = base, hasBase = hasBase }
      typeOf[C] = t
    end
    return t
  end
  nameOf[C] = name
  ownerOf[GetType] = C

  if C.GetUniqueID == nil then
    rawset(C, "GetUniqueID", GetUniqueID)
  end
  if C.ToString == nil then
    rawset(C, "ToString", ToString)
  end
  if C.IsA == nil then
    rawset(C, "IsA", IsA)
  end
  rawset(C, "GetType", GetType)
  return C
end

-- The global `class` (§6). Its name comes from the `_` field of the thread's current
-- global table (§6.10).
local function class(...)
  local base, hasBase = classify(3, ...)
  local name = ""
  if getfenv ~= nil then
    local env = getfenv(0)
    if type(env) == "table" then
      local marker = env._
      if type(marker) == "table" then
        local n = marker.Name
        if type(n) == "string" then
          name = n
        end
      end
    end
  end
  return newClass(base, hasBase, name)
end

-- A built-in class (§5.5): Tessera's mechanism and a fixed name, whatever the global
-- `class` is now. `base` may be nil for a root class.
local function declare(base, name)
  local b, hasBase = classify(3, base)
  return newClass(b, hasBase, name)
end

return {
  class = class,
  declare = declare,
  TypeTable = TypeTable,
}
