// Feed fields are third-party text: some are HTML, others carry character
// references. Everything reaches the screen as a text node, so this is the one
// place that decides what a feed string looks like once it is displayed.

/// Character references to their characters, once. The browser's own parser
/// handles named and numeric references; assigning to a textarea never runs
/// what it decodes, so markup in a title stays inert.
export function decodeEntities(value?: string | null): string {
  const text = String(value ?? '');
  if (!text.includes('&')) return text;
  const area = document.createElement('textarea');
  area.innerHTML = text;
  return area.value;
}

/// Block-level tags become line breaks, every other tag disappears, and what is
/// left is decoded, so a body or summary reads as prose.
export function plainText(value?: string | null): string {
  const spaced = String(value ?? '').replace(/<\s*\/?\s*(?:br|p|div|li|ul|ol|h[1-6]|tr|blockquote)\b[^>]*>/gi, '\n');
  const stripped = spaced.replace(/<[^>]*>/g, '');
  return decodeEntities(stripped)
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/// Bodies that are only a link label say nothing; aggregators ship them as
/// "Comments" or "Read more" and they would otherwise fill the list row.
const LINK_ONLY = /^(?:comments?|read more|continue reading|permalink|link|\d+ comments?)$/i;

/// A single line for a list row: plain, collapsed, and empty when the feed only
/// offered link furniture.
export function summaryLine(value?: string | null): string {
  const text = plainText(value).replace(/\s+/g, ' ');
  return LINK_ONLY.test(text.trim().replace(/[.:]$/, '')) ? '' : text;
}
