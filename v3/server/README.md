# Server slice

Node 24 built-in HTTP and SQLite, zero runtime package dependencies. `npm start` serves the UI and API at loopback port 8787. `npm test` uses injected fixture transport, no external network or credentials. This is a working prototype, not a hardened production release.

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

**Limits:** one coordinator, bearer-authenticated trusted transport rather than signed event protocol, no peer discovery/autojoin, automatic origin-poll scheduler/remote poller, lease renewal worker, source removal propagation, snapshot/resync/retention protocol, tombstones, per-peer quotas or restore fencing yet. Events retained without compaction. Manual local refresh is the available origin polling trigger. This is not a globally exactly-once polling guarantee. Disable sharing stops future delivery but does not recall previous copies. Consumer maps are operator-authorized, not discovered from content.

## Security and implementation limits

Loopback mode has Host allowlisting and same-origin/JSON mutation checks. Remote binding requires a >=24-character owner bearer token (UI may provide one), plus external HTTPS. There is no password/session/device account system yet. Public `/public/feed.json` includes only locally published posts; drafts are never returned there. Explicit publication is local only, no federation delivery.

All source and agent network requests use one HTTP(S) fetcher: credential-free URLs, private address rejection, DNS address pinning, redirect revalidation, response/body/time caps, no decompression. The parser is intentionally a bounded RSS/Atom/JSON/ATProto adapter rather than a full XML implementation; DTDs/entities rejected; output rendered as plain text. It is not a full RSS specification validator. Native RSS RDF and malformed XML coverage remain pending.

Keyword tagging is deterministic, not model classification or embeddings. Agent runs only extract feed discovery links from a supplied public website and require approval before subscribing; no browser, search engine, remote model, arbitrary execution or autonomous publishing.
