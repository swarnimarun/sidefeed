// sidefeed reader. Read-only by design: it never asks for the admin token and
// never writes to the API. Writes stay on the API, behind the bearer token.
const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];

// ---------------------------------------------------------------- text helpers
const escapeHtml = (value) => String(value ?? '').replace(/[&<>"']/g, (char) => (
  { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]
));

// Feed URLs come from third parties, so only http(s) survives.
const safeUrl = (value) => {
  try {
    const url = new URL(String(value ?? ''));
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : '';
  } catch { return ''; }
};

// Item bodies are third-party HTML. Strip tags and decode entities once here,
// then escape on the way out, so nothing from a feed can reach innerHTML raw.
const plainText = (value) => String(value ?? '')
  .replace(/<\s*(br|\/p|\/div|\/li|\/h[1-6]|\/tr)\s*\/?>/gi, '\n')
  .replace(/<[^>]*>/g, '')
  .replace(/&nbsp;/gi, ' ').replace(/&amp;/gi, '&').replace(/&lt;/gi, '<')
  .replace(/&gt;/gi, '>').replace(/&quot;/gi, '"').replace(/&#39;/gi, "'")
  .replace(/[ \t]+/g, ' ')
  .replace(/\n{3,}/g, '\n\n')
  .trim();

const preview = (value, max = 160) => {
  const text = plainText(value).replace(/\s+/g, ' ');
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
};

// Feeds repeat their title, or emit link text like "Comments", as the summary.
const usefulSummary = (item, title) => {
  const text = preview(item.summary || item.content);
  if (!text || text.length < 40) return '';
  return text.toLowerCase() === title.toLowerCase() ? '' : text;
};

const linkOnlyBody = /^(comments?|read more|continue reading|permalink|link|via\s+.{0,40})$/i;
const usefulBody = (item, title) => {
  const text = plainText(item.content || item.summary);
  if (!text || text.length < 24 || linkOnlyBody.test(text)) return '';
  return text.toLowerCase() === title.toLowerCase() ? '' : text;
};

// Aggregators post the story and its discussion as two separate links. Hacker
// News puts the article in the item url and a "Comments" anchor in the summary;
// Reddit does the reverse. Classify both so neither link is lost, and so an
// empty body does not hide the discussion.
const discussionHosts = /(^|\.)(news\.ycombinator\.com|reddit\.com|lobste\.rs|tildes\.net|lemmy\.[a-z.]+)$/i;
const discussionPath = /\/item\b|\/comments\/|\/s\/|\/r\/|\/~|\/post\//i;
const discussionLabel = /^(comments?|\d+\s+comments?|\d+\s+replies|discuss(ion)?|replies|thread|join the discussion)$/i;

const isDiscussion = (href, text) => {
  if (discussionLabel.test(String(text ?? '').trim())) return true;
  try {
    const url = new URL(href);
    return discussionHosts.test(url.hostname) && discussionPath.test(url.pathname + url.search);
  } catch { return false; }
};

// Anchors are read through DOMParser, which is inert: nothing from the feed
// runs and no subresource is fetched. Only href and text are kept.
const anchorsIn = (html) => {
  if (!html || !/<a[\s>]/i.test(html)) return [];
  try {
    return [...new DOMParser().parseFromString(html, 'text/html').querySelectorAll('a[href]')]
      .map((anchor) => ({ href: safeUrl(anchor.getAttribute('href')), text: anchor.textContent || '' }))
      .filter((anchor) => anchor.href);
  } catch { return []; }
};

const linksFor = (item) => {
  const candidates = [{ href: safeUrl(item.url), text: '' }, ...anchorsIn(item.summary), ...anchorsIn(item.content)]
    .filter((candidate) => candidate.href);
  const discussion = candidates.find((candidate) => isDiscussion(candidate.href, candidate.text));
  const article = candidates.find((candidate) => candidate !== discussion && !isDiscussion(candidate.href, candidate.text));
  return { article: article?.href || '', discussion: discussion?.href || '' };
};

const clockTime = (value) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
};

const longDate = (value) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString(undefined, {
    year: 'numeric', month: 'long', day: 'numeric',
  });
};

// Items arrive newest first, so a day heading is just a change of date.
const dayLabel = (value) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const now = new Date();
  if (date.toDateString() === now.toDateString()) return 'today';
  if (date.toDateString() === new Date(now.getTime() - 86400000).toDateString()) return 'yesterday';
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short', ...(sameYear ? {} : { year: 'numeric' }) });
};

// ---------------------------------------------------------------- read state
// Which items have been opened, remembered per browser. Small interface over
// storage plus pruning: callers only ask has/add.
const read = (() => {
  const KEY = 'sidefeed-read';
  const LIMIT = 500;
  let ids = new Set();
  try { ids = new Set(JSON.parse(localStorage.getItem(KEY) || '[]')); } catch { ids = new Set(); }
  const persist = () => { try { localStorage.setItem(KEY, JSON.stringify([...ids].slice(-LIMIT))); } catch { /* storage is optional */ } };
  return {
    has: (id) => ids.has(id),
    add(id) { if (id && !ids.has(id)) { ids.add(id); persist(); } },
  };
})();

// ---------------------------------------------------------------- state
const state = {
  feeds: [], slug: '', items: [], selected: null, stream: null,
  mode: 'feed',        // feed | recent | search
  query: '',           // active search text
  tags: [],            // selected tags, server side filtered
  matchAll: false,     // every tag instead of any
  unreadOnly: false,
  oldest: false,
  hours: 48,
};

const currentPath = () => location.pathname.replace(/\/+$/, '') || '/';
const pushRoute = (path) => { if (location.pathname + location.search !== path) history.pushState({}, '', path); };

// Prefer what the feed shipped; fall back to the generated summary so link-only
// items, which have no body at all, still say something useful in the list.
const rowSummary = (item, title) => usefulSummary(item, title)
  || (item.ai_summary ? preview(item.ai_summary, 160) : '');

async function api(path) {
  const response = await fetch(path, { headers: { accept: 'application/json' } });
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  return response.json();
}

const currentFeed = () => state.feeds.find((feed) => feed.slug === state.slug);
const isNarrow = () => matchMedia('(max-width: 900px)').matches;

// ---------------------------------------------------------------- panes
const panes = {
  feeds: { chip: '#toggle-feeds', hide: 'hide-feeds' },
  items: { chip: '#toggle-items', hide: 'hide-items' },
  article: { chip: '#toggle-article', hide: 'hide-article' },
};

function togglePane(name, force) {
  const pane = panes[name];
  const folded = force === undefined ? !document.body.classList.contains(pane.hide) : !force;
  document.body.classList.toggle(pane.hide, folded);
  const chip = $(pane.chip);
  chip.setAttribute('aria-pressed', String(!folded));
  chip.title = `${folded ? 'unfold' : 'fold'} ${name} pane`;
  chip.setAttribute('aria-label', chip.title);
}

// ---------------------------------------------------------------- render
function renderFeeds() {
  const pages = `
    <li><button type="button" data-page="recents"${state.mode === 'recent' ? ' class="active" aria-current="true"' : ''}>recents</button></li>
    <li><button type="button" data-page="updates"${state.mode === 'updates' ? ' class="active" aria-current="true"' : ''}>updates</button></li>
    <li><button type="button" data-page="search"${state.mode === 'search' ? ' class="active" aria-current="true"' : ''}>search</button></li>`;
  $('#feeds').innerHTML = pages + state.feeds.map((feed) => `
    <li>
      <button type="button" data-slug="${escapeHtml(feed.slug)}"${state.mode === 'feed' && feed.slug === state.slug ? ' class="active" aria-current="true"' : ''}>
        ${escapeHtml(feed.title)}
      </button>
    </li>`).join('');
  $('#feeds').querySelector('[data-page="recents"]').onclick = () => selectRecent();
  $('#feeds').querySelector('[data-page="updates"]').onclick = () => selectUpdates();
  $('#feeds').querySelector('[data-page="search"]').onclick = () => openSearch();
  for (const button of $$('#feeds button[data-slug]')) button.onclick = () => selectFeed(button.dataset.slug);
}

function renderItems() {
  let day = '';
  let dayIndex = -1;
  const rows = [];
  state.items.forEach((item, index) => {
    const label = dayLabel(item.published_at);
    if (label !== day) {
      day = label;
      dayIndex += 1;
      rows.push(`<li class="day"><button type="button" data-day="${dayIndex}" aria-expanded="true">${escapeHtml(label)}</button></li>`);
    }
    const title = item.title || 'Untitled';
    const summary = rowSummary(item, title);
    const source = state.mode === 'recent' && item.feedLabel ? `<span class="row-source">${escapeHtml(item.feedLabel)}</span> ` : '';
    const unread = !read.has(item.id);
    const classes = `row${state.selected === index ? ' active' : ''}`;
    rows.push(`
      <li>
        <button type="button" class="${classes}" data-index="${index}" data-day="${dayIndex}"${unread ? ' data-unread="true"' : ''}>
          <span class="row-title">${source}${escapeHtml(title)}</span>
          <time datetime="${escapeHtml(item.published_at)}" title="${escapeHtml(longDate(item.published_at))}">${escapeHtml(clockTime(item.published_at))}</time>
          ${summary ? `<span class="row-summary">${escapeHtml(summary)}</span>` : ''}
        </button>
      </li>`);
  });
  $('#items').innerHTML = rows.join('');
  for (const button of $$('#items .row')) button.onclick = () => selectItem(Number(button.dataset.index));
  for (const toggle of $$('#items .day button')) toggle.onclick = () => toggleDay(toggle);
}

function toggleDay(toggle) {
  const open = toggle.getAttribute('aria-expanded') === 'true';
  toggle.setAttribute('aria-expanded', String(!open));
  for (const row of $$(`#items .row[data-day="${toggle.dataset.day}"]`)) row.parentElement.hidden = open;
}

function renderArticle(item) {
  const feed = state.feeds.find((candidate) => candidate.slug === item.feedSlug);
  const title = item.title || 'Untitled';
  const { article, discussion } = linksFor(item);
  const body = usefulBody(item, title);
  // A generated summary stands in for a missing body; it is labelled so it is
  // never mistaken for the publisher's own words.
  const generated = !body && item.ai_summary ? plainText(item.ai_summary) : '';
  const feedName = item.feedLabel || feed?.title || item.feedSlug || state.slug;
  const tags = Array.isArray(item.tags) ? item.tags : [];
  $('#article').innerHTML = `
    <header class="article-head">
      <p class="kicker">
        ${escapeHtml(feedName)}${item.author ? ` <span class="sep">/</span> ${escapeHtml(item.author)}` : ''}
      </p>
      <h1>${escapeHtml(title)}</h1>
      <p class="article-meta">
        <time datetime="${escapeHtml(item.published_at)}">${escapeHtml(longDate(item.published_at))}</time>
        ${article ? ` <span class="sep">/</span> <a href="${escapeHtml(article)}" target="_blank" rel="noopener noreferrer">original</a>` : ''}
        ${discussion ? ` <span class="sep">/</span> <a href="${escapeHtml(discussion)}" target="_blank" rel="noopener noreferrer">comments</a>` : ''}
      </p>
  ${tags.length ? `<p class="tags">${tags.map((tag) => `<button type="button" class="tag${state.tags.includes(tag) ? ' on' : ''}" data-tag="${escapeHtml(tag)}" title="filter by ${escapeHtml(tag)}">${escapeHtml(tag)}</button>`).join('')}</p>` : ''}
    </header>
    <div class="article-body">${body ? escapeHtml(body) : (generated ? escapeHtml(generated) : '<p class="note">No body text in the feed.</p>')}</div>
    ${generated ? '<p class="ai-note">generated summary</p>' : ''}`;
  for (const button of $$('#article .tag[data-tag]')) button.onclick = () => toggleTag(button.dataset.tag);
}

async function renderDigest(slug) {
  const feed = currentFeed();
  $('#article').innerHTML = '<p class="note">Loading digest…</p>';
  try {
    const response = await fetch(`/feeds/${encodeURIComponent(slug)}/newsletter?format=text`);
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
    const digest = await response.text();
    $('#article').innerHTML = `
      <header class="article-head">
        <p class="kicker">digest <span class="sep">/</span> latest ${state.items.length} items</p>
        <h1>${escapeHtml(feed?.title || slug)}</h1>
        <p class="article-meta"><a href="/feeds/${encodeURIComponent(slug)}/newsletter" target="_blank" rel="noopener noreferrer">html version</a></p>
      </header>
      <pre class="digest-text">${escapeHtml(digest)}</pre>`;
  } catch (error) {
    $('#article').innerHTML = `<p class="note">Could not load the digest: ${escapeHtml(error.message)}</p>`;
  }
  $('#pane-article').scrollTop = 0;
}

function renderFeedLinks() {
  if (state.mode !== 'feed') {
    $('#feed-links').innerHTML = `<span class="filter-label">${state.mode === 'search' ? 'search' : `last ${state.hours} hours`}</span>`;
    return;
  }
  $('#feed-links').innerHTML = `
    <a href="/feeds/${encodeURIComponent(state.slug)}.rss">rss</a>
    <a href="/feeds/${encodeURIComponent(state.slug)}.json">json</a>
    <button type="button" id="digest">digest</button>`;
  $('#digest').onclick = () => renderDigest(state.slug);
}

// The tag browser lists what the current view actually contains, so it is the
// feed's own tags in feed mode and the whole public archive otherwise.
async function renderTagBrowser() {
  const block = $('#tag-block');
  let rows = [];
  try {
    if (state.mode === 'feed' && state.slug) {
      rows = await api(`/api/v1/feeds/${encodeURIComponent(state.slug)}/tags?limit=40`);
    } else {
      rows = await api(`/api/v1/tags?hours=${state.hours * 7}&limit=40`);
    }
  } catch { rows = []; }
  block.hidden = rows.length === 0;
  if (!rows.length) { $('#tags').innerHTML = ''; return; }
  $('#tags').innerHTML = rows.map((row) => {
    const active = state.tags.includes(row.tag);
    return `<button type="button" class="tag${active ? ' on' : ''}" data-tag="${escapeHtml(row.tag)}" title="${active ? 'remove' : 'add'} the ${escapeHtml(row.tag)} filter">${escapeHtml(row.tag)} <span class="count">${row.count}</span></button>`;
  }).join('');
  for (const button of $$('#tags .tag')) button.onclick = () => toggleTag(button.dataset.tag);
}

function renderFilterbar() {
  const bar = $('#filterbar');
  const parts = [];
  if (state.mode === 'recent' || state.mode === 'updates') {
    parts.push(`<span class="filter-group">window${[24, 48, 168].map((hours) => `<button type="button" class="opt${state.hours === hours ? ' on' : ''}" data-hours="${hours}">${hours === 168 ? '7d' : `${hours}h`}</button>`).join('')}</span>`);
  }
  parts.push(`<span class="filter-group">order<button type="button" class="opt${state.oldest ? '' : ' on'}" data-order="newest">newest</button><button type="button" class="opt${state.oldest ? ' on' : ''}" data-order="oldest">oldest</button></span>`);
  parts.push(`<span class="filter-group"><button type="button" class="opt${state.unreadOnly ? ' on' : ''}" data-unread="1">unread only</button></span>`);
  if (state.tags.length > 1) {
    parts.push(`<span class="filter-group">match<button type="button" class="opt${state.matchAll ? '' : ' on'}" data-match="any">any</button><button type="button" class="opt${state.matchAll ? ' on' : ''}" data-match="all">all</button></span>`);
  }
  if (state.tags.length) {
    parts.push(`<span class="filter-group tags">${state.tags.map((tag) => `<button type="button" class="tag on" data-clear="${escapeHtml(tag)}">${escapeHtml(tag)} ✕</button>`).join('')}</span>`);
  }
  if (state.tags.length || state.unreadOnly || state.oldest) {
    parts.push('<button type="button" class="opt reset" data-reset="1">clear</button>');
  }
  bar.hidden = parts.length === 0;
  bar.innerHTML = parts.join('');
  for (const button of $$('#filterbar [data-hours]')) button.onclick = () => { state.hours = Number(button.dataset.hours); if (state.mode === 'updates') selectUpdates({ push: false }); else loadItems(state.slug, { quiet: true }); renderFilterbar(); renderFeedLinks(); renderTagBrowser(); };
  for (const button of $$('#filterbar [data-order]')) button.onclick = () => { state.oldest = button.dataset.order === 'oldest'; loadItems(state.slug, { quiet: true }); renderFilterbar(); };
  for (const button of $$('#filterbar [data-unread]')) button.onclick = () => { state.unreadOnly = !state.unreadOnly; loadItems(state.slug, { quiet: true }); };
  for (const button of $$('#filterbar [data-match]')) button.onclick = () => { state.matchAll = button.dataset.match === 'all'; loadItems(state.slug, { quiet: true }); renderFilterbar(); };
  for (const button of $$('#filterbar [data-clear]')) button.onclick = () => toggleTag(button.dataset.clear);
  const reset = $('#filterbar [data-reset]');
  if (reset) reset.onclick = () => { state.tags = []; state.unreadOnly = false; state.oldest = false; loadItems(state.slug, { quiet: true }); renderFilterbar(); renderTagBrowser(); };
}

/// Tags accumulate: each click narrows further, which is what makes the tag
/// browser usable for "show me the ray tracing papers".
function toggleTag(tag) {
  const index = state.tags.indexOf(tag);
  if (index >= 0) state.tags.splice(index, 1); else state.tags.push(tag);
  state.selected = null;
  loadItems(state.slug, { quiet: true });
  renderFilterbar();
  renderTagBrowser();
}

// Pinned categories lead the updates page. Local to this browser, like read state.
const pins = (() => {
  const KEY = 'sidefeed-pins';
  let ids = [];
  try { ids = JSON.parse(localStorage.getItem(KEY) || '[]'); } catch { ids = []; }
  return {
    has: (slug) => ids.includes(slug),
    toggle(slug) {
      ids = ids.includes(slug) ? ids.filter((id) => id !== slug) : [...ids, slug];
      try { localStorage.setItem(KEY, JSON.stringify(ids)); } catch { /* storage is optional */ }
    },
    order: (list) => [...list].sort((left, right) => Number(ids.includes(right.feed)) - Number(ids.includes(left.feed))),
  };
})();

// The list and the digest share one pane; only one of them is ever shown.
function showDigest(flag) {
  $('#digest').hidden = !flag;
  $('#items').hidden = flag;
}

/// Updates is its own page: per-category summaries of what moved, with the few
/// items worth opening under each one.
async function selectUpdates({ push = true } = {}) {
  state.mode = 'updates';
  state.tags = [];
  state.selected = null;
  state.items = [];
  renderItems();
  renderFeeds();
  if (push) pushRoute('/updates');
  document.body.dataset.view = 'list';
  $('#feed-title').textContent = 'updates';
  $('#feed-sub').textContent = `summaries of what moved in the last ${state.hours} hours`;
  $('#feed-sub').hidden = false;
  $('#items-note').hidden = true;
  renderFeedLinks();
  renderFilterbar();
  if (state.stream) { state.stream.close(); state.stream = null; }
  showDigest(true);
  $('#digest').innerHTML = '<p class="note">Building the digest…</p>';
  try {
    renderUpdates(await api(`/api/v1/updates?hours=${state.hours}`));
  } catch (error) {
    $('#digest').innerHTML = `<p class="note">Could not build the digest: ${escapeHtml(error.message)}</p>`;
  }
  renderTagBrowser();
}

function renderUpdates(data) {
  const categories = pins.order(data.categories || []);
  if (!categories.length) {
    $('#digest').innerHTML = `<p class="note">Nothing published in the last ${data.window_hours} hours.</p>`;
    return;
  }
  const lead = data.summary ? `<p class="digest-lead">${escapeHtml(data.summary)}</p>` : '';
  const blocks = categories.map((category) => `
    <section class="digest-block">
      <header>
        <button type="button" class="digest-cat" data-feed="${escapeHtml(category.feed)}">${escapeHtml(category.title)}</button>
        <span class="count">${category.count}</span>
        <button type="button" class="opt${pins.has(category.feed) ? ' on' : ''}" data-pin="${escapeHtml(category.feed)}" title="${pins.has(category.feed) ? 'unpin' : 'pin to the top'}">pin</button>
      </header>
      ${category.summary ? `<p class="digest-summary">${escapeHtml(category.summary)}</p>` : ''}
      <ul class="digest-items">${(category.items || []).map((item) => {
        const url = safeUrl(item.url);
        const title = escapeHtml(item.title || 'Untitled');
        const when = escapeHtml(dayLabel(item.published_at));
        const tags = (item.tags || []).slice(0, 3).map((tag) => `<span class="tag static">${escapeHtml(tag)}</span>`).join('');
        return `<li>${url
          ? `<a href="${escapeHtml(url)}" target="_blank" rel="noopener noreferrer">${title}</a>`
          : `<span>${title}</span>`}<span class="meta">${when}${tags ? ` ${tags}` : ''}</span></li>`;
      }).join('')}</ul>
    </section>`).join('');
  $('#digest').innerHTML = lead + blocks;
  for (const button of $$('#digest [data-feed]')) button.onclick = () => selectFeed(button.dataset.feed);
  for (const button of $$('#digest [data-pin]')) button.onclick = () => { pins.toggle(button.dataset.pin); renderUpdates(data); };
}

// ---------------------------------------------------------------- actions
async function loadFeeds() {
  const note = $('#feeds-note');
  try {
    state.feeds = await api('/api/v1/public/feeds');
    renderFeeds();
    note.hidden = state.feeds.length > 0;
    if (!state.slug && state.feeds.length && currentPath() === '/') await selectFeed(state.feeds[0].slug);
  } catch (error) {
    note.hidden = false;
    note.textContent = `Could not load feeds: ${error.message}`;
  }
}

async function loadItems(slug, { quiet = false } = {}) {
  const note = $('#items-note');
  const previous = state.selected === null ? null : state.items[state.selected]?.id;
  const mode = state.mode;
  try {
    const filters = [
      state.tags.length ? `tag=${encodeURIComponent(state.tags.join(','))}` : '',
      state.matchAll ? 'matching=all' : '',
      state.oldest ? 'order=oldest' : '',
    ].filter(Boolean).join('&');
    let items;
    if (mode === 'search' && state.query) {
      // Search deliberately spans the whole archive: a window is for browsing,
      // not for finding the paper you half remember from 2013.
      const rows = await api(`/api/v1/search?q=${encodeURIComponent(state.query)}&limit=60${filters ? `&${filters}` : ''}`);
      items = rows.map((row) => ({ ...row, feedSlug: row.feed, feedLabel: row.feed_title }));
    } else if (mode === 'recent') {
      const rows = await api(`/api/v1/recent?hours=${state.hours}&limit=60${filters ? `&${filters}` : ''}`);
      items = rows.map((row) => ({ ...row, feedSlug: row.feed, feedLabel: row.feed_title }));
    } else {
      const page = await api(`/api/v1/feeds/${encodeURIComponent(slug)}/items?limit=60${filters ? `&${filters}` : ''}`);
      const feed = currentFeed();
      // Stamp the feed on every item so the reading pane never has to guess which
      // feed an article came from after the selection moves on.
      items = (page.items || []).map((item) => ({ ...item, feedSlug: slug, feedLabel: feed?.title || slug }));
    }
    // Unread is a per-browser notion, so it filters here rather than server side.
    if (state.unreadOnly) items = items.filter((item) => !read.has(item.id));
    state.items = items;
    state.selected = previous ? state.items.findIndex((item) => item.id === previous) : null;
    if (state.selected === -1) state.selected = null;
    renderItems();
    note.hidden = state.items.length > 0;
    if (!state.items.length) note.textContent = mode === 'search'
      ? `Nothing matches “${state.query}”.`
      : (state.tags.length ? `No items tagged ${state.tags.join(' + ')}.` : (mode === 'recent' ? 'Nothing published in that window.' : 'No items yet. sidefeed fetches sources on a timer.'));
  } catch (error) {
    if (quiet) return;
    state.items = [];
    renderItems();
    note.hidden = false;
    note.textContent = `Could not load items: ${error.message}`;
  }
}

/// Everything public from the last day or two, ranked across feeds. This is the
/// "what is worth a look" view rather than one feed at a time.
async function selectRecent({ push = true } = {}) {
  state.mode = 'recent';
  state.tags = [];
  state.selected = null;
  state.items = [];
  renderItems();
  renderFeeds();
  if (push) pushRoute('/recents');
  showDigest(false);
  document.body.dataset.view = 'list';
  $('#feed-title').textContent = 'recent';
  $('#feed-sub').textContent = 'across every feed, newest and most varied first';
  $('#feed-sub').hidden = false;
  $('#items-note').hidden = true;
  renderFeedLinks();
  renderFilterbar();
  if (state.stream) { state.stream.close(); state.stream = null; }
  await loadItems('');
  renderTagBrowser();
}

/// Search runs across every public feed, unless a feed is selected, in which
/// case it stays inside that feed.
async function runSearch({ push = true } = {}) {
  const query = $('#search').value.trim();
  if (!query) { clearSearch(); return; }
  state.query = query;
  state.mode = 'search';
  state.tags = [];
  state.selected = null;
  state.items = [];
  renderItems();
  renderFeeds();
  showDigest(false);
  if (push) pushRoute(`/search?q=${encodeURIComponent(query)}`);
  $('#feed-title').textContent = 'search';
  $('#feed-sub').textContent = 'across every feed';
  $('#feed-sub').hidden = false;
  $('#items-note').hidden = true;
  renderFeedLinks();
  renderFilterbar();
  if (state.stream) { state.stream.close(); state.stream = null; }
  await loadItems(state.slug);
  renderTagBrowser();
}

/// The search page before anything is typed: same shell, the box takes focus.
async function openSearch({ push = true } = {}) {
  state.mode = 'search';
  state.query = '';
  state.tags = [];
  state.selected = null;
  state.items = [];
  renderItems();
  renderFeeds();
  if (push) pushRoute('/search');
  showDigest(false);
  $('#feed-title').textContent = 'search';
  $('#feed-sub').textContent = 'type a word, or a few, and press enter';
  $('#feed-sub').hidden = false;
  $('#items-note').hidden = false;
  $('#items-note').textContent = 'Search runs across every public feed, the whole archive included.';
  renderFeedLinks();
  renderFilterbar();
  if (state.stream) { state.stream.close(); state.stream = null; }
  $('#search').focus();
  renderTagBrowser();
}

function clearSearch() {
  $('#search').value = '';
  state.query = '';
  if (state.mode === 'search') selectRecent();
}

/// Views are addressable, so a link to /updates or /search?q=… opens the same
/// thing a click would.
async function applyRoute() {
  const path = currentPath();
  if (path === '/recents') { await selectRecent({ push: false }); return; }
  if (path === '/updates') { await selectUpdates({ push: false }); return; }
  if (path === '/search') {
    const query = new URLSearchParams(location.search).get('q') || '';
    if (query) { $('#search').value = query; await runSearch({ push: false }); } else { await openSearch({ push: false }); }
  }
}

window.addEventListener('popstate', () => { applyRoute(); });

async function selectFeed(slug) {
  state.mode = 'feed';
  state.tags = [];
  state.query = '';
  $('#search').value = '';
  state.slug = slug;
  pushRoute('/');
  showDigest(false);
  state.selected = null;
  // Empty the list before fetching: rows from the previous feed must not stay
  // clickable while the new feed loads.
  state.items = [];
  renderItems();
  renderFeeds();
  document.body.dataset.view = 'list';
  const feed = currentFeed();
  $('#feed-title').textContent = feed ? feed.title : slug;
  $('#feed-sub').textContent = feed?.description || '';
  $('#feed-sub').hidden = !feed?.description;
  $('#items-note').hidden = true;
  renderFeedLinks();
  renderFilterbar();
  await loadItems(slug);
  renderTagBrowser();
  subscribe(slug);
}

function selectItem(index) {
  const item = state.items[index];
  if (!item) return;
  state.selected = index;
  read.add(item.id);
  renderItems();
  renderArticle(item);
  togglePane('article', true);
  document.body.dataset.view = 'reader';
  $('#pane-article').scrollTop = 0;
}

function move(step) {
  if (!state.items.length) return;
  const next = state.selected === null ? 0 : Math.min(state.items.length - 1, Math.max(0, state.selected + step));
  selectItem(next);
}

function subscribe(slug) {
  if (state.stream) { state.stream.close(); state.stream = null; }
  if (!('EventSource' in window)) return;
  const stream = new EventSource(`/api/v1/feeds/${encodeURIComponent(slug)}/stream`);
  stream.addEventListener('item', () => { if (state.mode === 'feed' && state.slug === slug) loadItems(slug, { quiet: true }); });
  stream.onerror = () => { /* EventSource reconnects on its own */ };
  state.stream = stream;
}

// ---------------------------------------------------------------- wiring
for (const [name, pane] of Object.entries(panes)) {
  const chip = $(pane.chip);
  chip.title = `fold ${name} pane`;
  chip.onclick = () => togglePane(name);
}
for (const rail of $$('.rail')) rail.onclick = () => togglePane(rail.dataset.rail, true);
let searchTimer = null;
$('#search').addEventListener('input', () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(runSearch, 350);
});
$('#search').addEventListener('keydown', (event) => {
  if (event.key === 'Enter') { clearTimeout(searchTimer); runSearch(); }
  if (event.key === 'Escape') { clearSearch(); }
});
$('#back').onclick = () => { document.body.dataset.view = 'list'; };

document.addEventListener('keydown', (event) => {
  if (event.target.matches('input, textarea, select')) return;
  if (event.key === '/') { $('#search').focus(); event.preventDefault(); }
  else if (event.key === 'j' || event.key === 'ArrowDown') { move(1); event.preventDefault(); }
  else if (event.key === 'k' || event.key === 'ArrowUp') { move(-1); event.preventDefault(); }
  else if (event.key === 'r' && state.slug) { loadItems(state.slug); }
  else if (event.key === '1') { togglePane('feeds'); event.preventDefault(); }
  else if (event.key === '2') { togglePane('items'); event.preventDefault(); }
  else if (event.key === '3') { togglePane('article'); event.preventDefault(); }
  else if (event.key === 'Escape' && isNarrow()) { document.body.dataset.view = 'list'; }
});

loadFeeds().then(applyRoute);
setInterval(() => { if (document.visibilityState === 'visible' && !state.query && (state.slug || state.mode === 'recent')) loadItems(state.slug, { quiet: true }); }, 300000);
