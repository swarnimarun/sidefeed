PRAGMA foreign_keys = ON;

CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'auto',
    title TEXT,
    etag TEXT,
    last_modified TEXT,
    last_polled_at TEXT,
    next_poll_at TEXT,
    last_error TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE items (
    id TEXT PRIMARY KEY,
    source_id TEXT,
    external_id TEXT NOT NULL,
    url TEXT,
    title TEXT,
    summary TEXT,
    content TEXT,
    author TEXT,
    published_at TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    tags_json TEXT NOT NULL DEFAULT '[]',
    raw_json TEXT,
    visibility TEXT NOT NULL DEFAULT 'public',
    FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX items_source_external ON items(source_id, external_id);
CREATE INDEX items_published ON items(published_at DESC, id DESC);

CREATE VIRTUAL TABLE items_fts USING fts5(
    item_id UNINDEXED, title, summary, content, author,
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER items_ai AFTER INSERT ON items BEGIN
  INSERT INTO items_fts(item_id,title,summary,content,author)
  VALUES(new.id,new.title,new.summary,new.content,new.author);
END;
CREATE TRIGGER items_ad AFTER DELETE ON items BEGIN
  DELETE FROM items_fts WHERE item_id=old.id;
END;
CREATE TRIGGER items_au AFTER UPDATE ON items BEGIN
  DELETE FROM items_fts WHERE item_id=old.id;
  INSERT INTO items_fts(item_id,title,summary,content,author)
  VALUES(new.id,new.title,new.summary,new.content,new.author);
END;

CREATE TABLE feeds (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,
    include_terms TEXT,
    exclude_terms TEXT,
    public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE TABLE feed_sources (
    feed_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    PRIMARY KEY(feed_id,source_id),
    FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
    FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
);
CREATE TABLE peers (
    id TEXT PRIMARY KEY,
    base_url TEXT NOT NULL UNIQUE,
    shared_secret TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_sync_at TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE fetch_leases (
    resource TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE TABLE embeddings (
    item_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(item_id,provider),
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE
);

