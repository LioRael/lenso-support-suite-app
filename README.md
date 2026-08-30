# Lenso Support Suite reference App

This repository is the reference Plugin Root and composition acceptance harness
for the P0/P1 Support slice:

- inbound Resend email resolves a Customer Directory contact and opens or
  appends to a Support Case;
- the Help Center searches published Knowledge Base articles and gives an
  authenticated requester access to case submission, status, messages, and
  text attachment upload;
- Support Attachment stores the case association while Content Vault owns the
  bytes and Support Case owns case-resource authorization.

It is **not a distributed Host executable**. The tests link the six real P0/P1
Plugin crates plus their real Content Vault provider and use the current Plugin
Root resolver. Existing
platform dependencies are represented by explicit test Host descriptors so the
suite graph can be checked without pretending those providers were delivered
here.

## App-owned Plugin Root

The only App-authored state is [`plugins/`](plugins/). Each business Plugin and
the existing Host-owned Web Ingress Instance has one typed `default.toml`
Instance patch:

```text
plugins/
  lenso.content-vault/default.toml
  lenso.customer-directory.postgres/default.toml
  lenso.support-email.resend/default.toml
  lenso.support-case.postgres/default.toml
  lenso.support-attachment.postgres/default.toml
  lenso.knowledge-base.postgres/default.toml
  lenso.help-center.web/default.toml
  lenso.web-ingress/default.toml
```

There is no `lenso.app.json`, enabled list, binding document, provider key, or
implementation selection in this repository. Host policy owns releases,
defaults, root Slots, private bindings, executable selection, and the immutable
Plan.

The values are deployment-shaped examples, not production identities:

- `org_support_demo` and `support@example.com` must be replaced together;
- the checked-in Ed25519 value is the public key from an RFC test vector and is
  safe only as a configuration fixture; replace it with the deployed Auth
  assertion public key and set `auth_issuer` to the matching issuer;
- every `*_secret` value is a reference. No PostgreSQL URL, S3 credential,
  Resend API key, webhook/reply-token secret, or Auth signing secret belongs in
  Plugin Root.

See [`docs/plugin-map.md`](docs/plugin-map.md) for the capability graph, exact
caller allowlists, disable/dependency behavior, and fixture boundary.

## Source build

This acceptance crate is a standalone source repository and is intentionally
`publish = false`. [`Cargo.toml`](Cargo.toml) pins the seven provider repositories
to immutable revisions. `Cargo.lock` and the source-boundary check keep the
suite on one runtime, core, and protocol baseline without sibling checkouts.

## Verification

Run from this repository:

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo tree --locked --duplicates
./scripts/check-source-boundary.sh
```

The acceptance suite proves:

- all seven real linked descriptors and native factories are present exactly
  once;
- the complete Plugin Root resolves with one provider for every required
  Capability;
- disabling Resend preserves the Help Center route, and disabling Help Center
  preserves the email route;
- disabling Knowledge Base or Support Attachment while Help Center remains
  fails resolution with the affected Plugin and Capability named;
- disabling Content Vault while Support Attachment remains fails closed on
  `lenso.content-vault@1`;
- configuration schemas accept the Instance patches and the exact caller
  boundaries are present in the resolved Plan.

These are pure Plugin Root resolution tests. They pass the same disabled
Instance identities represented on disk by
`plugins/<plugin-id>/default.disabled`; they do not uninstall packages, stage a
live Generation, or exercise runtime traffic. Because these are built-in Host
defaults, deleting only `default.toml` would restore the Host default
configuration rather than remove the Instance. Use
`lenso plugins disable <plugin-id> default` (or the equivalent `.disabled`
marker) when validating an absent default Instance.

`lenso app check` and `lenso app show` become the deployment commands only after
a custom Support Host publishes its matching `.lenso/host-catalog.json`.
Generic `lenso run` currently does not link these Plugins or install this App,
so it cannot be used as deployment proof.

## Deployment prerequisites

Before staging a real Generation, the product Host must link the seven real
Plugin factories plus production providers for Auth, Secrets, Access Control,
Organization Membership, Search, Search Index, HTTP egress, and
HTTP ingress/routing. It must close any privacy export and retention root Slots
used by the deployment by linking the real privacy orchestrator/caller
Instances; Customer Directory and Support Case already supply the provider-side
export and retention roles. The Host must then require every selected Instance
to pass readiness before routing switches.

Provision PostgreSQL and use each Plugin's explicit operator surface to create
or upgrade its owned schema. Plugin startup only performs fail-closed
schema/readiness checks and does not apply DDL:

- `CustomerDirectoryOperator::setup/upgrade(database_url, "support_customer")`
- `SupportCaseOperator::setup/upgrade(database_url, "support_case")`
- `SupportAttachmentOperator::setup/upgrade(database_url, "support_attachment")`
- `KnowledgeBaseOperator::setup/upgrade(database_url, "support_knowledge_base")`
- `ContentVaultOperator::setup/upgrade(database_url)` for the fixed
  `content_vault` schema

The schemas may share a database URL secret, but they remain separately owned.
An existing legacy Content Vault schema must first be taken fully offline and
passed through the exact matching `ContentVaultOperator::adopt_legacy_v1` or
`adopt_legacy_current` workflow, followed by `upgrade`; do not run adoption
concurrently with writers or DDL-capable sessions.
Provision the Content Vault S3-compatible bucket, region, access-key and
secret-key references independently. `content.maintenance/default` is a
deployment placeholder for the exact Instance allowed to sweep terminal
quarantine; link that Instance or replace the typed allowlist.

For Resend, verify the receiving domain and its DNS records, expose
`POST /webhooks/resend` over public TLS, subscribe that endpoint to
`email.received`, and map the Resend webhook signing secret, API key, and reply
token key into three distinct configured secret references. The reply-token key
must resolve to at least 32 bytes of high-entropy material. Existing-case replies
use `support+SUP-N.<full-HMAC-token>@…`; an RFC `From` value plus an enumerable
`SUP-N` is not authorization to append. Rotating the reply-token secret revokes
all previously issued reply addresses. The Host must also route both Resend and
Help Center HTTP endpoints without collapsing their distinct Plugin Instances.

Help Center accepts up to 8 MiB of decoded attachment bytes in a JSON Base64
field, which needs about 11.2 MiB on the wire before ordinary JSON overhead. The
Plugin Root therefore sets Web Ingress `max_request_body_bytes` to 12 MiB
(`12582912`). Every global load balancer, reverse proxy, API gateway, and Host
transport in front of it must allow at least the same request size; a smaller
upstream limit will reject a valid Help Center upload before the Plugin runs.
The acceptance harness checks this value against an explicitly labeled Ingress
descriptor fixture; it does not parse or start the legacy Web Ingress factory.
A deployment must still validate the real Ingress configuration and a
near-limit HTTP upload end to end.

The Auth provider must mint credential evidence that permits these exact
audiences for the authenticated requester flow:

- `lenso.http.endpoint@1:help.center.web.support.create`
- `lenso.http.endpoint@1:help.center.web.support.status`
- `lenso.http.endpoint@1:help.center.web.support.attachment.upload`
- `lenso.support-attachment@1:upload_and_attach`

The first three authorize the Help Center HTTP operations. Attachment upload
must carry both the HTTP upload audience and the downstream
`upload_and_attach` audience on the same verified assertion; having only one of
them fails closed. [`tests/fixtures/auth-credential-policy.toml`](tests/fixtures/auth-credential-policy.toml)
records this as test Host policy, not Plugin Root or an Auth runtime delivery.
The generic HTTP Endpoint descriptor exposes transport operations rather than
route-level Auth audiences, so this harness does not derive or execute the Help
Center verification chain. Likewise, it verifies three distinct Resend secret
references, not the runtime secret bytes or HMAC behavior; production readiness
must verify those external providers and secret values.

This source repository does not publish crates, change DNS, create secrets,
migrate databases, or activate a live Host.
