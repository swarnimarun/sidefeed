# Security model

Sidefeed consumes untrusted documents and should be deployed as an unprivileged
service. Source and peer URLs are limited to HTTP(S), DNS results are checked
against private and non-routable address ranges, every redirect is rechecked,
responses are bounded, and requests time out. DNS is validated at resolution
time only; the connection is not pinned to the validated address, so a DNS
change between check and connect (TOCTOU) could bypass the denylist. Pinning
the validated IP for the connection was deferred as too invasive for this
change. Treat network egress policy as a required second layer, not a
backstop, until pinning lands.

Set `SIDEFEED_ADMIN_TOKEN` on every internet-accessible node. Without it,
administrative endpoints are intentionally open for single-user localhost
deployments. Public feeds and the reader's saved list require no token; private
feeds and every other mutation do.

Token-free entry points are public reads, bookmarks (gated by `SIDEFEED_BOOKMARKS_REQUIRE_AUTH`),
webhook ingress by channel secret (`POST /api/v1/ingress/{slug}`), and the AP inbox
by signature (`POST /ap/v1/inbox`). Bookmarks are single-user and bounded: the node
refuses to store more than a fixed number of bookmarks, and saving only ever references
an item that already exists, so the route cannot be used to inject content. On a shared
or hostile network, front the service with a reverse proxy that restricts `POST`/`DELETE`
on `/api/v1/items/*/bookmark` if you do not want the saved list to be world-writable.

Peers are explicitly configured and authenticate exports with HMAC-SHA256.
Both peers must use the same 32-or-more-character secret. Signatures cover the
timestamp, method, path, and query and expire after five minutes. TLS is still
required to conceal feed data and signatures in transit.

Rotate a peer secret with `POST /api/v1/peers/{id}/rotate` (admin bearer):
the response carries the fresh secret exactly once plus `expires_old_at`.
The previous secret keeps verifying for 24 hours so the other node can roll
over without a synchronized maintenance window; after the grace expiry it
fails closed. Rotating again supersedes the earlier window. Copy the fresh
secret to the peer over an already-trusted channel and confirm a sync before
the expiry. Treat the SQLite file as secret-bearing: it holds peer secrets
(current and grace-window) alongside fetched content.

SQLite contains fetched content, peer secrets, and optional embedding vectors.
Back up the database with SQLite's online backup tooling or while Sidefeed is
stopped, and protect the file as a secret-bearing asset.

## ActivityPub read-plus-follow and webhook channels (lane-ingest)

ActivityPub support is read-plus-follow only: the node polls actor outboxes
and can send a signed Follow, but it is not a general-purpose server (no
multi-user inboxes, no relays). The node actor (`GET /ap/v1/actor`),
WebFinger (`GET /.well-known/webfinger`), and the inbox
(`POST /ap/v1/inbox`) are inert 404s unless `SIDEFEED_AP_ENABLED=1`; enable
it only when expecting Follow Accepts back.

The node holds one ed25519 signing seed in the `node_meta` table, generated
on first use. Inbox deliveries must carry an `hs2019`-style `Signature`
header over `(request-target) host date digest` (date required, five-minute
skew window) verifiable against the sender's advertised ed25519 key; RSA
signatures, the Mastodon default, are rejected, so follow handshakes with
RSA-only servers do not complete. `Accept{Follow}` flips the matching
source config to `following:true`; `Create` stores one item only from actors
the node already tracks, and anything else is accepted-and-ignored.

Webhook ingress (`POST /api/v1/ingress/{slug}`) is token-free by design:
possession of the channel secret is the credential, carried as a Bearer
token or an `x-sidefeed-signature` HMAC header. Only the SHA-256 hash of
the secret is stored. Batches are capped at 100 items (413 above) under the
global 2 MiB body limit; a channel bound to no source still ingests, with
items stored sourceless.

## Scoped API keys (lane-authsec, Task 1)

Management routes accept either the admin bearer or a scoped API key.
Mint with `POST /api/v1/keys` (admin-only); the plaintext `sf_…` token is
shown once and only its SHA-256 hash is stored. Revoke with
`DELETE /api/v1/keys/{id}`; revoked or unknown tokens fail closed as `401`,
a live key without the required scope fails as `403`.

Scopes are opaque strings. Management reads require `read:private`, writes
require `write:private`, and bookmark writes (when gated) require
`bookmarks:write`. The admin bearer passes every scope check.

- `SIDEFEED_API_KEYS_ENABLED=1` requires scoped keys for management even when
  no admin token is set. At `0` (default) the localhost default stays open.
- `SIDEFEED_BOOKMARKS_REQUIRE_AUTH=1` routes bookmark writes through
  `require_scope(_, "bookmarks:write")`. At `0` (default) bookmarks stay a
  bounded token-free write.

## Model files are operator-supplied

`SIDEFEED_ONNX_EMBED_MODEL` points at a `.onnx` embedding artifact (e.g. a
quantized MiniLM-L6-v2) plus its tokenizer file. Model files are trusted,
operator-supplied input: they are memory-mapped by the inference runtime, so
only load files you fetched yourself from a source you trust. A missing or
unreadable path keeps the provider disabled; ingestion and delivery never wait
on it.

## Rate limits (lane-authsec, Tasks 1 + 8)

Key management (`POST /api/v1/keys`, `DELETE /api/v1/keys/{id}`) has a strict
per-IP bucket: 1 rps sustained, burst 10. The global governor defaults to
`SIDEFEED_RATE_LIMIT_RPS=10` sustained and `SIDEFEED_RATE_LIMIT_BURST=30`
burst per IP (single-process in-memory buckets; no multi-replica sharing).
Exceeding either returns `429` with a `Retry-After` header. Nothing is
exempt; `/healthz` is cheap and limiting it is intentional.
