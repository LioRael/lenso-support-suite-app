# Support Plugin map

## Business graph

| Consumer | Required Capability | Selected provider |
| --- | --- | --- |
| `lenso.support-email.resend/default` | `lenso.customer-directory@1` | `lenso.customer-directory.postgres/default` |
| `lenso.support-email.resend/default` | `lenso.support-intake@1` | `lenso.support-case.postgres/default` |
| `lenso.help-center.web/default` | `lenso.knowledge-base@1` | `lenso.knowledge-base.postgres/default` |
| `lenso.help-center.web/default` | `lenso.support-intake@1` | `lenso.support-case.postgres/default` |
| `lenso.help-center.web/default` | `lenso.support-attachment@1` | `lenso.support-attachment.postgres/default` |
| `lenso.support-attachment.postgres/default` | `lenso.support-case-authorization@1` | `lenso.support-case.postgres/default` |
| `lenso.support-attachment.postgres/default` | `lenso.content-vault@1` | `lenso.content-vault/default` |
| `lenso.support.web/default` | `lenso.auth@1` | production Auth (`fixture.support-platform/default` in composition tests) |
| `lenso.support.web/default` | `lenso.support-case@1` | `lenso.support-case.postgres/default` |

The Resend, Help Center, and Support Web Plugins each provide
`lenso.http.endpoint@1` from the Host's many-valued `web` Slot. The production
Host owns ingress attachment and routing; the App does not author an HTTP
binding file.

## Exact caller policy

| Target configuration | Allowed exact caller Instances |
| --- | --- |
| Customer Directory `resolve_callers` | `lenso.support-email.resend/default` |
| Support Case `business_callers` | `lenso.support.web/default` |
| Support Case `intake_callers` | `lenso.support-email.resend/default`, `lenso.help-center.web/default` |
| Support Case `resource_callers` | `lenso.support-attachment.postgres/default` |
| Support Attachment `business_callers` | `lenso.help-center.web/default` |
| Knowledge Base `public_read_grants` | `lenso.help-center.web/default` for `org_support_demo` |
| Content Vault `maintenance_callers` | `content.maintenance/default` |

`support.admin/default` remains a deployment placeholder for Customer Directory
administration and Knowledge Base authoring; `privacy.export/default` and
`privacy.retention/default` are placeholders for export and retention callers.
A production Host must either link those exact Instances or change the typed
allowlists to the Instances it actually owns.
Customer Directory and Support Case already provide their real export-source
and retention-participant Capabilities; what remains outside this suite is the
privacy orchestrator/caller Instance that invokes them.

Authenticated Help Center support requests attach the Auth assertion to the
same invocation context. Support Case compares its requester subject directly;
Support Attachment asks Support Case Authorization before it reserves, uploads,
claims, or associates Content Vault content. Published Knowledge Base reads are
public only for the exact caller-and-organization grant above; authoring remains
authenticated and separately permissioned. Support Web authenticates the
ingress credential, attaches the resulting user assertion, and delegates all
case visibility and mutation decisions to Support Case.

The test Host Auth policy contains the exact credential audiences
`lenso.http.endpoint@1:help.center.web.support.create`,
`lenso.http.endpoint@1:help.center.web.support.status`,
`lenso.http.endpoint@1:help.center.web.support.attachment.upload`, and
`lenso.support-attachment@1:upload_and_attach`. The upload path requires both its
HTTP audience and its downstream Support Attachment audience.
The same fixture includes the eight downstream Support Case audiences used by
Support Web: `add_message`, `assign_case`, `create_case`, `get_case`,
`list_cases`, `list_messages`, `transition_case`, and `update_case` under
`lenso.support-case@1`.
The generic HTTP Endpoint descriptor exposes transport operations rather than
route-level Auth audiences, so the acceptance fixture pins these exact policy
values without claiming Auth issuance/verification or runtime secret material.

## Test Host fixture boundary

`tests/plugin_root.rs` derives `fixture.support-platform` only for Capabilities
required by the eight real descriptors but not provided within this suite:

- `lenso.access-control@1`
- `lenso.auth@1`
- `lenso.http.client@1`
- `lenso.organization-membership@1`
- `lenso.search@1`
- `lenso.search-index@1`
- `lenso.secrets@1`

That descriptor has no implementation, persistence, security behavior, or
readiness claim. It is not written under `plugins/`, shipped as a package, or
presented as delivery evidence. Production providers and any Host-private
bindings remain Host work.

The existing `lenso.web-ingress` package has a native factory but has not yet
adopted linked descriptor inventory. The test Host therefore supplies a second
minimal descriptor using its real package identity, root Slot, typed
`max_request_body_bytes` field, and a many-valued
`lenso.http.endpoint@1` requirement. This descriptor is also fixture-only; it
does not stand in for listener lifecycle, routing, middleware, admission, or
readiness. The App patch sets its request-body limit to 12 MiB so an 8 MiB
decoded attachment can survive JSON Base64 expansion. Production ingress and
every upstream proxy must enforce a limit no smaller than 12 MiB.
This proves configuration resolution against the fixture schema, not that the
legacy Web Ingress factory or an upstream proxy accepts a near-limit request;
deployment must test that path end to end.

The suite directly Git-links real Provider crates for Content Vault, Customer
Directory, Resend, Support Case, Support Attachment, Knowledge Base, and Help
Center, plus Support Web. Their macro-generated descriptors are obtained from
`NativePluginRegistry::host_catalog`; the seven exported descriptor constants
are compared byte-for-structure, while Content Vault is checked for its exact
identity, root Slot, provided Capability, required Secrets role, and unique
factory. Resolution does not replace any business Capability with a fixture.
The Web Ingress fixture binds all three real HTTP Endpoint providers in
deterministic order.

## Disable and dependency matrix

| Change | Expected resolution result | Product effect |
| --- | --- | --- |
| Disable Resend | Resolves | Help Center support and knowledge path remains |
| Disable Help Center | Resolves | Email intake remains |
| Disable Support Web | Resolves | Only the agent page and routes disappear; email and requester intake remain |
| Keep Support Web without Auth | Fails closed on `lenso.auth@1` | The agent surface cannot authenticate |
| Keep Support Web without Support Case | Fails closed on `lenso.support-case@1` | The agent surface cannot list or mutate cases |
| Disable Customer Directory while Resend remains | Fails closed on `lenso.customer-directory@1` | Email sender resolution is unavailable |
| Disable Support Case while Support Attachment remains | Fails closed first on `lenso.support-case-authorization@1` | Attachment authorization is unavailable; Resend and Help also lose intake |
| Disable Support Case after Help Center and Support Attachment, while Resend remains | Fails closed on `lenso.support-intake@1` | Email intake cannot open or append cases |
| Disable Knowledge Base while Help Center remains | Fails closed on `lenso.knowledge-base@1` | Help Center cannot promise article search/read |
| Disable Support Attachment while Help Center remains | Fails closed on `lenso.support-attachment@1` | Help Center cannot promise attachment upload |
| Disable Content Vault while Support Attachment remains | Fails closed on `lenso.content-vault@1` | Attachment bytes cannot be reserved, uploaded, or claimed |

These checks pass disabled Instance identities to the pure resolver; they do not
uninstall a package or prove a live Generation transition. For these built-in
Host defaults, deleting `default.toml` alone restores the Host default. Use
`lenso plugins disable <plugin-id> default`, which creates `default.disabled`,
to make the Instance absent from the selected Plan.

Disable dependents before providers: Resend before Customer Directory; Help
Center before Knowledge Base; Help Center, then Support Attachment, then Content
Vault; and Resend plus Help Center plus Support Web, then Support Attachment,
before Support Case.
Replacing a dependent with a new descriptor that drops the requirement is an
equivalent staged path. Resolution is deterministic and never uses runtime
fallback or discovery order.
