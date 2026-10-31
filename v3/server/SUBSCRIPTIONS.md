# Public-source subscriptions

`previewSubscription({input,kind='rss'},{fetcher,now})` returns `{source,items,poll,warnings}`. `source.subscription` must survive storage. Preview does public GETs only. The API owns a separate explicit local subscribe confirmation. This is **not** an ActivityPub Follow, authenticated account action or firehose subscription.

`pollSubscription(source,{fetcher,poll,now,maxPages=2,backfill=false})` returns items and updated poll metadata. `now` is epoch milliseconds. Maximum five pages / 500 entries per response / 2 MB response; default two pages. Persist `poll` even on 429/503 (`error` returned); exceptions carry `error.poll`. Poll metadata includes conditional HTTP validators, failure count and next eligible poll time. Caller enforces nextPollAt; this module does not run a scheduler. A normal refresh starts at the head; `cursor` is a backfill continuation **only**, never the head polling checkpoint. Dedupe is by canonical externalId; CID and reply strong references are retained. Never infer deletions from absence on bounded pages.

## Supported scope

* Default RSS: HTTP(S) feed URL or HTML site with advertised RSS/Atom/JSON Feed links; deterministic first advertised link selected, alternatives returned. No hidden HTML scraping or paywall bypass. RSS 1 RDF, RSS 2 and Atom use pinned vendored sax 1.6.1 strict namespace parsing with byte/depth/node/attribute caps, DTD rejection, predefined/numeric entities, CDATA, RDF IDs/order, Atom alternate links/xml:base/author inheritance. UTF-8 only; no complete RSS/Atom schema-validation claim. JSON Feed parsing is separate and tolerates missing version/title for legacy feeds.
* Bluesky: handle, `@handle`, DID plc/web or bsky.app profile URL. Official public AppView `resolveHandle` then bounded `getAuthorFeed`. Canonical DID retained independently of mutable handle. This is trusted AppView resolution, **not independent bidirectional identity verification**. Authored posts/replies retained; reposts by other authors excluded. No authenticated API, write actions or deletion inference. AT URIs and CID retained as provenance, not locally cryptographically verified.
* ActivityPub: account/WebFinger or actor URL. Credential-free GET with ActivityStreams Accept. WebFinger subject must match. Conservative same-origin WebFinger actor, actor ID, outbox/page/object IDs; cross-origin aliases/migrations unsupported. Public-addressed embedded Note/Article/Page Create/Update and explicit Delete/Tombstone only. Remote object links are not dereferenced. Followers-only/unaddressed content excluded, explicit object addressing takes precedence. Collection and ordered collection link pagination and embedded first/next pages with same-origin IDs supported. Servers requiring signatures/auth are unsupported. JSON-LD contexts are never fetched/expanded. No authenticated federation Follow or delivery.
* Public network protection is centralized in `safeFetch`: DNS pinning, private IP rejection, every redirect revalidation, capped time/body, allowlisted Accept/conditional headers only. Fixture injection is test-only trusted transport, never user-controlled.

## Reference basis (reviewed 2026-10-31)

* AT author feed lexicon: https://raw.githubusercontent.com/bluesky-social/social-app/main/lexicons/app/bsky/feed/getAuthorFeed.json (actor, cursor, limit <=100, unauthenticated public view).
* Handle resolver: https://raw.githubusercontent.com/bluesky-social/atproto/main/lexicons/com/atproto/identity/resolveHandle.json (DID response; not necessarily bidirectional verification).
* ActivityPub actor/outbox/public addressing: https://www.w3.org/TR/activitypub/
* ActivityStreams collections, objects, tombstones: https://www.w3.org/TR/activitystreams-vocabulary/
* RSS 1 RDF: https://web.resource.org/rss/1.0/spec
* RSS 2: https://www.rssboard.org/rss-specification
* Atom: https://www.rfc-editor.org/rfc/rfc4287
* JSON Feed: https://www.jsonfeed.org/version/1.1/
* WebFinger: https://www.rfc-editor.org/rfc/rfc7033

Tests use deterministic fixture transport only. No live origin interoperability, federated identities, provider credentials, signatures or real account actions have been tested.
