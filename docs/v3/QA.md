# Sidefeed V3 prototype — independent QA

Verified 2026-10-31 at `ab40b23b17e948eee63ba6bf6e41e3e6dc151ca4`: [reading-room CI run](https://github.com/swarnimarun/sidefeed/actions/runs/41211874519) passed both jobs, including **53 automated tests, 16 syntax checks, scripted desktop/mobile browser flows, and container build/start/token smoke**. This is prototype verification, not complete V3 or production readiness. No live provider/account, external social publication or production deployment was exercised.

The approved Commonplace × Dispatch reading-room design is implemented. The final successful run captured 14 screenshots in artifact `13046154447`. Actual desktop reader, compact mobile reader, expanded mobile context and publish views were visually reviewed and looked correct. The final test-only refinement waits for toast dismissal and captures compact/expanded mobile states separately; runtime code is unchanged. Screenshot inspection is not a full accessibility audit.

## Exact checks and results

From the repository root:

```sh
cd v3
npm test
# PASS: 53 tests (combined backend and independent suites)
npm run check
# PASS: all server modules, web app and check runner (16 files)
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
3. **Protected UI lacked token input:** static review found no bearer support. The UI now includes memory-only owner token connection/disconnection (not localStorage). Owner token UI remains statically checked; the browser fixture uses local unauthenticated mode.
4. **Revoked fleet peers displayed Registered:** The UI now shows revoked status; covered by source review; no separate revoked-peer screenshot acceptance is claimed.
5. Mutation hardening: JSON content-type and cross-site fetch metadata rejection added and verified by independent HTTP tests.

## Stage 2: models, thread identity and fleet privacy

The combined backend, integration, model/thread, UI and reading-room regression suite has **53 passing tests**. Provider behavior uses injected fixture transports or stubbed HTTPS/DNS sockets, never actual provider calls.

- Disabled configuration, missing confirmation/provider-specific consent, missing private-source consent, unsupported operation, duplicate IDs and nonexistent item IDs cause zero provider calls.
- Only selected title/content enters provider request. Read/save flags, source identity and credentials are not article payload. Artifacts retain model/provider/input hash provenance; classification does not publish, create drafts, emit fleet events or execute model-suggested tools.
- Input and daily request caps are enforced before dispatch; failures consume reservations. Invalid JSON/schema, tool-shaped outputs, excess labels, invalid/zero/nonfinite/oversized vectors rejected without artifacts. Late provider output after cancellation is fenced.
- Similarity excludes stale content and incompatible model/provider/endpoint/dimension vectors; source embeddings are not represented as a currency spending cap or model-version detection beyond configured identifiers.
- Owner bearer gates model status/jobs/artifacts. Provider transport rejects unsafe/private URLs before sockets. Stubbed redirect response produces one request only (no bearer forwarding); oversized response is aborted.
- Thread request uses fixed public AppView URL and canonical DID record identity. Mutable handles are not authoritative identity. CID/parent/root refs survive normalization; blocked/not-found placeholders contain no fabricated author/content. Traversal caps mark partial/truncated graph.
- Fleet peer push retains validated thread metadata and strips private/model fields. Independent regression initially confirmed arbitrary model metadata persisted on consumer; corrected allowlist now passes, while preserving local read/save flags and idempotent receipt handling.

## Browser and container evidence

The successful [reading-room CI run](https://github.com/swarnimarun/sidefeed/actions/runs/41211874519) at `ab40b23b17e948eee63ba6bf6e41e3e6dc151ca4` covers deterministic desktop/mobile interactions: empty state, source creation/fetch, read/save/detail, search, reload persistence, discovery approval cancel/confirm/failure, local publish cancel/confirm, Back/Forward, and responsive overflow checks. Stage-2 assertions cover disabled-model/no-dispatch behavior and a synthetic thread with normal/blocked nodes and an explicit partial-snapshot label. The initial search/navigation race is fixed and the full scripted flow now passes.

Module-serving and pure-render regressions verify JavaScript assets are served, unsafe model/thread content is escaped, cyclic/deep thread rendering terminates, disabled controls are explicit and delayed navigation preserves newer search input. Container CI built the image, started it and checked token behavior. It is not a production deployment, cross-architecture qualification or a full backup/restore drill.

The browser suite uses [browser.py](../../v3/qa/browser.py) and [fixture-server.mjs](../../v3/qa/fixture-server.mjs). Fixtures disable model providers and replace external requests. An earlier local browser launch was restricted by its sandbox; authorized remote CI subsequently supplied the browser evidence above. That historical local limitation is no longer a blocker to the reported scripted coverage. Current-design screenshot acceptance is limited to the desktop reader, compact/expanded mobile and publish views explicitly reviewed above; other scripted assertions are not a claim of exhaustive visual inspection.

## Reading-room and protected-flow checks

The selected interface replaces dashboard cards with stacked editorial excerpts, linen/ivory surfaces, serif headings and a compact archive. Related entries state their actual same-source/shared-keyword basis; they are not AI recommendations. Responsive context is a closed disclosure until requested. Source details use stored metadata, not invented verification labels.

The successful browser run also exercised a token-protected fixture on port 8800: connect, reload requiring reconnect, disconnect clearing private state, and no token persistence in local/session storage or cookies. Model-consent cancel/confirm and unsaved-draft cancellation passed using fixtures, without an external provider call. These supplement the existing disabled-model, thread, navigation, source and local-publication flows.

## Remaining verification and product boundaries

Not run: live DNS rebinding/redirect-chain integration, malformed/truncated/large upstream stress, prolonged SSE reconnect/backoff/revocation soak, multiple real VPS hosts, TLS/reverse-proxy setup, backup/restore recovery, load testing, real mobile-device/browser testing, accessibility audit. Tests use deterministic fixtures; they do not prove full RSS/Atom/XML specification coverage.

This slice has lexical search/keyword tags, opt-in model classification/embedding adapters with provenance and scoped consent, bounded public thread normalization, bounded feed-link discovery, local-only publication, and trusted single-authority coordinator/consumer transport. It does not prove real-provider interoperability/quality, autonomous web discovery, signed open federation, ActivityPub delivery, ATProto publishing/firehose, complete conversation history or real-account thread interoperability, full failover/lease-renewal automation, snapshot/resync/retention, or hardened account/session auth. See server README for protocol limits. Do not represent this QA as full V3 acceptance or public-deployment approval.
