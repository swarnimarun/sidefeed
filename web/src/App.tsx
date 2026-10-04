import { A, Route, Router, useNavigate, useParams, useSearchParams } from '@solidjs/router';
import { For, Show, createEffect, createResource, createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { filterQuery, feeds, feedItems, hostOf, recent, safeUrl, savedItems, search, type Filters as ApiFilters, type Item } from './api';
import { bookmarks, filtersFromParams, isBookmarked, isPinned, isRead, markRead, paramsFromFilters, pins, syncBookmarks, toggleBookmark, type Filters } from './state';
import { decodeEntities, summaryLine } from './text';
import { rowDate } from './time';
import { UpdatesView } from './views/Updates';
import { ArticlePane } from './views/Article';
import { FilterBar } from './views/Filters';
import './styles.css';

const withUnread = (items: Item[], unreadOnly: boolean) => (unreadOnly ? items.filter((item) => !isRead(item.id)) : items);

// ---------------------------------------------------------------- recents
function RecentsView() {
  const [params, setParams] = useSearchParams<Record<string, string>>();
  const filters = () => filtersFromParams(new URLSearchParams(params as Record<string, string>));
  const [items] = createResource(filters, (current) => recent(current.hours, filterQuery(current as ApiFilters)));
  const [selected, setSelected] = createSignal<Item | undefined>();
  return (
    <div class="split">
      <section class="list">
        <FilterBar filters={filters()} onChange={(next) => setParams(Object.fromEntries(new URLSearchParams(paramsFromFilters(next))))} showWindow />
        <ItemList items={withUnread(items() ?? [], filters().unreadOnly)} selected={(item) => item.id === selected()?.id} onSelect={setSelected} />
      </section>
      <ArticlePane item={selected()} onTag={() => undefined} onHost={() => undefined} onSelect={setSelected} />
    </div>
  );
}

// ---------------------------------------------------------------- one feed
function FeedView() {
  const params = useParams<{ slug: string }>();
  const [query, setQuery] = useSearchParams<Record<string, string>>();
  const filters = () => filtersFromParams(new URLSearchParams(query as Record<string, string>));
  const [items] = createResource(() => [params.slug, filters()] as const, ([slug, current]) => feedItems(slug, filterQuery(current as ApiFilters)).then((page) => page.items));
  const [selected, setSelected] = createSignal<Item | undefined>();
  const navigate = useNavigate();
  return (
    <div class="split">
      <section class="list">
        <FilterBar filters={filters()} onChange={(next) => setQuery(Object.fromEntries(new URLSearchParams(paramsFromFilters(next))))} />
        <ItemList items={withUnread(items() ?? [], filters().unreadOnly)} selected={(item) => item.id === selected()?.id} onSelect={setSelected} />
      </section>
      <ArticlePane
        item={selected()}
        onTag={(tag) => {
          const next = { ...filters(), tags: filters().tags.includes(tag) ? filters().tags.filter((value) => value !== tag) : [...filters().tags, tag] };
          setQuery(Object.fromEntries(new URLSearchParams(paramsFromFilters(next))));
        }}
        onHost={(host) => setQuery(Object.fromEntries(new URLSearchParams(paramsFromFilters({ ...filters(), host }))))}
        onCategory={() => navigate(`/${params.slug}`)}
        onSelect={setSelected}
      />
    </div>
  );
}

// ---------------------------------------------------------------- search
function SearchView() {
  const [params, setParams] = useSearchParams<Record<string, string>>();
  const value = () => params.q ?? '';
  const [term, setTerm] = createSignal(value());
  let timer: number | undefined;
  createEffect(() => setTerm(value()));
  onCleanup(() => clearTimeout(timer));
  const [items] = createResource(() => params.q, (q) => (q ? search(q) : Promise.resolve([])));
  const [selected, setSelected] = createSignal<Item | undefined>();
  return (
    <div class="split">
      <section class="list">
        <div class="searchbar">
          <input
            class="search"
            type="search"
            placeholder="search the whole archive"
            value={term()}
            autofocus
            onInput={(event) => {
              const next = event.currentTarget.value;
              setTerm(next);
              clearTimeout(timer);
              timer = window.setTimeout(() => setParams({ ...params, q: next || undefined }), 300);
            }}
          />
          <span class="count">{items()?.length ?? 0} hits</span>
        </div>
        <ItemList items={withUnread(items() ?? [], false)} selected={(item) => item.id === selected()?.id} onSelect={setSelected} />
      </section>
      <ArticlePane item={selected()} onTag={() => undefined} onHost={() => undefined} onSelect={setSelected} />
    </div>
  );
}

// ---------------------------------------------------------------- saved
function SavedView() {
  const [items, { refetch }] = createResource(savedItems);
  const [selected, setSelected] = createSignal<Item | undefined>();
  // Any star toggles the shared set, so the list follows the service rather
  // than the copy this resource fetched when it mounted.
  let primed = false;
  createEffect(() => {
    bookmarks();
    if (primed) void refetch();
    primed = true;
  });
  return (
    <div class="split">
      <section class="list">
        <div class="filterbar">
          <span class="side-label">saved</span>
          <span class="count">{items()?.length ?? 0}</span>
        </div>
        <ItemList items={items() ?? []} showFeed selected={(item) => item.id === selected()?.id} onSelect={setSelected} />
      </section>
      <ArticlePane item={selected()} onTag={() => undefined} onHost={() => undefined} onSelect={setSelected} />
    </div>
  );
}

// ---------------------------------------------------------------- shared pieces
function ItemList(props: { items: Item[]; selected: (item: Item) => boolean; onSelect: (item: Item) => void; showFeed?: boolean }) {
  return (
    <ol class="items">
      <For each={props.items} fallback={<li class="note">Nothing here.</li>}>
        {(item) => (
          <li class="item-row">
            <button
              type="button"
              class={`row${props.selected(item) ? ' active' : ''}`}
              data-unread={!isRead(item.id) ? 'true' : undefined}
              onClick={() => {
                markRead(item.id);
                props.onSelect(item);
              }}
            >
              <span class="row-title">
                <Show when={props.showFeed && (item.feed_title || item.feed)}>
                  <span class="row-source">{decodeEntities(item.feed_title || item.feed)} </span>
                </Show>
                {decodeEntities(item.title) || 'Untitled'}
              </span>
              <time>{rowDate(item)}</time>
              <Show when={summaryLine(item.ai_summary || item.summary)}>
                <span class="row-summary">{summaryLine(item.ai_summary || item.summary)}</span>
              </Show>
            </button>
            <button
              type="button"
              class={`star${isBookmarked(item.id) ? ' on' : ''}`}
              title={isBookmarked(item.id) ? 'remove bookmark' : 'bookmark'}
              aria-label={isBookmarked(item.id) ? 'remove bookmark' : 'bookmark'}
              onClick={() => void toggleBookmark(item.id)}
            >
              {isBookmarked(item.id) ? '\u2605' : '\u2606'}
            </button>
          </li>
        )}
      </For>
    </ol>
  );
}

function Sidebar() {
  const [available] = createResource(feeds);
  return (
    <aside class="sidebar">
      <ul class="feeds">
        <li><A href="/recents" activeClass="active">recents</A></li>
        <li><A href="/updates" activeClass="active">updates</A></li>
        <li><A href="/search" activeClass="active">search</A></li>
        <li><A href="/saved" activeClass="active">saved</A></li>
      </ul>
      <Show when={pins().length}>
        <p class="side-label">pinned</p>
        <ul class="feeds">
          <For each={(available() ?? []).filter((feed) => isPinned(feed.slug))}>
            {(feed) => <li><A href={`/${feed.slug}`} activeClass="active">{feed.title}</A></li>}
          </For>
        </ul>
      </Show>
      <p class="side-label">feeds</p>
      <ul class="feeds">
        <For each={available() ?? []}>
          {(feed) => <li><A href={`/${feed.slug}`} activeClass="active">{feed.title}</A></li>}
        </For>
      </ul>
      <p class="side-label">elsewhere</p>
      <ul class="feeds">
        <li><A href="/updates?hours=168">this week</A></li>
        <li><a href="/docs">api docs</a></li>
      </ul>
    </aside>
  );
}

/// The chrome lives in the router's root layout, the only place outside a route
/// that may use router primitives like A.
function Layout(props: { children?: JSX.Element }) {
  return (
    <>
      <header class="bar">
        <span class="wordmark">sidefeed</span>
        <nav class="bar-links"><a href="/docs">docs</a></nav>
      </header>
      <main class="layout">
        <Sidebar />
        {props.children}
      </main>
    </>
  );
}

export default function App() {
  onMount(() => void syncBookmarks());
  return (
    <Router root={Layout}>
      <Route path="/" component={RecentsView} />
      <Route path="/recents" component={RecentsView} />
      <Route path="/updates" component={UpdatesView} />
      <Route path="/search" component={SearchView} />
      <Route path="/saved" component={SavedView} />
      <Route path="/:slug" component={FeedView} />
    </Router>
  );
}
