// One place that knows the HTTP shape of the service. Components import these
// and never touch fetch directly.

export type Item = {
  id: string;
  title?: string | null;
  url?: string | null;
  summary?: string | null;
  content?: string | null;
  published_at: string;
  /// published | updated | fetched: where published_at came from, so the UI can
  /// label a fetch timestamp instead of showing it as the post date.
  date_source?: string | null;
  feed?: string;
  feed_title?: string;
  tags?: string[];
  ai_summary?: string | null;
};

export type Feed = { slug: string; title: string; description?: string | null };
export type SavedItem = Item & { feed: string; feed_title: string; saved_at: string };
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

/// The admin token lives in sessionStorage, never localStorage: closing the
/// tab forgets it, and the reader's persisted stores never see it. Only the
/// management views (Manage, Keys) read it; the reader stays token-free.
export const adminToken = (): string => sessionStorage.getItem('sidefeed-admin') || '';
export const setAdminToken = (token: string): void => {
  if (token) sessionStorage.setItem('sidefeed-admin', token);
  else sessionStorage.removeItem('sidefeed-admin');
};
export const adminHeaders = (): HeadersInit => {
  const token = adminToken();
  return token ? { authorization: `Bearer ${token}` } : {};
};

/// Authenticated management call. 401/403 name the fix (set the admin token)
/// instead of leaking a bare status; anything else keeps its status line.
async function mut<T>(method: string, path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method,
    headers: { 'content-type': 'application/json', ...adminHeaders() },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (response.status === 401 || response.status === 403) throw new ApiError('unauthorized: set the admin token');
  if (!response.ok) throw new ApiError(`${response.status} ${response.statusText}`);
  return (await response.json().catch(() => ({}))) as T;
}

export type ManagedSource = {
  id: string;
  url: string;
  kind: string;
  title?: string | null;
  last_error?: string | null;
};
export type ManagedFeed = {
  id: string;
  slug: string;
  title: string;
  description?: string | null;
  public: boolean;
};
export type MintedKey = {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  created_at: string;
  token: string;
};
export type AiStatus = {
  enrich: { provider: string; model: string | null; pending: number };
  embeddings: { provider: string; dimensions: number | null; pending: number };
};

export const listSources = () => mut<ManagedSource[]>('GET', '/api/v1/sources');
export const createSource = (url: string, kind = 'auto', config?: unknown) =>
  mut<ManagedSource>('POST', '/api/v1/sources', config === undefined ? { url, kind } : { url, kind, config });
export const pollSource = (id: string) =>
  mut<{ imported: number }>('POST', `/api/v1/sources/${encodeURIComponent(id)}/poll`);
export const followSource = (id: string) =>
  mut<unknown>('POST', `/api/v1/sources/${encodeURIComponent(id)}/follow`);
export const listManagedFeeds = () => mut<ManagedFeed[]>('GET', '/api/v1/feeds');
export const createFeed = (input: {
  slug: string;
  title: string;
  description?: string;
  public?: boolean;
  include_terms?: string;
  exclude_terms?: string;
}) => mut<ManagedFeed>('POST', '/api/v1/feeds', input);
export const attachSource = (slug: string, sourceId: string) =>
  mut<unknown>('POST', `/api/v1/feeds/${encodeURIComponent(slug)}/sources/${encodeURIComponent(sourceId)}`);

/// OPML import takes the file bytes as the body, so the caller reads the
/// picked file as text first. Auth failures name the token fix like `mut`.
export const importOpml = async (text: string): Promise<{ created: ManagedSource[]; skipped: number }> => {
  const response = await fetch('/api/v1/import/opml', {
    method: 'POST',
    headers: { 'content-type': 'application/xml', ...adminHeaders() },
    body: text,
  });
  if (response.status === 401 || response.status === 403) throw new ApiError('unauthorized: set the admin token');
  if (!response.ok) throw new ApiError(`${response.status} ${response.statusText}`);
  return (await response.json()) as { created: ManagedSource[]; skipped: number };
};

export const mintKey = (name: string, scopes: string[]) =>
  mut<MintedKey>('POST', '/api/v1/keys', { name, scopes });
export const revokeKey = (id: string) => mut<unknown>('DELETE', `/api/v1/keys/${encodeURIComponent(id)}`);

/// AI status is public (always 200, even with both providers disabled), so
/// the management view renders it without branching on auth.
export const aiStatus = () => get<AiStatus>('/api/v1/ai/status');

export const feeds = () => get<Feed[]>('/api/v1/public/feeds');
export const tags = (hours = 168) => get<TagCount[]>(`/api/v1/tags?hours=${hours}&limit=60`);
export const feedTags = (slug: string) => get<TagCount[]>(`/api/v1/feeds/${encodeURIComponent(slug)}/tags?limit=60`);
export const updates = (hours: number) => get<Updates>(`/api/v1/updates?hours=${hours}`);
export const recent = (hours: number, filters = '') => get<Item[]>(`/api/v1/recent?hours=${hours}&limit=60${filters}`);
export const search = (query: string, filters = '') =>
  get<Item[]>(`/api/v1/search?q=${encodeURIComponent(query)}&limit=60${filters}`);
export const feedItems = (slug: string, filters = '', cursor?: string) =>
  get<Page<Item>>(
    `/api/v1/feeds/${encodeURIComponent(slug)}/items?limit=60${filters}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ''}`,
  );

/// Saved items and the two mutations behind the reader's bookmark toggle. The
/// service is the source of truth; the UI stores only the resulting id set.
/// Writes are token-free by design (single-user reader), so a failure here is
/// surfaced by rolling the optimistic toggle back in `state.ts`.
export const savedItems = () => get<SavedItem[]>('/api/v1/bookmarks');
export const similarItems = (id: string, limit = 6) =>
  get<Item[]>(`/api/v1/items/${encodeURIComponent(id)}/similar?limit=${limit}`);

async function write(method: 'POST' | 'DELETE', path: string): Promise<void> {
  const response = await fetch(path, { method });
  if (!response.ok) throw new ApiError(`${response.status} ${response.statusText}`);
}
export const addBookmark = (id: string) => write('POST', `/api/v1/items/${encodeURIComponent(id)}/bookmark`);
export const removeBookmark = (id: string) => write('DELETE', `/api/v1/items/${encodeURIComponent(id)}/bookmark`);

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
