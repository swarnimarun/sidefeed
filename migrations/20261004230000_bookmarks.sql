-- Reader saved items. The reader stays token-free, so this is a single-user
-- store: one row per saved item and no per-user column. Adds are bounded in the
-- API so a public node cannot be filled through the bookmark route.
CREATE TABLE IF NOT EXISTS bookmarks (
  item_id    TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL
);
