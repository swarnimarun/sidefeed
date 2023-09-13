-- Add up migration script here
-- I might want to do indexing separately and use explicit rowid but mehhh!
CREATE TABLE IF NOT EXISTS urls (
   	url TEXT PRIMARY KEY,
	urltype INT DEFAULT 0
);

CREATE TABLE IF NOT EXISTS feeds (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
   	url TEXT,
    last_checked TEXT NOT NULL,
    last_modified TEXT NOT NULL,
	CONSTRAINT conts_url
		FOREIGN KEY (url)
		REFERENCES urls(url)
		ON DELETE CASCADE
);