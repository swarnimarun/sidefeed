import { A, Route, Router, useNavigate, useParams, useSearchParams } from '@solidjs/router';
import { For, Show, createEffect, createResource, createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { filterQuery, feeds, feedItems, hostOf, recent, safeUrl, savedItems, search, type Filters as ApiFilters, type Item } from './api';
import { bookmarks, filtersFromParams, isBookmarked, isPinned, isRead, markRead, paramsFromFilters, pins, syncBookmarks, toast, toastText, toggleBookmark, type Filters } from './state';
import { decodeEntities, summaryLine } from './text';
import { rowDate } from './time';
import { UpdatesView } from './views/Updates';
import { ArticlePane } from './views/Article';
import { FilterBar } from './views/Filters';
import { ManageView } from './views/Manage';
import { KeysView } from './views/Keys';
import { AiView } from './views/Ai';
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
        <ItemList
          items={withUnread(items() ?? [], filters().unreadOnly)}
          selected={(item) => item.id === selected()?.id}
          onSelect={setSelected}
          empty={<>No items yet — <A href="/manage">add a source in Manage</A>.</>}
        />
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
  const [first] = createResource(() => [params.slug, filters()] as const, ([slug, current]) => feedItems(slug, filterQuery(current as ApiFilters)));
  // Pages accumulate here; `cursor` is undefined while the first page is
  // pending and null once the server reports no `next_cursor`.
  const [items, setItems] = createSignal<Item[]>([]);
  const [cursor, setCursor] = createSignal<string | null | undefined>(undefined);
  const [loadingMore, setLoadingMore] = createSignal(false);
  createEffect(() => {
    const page = first();
    if (page) {
      setItems(page.items);
      setCursor(page.next_cursor ?? null);
    }
  });
  createEffect(() => {
    // A new feed or filter restarts pagination; the effect above
    // repopulates once the first page resolves.
    params.slug;
    filters();
    setItems([]);
    setCursor(undefined);
  });
  const loadMore = async () => {
    const next = cursor();
    if (next === null || next === undefined || loadingMore() || first.loading) return;
    setLoadingMore(true);
    try {
      const page = await feedItems(params.slug, filterQuery(filters() as ApiFilters), next);
      setItems((current) => [...current, ...page.items]);
      setCursor(page.next_cursor ?? null);
    } catch (failure) {
      toast((failure as Error).message);
    } finally {
      setLoadingMore(false);
    }
  };
  const [selected, setSelected] = createSignal<Item | undefined>();
  const navigate = useNavigate();
  return (
    <div class="split">
      <section class="list">
        <FilterBar filters={filters()} onChange={(next) => setQuery(Object.fromEntries(new URLSearchParams(paramsFromFilters(next))))} />
        <ItemList
          items={withUnread(items(), filters().unreadOnly)}
          selected={(item) => item.id === selected()?.id}
          onSelect={setSelected}
          onLoadMore={loadMore}
          hasMore={cursor() !== null && cursor() !== undefined}
          empty={<>No items in this feed yet — <A href="/manage">add a source in Manage</A>.</>}
        />
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
        <ItemList
          items={withUnread(items() ?? [], false)}
          selected={(item) => item.id === selected()?.id}
          onSelect={setSelected}
          empty={<>{value() ? 'No matches — try fewer or different terms.' : 'Type above to search the whole archive.'}</>}
        />
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
        <ItemList
          items={items() ?? []}
          showFeed
          selected={(item) => item.id === selected()?.id}
          onSelect={setSelected}
          empty={<>Nothing saved yet — star an item to keep it here.</>}
        />
      </section>
      <ArticlePane item={selected()} onTag={() => undefined} onHost={() => undefined} onSelect={setSelected} />
    </div>
  );
}

// ---------------------------------------------------------------- shared pieces
function ItemList(props: {
  items: Item[];
  selected: (item: Item) => boolean;
  onSelect: (item: Item) => void;
  showFeed?: boolean;
  onLoadMore?: () => void;
  hasMore?: boolean;
  empty?: JSX.Element;
}) {
  // Infinite scroll: a sentinel row at the end of the list appends the next
  // `next_cursor` page when it scrolls into view. Views without pagination
  // simply omit `onLoadMore` and render one page.
  let sentinel: HTMLLIElement | undefined;
  createEffect(() => {
    const load = props.onLoadMore;
    if (!sentinel || !load || !props.hasMore) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) load();
      },
      { rootMargin: '400px' },
    );
    observer.observe(sentinel);
    onCleanup(() => observer.disconnect());
  });
  return (
    <ol class="items">
      <For each={props.items} fallback={<li class="note">{props.empty ?? 'Nothing here.'}</li>}>
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
      <Show when={props.hasMore && props.items.length}>
        <li
          ref={(element) => {
            sentinel = element;
          }}
          class="note"
          aria-hidden="true"
        >
          loading more…
        </li>
      </Show>
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
      <p class="side-label">manage</p>
      <ul class="feeds">
        <li><A href="/manage" activeClass="active">sources</A></li>
        <li><A href="/keys" activeClass="active">keys</A></li>
        <li><A href="/ai" activeClass="active">ai</A></li>
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
      <Show when={toastText()}>
        <div class="toast" role="status">{toastText()}</div>
      </Show>
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
      <Route path="/manage" component={ManageView} />
      <Route path="/keys" component={KeysView} />
      <Route path="/ai" component={AiView} />
      <Route path="/:slug" component={FeedView} />
    </Router>
  );
}
