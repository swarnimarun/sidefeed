import { createSignal } from 'solid-js';
import { addBookmark, removeBookmark, savedItems } from './api';

// Small browser-local stores, kept out of components so every view shares them.

const readKey = 'sidefeed-read';
const readIds = new Set<string>(JSON.parse(localStorage.getItem(readKey) || '[]') as string[]);
export const isRead = (id: string) => readIds.has(id);
export const markRead = (id: string) => {
  if (readIds.has(id)) return;
  readIds.add(id);
  localStorage.setItem(readKey, JSON.stringify([...readIds].slice(-500)));
};

/// Saved items live on the service, not in localStorage: the list is shared
/// across a node's devices and survives a browser reset. This signal mirrors the
/// resulting id set so every star renders without a request per row. Toggles
/// are optimistic and roll back when the write fails.
const [bookmarks, setBookmarks] = createSignal<string[]>([]);
export { bookmarks };
export const isBookmarked = (id: string) => bookmarks().includes(id);
let bookmarksLoaded = false;
export async function syncBookmarks() {
  if (bookmarksLoaded) return;
  bookmarksLoaded = true;
  try { setBookmarks((await savedItems()).map((item) => item.id)); }
  catch { bookmarksLoaded = false; }
}
export async function toggleBookmark(id: string) {
  const on = isBookmarked(id);
  const previous = bookmarks();
  setBookmarks(on ? previous.filter((value) => value !== id) : [id, ...previous]);
  try { await (on ? removeBookmark(id) : addBookmark(id)); }
  catch { setBookmarks(previous); toast('could not save the bookmark'); }
}

/// One transient message rendered as a `role=status` live region in the
/// layout. Background writes (bookmark rollback, poll, page fetch) land here
/// so failures are announced instead of failing silently.
const [toastText, setToastText] = createSignal('');
export { toastText };
let toastTimer: number | undefined;
export function toast(message: string) {
  setToastText(message);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => setToastText(''), 4000);
}

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
