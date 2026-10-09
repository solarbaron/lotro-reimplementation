-- SPDX-License-Identifier: GPL-3.0-or-later
-- Synthetic test plugin by a second author: registers the command Stable's aliases run.
import "Turbine"
waypoint = { visits = {} }
local cmd = Turbine.ShellCommand()
function cmd:Execute(c, args) waypoint.visits[#waypoint.visits + 1] = args end
function cmd:GetShortHelp() return "go to a waypoint" end
Turbine.Shell.AddCommand("waypoint;wp", cmd)
