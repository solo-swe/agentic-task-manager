import type { Plugin } from 'vite';

/**
 * `pnpm run size`: what the shipped single-file dashboard is made of (T-342), largest first: each module's share of the
 * minified JavaScript as Rollup renders it, grouped by package for node_modules, and the CSS and the page's other
 * assets. Only when ATM_SIZE_REPORT is set, so ordinary builds stay quiet.
 */
export function sizeReport(): Plugin {
  return {
    name: 'atm-size-report',
    apply: 'build',
    generateBundle(_options, bundle) {
      if (!process.env.ATM_SIZE_REPORT) return;
      const parts = new Map<string, number>();
      const add = (name: string, bytes: number) => parts.set(name, (parts.get(name) ?? 0) + bytes);
      for (const file of Object.values(bundle)) {
        if (file.type === 'chunk') {
          for (const [id, module] of Object.entries(file.modules)) add(groupOf(id), module.renderedLength);
        } else {
          const source = file.source;
          add(`asset: ${file.fileName}`, typeof source === 'string' ? source.length : source.byteLength);
        }
      }
      const total = [...parts.values()].reduce((a, b) => a + b, 0);
      const rows = [...parts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 40);
      const kb = (n: number) => (n / 1024).toFixed(1).padStart(7);
      console.log(`\nWhat the dashboard is made of (before inlining; ${kb(total).trim()} KiB in all):`);
      for (const [name, bytes] of rows)
        console.log(`${kb(bytes)} KiB  ${((bytes / total) * 100).toFixed(1).padStart(5)}%  ${name}`);
    },
  };
}

/** A package for node_modules (`node_modules/@dagrejs/dagre`), a virtual module by its name, else the source path */
function groupOf(id: string): string {
  const clean = id.replace(/^\0/, '').replace(/\?.*$/, '');
  const pkg = /node_modules\/((?:@[^/]+\/)?[^/]+)/.exec(clean);
  if (pkg) return `package: ${pkg[1]}`;
  if (clean.startsWith('virtual:')) return clean;
  const src = /\/dashboard\/(src\/.*)$/.exec(clean);
  return src ? src[1] : clean;
}
