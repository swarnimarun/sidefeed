# Server slice

Node 24 built-in HTTP and SQLite, with the pinned vendored sax 1.6.1 XML dependency (BlueOak-1.0.0; see `vendor/README.md`); no runtime package installation required. `npm start` serves the UI and API at loopback port 8787. `npm test` uses injected fixture transport, no external network or credentials. This is a working prototype, not a hardened production release.

## Fleet API (owner calls `/api/v3`, peer credentials separate)

1. Create a source on coordinator and on consumer. On coordinator PATCH `/sources/{id}` with `{"share":true}` explicitly opts that source into replication.
2. Owner POST `/fleet/peers` with `{"name":"home","sourceIds":["COORDINATOR_SOURCE_ID"],"confirm":true}`. Save the one-time `token` securely; only its SHA-256 hash is stored. DELETE `/fleet/peers/{id}` revokes it.
3. Peer bearer credential accesses `/fleet/transport/lease` (POST `{sourceId}`), `/events` (GET cursor replay or POST `{sourceId,epoch,eventId,items}`), `/stream` (SSE with Last-Event-ID), and `/ack` (POST `{cursor}`). A 30-second peer lease fences accepted results. Local manual refresh acquires a 60-second local lease and checks it before accepting results. Normalized item fields: externalId, title, content (plain text), url, author, publishedAt. Peer ingress rejects stale owner/epoch and deduplicates event IDs.
4. On consumer, explicitly map coordinator IDs to **existing** local source IDs. Mark local sources paused to prevent independent manual polling. Run alongside the app with the same DB:

```sh
SIDEFEED_COORDINATOR=https://coordinator.example \
SIDEFEED_PEER_TOKEN='ONE_TIME_PEER_TOKEN' \
SIDEFEED_SOURCE_MAP='{"COORDINATOR_SOURCE_ID":"LOCAL_SOURCE_ID"}' \
npm run fleet:receive
```

Worker consumes cached content deltas over SSE, preserving private read/save state, persisting receive cursors and deduplication, reconnecting with bounded backoff. It never fetches source origins. Plain HTTP is accepted only for explicitly configured loopback test coordinators. Authentication failures terminate the worker. TLS must be provided externally. Secrets in this shell example are placeholders; use environment/secret injection rather than shell history in deployments.

**Limits:** one coordinator, bearer-authenticated trusted transport rather than signed event protocol, no peer discovery/autojoin, remote polling worker, lease renewal worker, source removal propagation, snapshot/resync/retention protocol, tombstones, per-peer quotas or restore fencing yet. Events retained without compaction. Manual refresh and the explicitly enabled bounded local scheduler trigger origin polling. This is not a globally exactly-once polling guarantee. Disable sharing stops future delivery but does not recall previous copies. Consumer maps are operator-authorized, not discovered from content.

## Security and implementation limits

Loopback mode has Host allowlisting and same-origin/JSON mutation checks. Remote binding requires a >=24-character owner bearer token (UI may provide one), plus external HTTPS. There is no password/session/device account system yet. Public `/public/feed.json` includes only locally published posts; drafts are never returned there. Explicit publication is local only, no federation delivery.

All source and agent network requests use one HTTP(S) fetcher: credential-free URLs, private address rejection, DNS address pinning, redirect revalidation, response/body/time caps, no decompression. The bounded feed adapter uses vendored sax in strict namespace mode for RSS 1 RDF, RSS 2 and Atom, with separate JSON Feed/ATProto adapters. DTDs/custom entities are rejected; predefined/numeric XML entities and CDATA are supported, output rendered as plain text. Byte/depth/node/attribute caps apply. It is not a full RSS/Atom schema validator; transport supports UTF-8 XML only. Malformed XML, namespaces, RDF identities/order and Atom alternate links have deterministic fixture coverage. See `SUBSCRIPTIONS.md` for public profile pulls.

Baseline keyword tagging is deterministic. Explicitly consented model classification, embeddings, cited summaries and bounded local-library research are implemented below. Website discovery extracts feed links and requires approval before subscribing. There is no autonomous browser, external search engine, arbitrary execution or autonomous publishing.

## Opt-in model provider (fixture-tested)

Set these **server-side only** to enable an OpenAI-compatible provider:

- `SIDEFEED_MODEL_ENDPOINT`: HTTPS base endpoint, e.g. `https://provider.example/v1` (no credentials/query in URL).
- `SIDEFEED_MODEL_KEY`: secret bearer credential, never returned to browser or stored in SQLite.
- `SIDEFEED_MODEL_PROVIDER`: human-readable consent identity (default `configured-provider`).
- `SIDEFEED_CLASSIFICATION_MODEL`, `SIDEFEED_EMBEDDING_MODEL`: explicit model IDs; each operation unavailable if its ID is absent.
- `SIDEFEED_MODEL_DAILY_REQUESTS`: daily reserved request cap (default 20, maximum 1000).

No provider is enabled by default; adding sources never invokes one. Owner POST `/api/v3/models/jobs` with operation `classify` or `embed`, 1–5 itemIds, `confirm:true`, and `consent:{provider:STATUS_PROVIDER,sendContent:true,includePrivate:true}` explicitly approves title/content export. Separate `includePrivate` consent is required unless all selected sources explicitly enable sharing. GET `/models/status` discloses provider, endpoint, models, caps and billing caveat. Jobs and artifacts persist with selected IDs, content hashes, consent, requested and actual model identity, dimensions and provenance. Structured output validation rejects malformed labels/vectors; model text never becomes a command or executable tool call.

Jobs run asynchronously and expose queued/running/succeeded/failed/canceled states. POST `/models/jobs/{id}/cancel` aborts current transport. Partial completed artifacts remain visible; cancel cannot recall content or guarantee no charge. Restart marks unfinished work failed with uncertain remote outcome. **No automatic paid retry.** An owner may submit a new explicitly consented job. Daily reservations include failed/canceled jobs conservatively.

Limits are request count, 12,000 input characters per item, 500 requested classification output tokens, bounded response bytes and deadlines—not currency-cost accounting. Provider usage is retained if returned. No pricing lookup, dollar-budget guarantee, local model runner, source suggestions, or arbitrary agent model tools are implemented. Model-generated feed suggestions are not implemented in this slice and remain required V3 backlog; deterministic discovery proposals still require separate approval.

POST `/models/similar` with `{itemId,limit}` computes cosine similarity locally only after real provider embeddings exist. It excludes changed-content artifacts and incompatible requested/actual model/provider/endpoint/dimensions. No synthetic embedding fallback. GET `/models/artifacts` returns current stored artifacts; `/models/jobs` lists jobs.

Provider POST transport pins public DNS, requires HTTPS and rejects every redirect (credentials never forwarded). Test suite uses fixture transports only; live-provider interoperability and billing have **not** been tested.

## Reader search, cited summaries and library research

`GET /api/v3/search` is local FTS5-ranked lexical search with bounded fuzzy spelling expansion and filters; no provider calls. `POST /api/v3/search` with `mode:hybrid` requires `consent:{provider,sendQuery:true,confirm:true}` and sends the query (not article content) for a real embedding. Only stored fresh item embeddings matching provider, endpoint, requested/actual model and dimensions participate. A query request reserves one request in the same daily model-job ledger. `GET /items/{id}/similar` is local related reading; method/provenance distinguish shared-term evidence from compatible stored embeddings.

`POST /models/jobs` accepts `operation:summary`, explicit selected itemIds and content/private consent. Query-conditioned summaries also require sendQuery consent. `SIDEFEED_SUMMARY_MODEL` selects its model; the classification model is fallback when configured. Summaries return source item-ID citations validated against supplied inputs, not a claim of automated factual entailment checking.

`POST /agents/search` accepts query, an explicit nonempty sourceIds scope, confirm and provider/query/content/private consent. This is **local-library agentic research, not autonomous public-web search**. The model may issue only strict JSON `search`, `read_items`, `finalize` actions; every returned action is validated, not executed as free text. Four provider requests maximum are reserved up front. Search examines only approved source scope; reads require previously discovered IDs and send at most 3,000 content characters per item. Context/output/step limits and cancellation fence late responses. Finalization must cite actually read items. Trace records tool steps and exported content hashes/truncation. Instructions embedded in sources cannot grant publication, follow, command execution, external search or arbitrary network tools. The provider can still produce inaccurate prose; citations identify evidence, not certify correctness.

Agent jobs share `/models/jobs` persistence, budget accounting, cancellation and restart-uncertain semantics; `GET /agents/search` filters them. Provider errors never create successful summaries. No automatic paid retry. All tests use mocked providers. No external search connector is configured or silently substituted.

## Public profile subscriptions and polling

`POST /subscriptions/preview` with input and kind (RSS default, `atproto`, `activitypub`) fetches public evidence without creating a subscription. `POST /subscriptions` with previewId and confirm:true commits the ten-minute preview once. It imports its normalized items and canonical public profile metadata; it never sends a federation Follow or authenticated social write. See `SUBSCRIPTIONS.md` for supported protocol subset and limitations.

Manual source refresh persists conditional ETag/Last-Modified state, pagination/backfill cursor and bounded retry backoff; new-source content never invokes a model. Explicit `SIDEFEED_AUTO_POLL=true` enables one sequential scheduler batch every 60 seconds, at most two due unpaused sources per batch, with five-minute normal source intervals and failure backoff. It is disabled by default. Fleet leases still fence coordinated refresh results. Consumer sources should remain paused to avoid independently polling their origins. This lightweight scheduler is not a durable distributed job/lease-renewal system.
