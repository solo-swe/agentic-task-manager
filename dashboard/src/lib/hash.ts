// The URL hash holds the dashboard's linkable state as parameters: "#view=kanban&about". A parameter without a value
// is a flag, written without "=".

export function hashParams(hash: string): URLSearchParams {
  return new URLSearchParams(hash.replace(/^#/, ''));
}

/** `params` as a hash, flags without "=": `#view=kanban&about`. Commas stay readable (`#sprint=a,b`). */
export function formatHash(params: URLSearchParams): string {
  const parts = [...params].map(([key, value]) =>
    value === ''
      ? encodeURIComponent(key)
      : `${encodeURIComponent(key)}=${encodeURIComponent(value).replace(/%2C/g, ',')}`,
  );
  return '#' + parts.join('&');
}
