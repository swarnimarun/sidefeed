import { createSignal } from 'solid-js';

// Small browser-local stores, kept out of components so every view shares them.

const readKey = 'sidefeed-read';
const readIds = new Set<string>(JSON.parse(localStorage.getItem(readKey) || '[]') as string[]);
export const isRead = (id: string) => readIds.has(id);
export const markRead = (id: string) => {
  if (readIds.has(id)) return;
  readIds.add(id);
  localStorage.setItem(readKey, JSON.stringify([...readIds].slice(-500)));
};

const pinKey = 'sidefeed-pins';
const [pins, setPins] = createSignal<string[]>(JSON.parse(localStorage.getItem(pinKey) || '[]') as string[]);
export { pins };
export const isPinned = (slug: string) => pins().includes(slug);
export const togglePin = (slug: string) => {
  const next = isPinned(slug) ? pins().filter((id) => id !== slug) : [...pins(), slug];
  setPins(next);
  localStorage.setItem(pinKey, JSON.stringify(next));
};

/// Filters are route-owned state: they live in the query string, so a filtered
/// view is a link you can send someone.
export type Filters = { tags: string[]; host: string; matchAll: boolean; oldest: boolean; unreadOnly: boolean; hours: number };

export const filtersFromParams = (params: URLSearchParams): Filters => ({
  tags: (params.get('tag') || '').split(',').map((tag) => tag.trim()).filter(Boolean),
  host: params.get('host') || '',
  matchAll: params.get('matching') === 'all',
  oldest: params.get('order') === 'oldest',
  unreadOnly: params.get('unread') === '1',
  hours: Number(params.get('hours') || 48),
});

export const paramsFromFilters = (filters: Filters): string => {
  const params = new URLSearchParams();
  if (filters.tags.length) params.set('tag', filters.tags.join(','));
  if (filters.host) params.set('host', filters.host);
  if (filters.matchAll) params.set('matching', 'all');
  if (filters.oldest) params.set('order', 'oldest');
  if (filters.unreadOnly) params.set('unread', '1');
  if (filters.hours !== 48) params.set('hours', String(filters.hours));
  const query = params.toString();
  return query ? `?${query}` : '';
};
