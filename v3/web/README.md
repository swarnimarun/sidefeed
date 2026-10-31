# Sidefeed V3 web client

No bundler, remote fonts, CDN, or runtime dependencies. The Node service serves this directory; run `npm start` from `v3/`, then open `http://127.0.0.1:8787`. Use the top-right **Connect** button for an owner bearer token when `SIDEFEED_TOKEN` is set. The token exists only in page memory; reloading requires reconnecting. Use HTTPS for remote connections.

## Implemented screens

- Inbox, saved stories, lexical search, plain-text detail, read/unread and save toggles.
- Source create, refresh, pause/resume and removal. RSS, Atom, JSON Feed, and public AT Protocol JSON adapter formats.
- Website discovery with explicit proposal approval/denial and inspectable tool trace.
- Private draft creation and editing, local live preview, explicit local-only publishing confirmation.
- Trusted peer/revocation, lease, and durable-event inspection (configuration is API-only).
- Live server capability report, honest AI/protocol/deployment limitations.

Data comes from `/api/v3`; the client never seeds fixtures automatically. Empty, loading, retry, failure, and disconnected states are explicit. Dynamic values are escaped and story text is not rendered as HTML. External links accept only HTTP(S) and use `noopener noreferrer`. Forms use labels, keyboard navigation, focus indicators, live status messages, and native confirmation dialogs. Responsive navigation and single-column layouts support mobile widths.

Draft text survives client navigation in memory, but not page reloads. Saving persists to the backend. Do not navigate away/reload an unsaved draft expecting durable autosave. Search is submitted, not live; fleet inspection refreshes on demand, not automatically. Thread snapshots and provider-gated model jobs are described below. Authenticated AT Protocol publishing and ActivityPub delivery remain unimplemented.

Validate syntax: `node --check web/app.js` from `v3/`. Independent browser QA is documented in the root verification report when available.

## Stage 2: explicit models and bounded threads

**AI & retrieval** loads provider status, models, resource caps, job records, and artifact provenance. It does not start a job on page load. Select specific stories and classify or embed; the approval dialog identifies provider endpoint, selected titles, content scope, private-interest disclosure, and possible provider charges (including failed/canceled requests). Only after confirmation does it submit provider-specific consent. Configured model availability and server limits gate actions; secrets remain server-side. Jobs are refreshed on demand and queued/running work can be canceled. A cancellation request is not a guarantee of zero provider charges.

Stored-vector similarity is explicitly labeled **semantic**, separate from reader lexical search. It makes a local API call, not a new remote model request. Results show model/dimensions, scores, and inspectable evidence. Classifications/embeddings include their job/model/input provenance rather than being presented as unquestionable facts.

Story detail can explicitly request an **AT Protocol thread snapshot** by canonical DID-based AT URI. The bounded public AppView response renders replies and unavailable/blocked placeholders as escaped text. It is always labeled partial, not a complete/live conversation or a subscription. Cycles and depth are bounded in rendering; only HTTP(S) links are actionable. Navigating away invalidates pending thread responses.

Validation: from `v3/`, run `node --check web/app.js && node --check web/model-ui.js && node --check web/thread-ui.js && node --test web/render.test.mjs`. Five unit tests cover escaping, unsafe URLs, unavailable nodes, cycles, unavailable models, model evidence, and delayed-hash navigation preserving newly typed search queries. These are unit checks, not a browser pass; consult the root verification report for actual browser-CI results.

## Reading-room design

The interface uses warm linen (`#EDE4D5`) around an ivory reading folio (`#FBF7EE`), espresso text (`#3D322A`), a darker brick action color (`#91452F`), and petrol utility links (`#365F68`). System Georgia/Times supplies editorial headings and article excerpts; Arial/Helvetica supplies navigation and controls. No remote font, decorative image, or external asset is required.

A 64px masthead sits above a compact 205px archive index (185px below 1200px). Stories form a vertical newspaper-like stack separated by hairlines, not dashboard cards. The lead is modestly larger, excerpts use a comfortable text measure, and saved/read actions stay distinct from opening a story. There are no fabricated collection names, invented counters, or inbox statistic tiles.

The contextual rail shows actual same-source or shared-keyword matches from the current loaded view, with the reason stated explicitly; these are not model recommendations. Source provenance uses the stored source record and fetch status, never a generated trust claim. Below 1050px this context moves into each article’s closed **Related & source details** disclosure. Below 760px, the archive is behind an explicit **Archive** menu, Escape closes it, and selecting a route closes it. Reading is a single column with 20px side padding, 28px lead headings, 24px subsequent headings, and 16px excerpt text. Mobile search is available through the archive’s Search route, rather than permanently occupying reading space.

Controls retain at least 44px targets and visible focus. All template styles are CSS classes; the server CSP is not relaxed. Additional unit checks cover evidence-based related entries and absence of inline template styles. Final visual/browser validation belongs to the project QA report, not to these static guarantees.
