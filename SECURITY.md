# Security model

Sidefeed consumes untrusted documents and should be deployed as an unprivileged
service. Source and peer URLs are limited to HTTP(S), DNS results are checked
against private and non-routable address ranges, every redirect is rechecked,
responses are bounded, and requests time out. Network policy remains a useful
second layer because DNS can change after resolution.

Set `SIDEFEED_ADMIN_TOKEN` on every internet-accessible node. Without it,
administrative endpoints are intentionally open for single-user localhost
deployments. Public feeds and the reader's saved list require no token; private
feeds and every other mutation do.

The bookmark routes are the one token-free write. They are single-user and
bounded: the node refuses to store more than a fixed number of bookmarks, and
saving only ever references an item that already exists, so the route cannot be
used to inject content. On a shared or hostile network, front the service with
a reverse proxy that restricts `POST`/`DELETE` on `/api/v1/items/*/bookmark` if
you do not want the saved list to be world-writable.

Peers are explicitly configured and authenticate exports with HMAC-SHA256.
Both peers must use the same 32-or-more-character secret. Signatures cover the
timestamp, method, path, and query and expire after five minutes. Rotate a key
by replacing the peer on both nodes during a maintenance window. TLS is still
required to conceal feed data and signatures in transit.

SQLite contains fetched content, peer secrets, and optional embedding vectors.
Back up the database with SQLite's online backup tooling or while Sidefeed is
stopped, and protect the file as a secret-bearing asset.

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

## Rate limits (lane-authsec, Tasks 1 + 8)

Key management (`POST /api/v1/keys`, `DELETE /api/v1/keys/{id}`) has a strict
per-IP bucket: 1 rps sustained, burst 10. The global governor defaults to
`SIDEFEED_RATE_LIMIT_RPS=10` sustained and `SIDEFEED_RATE_LIMIT_BURST=30`
burst per IP (single-process in-memory buckets; no multi-replica sharing).
Exceeding either returns `429` with a `Retry-After` header. Nothing is
exempt; `/healthz` is cheap and limiting it is intentional.
