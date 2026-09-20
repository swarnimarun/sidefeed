# Sidefeed implementation plan

Sidefeed is a small, self-hosted aggregation node. It normalizes content from
different protocols into one local event log, exposes personalized feeds, and
can exchange cached public items with trusted peers so a group does not fetch
the same origin repeatedly.

The default deployment is one Rust process and one SQLite file. Optional
integrations must not make the local path require an external service.

## Product boundaries

- Fetch public HTTP(S) sources only. Private-network targets are rejected to
  avoid turning a node into an SSRF proxy.
- Keep federation pull-based, signed, and explicitly peered. Sidefeed is not a
  general-purpose ActivityPub server in v1.
- Store a canonical item once, then project it into any number of feeds.
- Treat AI as a replaceable enrichment/filter stage. Ingestion and delivery
  continue to work when no model is configured.
- Ship useful primitives (JSON Feed, RSS, SSE, newsletter rendering) before
  coupling the service to any one mail or social provider.

## Milestones

### 1. Foundation and storage — complete

- Replace the prototype with an Axum/Tokio service and a small configuration
  surface.
- Add SQLite migrations for sources, canonical items, feeds, subscriptions,
  peers, fetch leases, and embeddings.
- Add health/readiness endpoints, structured errors, pagination, and tests.

### 2. Ingestion — complete

- Implement RSS, Atom, and JSON Feed discovery/parsing.
- Accept ActivityPub outbox/collection documents and normalize Note/Article
  activities.
- Support single-source creation and OPML import.
- Poll conditionally with ETag/Last-Modified, enforce response limits, and use
  leases to prevent duplicate work inside a node.

### 3. Feed products — complete

- CRUD named feeds and attach sources to them.
- Provide a chronological REST feed, FTS5 search, RSS 2.0, JSON Feed, and SSE.
- Render a newsletter-ready HTML/text digest and a concise social-thread JSON
  representation. Actual sending/posting remains an adapter responsibility.

### 4. Cooperative cache — complete

- Expose a bounded peer manifest and item endpoint.
- Authenticate peer requests with an HMAC signature and timestamp window.
- Pull items from configured peers before origin polling and merge by stable
  content ID, reducing duplicate origin requests across trusted nodes.

### 5. Filtering and AI — complete

- Add deterministic include/exclude filters and ranking hooks.
- Define an embedding provider interface with disabled, remote HTTP, and local
  CPU providers. The local provider uses Burn behind an opt-in feature so the
  default build stays small.
- Persist vectors for semantic retrieval and expose hybrid FTS/vector search.

### 6. Operations and hardening — complete

- Add a multi-stage container image, example configuration, graceful shutdown,
  request limits, timeouts, retention, and a non-root runtime.
- Add unit/integration tests, formatting/lint/test CI, and operator docs.
- Document trust boundaries, federation key rotation, backups, and deployment.

## Delivery sequence

Each milestone is committed separately on `feat/sidefeed-v1`. The pull request
checklist records tests and any deliberately deferred provider-specific work.

Implementation is complete on this branch. Newsletter email delivery and
posting to a specific social network remain adapters by design: the service
produces portable HTML, text, and thread JSON so credentials and provider SDKs
do not enter the core aggregator.
