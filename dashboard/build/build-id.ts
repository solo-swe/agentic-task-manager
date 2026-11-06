import { createHash } from 'node:crypto';

const META_RE = /(<meta name="atm-dashboard-build" content=")[^"]*(")/;

/**
 * The build ID: the first 12 hex digits of the SHA-256 of the built page with an empty build tag, so the same sources
 * always give the same ID, and any change to the page gives a new one, even under the same version.
 */
export function buildId(html: string): string {
  if (!META_RE.test(html)) throw new Error('atm-dashboard-build meta tag not found');
  return createHash('sha256').update(html.replace(META_RE, '$1$2')).digest('hex').slice(0, 12);
}

/**
 * Stamps <meta name="atm-dashboard-build"> into the finished page (after everything is inlined; build/finish-page.ts),
 * so a running server restarts on a new build even when the version number didn't change (docs/v4/dashboard.md D-5.5).
 */
export function setBuildId(html: string): string {
  return html.replace(META_RE, `$1${buildId(html)}$2`);
}
