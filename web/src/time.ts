// Dates are the one place where feed metadata is routinely missing or
// misleading, so the labels say where the timestamp came from instead of
// presenting every value as a post date.

export type Dated = { published_at: string; date_source?: string | null };

const sameDay = (left: Date, right: Date) =>
  left.getFullYear() === right.getFullYear() && left.getMonth() === right.getMonth() && left.getDate() === right.getDate();

/// List rows show a clock time only for something from today. Anything older
/// gets a calendar date, so a feed spanning years is readable. A timestamp the
/// feed did not provide is labelled as seen, because it is our fetch time.
export function rowDate(item: Dated, now = new Date()): string {
  const date = new Date(item.published_at);
  if (Number.isNaN(date.getTime())) return '';
  const day = date.toLocaleDateString(undefined, date.getFullYear() === now.getFullYear()
    ? { month: 'short', day: 'numeric' }
    : { year: 'numeric', month: 'short', day: 'numeric' });
  if (item.date_source === 'fetched') return `seen ${day}`;
  return sameDay(date, now) ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : day;
}

/// The full stamp for the article header, with the same seen/posted distinction.
export function fullDate(item: Dated): string {
  const date = new Date(item.published_at);
  if (Number.isNaN(date.getTime())) return '';
  const label = date.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
  return item.date_source === 'fetched' ? `seen ${label}` : label;
}
