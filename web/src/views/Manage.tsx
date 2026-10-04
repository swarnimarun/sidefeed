import { createResource, createSignal, For, Show } from 'solid-js';
import {
  attachSource,
  createFeed,
  createSource,
  followSource,
  importOpml,
  listManagedFeeds,
  listSources,
  pollSource,
} from '../api';
import { TokenField } from './Keys';

const KINDS = ['auto', 'rss', 'raw-json', 'webhook', 'activitypub'];

/// Sources, feeds, and OPML import behind the admin token. Every mutation
/// surfaces its failure inline; 401/403 name the fix (set the admin token).
export function ManageView() {
  const [sources, { refetch: refetchSources }] = createResource(listSources);
  const [feeds, { refetch: refetchFeeds }] = createResource(listManagedFeeds);
  const [notice, setNotice] = createSignal('');

  const [sourceUrl, setSourceUrl] = createSignal('');
  const [sourceKind, setSourceKind] = createSignal('auto');
  const [feedSlug, setFeedSlug] = createSignal('');
  const [feedTitle, setFeedTitle] = createSignal('');
  const [feedPublic, setFeedPublic] = createSignal(true);

  const run = async (work: () => Promise<unknown>, ok: string): Promise<void> => {
    setNotice('');
    try {
      await work();
      setNotice(ok);
    } catch (failure) {
      setNotice((failure as Error).message);
    }
  };

  const addSource = (event: Event) => {
    event.preventDefault();
    const url = sourceUrl().trim();
    if (!url) return;
    void run(() => createSource(url, sourceKind()), `added ${url}.`).then(() => {
      setSourceUrl('');
      void refetchSources();
    });
  };

  const addFeed = (event: Event) => {
    event.preventDefault();
    const slug = feedSlug().trim();
    const title = feedTitle().trim();
    if (!slug || !title) return;
    void run(() => createFeed({ slug, title, public: feedPublic() }), `created ${slug}.`).then(() => {
      setFeedSlug('');
      setFeedTitle('');
      void refetchFeeds();
    });
  };

  const importFile = async (event: Event & { currentTarget: HTMLInputElement }) => {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    const text = await file.text();
    await run(() => importOpml(text), `imported ${file.name}.`);
    event.currentTarget.value = '';
    void refetchSources();
  };

  return (
    <div class="split">
      <section class="list manage">
        <TokenField />
        <Show when={notice()}>
          <p class="note">{notice()}</p>
        </Show>

        <form class="manage-form" onSubmit={addSource}>
          <p class="side-label">add a source</p>
          <div class="manage-row">
            <input
              class="manage-input"
              value={sourceUrl()}
              placeholder="https://example.com/feed.xml or user@host"
              aria-label="source url"
              onInput={(event) => setSourceUrl(event.currentTarget.value)}
            />
            <select
              class="manage-input"
              value={sourceKind()}
              aria-label="source kind"
              onChange={(event) => setSourceKind(event.currentTarget.value)}
            >
              <For each={KINDS}>{(kind) => <option value={kind}>{kind}</option>}</For>
            </select>
            <button type="submit" class="manage-btn" disabled={!sourceUrl().trim()}>
              add
            </button>
          </div>
        </form>

        <div class="manage-form">
          <p class="side-label">import opml</p>
          <input type="file" accept=".opml,.xml,.opf" aria-label="opml file" onChange={importFile} />
        </div>

        <p class="side-label">sources ({sources()?.length ?? 0})</p>
        <Show when={sources()} fallback={<p class="note">loading sources…</p>}>
          <ul class="manage-list">
            <For each={sources() ?? []} fallback={<li class="note">No sources yet — add one above.</li>}>
              {(source) => (
                <li>
                  <span class="manage-source">
                    <strong>{source.title || source.url}</strong>
                    <code>{source.kind}</code>
                    <Show when={source.last_error}>
                      <span class="manage-error">{source.last_error}</span>
                    </Show>
                  </span>
                  <span class="manage-actions">
                    <button
                      type="button"
                      class="opt"
                      onClick={() =>
                        void run(() => pollSource(source.id), 'polled.').then(() => refetchSources())
                      }
                    >
                      poll
                    </button>
                    <Show when={source.kind === 'activitypub'}>
                      <button
                        type="button"
                        class="opt"
                        onClick={() => void run(() => followSource(source.id), 'follow sent.')}
                      >
                        follow
                      </button>
                    </Show>
                    <select
                      class="manage-input"
                      aria-label={`attach ${source.url} to feed`}
                      onChange={(event) => {
                        const slug = event.currentTarget.value;
                        if (slug)
                          void run(() => attachSource(slug, source.id), `attached to ${slug}.`);
                        event.currentTarget.value = '';
                      }}
                    >
                      <option value="">attach…</option>
                      <For each={feeds() ?? []}>{(feed) => <option value={feed.slug}>{feed.slug}</option>}</For>
                    </select>
                  </span>
                </li>
              )}
            </For>
          </ul>
        </Show>

        <form class="manage-form" onSubmit={addFeed}>
          <p class="side-label">create a feed</p>
          <div class="manage-row">
            <input
              class="manage-input"
              value={feedSlug()}
              placeholder="slug"
              aria-label="feed slug"
              onInput={(event) => setFeedSlug(event.currentTarget.value)}
            />
            <input
              class="manage-input"
              value={feedTitle()}
              placeholder="title"
              aria-label="feed title"
              onInput={(event) => setFeedTitle(event.currentTarget.value)}
            />
            <label class="check">
              <input
                type="checkbox"
                checked={feedPublic()}
                onChange={(event) => setFeedPublic(event.currentTarget.checked)}
              />
              public
            </label>
            <button type="submit" class="manage-btn" disabled={!feedSlug().trim() || !feedTitle().trim()}>
              create
            </button>
          </div>
        </form>

        <p class="side-label">feeds ({feeds()?.length ?? 0})</p>
        <Show when={feeds()} fallback={<p class="note">loading feeds…</p>}>
          <ul class="manage-list">
            <For each={feeds() ?? []} fallback={<li class="note">No feeds yet — create one above.</li>}>
              {(feed) => (
                <li>
                  <span>
                    <strong>{feed.title}</strong> <code>{feed.slug}</code>
                    {feed.public ? '' : ' (private)'}
                  </span>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </section>
    </div>
  );
}
