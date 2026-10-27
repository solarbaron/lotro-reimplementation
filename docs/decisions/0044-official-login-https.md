<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0044 — Official login: credentials only over HTTPS, `http://` addresses upgraded

**Status:** Accepted, 2026-10-27 (owner decision after the first live login run).

## Context

The owner's first live run of the official login (M8 plan step L1, `docs/m8-plan.md`) stopped
before anything was sent: one address of the login chain in the user's install was plain
`http://`, and the connector sends the password and the ticket only along a chain of addresses
that all arrived over HTTPS (`official-login.md` §6.2–§6.3). The chain is: the launcher
configuration's datacenter service, the datacenter document, the auth server, the launcher
configuration server, the login queue, and the chosen world's status URL. `official-login.md`
allows `http://` and `https://` for these and does not say what a client does with a plain one.

If a document that names the next address arrives over plain HTTP, anyone on the network path
can replace that address with a host of their own, including a valid `https://` one, and
collect the password or the ticket.

## Decision

1. **Upgrade.** An address of the login chain that the install or a service document gives as
   `http://` is requested as `https://` at the same host and path (query kept). An explicit
   port of the `http://` form is not carried over: the default HTTPS port is used. This HTTPS
   form is derived from an address the install gave; it is not a fallback or built-in address
   in the sense of [decision 0042](0042-m8-remediation-decisions.md) item 3.
2. **Provenance.** A document read over HTTPS counts as HTTPS provenance, and the addresses
   inside it are rewritten to their HTTPS form. An answer read over plain HTTP never counts.
3. **No plain fallback for credential steps.** The datacenter document, the launcher
   configuration, the auth server, the queue and the chosen world's status (its answer names
   the server that receives the ticket) are never read over plain HTTP. If the HTTPS exchange
   fails (connection, TLS, or an HTTP answer that is not the service's own), the login stops
   with its own message (`InsecureService`) and a log line naming the step and the scheme,
   never a host, path or account. A real answer from the service over working HTTPS (a closed
   world, a maintenance answer, a fault) is shown as that answer.
4. **World list only.** The status queries that only colour the world list may fall back to
   plain HTTP; a world whose status was read that way is shown as Unknown, and joining it asks
   its status again over HTTPS.

## Consequences

- If a service of the chain does not answer over HTTPS at all, the official login stops at
  that step with a clear message; Tessera never sends the account or the ticket over plain
  HTTP.
- `official-login.md` §2, §3.3 and §6.2–§6.3 and `official-entry.md` §8.6 need a spec pass for
  this rule (questions in `docs/m8-remediation.md` §15); until then this record is the rule.
