# Sidefeed V3 integration contracts

Design baseline: 2026-10-31. This document specifies the target and acceptance gates; it is not a claim that these integrations are production-ready. The first implementation slice is a Node 24 / SQLite local backend and web UI, without configured external model or publishing credentials. Preserve AI, RSS/Atom/JSON Feed and ActivityPub read/publishing scope; AT Protocol and trusted-fleet delivery are additive.

## Implemented adapter slice

`app/server/threads.mjs` now normalizes bounded public AppView thread responses into canonical DID-keyed nodes and reply/root edges; `adapters.mjs` uses the same post normalizer for feed responses. CID edits retain item identity. Cross-author replies, blocked/not-found/missing placeholders, traversal caps and a fixed public AppView URL builder are covered by synthetic tests in `app/test/threads.test.mjs`. Every graph explicitly reports a partial AppView snapshot; this is not continuous thread following. Routes, persisted/fleet metadata and UI use this contract as separately integrated layers. No handle resolver, stream, OAuth or remote publishing is implemented by this adapter. The discovery/model and fleet target design below does not imply all controls are implemented.

## 1. AT Protocol: begin with public reads

Use the Bluesky AppView for application feeds: `app.bsky.feed.getAuthorFeed` for an actor, `getFeed` for a generator URI, and `getPostThread` for a post. Most public reads can use `https://public.api.bsky.app` without credentials. Authenticated requests route via the user's PDS, not blindly to the public host. An AppView is an indexed application view, not a repository or a generic feed generator. [Official API routing](https://docs.bsky.app/docs/api/app-bsky-feed-get-feed)

**Adapter contract (target):**

- Store source kind, service origin, endpoint, actor DID or generator URI, query/filter configuration and adapter version. Resolve handles for convenience; persist DID identity.
- Item key: canonical `at://<DID>/<collection>/<rkey>`; CID identifies a content version, not a new logical item. Keep handle and display URL separate. Never lower-case record keys. AT URIs need a protocol-aware parser rather than assumptions that their authority is an HTTP host. [AT URI specification](https://atproto.com/specs/at-uri-scheme)
- Preserve `reply.parent` and `reply.root` URI/CID references, actor DID, record CID, repository revision when available, fetched time, source attribution, labels and deletion state. A repost is a separate feed reason referencing the same post, not a duplicate article.
- Thread graph: nodes by canonical URI, typed parent/reply/quote edges, missing-node placeholders and bounded traversal. `getPostThread` supplies ancestor/descendant views; blocked/not-found nodes must remain placeholders rather than silently disappearing. Show partial coverage and last refresh. [Official thread guide](https://bsky.network/docs/bluesky-api/viewing-threads/)
- Persist pagination cursor with the exact source/query/service identity. Bounded refresh and rate-limit backoff are necessary; feed pagination is not a durable change log. Do not infer deletion from an item falling out of a feed page.

### Streaming is a later adapter, not a renamed poller

`com.atproto.sync.subscribeRepos` is a CBOR repository event stream from a PDS or relay, with sequence resumption and repository commits; it is not a hydrated AppView feed. PDS scope differs from relay scope. Verification, repository resync, identity refresh and account transitions require explicit handling. [Sync specification](https://atproto.com/specs/sync)

Jetstream offers easier filtered JSON consumption, but pin endpoint **and wire version**. Current official documentation distinguishes v1-compatible and v2 interfaces, inclusive replay, instance-local cursors and at-least-once delivery. Treat a cursor as opaque, endpoint-scoped state: do not interchange AppView cursors, legacy timestamp cursors, Jetstream sequence cursors or relay sequences. Checkpoint only after the corresponding records/tombstones commit. On endpoint migration or expired replay, perform a supported backfill/reconciliation; never assume copying a cursor gives lossless failover. Keep identity/account/delete/sync markers required by the chosen wire model. A record JSON response is not itself proof of cryptographic verification. [Official Jetstream implementation and protocol documentation](https://github.com/bluesky-social/jetstream/blob/main/docs/README.md)

Filtering only the followed author's DID misses replies authored by other people. Use bounded AppView thread refresh first. A future fleet-owned stream can index reply references across its permitted collection scope, then route matching thread changes; make coverage explicit rather than claiming every reply is observed.

### Authentication and hosting

Public reading needs outbound HTTPS only. Streaming needs an always-running worker, outbound WSS, persistent checkpoints, disk limits and backpressure; do not host a full-network archive merely to follow a few threads. OAuth login/publishing needs durable session storage and an appropriate publicly reachable metadata/callback deployment. Keep credentials and session keys local; never replicate them through the fleet.

Use the maintained Node OAuth client rather than hand-writing authentication to preserve a zero-dependency label. AT Protocol's OAuth profile requires PKCE, PAR and DPoP; a confidential server client additionally uses asymmetric client authentication. Request only the needed permissions and serialize session refresh. [OAuth specification](https://atproto.com/specs/oauth), [official SDK guidance](https://atproto.com/guides/about-oauth)

Publishing is a distinct, user-approved action with an exact destination/account and payload preview. Retry through a durable publication ledger and idempotent intent; a model suggestion does not authorize a post. Keep ActivityPub publishing as its own protocol adapter and mapping, not an assertion that AT Protocol is ActivityPub-compatible.

## 2. Website discovery and agent/model harness

**Principle:** deterministic fetching and extraction first; model-assisted planning second. A classifier or link detector alone must not be marketed as an autonomous website-search agent.

Target workflow:

1. User supplies a website or approves a search query/provider and budget. An optional search-provider adapter produces candidate URLs; no credential means disabled, not simulated results.
2. Fetcher retrieves allowed pages with a descriptive user agent, conditional requests, host concurrency limits, response-size/time caps and a redirect limit. Reject non-HTTP(S), URL credentials, private/link-local/metadata destinations and DNS rebinding at connection time. Apply the same checks to every redirect and discovered URL.
3. Deterministic extractor prefers declared RSS/Atom/JSON Feed links, then publisher APIs, then approved HTML extraction recipes. Preserve original URL, retrieved time, content hash, extraction version and evidence snippets. Avoid forwarding full copyrighted articles when references/excerpts suffice.
4. A read-only planner may rank candidates, propose selectors and explain confidence using only bounded source text. A provider adapter validates structured output against a closed schema. Untrusted webpages, feed text, remote node payloads and model output are data, never tool instructions.
5. Store a proposal with evidence and an immutable plan hash. User approves the specific subscription/recipe/schedule before activation. Changed destinations, costs, privileges or extraction plans invalidate approval.
6. Approved jobs execute through deterministic tools, not generated shell/JavaScript. Persist status, attempts, budget use and provenance. The UI shows pending/running/blocked/failed/completed honestly, with cancel and bounded retry.

Robots rules guide crawler access but are not authorization. Respect applicable website terms, paywalls and authentication boundaries; never evade blocks, CAPTCHA or access controls. A disallowed or inaccessible site is a visible blocked result, with a publisher-supported feed/API suggested where available. [Robots Exclusion Protocol](https://www.rfc-editor.org/rfc/rfc9309)

Suggested narrow capabilities: `search.read`, `fetch.public`, `feed.discover`, `extract.preview`, `classify`, `summarize`, `proposal.create`. Exclude `subscribe.commit`, `publish`, `peer.trust`, credential access and unrestricted network/shell from the model. The controller alone enforces per-job request, page, byte, token, cost, elapsed-time and concurrency budgets before dispatch. Default provider disabled; secrets supplied server-side and redacted from logs.

Provenance follows each derived item: source URL and content hash, extraction/model version, generation time, evidence links, confidence, and whether it is source text or model inference. Fleet signatures attest which node supplied data, not that an article is true. Never send private mailbox content, session tokens or private feed URLs to an external model without separately scoped consent.

## 3. Trusted fleet: turn shared pulls into internal pushes

Fleet delivery is a custom Sidefeed protocol, not a claim of universal origin push. An assigned node still polls a non-push RSS or website origin. Use WebSub/native push where supported; share the resulting permitted public changes inside the fleet. Do not broaden origin access or defeat rate limits by rotating nodes.

Target flow: normalized source identity → explicit source-sharing policy → owner lease → conditional origin fetch → transactional item/thread delta + outbox → authenticated peer delivery → durable inbox dedup + acknowledgement. Independent consumer read state stays local. Do not share private/authenticated sources in the initial version.

Required envelope fields: protocol version, event ID, origin node/key ID, source identity, entity URI/ID, operation, source version/hash, thread edges where available, observation time, owner epoch, and provenance. Sign canonical bytes; bind peer identity to enrolled keys over TLS. Explicit approval enrolls a peer and grants bounded source scopes. Support revocation and key rotation. Validate size/schema/signature/scope and reject replay before applying data.

Use per-source leases and fencing epochs to reject obsolete owner results. Fencing cannot cancel a request already sent to an origin: failover can briefly overlap. Promise at-least-once delivery with idempotent application, not exactly-once polling. Persist retry/backoff, receiver watermarks and bounded replay retention; expose gaps requiring snapshot recovery. Forwarding preserves event ID and original provenance, with loop suppression. Propagate tombstones and account/source takedowns; do not resurrect deleted records from replay or an old snapshot.

Local UI live updates (for example SSE) and distributed delivery are different layers. A demo peer list or manually shared bearer token is not by itself a trusted production fleet. Display owner, last success, cursor lag, retry state, incomplete coverage and estimated avoided polls; label estimates rather than claiming measured savings.

## 4. Prioritized implementation and acceptance gates

| Priority | Deliverable | Required acceptance evidence |
|---|---|---|
| P0 | Honest local UI and adapter state | Missing model/publishing credentials show disabled; unavailable protocols do not silently create functioning-looking sources. Empty/error states remain usable. |
| P0 | RSS/Atom/JSON and ATProto parser dispatch | An ATProto JSON fixture cannot fall into JSON Feed parsing. Repeated URI changes CID without duplicate item. Malformed/oversized input and unsafe links rejected. |
| P0 | Public-feed fetch and discovery | Fixture server proves redirects are checked; private/DNS-rebinding targets blocked; feed discovery returns attributable proposals. 304 and 429 preserve state and back off. |
| P0 | Agent approval boundary | Malicious webpage asking to publish/add peers cannot invoke those actions. Discovery alone creates no subscriptions. Approving edited proposals fails plan-hash validation. |
| P1 | ATProto pagination and threads | Actor/generator cases, parent/root CID preservation, cross-author reply, blocked/missing placeholder, duplicate repost, update/delete and partial-depth fixtures pass. |
| P1 | Fleet end-to-end delivery | Two nodes receive one origin result; dropped ACK causes harmless redelivery; stale epoch rejected; restart replays durable outbox; revoked or wrong-scope peer rejected. Test lease-loss overlap explicitly. |
| P1 | Bounded model/search provider | Mock provider validates schema, cancellation and all budgets; actual provider remains opt-in. Evidence links survive proposal → approved source. Secrets absent from logs/fleet. |
| P2 | Stream-owned ATProto source | Chosen-version fixtures cover replay duplicates, disconnect, transactional checkpoint, too-old cursor, endpoint change, deletion and identity/account transitions. Recovery demonstrates no silent completeness claim. |
| P2 | Authenticated publishing | OAuth deployment/session tests, exact account/payload approval, denied approval, refresh concurrency, ambiguous timeout reconciliation, deletion and durable publication ledger. Include separate ActivityPub interoperability tests. |
| P2 | Production web/fleet hardening | TLS, local-user auth, CSRF/origin checks, key lifecycle, quotas, migration/backup restore and resource measurements pass before public exposure. |

Tests must distinguish mocked protocol contracts from live integration checks. A successful offline fixture run does not establish that an external account, OAuth deployment, stream endpoint or trusted remote peer is configured.
