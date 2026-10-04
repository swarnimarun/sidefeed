// One place that knows the HTTP shape of the service. Components import these
// and never touch fetch directly.

export type Item = {
  id: string;
  title?: string | null;
  url?: string | null;
  summary?: string | null;
  content?: string | null;
  published_at: string;
  feed?: string;
  feed_title?: string;
  tags?: string[];
  ai_summary?: string | null;
};

export type Feed = { slug: string; title: string; description?: string | null };
export type TagCount = { tag: string; count: number };
export type DigestItem = { title: string; url?: string | null; published_at: string; tags: string[] };
export type Category = { feed: string; title: string; count: number; summary?: string | null; items: DigestItem[] };
export type Updates = { window_hours: number; generated_at: string; summary?: string | null; categories: Category[] };
export type Page<T> = { items: T[]; next_cursor?: string | null };

export class ApiError extends Error {}

export async function get<T>(path: string): Promise<T> {
  const response = await fetch(path, { headers: { accept: 'application/json' } });
  if (!response.ok) throw new ApiError(`${response.status} ${response.statusText}`);
  return (await response.json()) as T;
}

export const feeds = () => get<Feed[]>('/api/v1/public/feeds');
export const tags = (hours = 168) => get<TagCount[]>(`/api/v1/tags?hours=${hours}&limit=60`);
export const feedTags = (slug: string) => get<TagCount[]>(`/api/v1/feeds/${encodeURIComponent(slug)}/tags?limit=60`);
export const updates = (hours: number) => get<Updates>(`/api/v1/updates?hours=${hours}`);
export const recent = (hours: number, filters = '') => get<Item[]>(`/api/v1/recent?hours=${hours}&limit=60${filters}`);
export const search = (query: string, filters = '') =>
  get<Item[]>(`/api/v1/search?q=${encodeURIComponent(query)}&limit=60${filters}`);
export const feedItems = (slug: string, filters = '') =>
  get<Page<Item>>(`/api/v1/feeds/${encodeURIComponent(slug)}/items?limit=60${filters}`);

/// Mirrors the server's filter vocabulary, so every view builds its query the
/// same way and a new filter is one line here.
export type Filters = { tags?: string[]; host?: string; matchAll?: boolean; oldest?: boolean; unreadOnly?: boolean };

export function filterQuery(filters: Filters): string {
  const parts: string[] = [];
  if (filters.tags?.length) parts.push(`tag=${encodeURIComponent(filters.tags.join(','))}`);
  if (filters.matchAll) parts.push('matching=all');
  if (filters.oldest) parts.push('order=oldest');
  if (filters.host) parts.push(`host=${encodeURIComponent(filters.host)}`);
  return parts.length ? `&${parts.join('&')}` : '';
}

/// Feed URLs come from third parties: only http(s) is ever rendered as a link.
export function safeUrl(value?: string | null): string {
  try {
    const url = new URL(String(value ?? ''));
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : '';
  } catch {
    return '';
  }
}

export const hostOf = (value?: string | null): string => {
  try {
    return new URL(String(value ?? '')).host;
  } catch {
    return '';
  }
};
