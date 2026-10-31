# Sidefeed editorial web client

Fresh dependency-free ES modules. No bundler, remote fonts, CDN, inline template styles, or CSP exceptions.

## Boundaries

- `app.js`: rendering orchestration, loading, generic reader actions, memory-only connection.
- `api.js`: requests and page-memory owner token.
- `state.js`: reader state and pure filters.
- `router.js`: history/hash navigation and stale-thread invalidation.
- `components/shell.js`: masthead, archive filters/counts, utility menu, mobile navigation.
- `components/html.js`: escaping, HTTP(S)-only links, text helpers.
- `pages/reader.js`: digest, full text, provenance and related reading.
- `pages/workspace.js`, `actions/workspace.js`: source discovery/approval, private composition, local publishing, threads and node tools.
- `components/models.js`, `components/thread.js`: bounded evidence renderers.
- `reading-context.js`: actual same-source/shared-keyword matches.

## Reading contract

Desktop: 60px masthead, 210px reading archive, 65/35 article/context folio. Bold Georgia headlines 40px lead/28px secondary; excerpts 22px/20px. Mobile: 30px/24px headings, 17px excerpts, closed related disclosures, Archive drawer, fixed Feed/Saved/Sources/Compose navigation with safe-area clearance. Threads/node tools remain in Utilities (mobile: open Archive first).

Counts use `status.readingCounts` over the whole library. Filters request actual API query parameters. The API caps results at 1,000; the view states this at the cap. Sources are real subscriptions, never fictional collections. Search matches title, content, and author. Relatedness uses only loaded items and labels its evidence.

## Safety and validation

Tokens live only in module memory. No browser storage. External links permit HTTP(S) only; story content is plain text. Subscription proposals and local publication require confirmation. Provider jobs separately disclose destination, scope, private-interest content and potential charges. Page views never trigger model processing. Unsaved Compose text survives navigation in memory, not reload; Save persists it. Thread snapshots are explicit bounded public fetches, not subscriptions. Technical evidence begins collapsed.

From `app/`: `npm start`, `npm test`, `npm run check`. Renderer tests exercise actual components and router. `qa/browser.py` owns fixture-only browser interactions/captures; unit success is not a browser or design pass.

## Search, research, and public subscriptions

`pages/search.js` / `actions/search.js` render ranked server results directly: the client never substring-filters away fuzzy or embedding matches. GET search is local FTS5 with optional typo fallback, inclusive dates, source/channel/read/saved filters, counts, and pagination. POST hybrid search requires fresh query-only provider consent for each request (including another page); page loads and refreshes never silently repeat a paid semantic query. Related-reading lookup explicitly requests local shared-term/stored-vector matches; its provenance distinguishes terms from compatible embeddings.

Ask your library is a separate, explicit source-scoped workflow, not automatic search enhancement. It discloses the question, provider/destination, selected sources, local-only/private content, four-request bound, and potential charges before submitting. Answers show cited source links and collapsed tool steps. While Search is open, read-only job-status refresh continues for queued/running jobs; cancel requests cannot retract transmitted data or guarantee no billing. A missing server summary model disables this workflow. Provider secrets are never browser inputs.

`pages/subscriptions.js` / `actions/subscriptions.js` provide a default-RSS preview flow with explicit AT Protocol and ActivityPub modes. Preview fetches public data but does not subscribe. A second confirmation creates local public polling, never a remote federation Follow, authenticated account access, or publication. Preview edits/cancel/navigation invalidate delayed responses. Legacy exact-feed and discovery controls remain behind an advanced disclosure.

Frontend regression tests cover backend ranking preservation, negative-state/date filters, safe citations, provider-consent cancel/approve, stale search/preview responses, and local-subscription confirmation. Browser evidence is still required for release acceptance.
