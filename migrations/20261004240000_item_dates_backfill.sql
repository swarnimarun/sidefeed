-- Widen the first-seen reclassification added in 20261004220000. That pass used a
-- two-second window, but a poll parses and inserts a whole feed, so an undated
-- item's stamp and its fetch time can differ by more than two seconds: on a real
-- node a batch of 30 undated items differed by 0-11s. Real post dates sit far
-- from the fetch time (minutes to years), so a five-minute window separates the
-- two without touching them.
UPDATE items SET date_source = 'fetched'
WHERE date_source = 'published'
  AND ABS(strftime('%s', published_at) - strftime('%s', fetched_at)) <= 300;
