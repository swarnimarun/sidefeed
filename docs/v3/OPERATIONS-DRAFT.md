# V3 single-node operations contract — proposal, not a runnable release

Status: prepared 2026-10-31 from read-only inspection of `swarnimarun/sidefeed` main. User selected a fresh `v3` branch and a ground-up rewrite. Branch `v3` was created from verified main `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5`; old `v2` remains untouched. This main-branch audit supplies historical lessons, not V3 runtime claims. Hosting target remains unselected. No code, image, deployment, restore, or resource test was executed. Commands below describe the intended V3 interface; implement and test it before claiming support.

## Scope and deployment shape

Build V3 ground-up; Rust, embedded UI and SQLite are proposed technology choices, not a requirement to retain the V1 implementation. One writable application replica and one persistent local filesystem volume; no Kubernetes, Redis, PostgreSQL, separate frontend host, GPU, or paid API dependency for the default deployment. AI classification, embeddings and suggestions, networked readers/cooperative poll-to-push distribution, publishing/microblogging, and inbound/outbound ActivityPub are required product scope. External side effects and remote AI data transfer activate only after explicit operator configuration/consent. Staging implementation does not exclude these features. Avoid network filesystems for the SQLite write volume. Home servers behind CGNAT should work privately without port forwarding; public hosting is a separate opt-in path.

Proposed acceptance hardware: 1 vCPU, 1 GiB RAM and 10 GiB available disk, Linux amd64 or arm64. These are validation targets, not measured minimum requirements. Build images in CI, not on this host. Start the benchmark with 100 sources and 50,000 items, one user and no local model for the host-only baseline; separately measure AI-worker/remote-provider and multi-node federation profiles; record fixtures, source sizes, polling interval, disk use, peak RSS, CPU, refresh duration, search latency and image size. Measure at least one complete retention/poll cycle and a 24-hour soak. Adjust targets from evidence before publishing requirements. Reserve disk for both data and a complete restore copy; refuse operations when safe headroom is unavailable.

## Release-blocking defects in the current deployment

- `docker-compose.yml` sets `SIDEFEED_ADMIN_TOKEN: change-me`; copying `.env.example` does not override that literal. It publishes `8080:8080` on all host interfaces. V3 must fail on missing/placeholder secrets and bind localhost by default.
- Dockerfile copies Cargo.toml, migrations and src but not Cargo.lock or build.rs. Architecture review also found no reproducible frontend rebuild. Require a clean-context image build and prove it includes the current UI and migrations.
- SECURITY.md admits a DNS validation/connect gap. Connection pinning plus redirect tests is a public-exposure gate. Until resolved, egress network policy is required; a reverse proxy is not SSRF protection, particularly on a home LAN.
- Bookmarks are node-global and publicly writable by default. The private V3 profile must require authentication and prevent public listing of personal saved content. Do not present this as multi-user isolation.
- Current health endpoint is globally rate limited. Avoid restart storms from transient 429s; readiness and liveness semantics need explicit tests.

## Compose acceptance specification

Provide `compose.yaml`, `.env.example` and optional `compose.public.yaml` only after implementation. Avoid pretending that the current Compose file already satisfies this contract.

| Requirement | Acceptance evidence |
|---|---|
| Versioned prebuilt image pinned by digest | Fresh pull starts without Rust/Node/build tools on host; reported source SHA matches release |
| Local default exposure | Host bind is `127.0.0.1:8080`; another LAN machine cannot connect |
| Secret handling | Missing, empty and placeholder admin token fail startup; env is explicitly wired; effective config never prints secret values |
| Durable storage | Explicit `/var/lib/sidefeed` volume and DB URL agree; UID 10001 can write; data survives recreation |
| Non-root hardened runtime | No privileged mode, host networking or Docker socket; drop capabilities, no-new-privileges, read-only root filesystem with explicit writable temp/data |
| Predictable lifecycle | Graceful stop timeout exceeds measured drain; one replica; restart policy; failed migration never serves traffic |
| Bounded operation | Log rotation, fetch/concurrency limits, retention and disk-full behavior tested; no local model in baseline |
| Health | Included binary probe or documented probe dependency, startup grace, independent readiness/liveness, no false unhealthy due solely to shared rate limiter |
| Platform support | amd64 and arm64 images tested natively where available; label emulation-only evidence honestly |
| Private first-run | Setup/auth flow works from a fresh volume; no exposed unauthenticated bootstrap race |

Do not place secret defaults in committed examples. `.env` must be ignored and owner-readable only; env-based secrets remain visible to privileged host/container operators. Prefer file-based secrets when application support exists, but do not document nonexistent `_FILE` options.

## Installation and smoke procedure (to implement and automate)

1. Select release digest, host architecture, data path, private/public URL and backup destination. Verify host time synchronization, disk space, Docker/Compose compatibility and volume ownership. Do not reuse production data for the playground.
2. Generate a random admin secret locally; save it outside source control. Render and validate configuration without copying secrets into tickets/logs. Record image digest and non-secret settings.
3. Start privately with outbound side effects disabled until explicitly configured. Wait for schema migration and readiness. Capture exact image, schema version and health output.
4. Exercise management login, create source and feed, attach source, poll deterministic fixture, read item, search, bookmark, edit and delete/detach; verify RSS/JSON output. The current router's CRUD gaps are implementation work, not successful smoke checks.
5. Confirm denied unauthenticated management/private reads/bookmark writes; verify only intentionally public feeds are public. Test revoked and wrong-scope keys.
6. Restart and recreate the container without deleting the volume; confirm all intended state remains. Run backup and restore to a separate private instance before use with valuable data.
7. Public opt-in requires TLS proxy, correct forwarded-header trust, no direct public app port, tested SSRF protection/egress policy, authenticated private UI and rate-limit/size constraints. Test from outside the host; do not infer safety from a localhost check. Add DNS/router changes only with deployment authorization.

## Backup, restore and upgrade

**V1 evidence / V3 migration input:** SQLite holds content, bookmarks, feeds/sources, API key hashes, peer secrets and ActivityPub signing seed. Back up non-secret deployment config plus separately encrypted secrets, any custom UI overrides, and model/tokenizer manifests if opted in. The online README describes these features; inventory exact tables and external paths in implementation. A database backup is a secret-bearing asset.

**Baseline policy (proposed):** encrypted daily backup and before every migration; seven daily and four weekly copies with one off-host destination. Desired RPO 24 hours and RTO 60 minutes require a measured drill. Retention is operator-configurable and must disclose storage costs. Never prune the last verified restore point. Backup failures must surface in UI/logs and an optional operator notification channel.

**Consistent backup:** either use SQLite's supported online backup API/tooling in a tested maintenance command, or stop every writer and archive the entire data directory consistently, preserving DB and any WAL sidecars and ownership. Never copy only a live `.db` file; never independently copy live DB/WAL files. Quiesce writers for cold backups, confirm the app has stopped, then copy. Do not assume the current runtime image contains sqlite3, tar, curl or a backup subcommand. The helper and commands must be shipped and integration-tested.

**Backup manifest:** timestamp, source image digest and SHA, schema version, file list, size and cryptographic checksums, encryption format/key reference, verification status. Store encryption keys separately and test recovery access. Run SQLite integrity and foreign-key checks against the restored copy, not by modifying production. A checksum proves transfer integrity, not application recoverability.

**Restore drill:** disable public exposure and all outbound ingestion/federation/AI side effects; restore into a new empty volume, retain old volume; verify ownership/checksum; launch matching application version; check schema, counts and representative feeds/items/bookmarks/settings, private/public access and key behavior. Restore secrets carefully: restored keys may include credentials revoked after backup. Rotate/revoke as appropriate before reopening. Record duration, lost-data interval and checks. Only switch traffic after acceptance. Never run two writers against one SQLite file.

**Migration rules:** append-only versioned migrations, never edit released migration contents; serialize startup/migration ownership; document minimum prior version and expected lock duration; fail startup closed if migration fails. Test empty install and supported prior-version fixture upgrades, interruption/disk-full and retry, idempotent startup and integrity preservation. Separate destructive changes into a later release after an expand/contract compatibility window when possible.

**Upgrade:** record old digest/config/schema; verify recent restore-tested backup and space; maintenance mode/stop writes; take consistent pre-upgrade backup; pull target digest; migrate/start exactly one instance; run smoke checks; reopen writes only after pass. Announce downtime as single-node behavior, not zero downtime.

**Rollback decision:** use old image on current DB only if backward compatibility is explicitly tested for that schema. Otherwise stop writers, preserve failed state for diagnosis, restore pre-upgrade DB/config to a new volume, run old digest and verify privately before switching. Restoring loses post-backup writes; disclose and obtain operator acceptance. Do not automatically run down-migrations or delete volumes. Avoid `docker compose down -v` in normal operations.

## Required V3 network, AI and publishing operations

These are core release requirements, not optional roadmap exclusions. Lightweight operation is a deployment profile, not removal of the requested features.

**Cooperative polling:** define an authoritative lease cohort per canonical source, authenticated membership, lease TTL, fencing token/epoch, renewal interval and bounded clock skew. Normally one current lease holder initiates polling per cohort. A node unable to renew must stop initiating requests before its lease expires; minority partitions fail closed. Fencing rejects stale results but cannot cancel a request already in flight at the external origin. Failover can therefore overlap bounded origin requests; deduplicate results and measure this residual duplication rather than promise exactly-once external HTTP. Do not claim global exactly-one polling across unrelated networks. New owners catch up from durable cursor/log before polling; consumers deduplicate item/event IDs. Delivery is durable at-least-once with ordered per-source cursors, acknowledgments, bounded retry/backoff, replay window and a snapshot catch-up path. Test three-node leader loss, delayed/reordered/duplicate notifications, network partition, reconnect, clock skew, coordinator loss and restore of an old snapshot. Define measurable failover and staleness SLOs from tests. Display poll owner/lease health, delivery lag, retries and dead letters.

**Home-server topology:** permit outbound persistent subscriptions/catch-up for NATed consumers; hosting public publishing/ActivityPub inboxes needs a reachable HTTPS endpoint or an explicitly selected relay. An optional relay is not evidence the host can receive inbound federation behind CGNAT. Document relay trust, costs and outage behavior. Never silently relay private feeds or tokens.

**AI:** persistent bounded queue, per-job idempotency, retry cap, cancellation, model/version and embedding-dimension metadata, reindex/reclassification migration and separate worker resource limits. Enforce operator budgets on requests/tokens/concurrency and storage; show cost estimates as estimates, and hard-stop configured budget exhaustion. Remote providers require explicit disclosure/consent for transmitted content, credentials and retention implications. Local model weights remain operator-chosen trusted input. Recommendations/classification cannot directly authorize publishing or tool actions; hostile feed text is untrusted data. Missing/failing providers must preserve ingestion and expose deferred job status, not fabricate results. Measure latency, queue growth, costs and model footprint for each advertised profile.

**Publishing/microblogging:** durable drafts, explicit publish action, authenticated ownership, visibility controls, stable canonical IDs, revision/deletion semantics, media quotas/type validation, sanitization and delivery outbox. Persist publish intent and delivery status transactionally; retries must not create duplicate public posts. Test private drafts never enter public feeds, search or federation; queued publishing survives crash; revocation/deletion/tombstone behavior is explicit. Signing keys and actor identity are protected backup assets. A restored node must remain in recovery mode until leases are reacquired and outbox entries reconciled; replay must not republish old posts.

**ActivityPub:** inbound/outbound supported Create/Update/Delete/Follow/Accept/Undo flows, actor discovery and outbox, signature/digest/date/replay protection, actor/key ownership verification, safe remote key fetch and SSRF checks, key rotation, audience validation, blocking and abuse limits. Keep a tested interoperability matrix against representative Mastodon/other peers; V1 ed25519-only support is not proof of V3 compatibility. Test delivery failures, retries, duplicate inbound objects, forged actors, revoked keys, oversized content and unreachable recipients. Federated deletion is best effort outside this node; disclose that limit. No live posting or external follow tests without authorization; use controlled test peers.

**Restore additions:** include durable jobs, event cursors, membership/lease metadata, actor keys, media and delivery outbox in manifests. Restoring does not resurrect an old leadership lease: fence and reacquire authority, catch up peer state and reconcile acknowledged delivery before releasing side effects. Verify restored AI jobs do not double-charge and old outbox jobs do not duplicate publication.

## Routine care and incident procedure

Track last successful poll per source, failed-source count, queue age, disk free/data growth, process memory/restarts, HTTP errors, migration version, last backup and last successful restore test. Do not require a heavyweight monitoring stack; structured logs and a small status page are enough initially. Redact bearer tokens, peer secrets and sensitive query parameters/URLs. Bound log size.

For full disk: stop new work, preserve database, add space or remove only known disposable artifacts under operator control, then verify integrity and resume. Do not delete WAL files. For corrupt DB: stop writers, preserve evidence, restore verified backup privately. For auth/SSRF incident: remove public exposure, restrict egress, rotate affected credentials, preserve redacted evidence and assess private-data exposure. Changes to firewall/router/production state require specific authorization.

## Source evidence (read 2026-10-31, main may change)

- [Compose](https://github.com/swarnimarun/sidefeed/blob/main/docker-compose.yml), blob `665492cd2e1f2e0a70dbf27cdffa42e5de570556`
- [Dockerfile](https://github.com/swarnimarun/sidefeed/blob/main/Dockerfile), blob `b192db5d5918cebdae1b179912c1380f9fedb103`
- [Security](https://github.com/swarnimarun/sidefeed/blob/main/SECURITY.md), blob `3c29ef0d238423d82cb45c3bba1dcfcf12085e44`
- [Environment example](https://github.com/swarnimarun/sidefeed/blob/main/.env.example), blob `e5f7564283e51df24aeeff0d808b5341ded34154`
- [README](https://github.com/swarnimarun/sidefeed/blob/main/README.md), blob `a50ef5b540e5b2cd5af9dc89d648ee1592e3f9b2`
- [Lifecycle](https://github.com/swarnimarun/sidefeed/blob/main/src/main.rs), blob `783c2c8f74cbdda25d1e419fcbbf09fc11fe9bad`

Historical audit findings are based on static source inspection, not runtime verification. All proposed deployment/security tests remain unrun.
