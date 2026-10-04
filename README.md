# sidefeed

Sidefeed is a lightweight Rust service that turns RSS, Atom, JSON Feed, and
ActivityPub collections into named feeds. One SQLite file holds canonical
items, full-text indexes, feed definitions, peer state, and optional vectors.

It can render the same feed as REST JSON, RSS, JSON Feed, an SSE stream, a
newsletter-ready document, or a short social thread. Trusted Sidefeed nodes can
pull signed batches from each other, reuse already-fetched public items, and
avoid repeatedly hitting the same origin.

## Run it

```sh
cp .env.example .env
docker compose up --build
```

For a native build, install stable Rust and run `cargo run`. The default listen
address is `0.0.0.0:8080`; data is stored in `sidefeed.db`.

Open `http://localhost:8080/` for the reader: an unauthenticated view of every
feed marked `public`. It never asks for a token. Its only writes are bookmarks
and on-demand summaries, so adding sources and feeds still goes through the API
below, with reference documentation at `/docs` and an OpenAPI 3.1 document at
`/openapi.json`.

The reader can bookmark an item from a list row or the article header; saved
items live on the node and appear under `/saved`. `GET /api/v1/bookmarks`
returns them and `POST`/`DELETE /api/v1/items/{id}/bookmark` toggles one. A
related-reading list under each article comes from
`GET /api/v1/items/{id}/similar`, which blends the same source, the same link
host, shared derived tags, and shared title keywords.

The reader is compiled into the Sidefeed binary, so there is no separate asset
server. Point `SIDEFEED_WEB_DIR` at a directory to override `index.html`,
`app.js`, `styles.css`, `docs.html`, or `openapi.json`. Those files are read per
request, so UI edits go live without rebuilding the binary or restarting the
service.

Set a strong `SIDEFEED_ADMIN_TOKEN` outside a localhost-only deployment. Pass it
as `Authorization: Bearer <token>` to management routes. Feeds marked `public`
can be read without a token; everything else requires it.

## First feed

```sh
# Create a source.
curl -sS http://localhost:8080/api/v1/sources \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer change-me' \
  -d '{"url":"https://example.com/feed.xml"}'

# Use the source id from that response to create and connect a feed.
curl -sS http://localhost:8080/api/v1/feeds \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer change-me' \
  -d '{"slug":"reading","title":"Reading","public":true}'
curl -X POST http://localhost:8080/api/v1/feeds/reading/sources/SOURCE_ID \
  -H 'authorization: Bearer change-me'

# Fetch now, then read it in any supported form.
curl -X POST http://localhost:8080/api/v1/sources/SOURCE_ID/poll \
  -H 'authorization: Bearer change-me'
curl http://localhost:8080/api/v1/feeds/reading/items
curl http://localhost:8080/feeds/reading.rss
curl http://localhost:8080/feeds/reading.json
curl http://localhost:8080/feeds/reading/newsletter
curl http://localhost:8080/feeds/reading/thread.json
```

Import an OPML document by posting it as the request body to
`/api/v1/import/opml`. Only feeds created with `public: true` appear in the
reader; `GET /api/v1/public/feeds` is the unauthenticated index it uses. Search
uses SQLite FTS5 at
`/api/v1/feeds/{slug}/search?q=terms`. Live consumers can subscribe to
`/api/v1/feeds/{slug}/stream`.

## Peer cache

Configure each node with the other node's public base URL and the same random
secret:

```sh
curl -sS http://localhost:8080/api/v1/peers \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer change-me' \
  -d '{"base_url":"https://friend.example","shared_secret":"at-least-32-random-characters-here"}'

curl -X POST http://localhost:8080/api/v1/peers/PEER_ID/sync \
  -H 'authorization: Bearer change-me'
```

Synchronization is pull-based and only exports items marked public. Items use a
stable URL-derived ID where possible, so peer copies and origin copies converge
instead of multiplying.

## Filtering and embeddings

Feeds accept comma-separated `include_terms` and `exclude_terms`. These filters
are deterministic and always available.

AI is optional:

- `SIDEFEED_EMBEDDING_PROVIDER=remote` calls the JSON endpoint in
  `SIDEFEED_EMBEDDING_URL`. OpenAI-compatible `data[0].embedding` and a direct
  `embedding` field are accepted. A bearer token is optional.
- Build with `--features burn-local` and set the provider to `burn-local` for a
  384-dimensional, CPU-only lexical encoder backed by Burn's ndarray backend.
  It is intentionally tiny; use a remote model when semantic quality matters.

Create a vector with `POST /api/v1/items/{id}/embed`, then query hybrid
FTS/vector results at `/api/v1/feeds/{slug}/semantic?q=terms`.

## Configuration

| Variable | Default | Purpose |
|---|---:|---|
| `SIDEFEED_LISTEN` | `0.0.0.0:8080` | HTTP listen address |
| `SIDEFEED_DATABASE_URL` | `sqlite://sidefeed.db?mode=rwc` | SQLite URL |
| `SIDEFEED_PUBLIC_URL` | `http://localhost:8080` | Absolute output links |
| `SIDEFEED_ADMIN_TOKEN` | unset | Protect management and private feeds |
| `SIDEFEED_FETCH_INTERVAL_SECONDS` | `900` | Source polling interval |
| `SIDEFEED_FETCH_TIMEOUT_SECONDS` | `20` | Per-request timeout |
| `SIDEFEED_MAX_RESPONSE_BYTES` | `5242880` | Origin/peer body limit |
| `SIDEFEED_PEER_MAX_ITEMS` | `500` | Maximum peer batch |
| `SIDEFEED_RETENTION_DAYS` | `90` | Delete older cached items; `0` keeps all |
| `SIDEFEED_EMBEDDING_PROVIDER` | `disabled` | `disabled`, `remote`, `burn-local`, or `onnx-local` |
| `SIDEFEED_ONNX_EMBED_MODEL` | unset | Path to MiniLM `.onnx` for `onnx-local` |
| `SIDEFEED_AP_ENABLED` | `0` | Serve node actor, WebFinger, and signed inbox |
| `SIDEFEED_API_KEYS_ENABLED` | `0` | Require scoped keys even without admin token |
| `SIDEFEED_BOOKMARKS_REQUIRE_AUTH` | `0` | Gate bookmarks behind `bookmarks:write` |
| `SIDEFEED_RATE_LIMIT_RPS` | `10` | Global per-IP sustained rate |
| `SIDEFEED_RATE_LIMIT_BURST` | `30` | Global per-IP burst |
| `SIDEFEED_ENRICH_PROVIDER` | `disabled` | `disabled`, `heuristic`, or `openai` |
| `SIDEFEED_ENRICH_URL` | unset | Chat endpoint for `openai` enrichment |
| `SIDEFEED_ENRICH_MODEL` | unset | Chat model for `openai` enrichment |

Scoped API keys (`POST /api/v1/keys`, `DELETE /api/v1/keys/{id}`) gate management
reads (`read:private`) and writes (`write:private`); the admin bearer passes every
scope. Webhook ingress (`POST /api/v1/ingress/{slug}`) is token-free by channel
secret (Bearer or HMAC), capped at 100 items. ActivityPub is read-plus-follow only:
the node polls outboxes and sends signed Follows when `SIDEFEED_AP_ENABLED=1`; the
inbox verifies ed25519 `Signature` headers and tracks `Accept{Follow}`/`Create`.
`GET /api/v1/ai/status` reports live enrich/embedding providers and backlogs, and
`POST /api/v1/feeds/{slug}/ask` answers feed-scoped questions extractively when no
chat model is configured.

See [PLAN.md](PLAN.md) for delivery boundaries and [SECURITY.md](SECURITY.md)
before exposing a node publicly.

## Development

```sh
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --features burn-local
```

`cargo test` includes black-box E2E coverage of the real Axum router and a
temporary migrated SQLite database: UI/docs availability, health, management
authentication, feed creation, source attachment, item retrieval, FTS search,
RSS, JSON Feed, newsletter HTML, and social-thread JSON.

The project is licensed under MIT.
