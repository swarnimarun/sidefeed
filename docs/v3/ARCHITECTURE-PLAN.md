# Sidefeed V3 — ground-up integrated feed platform

2026-10-31 · proposed architecture and implementation plan · intended repository location: `docs/v3/`

## Confirmed direction and baseline

V3 is a **ground-up rewrite**, with AI classification, embeddings and suggestions; networked readers sharing polling and push updates; complete feed management; publishing/microblogging; and inbound/outbound ActivityPub in core scope. Stages below are implementation dependencies, not feature exclusions. Cheap VPS/homeserver operation remains a requirement. Runtime enablement of expensive providers is configurable; this does not remove AI from the product.

The `v3` branch starts from verified main commit `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5`. The pre-existing `v2` branch at `2e3010fc07a04d3a08c74002b9de61791be046a9` is an older Loco/React prototype and is preserved untouched. Neither old codebase dictates the rewrite architecture. Preserve history and data; no destructive pruning/reset is part of this planning step.

Complete product loop: install → import/subscribe → coordinate ingestion → classify/index → read/search/save/recommend → compose/publish/share → receive social updates → backup/restore/upgrade.

## Proposed stack and module boundaries

**Proposed, not dependency-validated:** Rust/Tokio/Axum for the new backend; SQLite WAL/FTS5 with versioned SQL migrations; SolidJS/TypeScript/Vite for the embedded web application; a database-backed job queue and transactional outbox; local media storage. Reusing languages/libraries is not reusing V1 implementation. Ratify concrete versions and license/security/MSRV compatibility during M0. One service, local database and media directory is the compact baseline—no mandatory Redis, Postgres, broker, cloud account or GPU. Heavy inference may run in an explicitly enabled worker or remote provider.

Modules with explicit domain interfaces:

1. Identity/access: owner bootstrap, sessions, API credentials, actors, devices.
2. Sources: RSS 1.0/2.0, Atom, JSON Feed, discovery, OPML import/export, credentials and lifecycle.
3. Ingestion: hardened fetcher, parsers, normalization, item/source membership, revisions, provenance, tombstones.
4. Cooperative network: cohorts, leases, origin polling, durable events, push fanout, replay/resync.
5. Reader: synchronized read/saved state, organization/rules, lexical/semantic search, suggestions, notifications.
6. AI: classifications, embeddings, related-reading/source suggestions, feedback, providers, consent and budgets.
7. Publishing: drafts, previews, scheduling, posts/articles/replies, media, public outputs, delivery state.
8. Federation: ActivityPub actors/discovery/inbox/outbox, follows, delivery, interactions and moderation.
9. Operations: V1 importer, backups/restores, worker/resource controls, health, metrics, releases.

Domain writes and associated outbox/job records commit together. Network calls never hold a database transaction. Workers are idempotent and restart-safe; jobs have bounded retries, cancellation and inspectable failures. Rendering, search and ingestion remain usable if inference or federation is unavailable.

## Architecture decision records

### ADR-001 — rewrite and migration

Accepted direction: new implementation on `v3`, preserving V1 and existing `v2`. Use a fresh V3 schema and API; do not edit applied V1 migration checksums. Provide an explicit read-only V1 importer into a new database with dry-run report, mapping/counts, errors and repeatability. Migrate only through consistent SQLite backup snapshots, not a blind live file copy. Keep the old deployment intact until import/cutover verification. Document rollback as restoring the old app/data, not assuming schema downgrade safety.

### ADR-002 — compact single-host baseline

Proposed single service instance per local SQLite database, embedded UI and bounded workers. No SQLite-on-network-filesystem support. Optional network coordinator and AI worker roles may be separate processes without making them prerequisites for local reading. Build images off-target. Measure rather than promise performance. Vector index library choice remains unvalidated: benchmark bounded scans for small libraries and select a compatible indexed implementation before making large-library latency claims.

### ADR-003 — owner and device state

Proposed owner account with multiple device sessions. Include owner identifiers from the start so bookmarks/read state are never accidentally node-global. Household/multi-user UX is a decision to resolve, not an assumed user requirement. Secure HTTP-only browser cookies, CSRF protection, session expiry/revocation, hashed scoped API credentials. Server-authoritative read/save state with versions and explicit offline-conflict rules. Public publishing is separate from private interests, subscriptions, annotations and AI artifacts.

### ADR-004 — cooperative polling guarantee

Proposed strict mode: each trust cohort uses **one authoritative lease service**, backed by durable state. Source identity includes canonical fetch identity and privacy/credential realm. One current holder is authorized per source in that cohort; unrelated cohorts and Internet clients are outside the guarantee. Never claim globally exactly one origin request.

Lease fields: cohort, source, owner, monotonically increasing fencing epoch, expiry and signature. Renew early. On renewal failure, stop initiating polls; require enough remaining lease time for a bounded request, abort overdue calls and reject late results. Authority handoff waits for expiry plus maximum in-flight request window. Accepted ingest and fanout validate epoch, owner and audience. Fencing rejects stale results; it cannot stop a request already received by an external origin. The honest guarantee is one valid authorized poller and normally one origin poller under stated timing assumptions, with idempotent handling of duplicates—not mathematically exactly-once external requests under arbitrary pauses/partitions.

During authority outage, strict mode sacrifices fresh polling rather than self-electing a second owner. No automatic minority self-election. Authority transfer requires shutting down/fencing old authority, durable state transfer and expiry barrier. Optional HA authority must use a proven quorum/consensus implementation; gossip/consistent hashing alone is not a safety mechanism. Restored stale snapshots must not resume authority automatically. An explicitly selected availability-first emergency mode may poll locally while disconnected but clearly reports relaxed uniqueness.

### ADR-005 — push delivery and recovery

Prefer publisher-advertised WebSub first; cooperative conditional polling is fallback for legacy sources. WebSub distributes content, not poller election. In a cohort, the holder publishes normalized item deltas or cached payloads—not merely “changed” notices causing every reader to refetch origin.

Durable envelope: event ID, schema version, cohort sequence, source/item identity, revision/hash, operation/tombstone, lease epoch, timestamp, audience and provenance. Sign a specified canonical representation. At-least-once delivery with deduplication, persisted acknowledgments, bounded replay and explicit snapshot/resync when cursors expire. Sequence is cohort-scoped, not a global Internet order. Backoff/jitter, per-peer quotas, backpressure, heartbeat/reconnect, dead letters and version negotiation are required. Notify the reader only after durable local ingest.

Homeservers behind NAT use outbound SSE/WebSocket/long-poll relay connections with cursor replay. Direct webhooks/WebSub callbacks require reachability; a reachable relay may terminate the callback and distribute onward. Do not require opening every home node to inbound traffic. Retain ordinary RSS outputs for compatibility. Content replication and lightweight device notifications have separate audience checks.

### ADR-006 — trust, privacy and content rights

Explicit cohort membership and per-source grants; cohort access never grants all private feeds. Authenticated/tokenized sources remain local-only unless the owner explicitly authorizes sharing with a named audience. Feed URLs themselves may be secrets; do not expose them in manifests/logs. Never share origin credentials, private read state, subscriptions, embeddings or summaries by default. Even public subscription choices reveal interests to the coordinator; disclose that trust boundary.

Peer identity rotation/revocation and source-grant revocation stop future access, but delivered copies cannot reliably be recalled. Public availability is not blanket permission to republish complete copyrighted articles. Preserve attribution, canonical links and source policies. Publishing a link/quote differs from claiming fetched content as authored content.

### ADR-007 — AI as first-class functionality with controlled execution

Classification, embeddings, semantic retrieval, related-reading and source/content suggestions are core UI/API/data features. Implement small local providers and deterministic fallback ranking while honestly labeling quality/capability. Heavy models are explicitly enabled local workers or operator-selected remote endpoints. No automatic paid call, model download or content export on subscription/import.

Remote processing requires recorded consent identifying endpoint/provider, source scope, fields transmitted and budget; private material requires separate explicit consent. Enforce batch/concurrency/token/time/cost limits and cancellation. Store model/provider/version/dimensions/input hash and artifact provenance; model changes create controlled reindex jobs, never mixed-vector assumptions. User labels override suggestions. Include correction feedback and explanations. Provider failures must not block ingestion/reading.

Feed content is untrusted data, not instructions. No model-initiated publishing, network trust changes, credential access or arbitrary tool execution. Evaluate classification quality, retrieval relevance and suggestion usefulness on labeled fixtures; record cold-start behavior and confidence calibration. Allow artifact deletion/export and source-specific opt-out.

### ADR-008 — publishing state machine

Draft → scheduled/approved → published → edited/deleted. Autosave and preview never publish. Explicit publish confirms audience and destinations. Stable object URLs, revision history, tombstones and reusable idempotency/activity IDs. Track local publication separately from each destination's delivery. Scheduled timestamps persisted in UTC with user timezone display, cancel/reschedule and restart recovery. Media: content/type/size quotas, safe rendering, alt text and ownership. AI assistance cannot bypass author approval. Do not silently republish ingested articles as original work.

### ADR-009 — ActivityPub interoperability

Core support: local actors and WebFinger, actor documents, inbox/outbox collections, Follow/Accept/Reject/Undo, Create/Update/Delete for Note/Article, replies and the explicitly tested Like/Announce/Undo interaction matrix. Preserve remote IDs/authorship and deletion tombstones; inbound content enters the reader, outgoing authored content enters durable federation delivery. Additional extension support is documented rather than guessed.

ActivityPub semantics alone do not specify a universally deployed HTTP-signature profile. Test legacy RSA HTTP Signatures and RFC 9421 compatibility against pinned real server versions; do not repeat an ed25519-only implementation or assume RFC9421-only interoperability. Include authorized-fetch/signed GET support where required. Verify signer/key/actor ownership AND authority to mutate the target object: a valid signature is not authorization to edit another actor's post. Enforce recipients/visibility, safe key dereference, digest, request target/host/time and replay checks. Followers-only federation is not end-to-end encryption or guaranteed recall.

### ADR-010 — resource bounds and observability

Track last successful origin fetch, lease owner/epoch, renewal/failover state, queue age, fanout lag, cursors, AI backlog/budget, publication delivery and disk/media growth. Redact credentials/private content. Separate process liveness, readiness and worker health. Retention separately covers raw payloads, cached items, saved/authored content, revisions, logs and media. Preserve saved/authored content unless explicitly deleted by policy. Backup/maintenance may not starve reading.

## Data model

Stable opaque local IDs plus external canonical URIs; versioned rows and explicit uniqueness/foreign-key invariants.

| Domain | Proposed entities |
|---|---|
| Identity | owners, devices, sessions, api_keys, audit_events |
| Sources | sources, aliases, credentials, subscriptions, folders, feed_definitions, feed_sources; privacy realm in source identity |
| Content | items, item_revisions, item_sources (many-to-many), authors, attachments, tombstones, provenance |
| Reader | read_states(owner,item,version), bookmarks, labels, rules, notification_preferences |
| AI/search | FTS index, model_registry, embeddings(item,revision,model,dimensions), classifications, suggestions, feedback, consents, budgets |
| Network | cohorts, peers, grants, source_leases(epoch/expiry), cohort_events(sequence), peer_subscriptions, delivery_attempts, cursors |
| Publishing | actors, drafts, posts, revisions, scheduled_publications, audiences, media |
| ActivityPub | remote_actors, actor_keys, follows, inbox_dedup, activities, recipients, federation_deliveries, moderation_rules |
| Runtime | jobs, transactional_outbox, dead_letters, schema_versions, backup_records |

Use composite `(sort_time,id)` opaque cursors. Apply filters before logical page completion. Canonical dedup never loses second-source membership. Persist ingest dedup and delivery progress transactionally. Embeddings have compatible dimensionality/model identity checks. DB backups include a manifest of referenced media/model configuration; credentials and actor keys require protected backups.

## API/event contract plan

New application routes under `/api/v3`, with one OpenAPI/typed-client source of truth. Stable error codes, field errors and request IDs; pagination; optimistic concurrency; idempotency for import/publish; explicit async job states.

- Setup/session: bootstrap, login/logout, device/session revoke, scoped API-key lifecycle.
- Sources: discovery preview, CRUD/pause, credentials, poll request/history, OPML preview/commit/export.
- Feeds: CRUD, attach/detach, rule editing, visibility and share grants.
- Reader: items/detail/search, batch read/save sync, labels, suggestions/feedback, resumable events and notification settings.
- AI: providers/models, consent/budgets, classification/embed/reindex jobs, cancel/status, semantic search and explanations.
- Network: invite/join/revoke, source grants, lease acquire/renew/release, signed event push, ACK/catch-up/snapshot and authority status. Peer credentials are not browser admin tokens.
- Publishing: drafts/revisions, preview, schedule/cancel, explicit publish/edit/delete, media, outputs and per-destination delivery/retry.
- Federation: standard actor/WebFinger/inbox/outbox/object routes, separate from management API.
- Operations: health/readiness, scoped metrics, importer/backup status. Destructive restore is not an unauthenticated endpoint.

Define payload/queue maxima, event schema negotiation, critical-field validation, signed canonical bytes, replay windows and audience authorization before protocol implementation.

## Security and abuse requirements

Safe bootstrap, no default shared password/open public administration. Secure sessions/CSRF, scoped authorization, trusted-proxy configuration, rate limits and credential recovery. TLS for remote access.

One hardened fetcher for feeds, WebSub hubs, AP actors/keys and media: HTTP(S) only, DNS and resolved-IP policy on every hop with connection pinning; deny private/link-local/metadata targets by default, strip credentials on cross-origin redirect, cap bytes/decompression/redirects/time/depth. Narrow explicit LAN allowlists cannot become peer-controlled capabilities. Disable XML external entities. Sanitize HTML, links and media; CSP, safe asset paths and no untrusted scripts.

Peer mutual authentication/rotation, grants, freshness/replay checks, fenced results, quotas and revoked-peer tests. Federation moderation/block/mute/report controls, inbox spam limits and object-authority checks. AI prompt injection isolation and consent audit. Secret-bearing backups encrypted/protected; restore reconciles leases, outbox and scheduled publication before workers resume to prevent stale authority and duplicate sends.

## Milestones and acceptance gates

### M0 — fresh scaffold, ADR ratification, fixtures

New layout/schema, deterministic builds, CI, OpenAPI conventions, protocol corpus and threat model. Ratify dependency versions, owner UX, provider defaults, interaction matrix and cohort authority topology. Gate: safe empty install and clean CI; every user feature mapped to a milestone. No existing-code evolution assumption.

### M1 — source-to-reader vertical slice

Full source/feed lifecycle, discovery/OPML, safe parsing, revisions/membership, persistent read/save state, search and local events. Gate: real HTTP fixture + browser import/fetch/read/save/search/edit/pause/remove/export/restart; tied timestamps and shared URLs correct; no private-state leaks.

### M2 — coordinated polling and push

Authority/lease machine, cohorts/grants, durable fanout, outbound home-node stream, replay/resync and WebSub adapter. Gate: three nodes share a source and an origin request counter shows only current authorized polling during healthy strict mode. Fault tests cover lease expiry/renewal loss, process pause, slow/late HTTP, partitions, authority restart/restore, stale epoch, duplicated/reordered delivery, revoked peers and offline catch-up. Assert accepted-results safety and stated timing assumptions—not global exactly-once requests.

### M3 — integrated AI and suggestions

Local and remote provider adapters, consent/budgets, classification/embeddings, semantic search, suggestions and corrections. Gate: labeled quality report; explicit model-version reindex; ingestion survives outage; zero unauthorized remote transmission; private-data permission and budgets enforced; results visible in everyday reader workflow. Core AI completion is required for V3, even when individual deployments disable expensive providers.

### M4 — publishing and microblogging

Draft editor, previews/autosave, identity, posts/articles/replies, attachments, scheduling, public site and RSS/Atom/JSON outputs, revision/delete and durable delivery. Gate: drafts remain private, audience confirmed, retry/restart cannot double-publish, scheduling/cancel/edit/delete work, media safe, fetched content not misattributed.

### M5 — bidirectional ActivityPub

Actors/discovery, follows, inbox/outbox, delivery/retry, inbound reader normalization and approved interaction lifecycle. Gate: fixtures plus authorized real-peer testing against pinned releases demonstrate local publish delivery, incoming follow/post/reply, edit/delete/undo idempotence, privacy, key handling, blocks and signed-fetch behavior. Publish exact compatibility matrix and limitations. Public TLS domain/reachability required for conventional federation, not for local-reader operation.

### M6 — unified release, import and operations

One coherent UI covering all required scope, V1 importer, backup/restore, versioned amd64/arm64 images, deployment profiles and resource dashboards. Gate: fresh install/import verified; constrained-host soak with ingestion + network + AI budgets + publication/federation; restore without stale leases/duplicate posts; final checks against exact release SHA. M1 alone is not a completed V3.

## Resource/release verification

Align initial benchmark experiment with operations plan: **1 vCPU, 1 GiB RAM and 10 GiB disk**, excluding explicitly enabled heavyweight model files/processes. This is an unmeasured proposed test configuration, NOT a minimum or performance promise. Report both application and total host/worker costs. Define source/item count, body size, retained history/media, reader/peer concurrency, poll frequency, AI throughput and fanout before choosing latency/RSS/CPU/disk gates. A 512 MiB run may be a later stretch experiment, not release promise.

CI: formatter/check/test/clippy, frontend typecheck/build/unit/browser, real-origin ingestion, contract drift, lease property/fault tests, AP/WebSub fixtures, import/migration/restore, Docker build/start/persistence, dependency/license/security scan, generated-asset reproducibility. Release: exact SHA/image digest, SBOM and artifact provenance as supported by repository setup, operator docs, rollback boundaries, measured budgets and protocol compatibility report. Local-only/networked/federation deployment profiles must not hide missing core product features.

## V1 evidence and lessons

Observed baseline [main115d6d0](https://github.com/swarnimarun/sidefeed/tree/115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5) had [successful CI](https://github.com/swarnimarun/sidefeed/actions/runs/37231310359), not verified runtime readiness. Evidence paths at that commit: `src/api.rs`, `src/store.rs`, `src/ingest.rs`, `src/auth.rs`, `src/config.rs`, `web/src/api.ts`, `web/src/state.ts`, `web/src/views/Manage.tsx`, `Dockerfile`, `docker-compose.yml`, `tests/e2e.rs`, `.github/workflows/ci.yml`, `SECURITY.md`, `PLAN.md`.

Turn lessons into regression tests: missing edit/delete/detach/export; public-only reader despite private backend; authenticated bookmark flag unsupported by reader; browser-only read state; timestamp-only cursors; filters after LIMIT; one-source dedup membership; retention deletes bookmarks; hardcoded Compose secret and unwired env; image ignores lockfile/frontend build; DNS check/connect gap; API-only fixtures called whole-product E2E; ed25519-only AP compatibility. [PR #2](https://github.com/swarnimarun/sidefeed/pull/2) and [PR #3](https://github.com/swarnimarun/sidefeed/pull/3) target obsolete Actix code: preserve useful requirements, not patches; closing requires separate approval.

## Official references and interpretation

- [W3C ActivityPub](https://www.w3.org/TR/activitypub/): actor/inbox/outbox, activities, addressing and delivery semantics; use its security considerations.
- [W3C WebSub](https://www.w3.org/TR/websub/): publisher/hub/subscriber discovery, verification, subscription and notification protocol; it does not elect Sidefeed pollers.
- [RFC 9421](https://www.rfc-editor.org/rfc/rfc9421): HTTP Message Signatures; not proof of universal ActivityPub support.
- [Mastodon security documentation](https://docs.joinmastodon.org/spec/security/): deployed signature and authorized-fetch interoperability reference; pin versions in tests.
- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110): HTTP validators/conditional requests and semantics; implement ETag/Last-Modified, per-host limits, backoff/jitter and Retry-After handling.

Standards reviewed 2026-10-31. Lease architecture, schema and AI design are Sidefeed proposals, not claims those standards define them.

## Decisions still needed

Confirmed: `v3` fresh rewrite and comprehensive feature scope. Remaining: owner/household UX, concrete model/provider defaults and consent, initial cohort membership/authority operator, strict versus explicit emergency availability mode, target protocol interoperability versions, workload budgets, V1 data to import, and deployment host/domain/backup/access arrangements. No ambiguity here authorizes reducing requested scope or deploying publicly without an approved target.
