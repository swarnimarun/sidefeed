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
const state = { feeds: [], slug: '', items: [], selected: null, stream: null };

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
  $('#feeds').innerHTML = state.feeds.map((feed) => `
    <li>
      <button type="button" data-slug="${escapeHtml(feed.slug)}"${feed.slug === state.slug ? ' class="active" aria-current="true"' : ''}>
        ${escapeHtml(feed.title)}
      </button>
    </li>`).join('');
  for (const button of $$('#feeds button')) button.onclick = () => selectFeed(button.dataset.slug);
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
    const summary = usefulSummary(item, title);
    const unread = !read.has(item.id);
    const classes = `row${state.selected === index ? ' active' : ''}`;
    rows.push(`
      <li>
        <button type="button" class="${classes}" data-index="${index}" data-day="${dayIndex}"${unread ? ' data-unread="true"' : ''}>
          <span class="row-title">${escapeHtml(title)}</span>
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
  const feedName = feed?.title || item.feedSlug || state.slug;
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
    </header>
    <div class="article-body">${body ? escapeHtml(body) : '<p class="note">No body text in the feed.</p>'}</div>`;
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
  $('#feed-links').innerHTML = `
    <a href="/feeds/${encodeURIComponent(state.slug)}.rss">rss</a>
    <a href="/feeds/${encodeURIComponent(state.slug)}.json">json</a>
    <button type="button" id="digest">digest</button>`;
  $('#digest').onclick = () => renderDigest(state.slug);
}

// ---------------------------------------------------------------- actions
async function loadFeeds() {
  const note = $('#feeds-note');
  try {
    state.feeds = await api('/api/v1/public/feeds');
    renderFeeds();
    note.hidden = state.feeds.length > 0;
    if (!state.slug && state.feeds.length) await selectFeed(state.feeds[0].slug);
  } catch (error) {
    note.hidden = false;
    note.textContent = `Could not load feeds: ${error.message}`;
  }
}

async function loadItems(slug, { quiet = false } = {}) {
  const note = $('#items-note');
  const previous = state.selected === null ? null : state.items[state.selected]?.id;
  try {
    const page = await api(`/api/v1/feeds/${encodeURIComponent(slug)}/items?limit=60`);
    // Stamp the feed on every item so the reading pane never has to guess which
    // feed an article came from after the selection moves on.
    state.items = (page.items || []).map((item) => ({ ...item, feedSlug: slug }));
    state.selected = previous ? state.items.findIndex((item) => item.id === previous) : null;
    if (state.selected === -1) state.selected = null;
    renderItems();
    note.hidden = state.items.length > 0;
    if (!state.items.length) note.textContent = 'No items yet. sidefeed fetches sources on a timer.';
  } catch (error) {
    if (quiet) return;
    state.items = [];
    renderItems();
    note.hidden = false;
    note.textContent = `Could not load items: ${error.message}`;
  }
}

async function selectFeed(slug) {
  state.slug = slug;
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
  await loadItems(slug);
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
  stream.addEventListener('item', () => { if (state.slug === slug) loadItems(slug, { quiet: true }); });
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
$('#back').onclick = () => { document.body.dataset.view = 'list'; };

document.addEventListener('keydown', (event) => {
  if (event.target.matches('input, textarea, select')) return;
  if (event.key === 'j' || event.key === 'ArrowDown') { move(1); event.preventDefault(); }
  else if (event.key === 'k' || event.key === 'ArrowUp') { move(-1); event.preventDefault(); }
  else if (event.key === 'r' && state.slug) { loadItems(state.slug); }
  else if (event.key === '1') { togglePane('feeds'); event.preventDefault(); }
  else if (event.key === '2') { togglePane('items'); event.preventDefault(); }
  else if (event.key === '3') { togglePane('article'); event.preventDefault(); }
  else if (event.key === 'Escape' && isNarrow()) { document.body.dataset.view = 'list'; }
});

loadFeeds();
setInterval(() => { if (state.slug && document.visibilityState === 'visible') loadItems(state.slug, { quiet: true }); }, 300000);
