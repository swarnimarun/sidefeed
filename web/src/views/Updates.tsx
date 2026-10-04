import { A, useSearchParams } from '@solidjs/router';
import { For, Show, createResource } from 'solid-js';
import { updates } from '../api';
import { isPinned, togglePin } from '../state';
import { decodeEntities } from '../text';

/// The digest page: per-category summaries of what moved, with the items worth
/// opening, and pinned categories first.
export function UpdatesView() {
  const [params, setParams] = useSearchParams<Record<string, string>>();
  const hours = () => Number((params as Record<string, string>).hours || 48);
  const [data] = createResource(hours, updates);
  const ordered = () => {
    const categories = data()?.categories ?? [];
    return [...categories].sort((left, right) => Number(isPinned(right.feed)) - Number(isPinned(left.feed)));
  };
  return (
    <section class="reader">
      <div class="digest">
        <p class="kicker">updates</p>
        <Show when={data()} fallback={<p class="note">Building the digest…</p>}>
          <Show when={ordered().length} fallback={<p class="note">Nothing published in the last {hours()} hours.</p>}>
            <Show when={data()!.summary}>
              <p class="digest-lead">{data()!.summary}</p>
            </Show>
            <For each={ordered()}>
              {(category) => (
                <section class="digest-block">
                  <header>
                    <A class="digest-cat" href={`/${category.feed}`}>{decodeEntities(category.title)}</A>
                    <span class="count">{category.count}</span>
                    <button type="button" class={`opt${isPinned(category.feed) ? ' on' : ''}`} onClick={() => togglePin(category.feed)}>
                      {isPinned(category.feed) ? 'unpin' : 'pin'}
                    </button>
                  </header>
                  <Show when={category.summary}>
                    <p class="digest-summary">{category.summary}</p>
                  </Show>
                  <ul class="digest-items">
                    <For each={category.items}>
                      {(item) => (
                        <li>
                          <Show when={item.url} fallback={<span>{decodeEntities(item.title)}</span>}>
                            <a href={item.url ?? '#'} target="_blank" rel="noopener noreferrer">{decodeEntities(item.title)}</a>
                          </Show>
                          <span class="meta">
                            {new Date(item.published_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                            <For each={item.tags.slice(0, 2)}>{(tag) => <span class="tag static">{tag}</span>}</For>
                          </span>
                        </li>
                      )}
                    </For>
                  </ul>
                </section>
              )}
            </For>
          </Show>
        </Show>
        <p>
          <button type="button" class={`opt${hours() === 24 ? ' on' : ''}`} onClick={() => setParams({ hours: '24' })}>24h</button>
          <button type="button" class={`opt${hours() === 48 ? ' on' : ''}`} onClick={() => setParams({ hours: '48' })}>48h</button>
          <button type="button" class={`opt${hours() === 168 ? ' on' : ''}`} onClick={() => setParams({ hours: '168' })}>7d</button>
        </p>
      </div>
    </section>
  );
}
