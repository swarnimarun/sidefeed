# Sidefeed V3 prototype — independent QA

Verified 2026-10-31. [Fresh-frontend CI](https://github.com/swarnimarun/sidefeed/actions/runs/41214495602) at `723557df29b431041a0cb58cb657e7325285439c` passed **62 tests, 23 recursive JavaScript syntax checks, the full browser/protected-token/consent/draft/navigation suite and container smoke**. Actual desktop/mobile screenshots in artifact `13047443684` passed visual comparison against five selected-concept criteria: working masthead navigation, reading-only archive, bold serif hierarchy, 65/35 article/context layout, and phone bottom navigation with the third headline above it. This is objective comparison evidence, not pixel identity or user approval of the latest delivery. No real external provider/account call, social publication or host deployment was performed.

## Fresh frontend scope

The current web client separates API/token handling, state/filtering, history routing, shell, reading pages, workspace pages/actions and model/thread evidence into ES modules. Core masthead routes are Feed, Threads, Sources, Compose and Search; a reading-only archive contains actual source/read/save filters. Node tools live in Utilities. Mobile has a reading drawer, bottom navigation, and closed related/source disclosures. Sources and settings disclose technical controls progressively; Threads is an explicit dedicated page. Related stories use real source/tag matches, not fabricated model output.

The adapted browser script retains the three ingested synthetic stories and both isolated fixture nodes: 8799 has a disabled provider for honest screenshots; 8800 requires a synthetic owner token and uses a mock-only classifier to test consent. It preserves read/save/reload, real navigation/history, related/source details, source/discovery confirmation, private-draft editing/publication, memory-only authentication, model approval cancel/confirm and bounded thread assertions. Module routes, escaped story rendering, genuine filters and the actual router's duplicate-event/search race also have independent unit coverage.

## Exact checks and results

From the repository root:

```sh
cd v3
npm test
# PASS: 62 tests (combined backend and independent suites)
npm run check
# PASS: all server modules, web app and check runner (23 files)
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
3. **Protected UI lacked token input:** static review found no bearer support. The UI now includes memory-only owner token connection/disconnection (not localStorage). The protected fixture exercises memory-only connect/reload/disconnect; the adapted protected browser flow passed on this rewrite.
4. **Revoked fleet peers displayed Registered:** The UI now shows revoked status; covered by source review; no separate revoked-peer screenshot acceptance is claimed.
5. Mutation hardening: JSON content-type and cross-site fetch metadata rejection added and verified by independent HTTP tests.

## Stage 2: models, thread identity and fleet privacy

The combined backend, integration, model/thread, UI and reading-room regression suite has **62 passing tests**. Provider behavior uses injected fixture transports or stubbed HTTPS/DNS sockets, never actual provider calls.

- Disabled configuration, missing confirmation/provider-specific consent, missing private-source consent, unsupported operation, duplicate IDs and nonexistent item IDs cause zero provider calls.
- Only selected title/content enters provider request. Read/save flags, source identity and credentials are not article payload. Artifacts retain model/provider/input hash provenance; classification does not publish, create drafts, emit fleet events or execute model-suggested tools.
- Input and daily request caps are enforced before dispatch; failures consume reservations. Invalid JSON/schema, tool-shaped outputs, excess labels, invalid/zero/nonfinite/oversized vectors rejected without artifacts. Late provider output after cancellation is fenced.
- Similarity excludes stale content and incompatible model/provider/endpoint/dimension vectors; source embeddings are not represented as a currency spending cap or model-version detection beyond configured identifiers.
- Owner bearer gates model status/jobs/artifacts. Provider transport rejects unsafe/private URLs before sockets. Stubbed redirect response produces one request only (no bearer forwarding); oversized response is aborted.
- Thread request uses fixed public AppView URL and canonical DID record identity. Mutable handles are not authoritative identity. CID/parent/root refs survive normalization; blocked/not-found placeholders contain no fabricated author/content. Traversal caps mark partial/truncated graph.
- Fleet peer push retains validated thread metadata and strips private/model fields. Independent regression initially confirmed arbitrary model metadata persisted on consumer; corrected allowlist now passes, while preserving local read/save flags and idempotent receipt handling.

## Browser and container evidence

[Fresh-frontend CI](https://github.com/swarnimarun/sidefeed/actions/runs/41214495602) at `723557df29b431041a0cb58cb657e7325285439c` passed **62 tests, 23 recursive JavaScript syntax checks, the full browser/protected-token/consent/draft/navigation suite and container smoke**. Actual desktop/mobile screenshots in artifact `13047443684` passed visual comparison against five selected-concept criteria: working masthead navigation, reading-only archive, bold serif hierarchy, 65/35 article/context layout, and phone bottom navigation with the third headline above it. This is objective comparison evidence, not pixel identity or user approval of the latest delivery.

The suite is [browser.py](../../v3/qa/browser.py), backed by [fixture-server.mjs](../../v3/qa/fixture-server.mjs). Synthetic fixtures cover reader/source/discovery, private drafts/local publication, dedicated Threads, memory-only token connect/reload/disconnect, model approval cancel/confirm and navigation. The screenshot review covers actual rendered desktop/mobile interfaces, not generated mockups. Earlier styling-only evidence does not substitute for this fresh implementation's checks.

## Remaining verification and product boundaries

Not run: live DNS rebinding/redirect-chain integration, malformed/truncated/large upstream stress, prolonged SSE reconnect/backoff/revocation soak, multiple real VPS hosts, TLS/reverse-proxy setup, backup/restore recovery, load testing, real mobile-device/browser testing, accessibility audit. Tests use deterministic fixtures; they do not prove full RSS/Atom/XML specification coverage.

This slice has lexical search/keyword tags, opt-in model classification/embedding adapters with provenance and scoped consent, bounded public thread normalization, bounded feed-link discovery, local-only publication, and trusted single-authority coordinator/consumer transport. It does not prove real-provider interoperability/quality, autonomous web discovery, signed open federation, ActivityPub delivery, ATProto publishing/firehose, complete conversation history or real-account thread interoperability, full failover/lease-renewal automation, snapshot/resync/retention, or hardened account/session auth. See server README for protocol limits. Do not represent this QA as full V3 acceptance or public-deployment approval.
