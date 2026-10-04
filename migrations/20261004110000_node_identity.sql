-- Node identity for ActivityPub read-plus-follow. The node is not a general
-- ActivityPub server: this table holds exactly one ed25519 signing seed plus
-- small flags, so the node can sign Follow activities and verify inbox
-- deliveries without any new service or key file on disk.
CREATE TABLE IF NOT EXISTS node_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
