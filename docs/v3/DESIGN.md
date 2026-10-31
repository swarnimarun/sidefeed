# Commonplace × Dispatch: fresh editorial frontend

The selected direction combines Commonplace's warm archive identity with Dispatch's stacked stories and contextual reading margin. This implementation is a fresh dependency-free frontend, not a recoloring of the previous interface. Visual comparison against the chosen concepts passed; the latest delivery has not yet received user approval. No pixel-identical claim is made.

## Implemented reading experience

- A working 60px masthead exposes Feed, Threads, Sources, Compose and Search. The 210px archive is reading-only: real source/read/save filters, not administration or invented collections.
- An ivory folio uses a 65/35 article/context composition with stacked stories and fine rules instead of dashboard cards. Bold Georgia headlines are 40px for the lead and 28px for subsequent stories; excerpts are 22px/20px.
- Related reading states actual same-source/shared-keyword evidence from loaded items. Source provenance reflects stored source/fetch metadata. Neither is a generated trust claim or model recommendation.
- Phone layouts use 30px/24px headings, 17px excerpts, a reading drawer, closed related disclosures and fixed Feed/Saved/Sources/Compose navigation with safe-area clearance. The reviewed fixture shows the third headline above the bottom bar; real content length and viewport can change density.
- Sources combine progressive controls with discovery/approval. Compose is a paper-like editor with explicit save/publish actions. Threads has its own page. Node tools are in Utilities and advanced JSON/evidence starts collapsed.
- Counts come from library-wide `status.readingCounts`; actual API filters and author-aware search drive the reader. The 1,000-item result cap is disclosed. No fictional collections, automatic sample stories or unsupported account destinations are inserted for visual resemblance.

## Source structure and tokens

`web/api.js`, `state.js` and `router.js` separate requests/token memory, state/filtering and navigation. `components/`, `pages/` and `actions/` separate safe rendering and workspace behavior; `app.js` orchestrates. Three legacy root UI helpers were removed. See the [web source guide](../../v3/web/README.md) for current boundaries.

| Token | Value |
|---|---|
| Linen shell | `#EDE4D5` |
| Ivory folio | `#FBF7EE` |
| Espresso text | `#3D322A` |
| Brick action | `#91452F` |
| Petrol utility | `#365F68` |
| Editorial type | System Georgia/Times |
| Utility type | Arial/Helvetica |

No bundler, remote font, CDN, inline template style or CSP exception is required. Text is escaped, links are HTTP(S)-only, and token credentials remain in page memory. Read/save controls are separate from opening a story. Provider jobs, subscription proposals and local publication retain explicit consent; page views never trigger model processing. These properties do not replace a comprehensive accessibility or security audit.

## Concepts versus actual product

Design images were illustrative references, not runtime evidence. Their fictional stories, authors, source proposals, collections and publishing destinations are not actual data. The implementation uses real API-backed state and explicit empty/loading/error views. Mobile related context is an inline disclosure, not a promised bottom sheet. Local publishing remains local-only; RSS is a source channel, not a social publishing destination.

## Verified evidence and limits

[CI run 41214495602](https://github.com/swarnimarun/sidefeed/actions/runs/41214495602) at `723557df29b431041a0cb58cb657e7325285439c` passed 62 tests, 23 recursive JavaScript syntax checks, full browser/protected-auth/model-consent/draft/navigation flows and container smoke. Actual screenshots from artifact `13047443684` passed five comparison criteria: functional masthead, reading-only archive, bold serif hierarchy, 65/35 context and phone bottom navigation with the third headline visible above it.

This evidence applies to the fresh frontend, not an earlier styling pass. It is objective visual comparison, not user approval or pixel identity. Real accounts/providers, full ActivityPub/authenticated ATProto, autonomous agents, long-running multi-host behavior, real-device accessibility, load and production deployment remain unverified. See [QA](QA.md) for detailed boundaries.
