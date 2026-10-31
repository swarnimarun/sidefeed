# Sidefeed V3 web client

No bundler, remote fonts, CDN, or runtime dependencies. The Node service serves this directory; run `npm start` from `app/`, then open `http://127.0.0.1:8787`. Use the top-right **Connect** button for an owner bearer token when `SIDEFEED_TOKEN` is set. The token exists only in page memory; reloading requires reconnecting. Use HTTPS for remote connections.

## Implemented screens

- Inbox, saved stories, lexical search, plain-text detail, read/unread and save toggles.
- Source create, refresh, pause/resume and removal. RSS, Atom, JSON Feed, and public AT Protocol JSON adapter formats.
- Website discovery with explicit proposal approval/denial and inspectable tool trace.
- Private draft creation and editing, local live preview, explicit local-only publishing confirmation.
- Trusted peer/revocation, lease, and durable-event inspection (configuration is API-only).
- Live server capability report, honest AI/protocol/deployment limitations.

Data comes from `/api/v3`; the client never seeds fixtures automatically. Empty, loading, retry, failure, and disconnected states are explicit. Dynamic values are escaped and story text is not rendered as HTML. External links accept only HTTP(S) and use `noopener noreferrer`. Forms use labels, keyboard navigation, focus indicators, live status messages, and native confirmation dialogs. Responsive navigation and single-column layouts support mobile widths.

Draft text survives client navigation in memory, but not page reloads. Saving persists to the backend. Do not navigate away/reload an unsaved draft expecting durable autosave. Search is submitted, not live; fleet inspection refreshes on demand, not automatically. Thread detail is story text only: conversation replies are pending. Provider configuration, embeddings, authenticated AT Protocol publishing, and ActivityPub delivery are not implemented.

Validate syntax: `node --check web/app.js` from `app/`. Independent browser QA is documented in the root verification report when available.
