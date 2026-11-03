// Every address the dashboard turns into a link from task data or a project's summary goes through safeUrl (T-225,
// docs/security-review.md F9): only http(s), so a `javascript:` or `data:` URL pushed with a task never becomes an
// href. Markdown links have their own, slightly wider rule (compmd.ts safeHref: also mailto: and #anchors).

/** `value` as a link target if it is an absolute http(s) address, else null */
export function safeUrl(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const text = value.trim();
  if (!/^https?:\/\//i.test(text)) return null;
  try {
    const { protocol } = new URL(text);
    return protocol === 'http:' || protocol === 'https:' ? text : null;
  } catch {
    return null;
  }
}
