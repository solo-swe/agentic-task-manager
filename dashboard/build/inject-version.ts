// atm's version in <meta name="atm-dashboard-version"> (build/finish-page.ts stamps it, from Cargo.toml: see
// build/workspace.ts); `atm service` compares it.

const META_RE = /(<meta name="atm-dashboard-version" content=")[^"]*(")/;

export function setDashboardVersion(html: string, version: string): string {
  if (!META_RE.test(html)) throw new Error('atm-dashboard-version meta tag not found in index.html');
  return html.replace(META_RE, '$1' + version + '$2');
}
