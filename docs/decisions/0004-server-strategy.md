<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004 — Server strategy

**Status:** Accepted, 2026-10-03. **The staging gate is superseded** by
[decision 0034](0034-official-server-compatibility.md) (owner, 2026-10-11): M8 is in scope.
The text below is kept as it was accepted.

## Context
The owner's stated long-term goal is a client that is compatible with the official servers,
alongside an offline sandbox and a project-owned test server for earlier milestones.

## Decision
- Milestones M0–M7 target only the in-process sandbox backend and a project-owned server
  speaking a protocol this project designs (`crates/net`, M7).
- Official-server compatibility is milestone **M8**, deferred until a separate decision
  places the official protocol, login and launcher work in scope.
- Sessions must stop at the M8 boundary and ask the owner before starting M8.

## Consequences
`crates/net` keeps a neutral `Backend` trait so a later compatibility backend could plug in
without restructuring the client.

## Note (2026-10-11)
[Decision 0034](0034-official-server-compatibility.md) supersedes the second and third
bullets of "Decision": official-server compatibility (M8) is in scope by the owner's
decision, and sessions no longer stop at M8. The first bullet's history stands: M0–M7 were
built against the sandbox and the project-owned server. The neutral `Backend` trait of
"Consequences" is how M8 plugs in.
