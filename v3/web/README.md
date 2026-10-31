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
