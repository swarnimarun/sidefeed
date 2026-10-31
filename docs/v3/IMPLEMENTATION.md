# Sidefeed V3: run the working prototype

This is a ground-up local prototype, not the completed V3 product or a production deployment. The implementation lives in repository directory `v3/`; the architecture and procedural plans live in `docs/v3/`. V1 and the older `v2` branch are not replaced.

## Requirements and native run

Node.js 24 or newer is required for built-in SQLite. The XML parser is pinned, bundled sax 1.6.1 (BlueOak-1.0.0); there is no `npm install` step. Preserve its bundled license when distributing. See [vendor provenance](../../v3/server/vendor/README.md). SQLite may print an experimental-feature warning on the selected Node release.

From the repository root:

```sh
cd v3
npm start
```

Open **http://127.0.0.1:8787**. Default binding is loopback; without a configured token this is a single-owner, trusted-local-machine mode, not protection against other users/processes on the same machine. Stop with Ctrl-C. Data defaults to `v3/data/sidefeed.sqlite` and survives restarts.

| Setting | Default | Behavior |
|---|---|---|
| `HOST` | `127.0.0.1` | Non-loopback binding requires a token of at least 24 characters |
| `PORT` | `8787` | HTTP port |
| `SIDEFEED_DB` | `data/sidefeed.sqlite` | SQLite path, relative to working directory unless absolute |
| `SIDEFEED_TOKEN` | unset | When set, owner API calls require `Authorization: Bearer <token>`; peer transport uses separately granted credentials |

The native server does not automatically load `.env`. Set environment variables in the launching shell or your service manager. Generate a secret rather than copying an example:

```sh
export SIDEFEED_TOKEN="$(node -e "process.stdout.write(require('node:crypto').randomBytes(32).toString('hex'))")"
npm start
```

In the web UI, choose **Connect** and enter the owner token (the form also appears after a 401 response). The token stays in page memory only; reloading requires reconnecting. **Disconnect** clears the token and private view/editor data.

Store the token in a password manager or protected service configuration, not source control or logs. A token does not encrypt HTTP. Do not expose the process directly to the internet; remote use requires TLS and additional security/operational review.

## Checks

Run from `v3/`:

```sh
npm test
npm run check
```

[Reader-feature CI](https://github.com/swarnimarun/sidefeed/actions/runs/41218120045) at `2d1d21a06bd3a0d591b6023d69e48ed74d2d9fd0` passed **124 tests, 32 syntax checks, the full browser suite and container smoke**. Artifact `13048654690` contains 20 actual screenshots; Search, fuzzy results, Ask and public ATProto/ActivityPub subscription previews were visually reviewed. All provider and social-origin responses were fixtures; no live provider/account interoperability or host deployment was tested. See [QA](QA.md).

Tests use Node's built-in test runner. `check` performs syntax checks on all server modules and the web application; it is not a full security audit or browser test. See the commit's verification report for observed results; listing a command here does not assert it has passed on every platform.

## Container packaging

The Dockerfile uses the official `node:24-bookworm-slim` image and runs as the non-root `node` user (UID 1000). Its data volume is `/data`; the image initializes ownership for a new named volume. Existing/bind-mounted directories require matching write permissions. The image tag is not digest-pinned yet: record/pin a validated digest before release.

The Compose file requires `SIDEFEED_TOKEN`, publishes only `127.0.0.1:8787`, uses a read-only root filesystem, drops capabilities and rotates logs. The server listens on all interfaces **inside the container**, so its minimum-24-character token check applies even though the host port is loopback-only. An empty token fails Compose interpolation; a short token fails application startup.

After generating/exporting a token as above, the intended local container workflow is:

```sh
# From v3/, on a host with Docker and Compose installed:
docker compose build
docker compose up -d
docker compose logs --tail=50
# Stop without deleting stored data:
docker compose down
```

**Verification boundary:** [fresh-frontend CI run](https://github.com/swarnimarun/sidefeed/actions/runs/41218120045) at `2d1d21a06bd3a0d591b6023d69e48ed74d2d9fd0` passed container build/start/token smoke. Local Docker unavailability was a historical tooling limitation, not a failure of that remote check. This does not validate every operator's Compose host, both CPU architectures, backup/restore or production readiness. Require fresh-volume persistence/recreation and non-root ownership checks on the intended host before valuable use. No external deployment was performed. No healthcheck is declared yet; absence of a failing probe is not evidence of readiness. Never use `docker compose down -v` unless intentional permanent data deletion is authorized.

## What this slice implements

The authoritative runtime summary is `GET /api/v3/status` (owner token required if configured).

- Local SQLite-backed sources/items/read-save state, named feeds, FTS5/fuzzy/hybrid search, RSS 1/2/Atom/JSON input and public ATProto/ActivityPub subscription previews. `GET /api/v3/threads?uri=<DID-based-at://post-URI>` retrieves a bounded public AppView thread graph, preserving reply/root relationships and unavailable-node states. The graph is explicitly partial, not a complete historical thread; it does not authenticate accounts or validate repository signatures.
- On-demand conditional refresh plus optional local auto-polling: `SIDEFEED_AUTO_POLL=true` (disabled by default), sequential batches every 60 seconds with at most two due unpaused sources, normal five-minute intervals, backoff and lease guards. This is not distributed scheduler/failover automation.
- Deterministic website feed discovery with human approval before subscription and baseline keyword tagging. Separately, an explicitly configured OpenAI-compatible provider supports real classification/embedding jobs with consent, persistent provenance and local cosine similarity over compatible provider embeddings; see configuration below. Discovery is not an autonomous model-driven agent.
- Local drafts and explicit local publishing, with published posts exposed at `/public/feed.json`. This endpoint is intentionally public: any party able to reach the server can read published posts even if owner API authentication is enabled. It does not send posts to ActivityPub or ATProto.
- Trusted-peer source grants, coordinator-local leases with epochs, credential revocation, cached event replay, authenticated SSE deltas and acknowledgments. `npm run fleet:receive` runs the receiver CLI with `SIDEFEED_COORDINATOR`, `SIDEFEED_PEER_TOKEN`, `SIDEFEED_SOURCE_MAP` (JSON remote-to-existing-local source IDs) and `SIDEFEED_DB`; use HTTPS except on loopback and manually pause mapped local sources to avoid independent polling. See [server configuration and API guide](../../v3/server/README.md) for exact setup and limitations. The receiver persists its cursor and reconnects. A controlled test covers cached SSE delivery to a second node and idempotent replay. These are protocol primitives, not a deployed autonomous multi-node push mesh, quorum failover or a guarantee of globally unique origin polling. Already-in-flight origin requests can overlap failover despite fencing stale results.
- A fresh modular Commonplace × Dispatch reading-room implementation: warm linen/ivory, stacked serif excerpts, reading-only archive, working masthead navigation, phone bottom navigation and responsive related/source context based on actual same-source/shared-keyword matches—not model recommendations. Model consent/job/artifact controls and a thread inspector remain available, with explicit empty states. See [design contract](DESIGN.md) and [web tokens/source guide](../../v3/web/README.md). An explicit demo API can load labeled fixtures; it does not fetch news or connect to a live federation network.

Use only trusted test data while evaluating. Do not infer production readiness, third-party protocol interoperability or measured cheap-host capacity from this first slice.

## Opt-in model setup and limits

Provider configuration is server-side only. The default remains disabled; subscribing or refreshing never calls a model. For a native run, set these before `npm start`:

| Variable | Purpose |
|---|---|
| `SIDEFEED_MODEL_ENDPOINT` | HTTPS OpenAI-compatible base URL, no embedded credentials/query |
| `SIDEFEED_MODEL_KEY` | Provider bearer credential; never send it to the browser |
| `SIDEFEED_MODEL_PROVIDER` | Identity displayed in consent; default `configured-provider` |
| `SIDEFEED_CLASSIFICATION_MODEL` | Explicit classification model ID |
| `SIDEFEED_EMBEDDING_MODEL` | Explicit embedding model ID |
| `SIDEFEED_SUMMARY_MODEL` | Summary/research model; configured classification model is fallback |
| `SIDEFEED_MODEL_DAILY_REQUESTS` | Daily reserved request cap, default 20, maximum 1000 |

Each operation requires its model ID. The supplied Compose file does **not** pass model variables or `SIDEFEED_AUTO_POLL` through. Enabling providers or auto-polling in containers requires an explicit private Compose override/service environment configuration. Do not commit credentials or assume a host-shell variable reaches the container automatically.

In the model UI, inspect provider/endpoint and selected content before consenting. Owner `POST /api/v3/models/jobs` accepts `operation` (`classify` or `embed`), 1–5 `itemIds`, `confirm:true` and `consent:{provider:STATUS_PROVIDER,sendContent:true,includePrivate:true}`; private-content export requires the separate `includePrivate` approval. Status, jobs and artifacts are available at `/api/v3/models/status`, `/jobs` and `/artifacts`. The [server guide](../../v3/server/README.md#opt-in-model-provider-fixture-tested) specifies the complete API and retention/provenance behavior.

Limits cover reserved request count, 12,000 input characters per item, 500 requested classification-output tokens, response bytes and deadlines—not a dollar budget or billing guarantee. Jobs persist state/content hashes, requested/actual model identities and returned usage. Cancel aborts transport but cannot recall content or guarantee no charge. Restart fails unfinished work as uncertain; there is no automatic paid retry. Failed/canceled jobs conservatively consume request reservations. Re-submission requires renewed explicit owner action.

`POST /api/v3/models/similar` with `{itemId,limit}` computes local cosine similarity only over real, compatible embeddings; changed-content and incompatible provider/model/dimension artifacts are excluded. No synthetic vector fallback is used. Model text is never executed as a command/tool action. Tests use fixtures; live-provider interoperability, model quality and billing remain unverified.

## Reader search and subscriptions

FTS5-ranked phrases/Unicode/filter search, indexed bounded fuzzy matching, consented query-embedding hybrid retrieval, stored-vector related reading, cited summaries and a scoped local-library research loop are implemented with fixtures. Public ATProto authored feeds and a conservative ActivityPub outbox subset support preview then explicit local subscription; neither performs an authenticated social write. See [SEARCH-SUBSCRIPTIONS.md](SEARCH-SUBSCRIPTIONS.md) for routes, consent, limits, conditional polling and parser licensing.

## Fresh frontend layout

The frontend was rewritten as dependency-free ES modules, not merely recolored. `web/api.js` owns requests and memory-only credentials; `state.js` owns reader/filter state; `router.js` owns history; `components/` owns the shell, safe rendering and evidence; `pages/` and `actions/` own reader/workspace flows. Three legacy root UI helpers were removed. See the [web source guide](../../v3/web/README.md).

Source controls are progressive, with discovery and approval integrated. Compose is a paper-like editor; Threads is a dedicated page. Utilities hold node tools, with advanced JSON/evidence collapsed. Library-wide `status.readingCounts`, server-side read filtering and author-aware search support genuine reading navigation. API lists remain capped at 1,000 and the view discloses the cap; related entries describe matches within loaded data, not the entire corpus.

## Data safety now

No backup/restore CLI is shipped in this slice. Before valuable use, stop every writer and take a consistent copy of the **whole data directory**, including any SQLite WAL sidecars. Do not copy only a live database file. Protect the backup as sensitive data; it includes private content and credential-related state. Restore to a separate volume/path and verify it privately before replacing an installation. Restore must not reactivate stale fleet ownership or replay external publication; those recovery workflows need implementation and tests before live federation. Automated backup, schema migration compatibility and rollback guarantees remain backlog work.

## Required V3 completion backlog

These remain required product scope; implementation stages do not remove them:

1. Complete and harden feed management: robust parsing/discovery, OPML roundtrip, organization, full lifecycle semantics, scheduling/backoff, retention, authenticated reading and private/public boundaries.
2. Complete AI workflows beyond fixture-tested provider jobs, summaries and local-library research: live-provider verification, live hybrid-retrieval quality, suggestions, local provider profiles, deliberate reindexing, recovery UX and cost visibility. Existing request/character/token limits are not monetary budgets. Model-generated source suggestions remain required product backlog, not implemented discovery behavior.
3. Networked readers and trusted fleets: autonomous polling workers, explicit source-sharing consent, normally one lease-authorized poll initiator per cohort, durable push/subscription delivery, gap recovery, failover/partition tests, peer lifecycle and observable queues.
4. Publishing/microblogging: author/visibility controls, durable outbox, media quotas, edits/deletes, stable identity, delivery status and recovery-safe retries.
5. Complete bidirectional ActivityPub beyond public outbox pulls: authenticated follow/accept/undo, inbound/outbound delivery, signed writes, visibility/abuse controls and live interoperability. Public preview/confirmed local subscription is implemented; authenticated federation is not.
6. ATProto: integration beyond public authored-feed subscriptions and bounded thread snapshots, authenticated account/publishing flows, stream/firehose or supported sync strategy, moderation/identity validation and protocol interoperability. A public AppView graph is not authenticated sync or complete reply recovery.
7. Agent discovery/harness: scoped tools, explicit human approval, provenance, budgets, auditable runs, prompt-injection boundaries and safe source/network proposals. Website discovery is deterministic; the model-backed library research loop is scoped to local sources, not autonomous external-web discovery.
8. Operations: verified amd64/arm64 images, CI/browser/security gates, backup/restore and migration drills, controlled multi-node/federation test rigs, resource measurements and an explicitly authorized deployment. Cheap-VPS/home-server sizing remains a target, not a measured guarantee.
