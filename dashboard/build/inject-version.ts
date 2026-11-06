import { readFileSync } from 'node:fs';

// The plugin version in <meta name="atm-dashboard-version"> (build/finish-page.ts stamps it); /atm:serve compares it.

const META_RE = /(<meta name="atm-dashboard-version" content=")[^"]*(")/;

export function readPluginVersion(pluginJsonPath: string): string {
  const { version } = JSON.parse(readFileSync(pluginJsonPath, 'utf8'));
  if (typeof version !== 'string' || !version) throw new Error('No version in ' + pluginJsonPath);
  return version;
}

export function setDashboardVersion(html: string, version: string): string {
  if (!META_RE.test(html)) throw new Error('atm-dashboard-version meta tag not found in index.html');
  return html.replace(META_RE, '$1' + version + '$2');
}
