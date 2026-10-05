<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0012 — Numeric constants from the client may be used

**Status:** Accepted, 2026-10-05

## Context
The project rules said: never hard-code "hostnames, keys or tables extracted from the client
binary"; load anything of that kind at runtime from the user's install. Disassembly authorised
by decision 0011 also yields numbers that exist only in the client's code and that the
original's behaviour depends on. Examples from the world specs:
- the scenery-placement hash multipliers and salts (`scenery.md`), without which trees and
  rocks land in a similar but different layout;
- the weather-pick hash (`environment.md`);
- the collision slope limits and support radius (`collision.md`);
- the height scale 2000/65535 (`terrain-texturing.md`).

Reading such numbers out of the executable at runtime would need an executable parser and
would break whenever a client update moves the code.

## Decision
The owner allows numeric constants. Specs may state them, and Tessera's code may contain them,
when all of these hold:

- **What counts:** numbers that define how the client computes from the user's own data.
  That covers multipliers, salts, seeds, thresholds, tolerances, unit scales and physics
  limits, plus small fixed arrays of such numbers that are part of one algorithm.
- **Recorded:** each one appears in a spec with `BIN` provenance (decision 0011), and the code
  that uses it cites that spec section (`Spec-Ref:`).
- **Still forbidden in the repository:**
  - cryptographic or authentication keys, certificates and hostnames;
  - anything belonging to the network protocol, login, launcher or anti-tamper, which stays
    gated by decision 0004;
  - bulk data tables. Game content stays in the user's install and is read at runtime, as the
    content rules already require.

## Consequences
- CLAUDE.md, `docs/LEGAL.md` §7 and §11, `docs/CLEAN_ROOM.md` §5.1 and §9, and
  `docs/ARCHITECTURE.md` §2 now say "bulk data tables" and point here.
- Implementers may reproduce the original's exact scenery placement, weather pick and
  collision limits.
- If a client update changes a constant, the spec records the new value with the build it was
  read from. Code that has to support several builds selects the value by build.
