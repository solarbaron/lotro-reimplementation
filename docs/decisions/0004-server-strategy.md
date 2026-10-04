# 0004 — Server strategy

**Status:** Accepted, 2026-10-03

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
