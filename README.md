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

Set a strong `SIDEFEED_ADMIN_TOKEN` outside a localhost-only deployment. Pass it
as `Authorization: Bearer <token>` to management routes.

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
`/api/v1/import/opml`. Search uses SQLite FTS5 at
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
| `SIDEFEED_EMBEDDING_PROVIDER` | `disabled` | `disabled`, `remote`, or `burn-local` |

See [PLAN.md](PLAN.md) for delivery boundaries and [SECURITY.md](SECURITY.md)
before exposing a node publicly.

## Development

```sh
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --features burn-local
```

The project is licensed under MIT.
