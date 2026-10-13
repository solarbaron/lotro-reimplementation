<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0004 — Server strategy

**Status:** Accepted, 2026-10-03. **The gate part is superseded** by
[decision 0034](0034-official-server-compatibility.md) (owner, 2026-10-11): M8 is in scope
without the written legal review (G1) and the permission request (G2); G4 and G5 stay
binding. The text below is kept as it was accepted.

## Context
The owner's stated long-term goal is a client that is compatible with the official servers.
The legal research (`docs/LEGAL.md`) rates that as the highest-risk tier: EULA §4.3–4.5,
ToS §7.1/§12, DMCA §1201 theories accepted in *Daybreak v. Takahashi* (2025–26), and account
bans for users.

## Decision
- Milestones M0–M7 target only the in-process sandbox backend and a project-owned server
  speaking a protocol this project designs (`crates/net`, M7).
- Official-server compatibility is milestone **M8**, gated: no code for the official
  protocol, login or launcher lands until a written legal review (ideally with permission
  from the rights holders) is recorded here as a new decision.
- Sessions must stop at the M8 gate and ask the owner.

## Consequences
`crates/net` keeps a neutral `Backend` trait so a later compatibility backend could plug in
without restructuring the client.

## Note (2026-10-11)
[Decision 0034](0034-official-server-compatibility.md) supersedes the second and third
bullets of "Decision": official-server compatibility (M8) is in scope by the owner's
decision, sessions no longer stop at M8, and the legal review is no longer a precondition.
The first bullet's history stands: M0–M7 were built against the sandbox and the
project-owned server. The neutral `Backend` trait of "Consequences" is how M8 plugs in.
