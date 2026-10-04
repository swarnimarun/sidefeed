import { For, Show, createMemo, createResource, createSignal } from 'solid-js';
import { hostOf, safeUrl, similarItems, type Item } from '../api';
import { isBookmarked, toggleBookmark } from '../state';
import { decodeEntities, plainText } from '../text';
import { fullDate } from '../time';

/// Discussion links live in the item body for aggregators; the backend keeps the
/// item URL pointing at the article, so both can be offered.
function firstDiscussion(item: Item): string {
  const html = `${item.summary ?? ''} ${item.content ?? ''}`;
  const match = html.match(/<a[^>]+href=["']([^"']+)["'][^>]*>\s*(?:\[\s*)?(\d+\s+)?comments?/i);
  return match ? safeUrl(match[1]) : '';
}

export function ArticlePane(props: { item?: Item; onTag: (tag: string) => void; onHost: (host: string) => void; onCategory?: () => void; onSelect?: (item: Item) => void }) {
  const [generated, setGenerated] = createSignal<string>('');
  const [generating, setGenerating] = createSignal(false);
  const [error, setError] = createSignal('');
  const article = () => safeUrl(props.item?.url);
  const discussion = createMemo(() => (props.item ? firstDiscussion(props.item) : ''));
  const body = createMemo(() => plainText(props.item?.content || props.item?.summary));
  // Related items are fetched per article, so switching articles refetches.
  const [related] = createResource(() => props.item?.id, (id) => (id ? similarItems(id, 6) : Promise.resolve([] as Item[])));

  const generate = async () => {
    const item = props.item;
    if (!item?.id || !item.feed) return;
    setGenerating(true);
    setError('');
    try {
      const response = await fetch(`/api/v1/feeds/${encodeURIComponent(item.feed)}/items/${encodeURIComponent(item.id)}/summarize`, { method: 'POST' });
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      const atoms = (await response.json()) as { summary?: string | null };
      setGenerated(atoms.summary ?? '');
    } catch (failure) {
      setError((failure as Error).message);
    } finally {
      setGenerating(false);
    }
  };

  return (
    <section class="reader">
      <Show when={props.item} fallback={<article><p class="note">Pick an item, or ask for a summary of one.</p></article>}>
        {(item) => (
          <article>
            <header class="article-head">
              <p class="kicker">
                <Show when={props.onCategory} fallback={<span>{decodeEntities(item().feed_title || item().feed)}</span>}>
                  <button type="button" class="linklike" onClick={props.onCategory}>{decodeEntities(item().feed_title || item().feed)}</button>
                </Show>
              </p>
              <h1>{decodeEntities(item().title) || 'Untitled'}</h1>
              <p class="article-meta">
                <time>{fullDate(item())}</time>
                <button
                  type="button"
                  class={`opt${isBookmarked(item().id) ? ' on' : ''}`}
                  onClick={() => void toggleBookmark(item().id)}
                >
                  {isBookmarked(item().id) ? 'saved \u2605' : 'save \u2606'}
                </button>
                <Show when={article()}>
                  {' / '}
                  <a href={article()} target="_blank" rel="noopener noreferrer">original</a>
                </Show>
                <Show when={hostOf(article())}>
                  {' '}
                  <button type="button" class="opt" title={`everything from ${hostOf(article())}`} onClick={() => props.onHost(hostOf(article()))}>
                    more from {hostOf(article())}
                  </button>
                </Show>
                <Show when={discussion()}>
                  {' / '}
                  <a href={discussion()} target="_blank" rel="noopener noreferrer">comments</a>
                </Show>
              </p>
              <Show when={item().tags?.length}>
                <p class="tags">
                  <For each={item().tags}>
                    {(tag) => <button type="button" class="tag" onClick={() => props.onTag(tag)}>{tag}</button>}
                  </For>
                </p>
              </Show>
            </header>
            <Show
              when={body() || item().ai_summary || generated()}
              fallback={
                <div class="article-body">
                  <p class="note">No body text in the feed.</p>
                  <p>
                    <button type="button" class="opt" disabled={generating()} onClick={generate}>
                      {generating() ? 'summarising…' : 'generate a summary'}
                    </button>
                    <Show when={error()}> <span class="note">{error()}</span></Show>
                  </p>
                </div>
              }
            >
              <div class="article-body">{body() || item().ai_summary || generated()}</div>
              <Show when={!body() && (item().ai_summary || generated())}>
                <p class="ai-note">generated summary</p>
              </Show>
            </Show>
            <Show when={related()?.length}>
              <section class="related">
                <p class="side-label">similar</p>
                <ul>
                  <For each={related()}>
                    {(row) => (
                      <li>
                        <button type="button" class="related-item" onClick={() => props.onSelect?.(row)}>
                          <span class="related-title">{decodeEntities(row.title) || 'Untitled'}</span>
                          <span class="related-source">{decodeEntities(row.feed_title || row.feed)}</span>
                        </button>
                      </li>
                    )}
                  </For>
                </ul>
              </section>
            </Show>
          </article>
        )}
      </Show>
    </section>
  );
}
