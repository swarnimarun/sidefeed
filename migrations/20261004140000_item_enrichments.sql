-- Derived per-item artifacts (tags, generated summaries). Everything lives on
-- disk so a restart never recomputes, and only a tiny bounded cache keeps any of
-- it in memory. `model` names the enricher that produced a row, so two providers
-- can coexist and a change of model is just a new name.
CREATE TABLE IF NOT EXISTS item_tags (
  item_id    TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  tag        TEXT NOT NULL,
  model      TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (item_id, tag, model)
);

CREATE INDEX IF NOT EXISTS item_tags_by_tag ON item_tags(tag, model);

CREATE TABLE IF NOT EXISTS item_enrichments (
  item_id    TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  kind       TEXT NOT NULL,
  model      TEXT NOT NULL,
  value      TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (item_id, kind, model)
);
