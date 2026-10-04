import { For, Show } from 'solid-js';
import type { Filters } from '../state';

/// One filter bar for every view. It reports a whole new filter object rather
/// than mutating, because the filters live in the URL.
export function FilterBar(props: { filters: Filters; onChange: (next: Filters) => void; showWindow?: boolean }) {
  const patch = (change: Partial<Filters>) => props.onChange({ ...props.filters, ...change });
  const active = () => props.filters.tags.length > 0 || props.filters.host !== '' || props.filters.unreadOnly || props.filters.oldest;
  return (
    <div class="filterbar">
      <Show when={props.showWindow}>
        <span class="filter-group">
          window
          <For each={[24, 48, 168]}>
            {(hours) => (
              <button type="button" class={`opt${props.filters.hours === hours ? ' on' : ''}`} onClick={() => patch({ hours })}>
                {hours === 168 ? '7d' : `${hours}h`}
              </button>
            )}
          </For>
        </span>
      </Show>
      <span class="filter-group">
        order
        <button type="button" class={`opt${props.filters.oldest ? '' : ' on'}`} onClick={() => patch({ oldest: false })}>newest</button>
        <button type="button" class={`opt${props.filters.oldest ? ' on' : ''}`} onClick={() => patch({ oldest: true })}>oldest</button>
      </span>
      <span class="filter-group">
        <button type="button" class={`opt${props.filters.unreadOnly ? ' on' : ''}`} onClick={() => patch({ unreadOnly: !props.filters.unreadOnly })}>unread</button>
      </span>
      <Show when={props.filters.tags.length > 1}>
        <span class="filter-group">
          match
          <button type="button" class={`opt${props.filters.matchAll ? '' : ' on'}`} onClick={() => patch({ matchAll: false })}>any</button>
          <button type="button" class={`opt${props.filters.matchAll ? ' on' : ''}`} onClick={() => patch({ matchAll: true })}>all</button>
        </span>
      </Show>
      <Show when={props.filters.host}>
        <button type="button" class="tag on" title="clear the site filter" onClick={() => patch({ host: '' })}>
          {props.filters.host} ✕
        </button>
      </Show>
      <For each={props.filters.tags}>
        {(tag) => (
          <button type="button" class="tag on" title={`remove ${tag}`} onClick={() => patch({ tags: props.filters.tags.filter((value) => value !== tag) })}>
            {tag} ✕
          </button>
        )}
      </For>
      <Show when={active()}>
        <button type="button" class="opt reset" onClick={() => props.onChange({ ...props.filters, tags: [], host: '', unreadOnly: false, oldest: false })}>clear</button>
      </Show>
    </div>
  );
}
