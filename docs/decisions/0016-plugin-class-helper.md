<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0016 — Ship our own plugin `class` helper; extended scripting later

**Status:** Accepted, 2026-10-06

## Context
`plugin-api-usage.md` found that 37 of 143 published plugin distributions import a `class`
helper and some sample utilities from a package the original publisher released in 2010.
Users normally install that package themselves under `Plugins/`. Its code is third-party
publisher material, so Tessera does not bundle it. Without some `class` helper, those plugins fail
to load.

The owner also wants scripting that goes well beyond the original plugin API in the future,
while standard plugins keep working.

## Decision
1. **Class helper.** Tessera ships its own clean-room implementation of the `class` helper,
   and of the small set of sample utilities the census shows plugins depend on.
   - **How it is written:** a spec states the behaviour plugins rely on, using facts only:
     how classes are declared, inheritance, constructor calls, `IsA`-style checks, and
     metatable behaviour. A clean-room implementer then writes the code from that spec. No
     line of the original package is copied.
   - **Interop names:** the helper is reachable under the import path plugins already use.
     That path is an interop identifier only (see `CLAUDE.md` Naming rules and
     `tools/content-allowlist.txt`).
   - **Precedence:** if a user has installed the original package, the user's copy takes
     precedence, so plugins see exactly what they saw before.
2. **Extended scripting.** Tessera will get its own extended scripting API in a separate
   namespace. The compatibility API of M5 stays unchanged, so standard plugins run as they
   do in the original.
   - Its architecture covers capabilities, the security model, the namespace, versioning,
     coexistence with standard plugins, and which milestone it lands in.
   - That architecture is a separate decision, proposed for the owner.
   - Extended scripts run offline and on
     Tessera's own server (decision 0004). (Amended
     2026-10-11 by [decision 0034](0034-official-server-compatibility.md) §4: extended
     scripts may run on an official server after the user's explicit opt-in, off by
     default; automation stays offline only.)

## Consequences
- M5 gains the clean-room class helper as a deliverable. Its spec is part of the M5 spec set.
- The 22 distributions that need only the class helper beyond the first API surface
  (`plugin-api-usage.md` §7) become loadable without the user installing anything else.
- A proposed decision on extended scripting follows, together with a roadmap placement after
  M5.
