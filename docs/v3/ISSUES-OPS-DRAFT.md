# V3 operations issue pack — ready to copy, not filed

Each issue requires an owner, linked implementation PR and exact-commit verification before closure. Dependencies are issue IDs below, not GitHub numbers. Architecture/core backlog is maintained separately.

## OPS-01 — Reproducible minimal production image
**Priority:** P0. **Depends on:** verified `v3` source SHA and new UI/build architecture.

Build the actual frontend and Rust binary from lockfiles in a clean multi-stage build, include build.rs/assets/migrations correctly, and publish lightweight amd64/arm64 candidates with the required capabilities and separately configured heavy workers only after authorization.

Acceptance:
- [ ] Clean build succeeds without untracked workspace files; source SHA/current UI demonstrably included.
- [ ] Runtime non-root with explicit durable DB directory; persistence/recreation test passes.
- [ ] No compiler, Node, model weights or development secrets in runtime image.
- [ ] Toolchain/dependency/image versions pinned under a documented update policy; image digest and licenses recorded.
- [ ] Default and optional-feature support claims match actual tests; image size measured, not guessed.

## OPS-02 — Secure private-first Compose and setup
**Priority:** P0. **Depends on:** OPS-01; auth/bootstrap and bookmark privacy implementation.

Replace literal `change-me`, wire explicit env configuration, fail unsafe secrets and bind host loopback by default. Provide a distinct opt-in public profile, not accidental exposure.

Acceptance:
- [ ] Missing/empty/placeholder admin credentials fail startup or enter a genuinely local-only secure setup flow; no unauthenticated LAN bootstrap.
- [ ] Effective configuration changes from `.env` are verified without printing secrets.
- [ ] Unauthenticated private/management/bookmark mutation requests fail; saved-list privacy matches single-user contract.
- [ ] UID/storage ownership, rootfs hardening, logs, health/startup grace and graceful shutdown tested.
- [ ] Fresh host can pull/start image without build toolchain or paid service.
- [ ] Public profile remains blocked until SSRF/DNS pinning and reverse-proxy trust tests pass.

## OPS-03 — Consistent encrypted backup and isolated restore
**Priority:** P0 before valuable data. **Depends on:** OPS-01 and stable data inventory.

Ship a tested backup/restore helper and documented cold backup fallback. Do not assume a helper exists in current Sidefeed.

Acceptance:
- [ ] Snapshot is SQLite-consistent under writes, or writers are verifiably stopped; no live `.db` copy.
- [ ] Manifest records image/schema/checksums; secrets and backups encrypted, keys separately recoverable.
- [ ] Restore targets an empty alternate volume; no federation/fetch/AI outbound actions before verification.
- [ ] Integrity/foreign-key checks and representative app/auth/feeds/bookmark checks pass on restored instance.
- [ ] Wrong key, truncated backup, no disk space and bad ownership fail clearly without overwriting source data.
- [ ] Off-host copy, configurable retention and backup failure visibility tested; last good backup protected.
- [ ] Measured restore duration and data-loss window reported against proposed RTO/RPO.

## OPS-04 — Upgrade/migration/rollback compatibility contract
**Priority:** P0. **Depends on:** OPS-03; chosen schema changes.

Acceptance:
- [ ] Empty DB and each advertised prior-version fixture migrate correctly; counts and semantic invariants preserved.
- [ ] Startup has one migration owner; failure does not bind ready traffic; interrupted/disk-full migration recovery tested.
- [ ] Released migrations immutable; compatibility range and destructive-change policy documented.
- [ ] Old-image-on-new-schema rollback explicitly tested or prohibited; restore-based rollback rehearsed instead.
- [ ] Pre-upgrade manifest and snapshot retained; no volume deletion or silent post-backup write loss.

## OPS-05 — CI gates and release provenance
**Priority:** P1, required before release. **Depends on:** OPS-01 through OPS-04 and core management tests.

Acceptance:
- [ ] Existing Rust checks retained; UI build/tests and locked clean production build added.
- [ ] Auth/SSRF/private/public regressions, migration and restore tests run against deterministic fixtures.
- [ ] Required checks tied to exact commit; mutable actions replaced by reviewed pins; least privileges/no PR secrets.
- [ ] Candidate digests/source SHA/schema compatibility and test reports included in release artifacts.
- [ ] Publication jobs cannot deploy automatically; deployment remains separately authorized.

## OPS-06 — Cheap-host baseline and bounded operation
**Priority:** P1. **Depends on:** core features, OPS-02 and observability.

Acceptance:
- [ ] Reproducible 100-source/50,000-item fixture and complete workload definition committed without private data.
- [ ] Measure on proposed 1 vCPU/1 GiB RAM/10 GiB available disk Linux target, local model disabled for the host baseline, plus separate AI/network/publishing profiles; report actual machine and container limits.
- [ ] 24-hour soak records peak RSS/CPU, ingest backlog, storage/log growth, query latency, crashes/restarts and poll progress.
- [ ] Retention, log rotation, restart recovery, constrained memory and disk-full handling exercised without silent corruption.
- [ ] amd64/arm64 evidence distinguished; measured supported minimum published only after test.
- [ ] Test failure leads to tuning/revised honest target, not an unsupported performance claim.

## OPS-07 — First-install and home-server operator walkthrough
**Priority:** P1. **Depends on:** OPS-02 through OPS-06 and full core CRUD.

Acceptance:
- [ ] Fresh-volume private install completes source → feed → poll → read/search → bookmark → edit/delete/detach → OPML roundtrip.
- [ ] Recreate/restart preserves intended data; export/backup can leave the installation.
- [ ] Private access works without public router ports or paid services; CGNAT constraints explained without promising reachability.
- [ ] Optional public TLS profile and firewall/egress requirements documented and tested only on authorized target.
- [ ] Rollback and recovery walkthrough succeeds; unavailable commands/features are not presented as working.

## Generic bug report template

**Version/image digest and architecture:**
**Private/public deployment mode (no secrets):**
**Expected vs observed behavior:**
**Minimal fixture and reproduction:**
**Redacted logs, timestamps and request IDs:**
**Data loss/security exposure suspected:**
**Regression from last working version:**
**Acceptance criteria:** failing reproduction now passes; relevant regression test added; data compatibility and operator workaround documented.

For sensitive vulnerabilities, do not file tokens, private feed URLs or exploit-bearing personal data publicly. Follow the repository's verified private-reporting route once configured; do not invent a contact address.

## OPS-08 — Cooperative polling and push failure drills
**Priority:** P0 for network capability. **Depends on:** durable event protocol and lease authority design.

Acceptance:
- [ ] Three-node controlled test demonstrates normally one authorized poll initiator per source/cohort with fencing; scope excludes global Internet uniqueness and exactly-once origin HTTP. In-flight requests during failover may overlap; stale results are rejected and accepted results deduplicated, with residual request overlap measured.
- [ ] Expired or unrenewable leases stop polling; partition minority stops; delayed stale holder cannot publish accepted work.
- [ ] Crash/leader loss, coordinator loss, clock skew, reconnect and lease handover preserve eventual catch-up without unbounded origin traffic.
- [ ] At-least-once duplicates/reordering/replay are idempotent; gaps trigger snapshot/cursor catch-up, not silent data loss.
- [ ] Restored snapshots cannot resurrect lease ownership or regress peer cursors; reconciliation is observable.
- [ ] NATed readers work via outbound subscription; public ingress prerequisites and optional relay trust documented.
- [ ] Queue/replay/storage quotas, per-peer auth/revocation, private-feed isolation and measured failover/lag evidence included.

## OPS-09 — AI worker budgets, privacy and recovery
**Priority:** P0 for AI capability. **Depends on:** durable classification/embedding/suggestion job contracts.

Acceptance:
- [ ] Classification, embeddings and suggestions run end-to-end with deterministic provider fixtures and verified actual provider/profile before support claims.
- [ ] Separate CPU/RAM/concurrency/storage limits; bounded queues/retry cap/cancel; missing provider leaves explicit pending/failed status without stopping feed ingestion.
- [ ] Explicit remote content-sharing consent and credential redaction; no provider request before consent.
- [ ] Request/token/spend limits enforced; estimates labeled; unavailable usage accounting handled conservatively.
- [ ] Model/version/dimension provenance and reindex path tested; stale vectors cannot silently mix incompatible spaces.
- [ ] Crash/restore avoids duplicate charges where provider idempotency allows; otherwise disclose and reconcile uncertain requests before retry.
- [ ] Feed prompt injection cannot publish, change config or leak credentials; AI suggestions require user-controlled action boundaries.

## OPS-10 — Publishing and ActivityPub release/security matrix
**Priority:** P0 for publishing/federation capability. **Depends on:** author/visibility model, media storage and durable outbox.

Acceptance:
- [ ] Draft → preview → explicit publish → edit/update → delete flow works locally and against controlled federated peers.
- [ ] Draft/private data excluded from public feed, search, network export and federation; signed identity and audience checks tested.
- [ ] Create/Update/Delete/Follow/Accept/Undo interoperability matrix documents representative peers and supported signature schemes; historical V1 support is not reused as proof.
- [ ] Inbox signature/digest/date/replay/key ownership, remote key-fetch SSRF, content/media limits and abuse controls pass adversarial tests.
- [ ] Durable outbox retry/backoff/dead-letter observability; idempotent delivery prevents duplicate posting after crashes/replays.
- [ ] Actor key rotation/backup recovery, revocation and recovery-mode restore tested without republishing old posts.
- [ ] Federated deletion limits and unreachable-recipient behavior disclosed; controlled tests do not publish to real accounts without authorization.
