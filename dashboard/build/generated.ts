import { fileURLToPath } from 'node:url';

/** Where atm-cli/build.py writes the dashboard's generated TypeScript (the data model's types and the vocabularies):
 * `.generated/dashboard/`, never committed. The sources import it as `$generated/…` (vite.config.ts, vitest.config.ts);
 * `pnpm run generate` writes it. */
export const GENERATED = fileURLToPath(new URL('../../.generated/dashboard', import.meta.url));
