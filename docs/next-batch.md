# Next practical Support Plugins

The next batch starts from support work that a real team still cannot complete
with the current App. Priority is based on the user job and the deletion test,
not on adding new framework machinery.

Delivery state (2026-08-31): P0 Support Agent Workspace is implemented and
composed from Support Web revision
`73bc1244dccfc36cc2afec6442926d447aefcafa`. The core P1 Knowledge Author flow
is also implemented: Knowledge Author
`734bb769ce4dba02d57d9ad538e3e83afb5a2a74`, Knowledge Base `1.1.0`
`6b69e310820c0d5c00ec187339165bf7727c65ea`, and aligned Help Center
`091f6140d005c2ccd2a5ff158e4304385d576898`. The Suite composition pins these
immutable revisions; SLA observation and outbound email remain follow-ons.

## P0: Support Agent Workspace

- User job: an authenticated support agent lists cases, opens one, sends a
  public reply or internal note, assigns it, and advances its state.
- Reuse: align and compose the existing `lenso.support.web` Plugin; do not
  create a second dashboard Plugin.
- Provides: `lenso.http.endpoint@1` in the many-valued `web` Slot.
- Requires: exactly one `lenso.auth@1` and one `lenso.support-case@1`.
- Final authority: Auth authenticates; Support Case remains the final authority
  for visibility, messages, assignment, and revision-fenced transitions.
- Deletion test: removing this Plugin removes only the agent page and routes.
  Email intake, Help Center intake, attachments, cases, and knowledge remain.
- Acceptance: requester intake creates a case; an authenticated agent can list,
  open, reply, and transition it; invalid credentials return 401; stale
  revisions return 409; disabling the Plugin leaves Resend and Help Center
  composition valid.

## P1: Knowledge Author

- User job: an authorized knowledge editor creates a draft, reloads later,
  finds the draft, resumes editing with revision fencing, and publishes it.
- Capability owner work first: extend `lenso.knowledge-base@1` with bounded
  `get_draft` and `list_articles` operations. The Knowledge Base Plugin keeps
  the final organization, membership, RBAC, and row-level decision.
- Surface second: add a removable `lenso.knowledge-author.web` Plugin that
  provides `lenso.http.endpoint@1` and requires Auth plus Knowledge Base.
- Deletion test: removing the author surface removes author routes and UI but
  preserves drafts, revisions, publications, and public Help Center reads.
- Acceptance: create, reload/list, resume with CAS, publish, then observe the
  article through Help Center; unauthorized access is 403 and stale revision is
  409.
- Evidence is owner-split rather than a fabricated live Host: the Author Plugin
  runs the authenticated Kernel workflow and conflict tracer, Knowledge Base
  runs real PostgreSQL publication/re-read acceptance, and this Suite proves
  that Help Center public reads and author writes select that exact Provider.

## P1 follow-ons

- `lenso.support-sla-observer`: bridge a body-free, cursor-based Support Case
  observation role into the existing Support SLA Plugin. It owns only its
  durable cursor, forwarding receipt, and retries.
- `lenso.email.resend`: a replaceable outbound provider for the existing
  `lenso.email-dispatch@1` role. It stays separate from inbound
  `lenso.support-email.resend`, so inbound and outbound providers can be mixed.

The SLA observer and outbound Resend work start after the P0 graph and Knowledge
Author contract are green; neither is allowed to infer customer destinations
from an opaque requester subject.
