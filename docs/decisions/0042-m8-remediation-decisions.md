<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# 0042 — M8 remediation: decisions after the blocked audit

**Status:** Accepted, 2026-10-23. The owner delegated these decisions to the orchestrating
session ("decide what is best to resolve those issues and implement it"); the owner can
change any of them.

## Context

The first remediation pass of incident [0041](0041-incident-m8-spec-contamination.md)
(branch `m8-remediation`, commit `19acce5`) failed its default-mode clean-room audit
([`m8-remediation.md`](../m8-remediation.md) §7: B1–B4, N1–N10). The blocking points are
spec-side: reviewers who edited what they accepted, script class names without a source,
client-structure prose, and specs that contradict each other on the same wire facts. Fixing
them needs dirty-room work on the official protocol, which the orchestrating session does
not do. The audit also left open owner questions (N1, N5, N6, N7). This record answers them
and sets what may be built until the specs pass.

## Decisions

1. **Version discovery (N1, and 0041 O1 confirmed).** Tessera never scans the client
   binary's code. It also uses no hard-coded data-section signature and no diagnostic or
   error string as an anchor; step B2 of `official-login.md` §2.4.1 on the branch is
   dropped. The version string comes from, in order:
   - a value the user supplies (setting or command line);
   - the string table of the executable's version resource, a documented PE structure;
   - otherwise the user is asked.

   No anchor, signature or scan heuristic is kept for later.
2. **Admin and GM message classes (N5).** These are out of scope. Tessera never sends them.
   Received ones are ignored and counted, never acted on. Script RPC signatures are read from
   the install at run time, never embedded as a table (decision 0012).
3. **Service URLs and path fragments (N6).** Tessera hard-codes no service URL, host or path
   fragment. Every endpoint comes at run time from the user's install configuration files, as
   0034 §1 requires. If they do not give one, there is no fallback URL and the client
   reports that the install's configuration is missing it.

   XML namespace and SOAPAction identifiers are protocol identifiers that are never
   contacted. They may appear in a spec as interoperability names, and in code as
   constants, only in the message encoder.
4. **Incident 0041 stays open (N7, O3).** O3 is not in force until a remediation pass passes
   the default-mode audit. `M8_PAUSED` stays `true` in the milestone workflow. No
   official-protocol message handler is built from the M8 specs until then: the play
   messages, the gameplay messages, the chat-server client and the live connector.
5. **What may be built now.** Clean work on the existing `crates/compat` that needs no fact
   from the M8 specs:
   - removing the code scan and its anchor (item 1), with the version source of item 1;
   - the neutral renames of the dirty side's W5 name table, which holds names only;
   - hardening and tests;
   - the consent gate of [`official-entry.md`](../specs/official-entry.md).

   The implementers of this work read neither the M8 specs on `main` (Revised or in Review,
   with the internal names of finding 4) nor the branch.
6. **Code provenance (0041 O2).** The `crates/compat` code is kept. It is checked for
   conformance once the specs pass, as O2 says. The provenance table of the first pass is not
   relied on: it has a malformed session ID and cites no evidence (N7). The code stays
   because the incident audit found no contamination signs in it, not because of that table.
7. **Process for the second pass (B1).** Writer sessions write. Reviewer sessions only
   review: a reviewer that finds a problem lists it and sends the spec back, and never fixes
   it. Every reviewer of the second pass is new to the spec it reviews and took no part in
   the first pass. Each commit carries `Signed-off-by:`, `Clean-Room-Role:` and an
   `Assisted-by:` naming the tool (N8).

## Consequences

- M8 exit criteria 1 and 4 stay open. Of Phase 2, the code changes of item 5 and the
  consent gate go ahead now.
- The remediation agent continues on `m8-remediation` with `m8-remediation.md` §7 and this
  record. Step B2 of §2.4.1 and the fallback URL are removed from the login spec.
