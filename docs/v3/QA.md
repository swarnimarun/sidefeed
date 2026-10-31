# Sidefeed V3 prototype — independent QA

Verified 2026-10-31 against the local working tree. **19 automated tests passed. Browser visual/interaction QA is blocked, not passed. This is not complete V3 or production readiness.** The QA run performed no live publication, external account access, external origin fetch or deployment.

## Exact checks and results

From the repository root:

```sh
cd v3
node --test qa/security-unit.mjs qa/integration.mjs
# PASS: 12 tests
npm test
# PASS: 19 tests (combined backend and independent suites)
npm run check
# PASS: all server modules, web app and check runner (9 files)
node --check web/app.js
# PASS
```

Node v24.22.1, built-in HTTP/SQLite; fixture fetchers replace external network. Temporary test databases are removed after tests. Npm emitted an environment warning about unknown `http-proxy` config; no test failure.

### Independently exercised

- Empty item list; source creation; duplicate source rejection; repeated ingestion creates one item; lexical search covered by backend suite; read/save retained through re-ingest and database restart.
- Agent discovery is inert before confirmation, approval required, repeated approval conflicts, failed fetch recorded, denial terminal in backend tests.
- Private draft absent from public feed; explicit local publication confirmation; repeated publication does not duplicate; publication persists after restart.
- Owner bearer required and wrong token denied. Foreign/null Origin, nonlocal Host, simple text/plain mutations, and cross-site fetch metadata rejected.
- Credential-bearing/unsafe-scheme URLs rejected. Private IPv4, loopback/link-local/mapped IPv6 and metadata addresses rejected. Numeric/hex localhost addresses rejected before sockets. XML DTD/entity declarations rejected; unsafe item/discovery URL schemes removed.
- Trusted peers need explicit enabled source grants and owner confirmation. Unauthorized transport denied. Peer lease exclusive, local refresh blocked while peer owns it, expiry increments fencing epoch, stale push rejected, duplicate event suppressed, cursor replay returns cached item once, revoked credential denied.
- Latest backend suite: two-node cached SSE delivery, idempotent receive replay, durable receive cursor/private read-save preservation across SQLite restart. This does not exercise a deployed multi-host network or TLS proxy.
- In-flight refresh cannot restore revoked sharing/paused/title edits or resurrect a deleted source.

### Defects found and resolved

1. **ATProto parser unreachable:** generic JSON branch intercepted `{feed: ...}`. Independent regression now passes.
2. **Refresh race restored sharing after revocation:** stale source snapshot was persisted after awaited fetch, also permitting source resurrection. Independent suspended-fetch tests now pass for owner edits and deletion.
3. **Protected UI lacked token input:** static review found no bearer support. The UI now includes memory-only owner token connection/disconnection (not localStorage). Syntax/static verification only; browser interaction unverified.
4. **Revoked fleet peers displayed Registered:** The UI now shows revoked status; static verification only.
5. Mutation hardening: JSON content-type and cross-site fetch metadata rejection added and verified by independent HTTP tests.

## Browser status: BLOCKED / NOT RUN

Prepared [browser.py](../../v3/qa/browser.py) for deterministic desktop/mobile flows and [fixture-server.mjs](../../v3/qa/fixture-server.mjs) for local fixtures. Intended browser coverage: empty/read/save/detail/search; source fetch; discovery approval cancel/confirm/failure; local publish cancel/confirm; Back/Forward; 390px/mobile overflow and 1440px desktop screenshots. **None of those browser assertions ran.** The script is a starting point and may need selector adjustments for subsequent UI edits.

Attempt:

```sh
cd v3
node qa/fixture-server.mjs
python qa/browser.py
```

The installed browser could not launch under the execution sandbox restrictions. The standard permission route was also unavailable. The supported existing-browser fallback could not reach the execution-local fixture listener. No sandbox/network workaround was performed. The fixture server was stopped.

**No desktop/mobile screenshots were generated or uploaded. No visual/accessibility, responsive layout, token-dialog, cancellation, or browser console pass is claimed.** To close this gap, run the fixture server and script in the same authorized browser-capable local environment (or provide supported local-server exposure to its browser), then review rendered screenshots and affected interactions.

## Remaining verification and product boundaries

Not run: live DNS rebinding/redirect-chain integration, malformed/truncated/large upstream stress, prolonged SSE reconnect/backoff/revocation soak, multiple real VPS hosts, TLS/reverse-proxy setup, backup/restore recovery, load testing, mobile browser interaction, accessibility audit. Tests use deterministic fixtures; they do not prove full RSS/Atom/XML specification coverage.

This slice has lexical search/keyword tags, bounded feed-link discovery, local-only publication, and a trusted single-authority coordinator/consumer transport. It does not prove autonomous web discovery, model classification/embeddings, signed open federation, ActivityPub delivery, ATProto publishing/firehose, thread conversation UI, full failover/lease-renewal automation, snapshot/resync/retention, or hardened account/session auth. See server README for protocol limits. Do not represent this QA as full V3 acceptance or public-deployment approval.
