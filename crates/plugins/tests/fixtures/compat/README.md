<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Synthetic compatibility plugins

Plugins written for Tessera's tests (M5 exit criterion 4). They reproduce the usage
patterns of `docs/specs/plugin-api-usage.md` §6 and the shapes of the §9 plugin set
(windows, stretched children, context menus, quickslots, saved data, slash commands,
class-helper users) with our own code; no published plugin was copied or consulted.
The tests copy this tree into a temporary `Plugins/` folder and add generated images.
