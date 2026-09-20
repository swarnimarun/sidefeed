# Security model

Sidefeed consumes untrusted documents and should be deployed as an unprivileged
service. Source and peer URLs are limited to HTTP(S), DNS results are checked
against private and non-routable address ranges, every redirect is rechecked,
responses are bounded, and requests time out. Network policy remains a useful
second layer because DNS can change after resolution.

Set `SIDEFEED_ADMIN_TOKEN` on every internet-accessible node. Without it,
administrative endpoints are intentionally open for single-user localhost
deployments. Public feeds require no token; private feeds and all mutations do.

Peers are explicitly configured and authenticate exports with HMAC-SHA256.
Both peers must use the same 32-or-more-character secret. Signatures cover the
timestamp, method, path, and query and expire after five minutes. Rotate a key
by replacing the peer on both nodes during a maintenance window. TLS is still
required to conceal feed data and signatures in transit.

SQLite contains fetched content, peer secrets, and optional embedding vectors.
Back up the database with SQLite's online backup tooling or while Sidefeed is
stopped, and protect the file as a secret-bearing asset.

