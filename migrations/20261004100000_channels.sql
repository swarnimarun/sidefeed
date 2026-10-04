-- Per-source poller configuration and signed webhook ingress channels.
-- A source with kind `webhook` or `raw-json` keeps its poller settings here
-- instead of in the sources row, so new kinds never widen that table.
CREATE TABLE IF NOT EXISTS source_configs (
  source_id TEXT PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  config_json TEXT NOT NULL DEFAULT '{}'
);
-- A webhook channel is a named ingress endpoint bound to one source. Only the
-- SHA-256 hash of the shared secret is stored; the plaintext travels as a
-- Bearer token or signs the body via the `x-sidefeed-signature` HMAC header.
CREATE TABLE IF NOT EXISTS webhook_channels (
  id TEXT PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  secret_hash TEXT NOT NULL,
  source_id TEXT REFERENCES sources(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);
