-- Where an item's published_at came from. Feeds are inconsistent: many omit a
-- date entirely, and some only carry the last-modified time. Recording the
-- source lets the reader label a fetch timestamp as "seen" instead of
-- presenting it as the article's post date, and lets an upsert keep the first
-- real date it saw rather than re-stamping an undated item on every poll.
ALTER TABLE items ADD COLUMN date_source TEXT NOT NULL DEFAULT 'published';

-- Existing rows have no recorded provenance. The one signal that survives is
-- the timestamp itself: when an item carried no feed date, the parser stamped
-- it with the fetch time, so the two are equal to the second. Reclassify those
-- so they are labelled as seen rather than presented as post dates.
UPDATE items SET date_source = 'fetched'
WHERE ABS(strftime('%s', published_at) - strftime('%s', fetched_at)) <= 2;
