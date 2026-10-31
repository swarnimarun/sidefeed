# Reader search, library research and public subscriptions

This implementation is fixture-tested, not live-provider or social-account qualification. [CI run 41218120045](https://github.com/swarnimarun/sidefeed/actions/runs/41218120045) at `2d1d21a06bd3a0d591b6023d69e48ed74d2d9fd0` passed 124 tests, 32 syntax checks, browser flows and container smoke. Actual Search/fuzzy/Ask/public-profile preview screens were reviewed. No external provider or public-host deployment was performed.

## Find and relate local reading

- `GET /api/v3/search`: SQLite FTS5-ranked lexical search, phrases, Unicode, filters and stable pagination. Bounded indexed trigram/Damerau expansion handles spelling errors including leading-character errors and transpositions. This is local computation, not a model call or guarantee every typo resolves.
- `POST /api/v3/search` with `mode:hybrid`: explicitly consented query embedding via the configured provider, combined with lexical retrieval. Requires `consent:{provider,sendQuery:true,confirm:true}`; sends the query, not article text. One request is reserved in the shared daily ledger.
- `GET /api/v3/items/{id}/similar`: local related reading; method/provenance distinguish shared-term evidence from compatible stored embeddings. Query/item vectors must match provider, endpoint, requested/actual model and dimensions; stale-content embeddings are excluded. No synthetic fallback vectors.

Metadata-related entries are not advertised as model recommendations. Semantic quality depends on actual models and corpus; fixture success does not establish quality or latency at scale.

## Summaries and Ask over an approved library scope

`POST /api/v3/models/jobs` accepts `operation:summary`, explicit item IDs and provider/content/private consent. Query-conditioned summaries additionally require query consent. `SIDEFEED_SUMMARY_MODEL` selects the model; a configured classification model is fallback. Citations must refer to supplied item IDs; this validates attribution structure, not factual entailment.

`POST /api/v3/agents/search` accepts a query, a nonempty approved `sourceIds` scope, confirmation and provider/query/content/private consent. The model can request only strict, validated `search`, `read_items` and `finalize` actions. Search is local and scope-bound; reads can access only previously discovered IDs, exporting at most 3,000 content characters per item. Final answers cite actually read items. Trace/provenance records tool actions, hashes and truncation.

At most four provider requests are reserved before the loop; bounded context/output/steps and cancellation fence late responses. Jobs share persistent model-job state, restart-uncertain failure and no-automatic-paid-retry behavior. Request/character/token limits are **not currency budgets**. Cancellation cannot recall transmitted content or guarantee no charge. Source text cannot authorize shell execution, arbitrary URLs, publication, social following or external search. There is no external-web search connector silently substituted for local research.

Provider configuration is disabled by default. Credentials remain server-side; page views and new subscriptions do not trigger models. See the [server configuration guide](../../v3/server/README.md) for variables and API fields. Live provider compatibility, factual quality and billing remain unverified.

## Preview and explicitly subscribe

`POST /api/v3/subscriptions/preview` takes `input` and a `kind` (`rss`, `atproto`, `activitypub`) and fetches public evidence without adding a source. `POST /api/v3/subscriptions` with `previewId` and `confirm:true` consumes a ten-minute preview once, importing normalized items and canonical public metadata. This is a **local subscription**, not a remote Follow or authenticated account action.

| Input | Implemented subset | Important boundary |
|---|---|---|
| RSS/site URL | RSS 1 RDF, RSS 2, Atom, JSON Feed; advertised-link discovery | No hidden scraping/paywall bypass; alternatives exposed when discovery selects the first advertised feed |
| ATProto profile | Handle, @handle, DID or bsky.app profile; public AppView resolution and bounded authored feed | Mutable handle resolved to retained DID, but no independent bidirectional identity/crypto verification; no OAuth, publishing/firehose or deletion inference |
| ActivityPub account/actor | Matching WebFinger subject, same-origin actor/outbox/page/object identities; public embedded Create/Update Note/Article/Page and explicit Delete/Tombstone | No cross-origin migration/aliases, remote object dereference, private/followers-only content, JSON-LD expansion, signed/auth-required access or federation delivery |

Bounded public thread snapshots retain canonical AT URI/CID and parent/root relationships, with blocked/missing placeholders and explicit partial coverage. A preview or bounded feed page is not a complete account history. Absence from a page is never a deletion signal.

## Polling and parser operations

Manual refresh persists ETag/Last-Modified validators, handles 304, tracks backoff/next eligible time and bounded pagination/backfill. Backfill cursors are not head-poll checkpoints. `SIDEFEED_AUTO_POLL=true` opts into a sequential batch every 60 seconds, at most two due unpaused sources, normal five-minute intervals and failure backoff. It is off by default. The supplied Compose file does not forward this variable; enabling it there requires explicit service environment configuration. Fleet lease checks still apply; pause consumer sources to avoid independent origin polls. This is not a durable distributed scheduler or lease-renewal/failover service.

The XML adapter bundles **sax 1.6.1**, pinned upstream commit `961ceeefcc5186005466f2085aeb36f31b161e37`, under **BlueOak-1.0.0**. No npm install is required; this is not zero third-party dependency code. Preserve `server/vendor/sax.LICENSE.md` in distributions and follow [vendor provenance](../../v3/server/vendor/README.md). Strict namespace parsing supports predefined/numeric entities and CDATA, rejects DTDs/custom entities, and caps bytes/depth/nodes/attributes. XML transport is UTF-8 only; no full RSS/Atom schema-validation or universal XML conformance claim.

See [public-source details](../../v3/server/SUBSCRIPTIONS.md). Remaining release work includes live origin/provider interoperability, authenticated AP/AT writes, production security/accounts, fleet failover and recovery, backup/restore, long-running/load measurements and verified deployment profiles.
