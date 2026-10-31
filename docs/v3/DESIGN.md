# Commonplace × Dispatch: the implemented reading room

The selected direction combines Commonplace's warm archive identity with Dispatch's stacked stories, editorial hierarchy and contextual reading margin. The goal is several worthwhile excerpts, not one oversized hero article or a statistics dashboard.

## Applied interface

- Warm linen surrounds an ivory reading folio. A compact archive provides real routes; it is not a duplicate article queue.
- A 64px masthead and 205px archive index (185px below 1200px) frame vertically stacked articles separated by hairlines. The lead is modestly larger; subsequent stories retain readable headings and excerpts.
- Serif headings/excerpts provide the newspaper rhythm. Source labels and controls use a neutral sans serif. No remote font, decorative hero image, CDN or bundled design dependency is required.
- Open/read/save actions remain distinct. Compose is a deliberate separate workflow, not a permanent inline composer.
- Related reading uses actual same-source or shared-keyword matches from the currently loaded view, with its reason stated. It is not semantic retrieval or a model recommendation. Source provenance reflects the stored source and fetch status, not a generated trust score.
- No fabricated collection names, counters, source-approval items or sample stories are inserted to imitate a design image. Empty/loading/error states remain explicit.

## Tokens and responsive behavior

| Token | Implemented value |
|---|---|
| Linen shell | `#EDE4D5` |
| Ivory folio | `#FBF7EE` |
| Espresso text | `#3D322A` |
| Brick action | `#91452F` |
| Petrol utility | `#365F68` |
| Editorial type | System Georgia/Times |
| Utility type | Arial/Helvetica |

Below 1050px, contextual information moves into a closed **Related & source details** disclosure attached to the article. Below 760px, **Archive** explicitly opens navigation; Escape and selecting a route close it. Mobile reading is a single column with 20px side padding, 28px lead headings, 24px subsequent headings and 16px excerpt text. Search remains available through the archive route without occupying permanent reading space.

Controls retain 44px targets and visible focus. Dynamic content is escaped; links are restricted to HTTP(S); the server's content security policy is not relaxed for the design. These implementation choices do not substitute for a full accessibility audit. The [web source guide](../../v3/web/README.md#reading-room-design) records the current tokens and component behavior.

## Proposal versus implementation

The review images were illustrative design references, not runtime screenshots. Their fictional stories, authors, collection names, pending-source examples, precise viewport composition and posting destinations are not product data or proof of functionality. The implementation adapts the visual direction to existing API-backed routes and content.

The design proposal's mobile bottom sheet is implemented as an inline disclosure; its bottom navigation and speculative collection taxonomy are not promised features. Accent colors were darkened from the initial proposal. Article density varies with real text, viewport and state; there is no guarantee that exactly three titles fit every screen.

Model controls and the separate semantic-similarity workflow retain explicit consent and provenance. A metadata-related rail is not silently upgraded to AI. Local publishing is still local-only; an RSS source label is not an ActivityPub or ATProto publishing destination.

## Evidence and open work

[CI at `ab40b23b`](https://github.com/swarnimarun/sidefeed/actions/runs/41211874519) passed 53 tests, 16 syntax checks, browser flows and container smoke, capturing 14 screenshots. Actual desktop reader, compact mobile, expanded mobile context and publish views were visually reviewed from artifact `13046154447`. Token-protected reconnect/disconnect, model-consent cancel/confirm and unsaved-draft cancellation were exercised with fixtures; no real provider call was made.

The final successful run includes toast-dismissal waits and separate compact/expanded mobile captures. These test-only refinements do not change runtime behavior. See [QA](QA.md) for the final evidence boundary. Real devices, comprehensive accessibility, live providers, real federation accounts, full ActivityPub/authenticated ATProto, long-running multi-host behavior and production deployment remain outside this verification.
