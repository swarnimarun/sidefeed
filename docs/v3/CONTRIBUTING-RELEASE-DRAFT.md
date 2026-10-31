# V3 contribution, CI and release procedure — proposed

These are proposed engineering procedures; implementation and deployment verification remain outstanding. Branch `v3` was created from verified main `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5` for the authorized fresh rewrite; existing `v2` is untouched. These documents are intended for `docs/v3`.

## V3 branch/rewrite procedure

1. Use newly created `swarnimarun/sidefeed:v3` based on main `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5`. Recheck head before committing; append docs under `docs/v3` without resetting history. Do not modify the older `v2` prototype or create another branch. Preserve notices.
2. Record upstream/source SHA and current v3 SHA. Keep existing Sidefeed deployment and data untouched. Record branch protection/required checks separately; change settings only within authorization.
3. Ground-up rewrite with full required feature inventory: feed management, AI classification/embeddings/suggestions, cooperative poll/push readers, publishing/microblogging and bidirectional ActivityPub. Treat V1 as migration/evidence input, not implementation architecture to incrementally preserve. Stage by dependency without silently excluding requested scope.
4. Open a draft implementation PR with this plan, milestone scope and explicit unresolved decisions. No production deployment or automatic merge follows from creating a fork/PR.

## Working agreement

- Small vertical changes; include tests and migration/operational docs in the same PR. Do not narrow requested product scope without approval.
- New dependencies need justification, license/security review and effect on baseline build/image size. Required AI/network/publishing capabilities need independently bounded workers and explicit activation of external side effects; feed reading must remain available during provider failure.
- Do not modify published migrations; do not commit `.env`, tokens, real user feeds/data or model weights. Sanitize fixtures and failure logs.
- API changes update OpenAPI, examples, management UI and compatibility notes. UI changes include authenticated/private/public behavior, empty/error/loading and repeated/interrupted flows.
- A feature is done only with code, automated tests, documented operator behavior and observed result. A stub UI, static mockup or compile-only optional feature is not runtime completion.

## CI design

Historical main-branch `.github/workflows/ci.yml` runs `cargo check`, `cargo test`, clippy with warnings denied, plus compile checks for burn-local and onnx-local. It uses mutable action tags and stable toolchain, and does not demonstrate container/UI build, runtime optional-provider coverage, migration/restore drills or resource targets. Reuse test intent where applicable to the rewrite, not unverified workflow commands from another stack. Inspect the chosen V3 implementation and historical V1/V2 prototypes before writing executable CI. This deliverable contains planning Markdown only.

### Every PR, read-only credentials

1. Choose and document the rewrite stack first; the Rust-oriented proposals here apply only if that stack is selected. Check formatting and lint; Rust check/tests/clippy using committed lockfile and chosen toolchain. Add `--locked` once verified. Check UI from its actual lockfile with its supported build/lint/test commands; inspect repository instructions before selecting commands.
2. Unit/integration tests: parse RSS/Atom/JSON Feed, dedup, poll backoff/limits, feed CRUD and detach, search/filter, OPML import/export, bookmark persistence/privacy, scoped auth and revocation. Use deterministic fixtures/local servers, not live internet success as pass criterion.
3. Security regressions: secret defaults, public/private separation, redirects/DNS pinning/private ranges, malicious HTML/rendering, body/time limits, rate-limit behavior, sensitive log redaction.
4. Fresh DB and supported old-schema upgrade fixtures; failed migration never serves; restart/recovery persistence.
5. Build actual production image from clean checkout; prove fresh embedded UI and migration files, non-root startup, persistent volume and health behavior. Validate private Compose config with test-only secrets.
6. Three-node network harness tests lease/fencing, partitions, failover, duplicate/lost/reordered push and replay; AI fixtures test classification/embedding/suggestion pipelines, budgets, provider failures and no unauthorized data transmission; controlled ActivityPub peers test signatures and publishing/receive/update/delete/idempotent retries. No live external publishing in ordinary CI.
7. Image smoke E2E and backup/restore test against separate temporary volumes; attach redacted logs/results. Do not run destructive tests against user data.

### Release candidate

- Real amd64 and arm64 smoke tests where infrastructure permits; state any untested architecture. Compile optional features separately from core, and run provider tests before advertising support. Never download arbitrary model weights in default CI.
- Dependency/license/security scanning with triaged findings; pin third-party actions by reviewed SHA, least-privilege workflow permissions, no production secrets in PR jobs. Publishing only in protected authorized release jobs.
- Produce image digests, SBOM/provenance where supported, immutable source SHA, schema compatibility range, release notes and test report. Prove source-to-image traceability rather than claiming reproducible bytes without comparison.
- Run constrained-host benchmark and 24-hour soak per OPERATIONS-DRAFT; retain raw evidence. Verify disk/log bounds and no required paid service.
- Manual review of deployment/docs and a first-install walkthrough on a fresh volume by someone other than the implementer when feasible.

## PR template (copy into destination when approved)

### Purpose and issue
What user flow becomes possible? Link issue and identify staged dependencies and any requested behavior not yet implemented.

### Changes
API/UI/storage/config changes; new dependencies; private/public implications.

### Verification
- [ ] Exact commit, commands, results and environment attached
- [ ] Core tests, container build and relevant E2E passed (mark never-run/failed explicitly)
- [ ] Migration/backup/restore/backward compatibility checked, or justified not applicable
- [ ] No secrets/user data; security regression tests covered
- [ ] API/docs/environment example updated
- [ ] Screenshots and repeated/interrupted UI flow evidence where relevant

### Deployment and rollback
New settings, migration window, backup requirement, compatible prior image or restore-only rollback, known gaps. No implicit deployment approval.

## Release checklist

- [ ] Owner approves exact source commit/version and candidate scope
- [ ] Required checks pass for that exact SHA; no pending results called a pass
- [ ] Full required management, AI, network push, microblogging and bidirectional ActivityPub acceptance suites pass; a partial milestone is not the complete V3 release
- [ ] Clean install, V1 migration/import path, V3 upgrade and side-effect-safe restore pass
- [ ] Secret/bootstrap, bookmarks/privacy and SSRF blockers resolved for advertised exposure mode
- [ ] Images and manifests verified on supported architectures
- [ ] Resource report labels measurements vs targets and names fixture/workload
- [ ] Changelog includes removals, API/config/data changes and known limitations
- [ ] Backup compatibility, upgrade steps and rollback strategy published
- [ ] Release/image publication explicitly authorized; immutable tags/digests verified after push
- [ ] Deployment target and credentials confirmed; deployment authorization obtained separately
- [ ] Post-deploy smoke, persistence and backup checks run on exact deployed digest; failures retain rollback evidence

Recommended milestones: `v3-foundation` (new contracts/storage/auth/build), `v3-core` (complete management flow), `v3-network` (leased polling and durable push), `v3-ai` (classification/embeddings/suggestions), `v3-publishing` (microblogging and bidirectional ActivityPub), `v3-ops` (backup/upgrade/restore/resource validation across all profiles), `v3-playground` (authorized isolated full-scope test deployment). Do not assign dates until implementation estimates and environment access exist.

Evidence: [current CI](https://github.com/swarnimarun/sidefeed/blob/main/.github/workflows/ci.yml), blob `978859209b9589dba3ad840e42797be677bbff9c`; [Cargo manifest](https://github.com/swarnimarun/sidefeed/blob/main/Cargo.toml), blob `e3fa3fc4da8afc65fe6d9c17c4c5ee9e02ffeb84`. Read 2026-10-31; all proposed checks above remain unrun.
