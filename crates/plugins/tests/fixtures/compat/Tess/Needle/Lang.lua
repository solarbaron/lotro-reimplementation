-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin helper: image file names chosen by client language.
local suffix = { [Turbine.Language.English] = "en", [Turbine.Language.German] = "de", [Turbine.Language.French] = "fr" }
function ImagePath(name)
  return "Tess/Needle/Images/" .. name .. "_" .. (suffix[Turbine.Engine.GetLanguage()] or "en") .. ".tga"
end
