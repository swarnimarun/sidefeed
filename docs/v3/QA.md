# Sidefeed V3 prototype — independent QA

Verified 2026-10-31 against the local working tree. **49 automated tests passed, including stage 2 model/thread coverage. Local browser visual/interaction QA is blocked; remote CI is tracked separately. This is not complete V3 or production readiness.** The QA run performed no live publication, external account access, external origin fetch or deployment.

## Exact checks and results

From the repository root:

```sh
cd v3
npm test
# PASS: 49 tests (combined backend and independent suites)
npm run check
# PASS: all server modules, web app and check runner (15 files)
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

## Stage 2: models, thread identity and fleet privacy

Fifteen additional independent tests and nine implementation-owner tests were added; combined total, including six UI/module regressions, is **49 passing tests**. Provider behavior uses injected fixture transports or stubbed HTTPS/DNS sockets, never actual provider calls.

- Disabled configuration, missing confirmation/provider-specific consent, missing private-source consent, unsupported operation, duplicate IDs and nonexistent item IDs cause zero provider calls.
- Only selected title/content enters provider request. Read/save flags, source identity and credentials are not article payload. Artifacts retain model/provider/input hash provenance; classification does not publish, create drafts, emit fleet events or execute model-suggested tools.
- Input and daily request caps are enforced before dispatch; failures consume reservations. Invalid JSON/schema, tool-shaped outputs, excess labels, invalid/zero/nonfinite/oversized vectors rejected without artifacts. Late provider output after cancellation is fenced.
- Similarity excludes stale content and incompatible model/provider/endpoint/dimension vectors; source embeddings are not represented as a currency spending cap or model-version detection beyond configured identifiers.
- Owner bearer gates model status/jobs/artifacts. Provider transport rejects unsafe/private URLs before sockets. Stubbed redirect response produces one request only (no bearer forwarding); oversized response is aborted.
- Thread request uses fixed public AppView URL and canonical DID record identity. Mutable handles are not authoritative identity. CID/parent/root refs survive normalization; blocked/not-found placeholders contain no fabricated author/content. Traversal caps mark partial/truncated graph.
- Fleet peer push retains validated thread metadata and strips private/model fields. Independent regression initially confirmed arbitrary model metadata persisted on consumer; corrected allowlist now passes, while preserving local read/save flags and idempotent receipt handling.

Remote CI is a separate verification route: [initial V3 CI run](https://github.com/swarnimarun/sidefeed/actions/runs/41207198388) at commit `75ab226` passed container build/start/token smoke. That run predates stage 2; it is **not stage 2 deployment or browser acceptance**. Remote browser QA on that baseline passed initial empty state, source creation/fetch, read/save, detail and reload persistence, then failed because a search query was lost during asynchronous navigation/render. That race is fixed and its pure navigation regression passes; a fresh browser run is pending. Subsequent search, agent/publication, history and mobile assertions were not completed in that run. Fresh stage 2 CI/browser verification remains pending; the final result must be appended before broader acceptance.

Additional module-serving and pure-render tests verify new JavaScript assets are served, unsafe thread/model content is escaped, cyclic/deep thread rendering terminates, disabled model controls are explicit, and delayed hash navigation preserves newer search input.

The browser script now includes disabled-provider/no-dispatch assertions and an explicit synthetic AppView fetch with visible normal/blocked nodes and a partial-snapshot label. The fixture always disables provider configuration even if environment variables exist. Python compilation, fixture syntax, and a local HTTP smoke of disabled model status plus the two-node synthetic thread passed; new browser assertions await CI.

## Local browser status: BLOCKED / NOT RUN

Prepared [browser.py](../../v3/qa/browser.py) for deterministic desktop/mobile flows and [fixture-server.mjs](../../v3/qa/fixture-server.mjs) for local fixtures. Intended browser coverage: empty/read/save/detail/search; source fetch; discovery approval cancel/confirm/failure; local publish cancel/confirm; Back/Forward; 390px/mobile overflow and 1440px desktop screenshots. **None of those browser assertions ran in this local QA environment.** The script is a starting point and may need selector adjustments for subsequent UI edits.

Attempt:

```sh
cd v3
node qa/fixture-server.mjs
python qa/browser.py
```

The installed browser could not launch under the execution sandbox restrictions. The standard permission route was also unavailable. The supported existing-browser fallback could not reach the execution-local fixture listener. No sandbox/network workaround was performed. The fixture server was stopped.

**This local QA run generated/uploaded no desktop/mobile screenshots. It does not claim a visual/accessibility, responsive layout, token-dialog, cancellation, or browser console pass.** To close this gap, run the fixture server and script in the same authorized browser-capable local environment (or provide supported local-server exposure to its browser), then review rendered screenshots and affected interactions.

## Remaining verification and product boundaries

Not run: live DNS rebinding/redirect-chain integration, malformed/truncated/large upstream stress, prolonged SSE reconnect/backoff/revocation soak, multiple real VPS hosts, TLS/reverse-proxy setup, backup/restore recovery, load testing, mobile browser interaction, accessibility audit. Tests use deterministic fixtures; they do not prove full RSS/Atom/XML specification coverage.

This slice has lexical search/keyword tags, opt-in model classification/embedding adapters with provenance and scoped consent, bounded public thread normalization, bounded feed-link discovery, local-only publication, and trusted single-authority coordinator/consumer transport. It does not prove real-provider interoperability/quality, autonomous web discovery, signed open federation, ActivityPub delivery, ATProto publishing/firehose, browser-rendered conversation UI, full failover/lease-renewal automation, snapshot/resync/retention, or hardened account/session auth. See server README for protocol limits. Do not represent this QA as full V3 acceptance or public-deployment approval.
