// `pnpm run watch:page`, which `cargo tauri dev` starts (beforeDevCommand in app/src-tauri/tauri.conf.json): the page
// built again (`pnpm run build:page`) whenever a source of it changes, so the window shows the change at once: the Tauri
// CLI serves `dist/` with its own live-reload server and reloads the window when the page is written. One build at a
// time; changes during a build bring one more after it. CompMD's runtime (`dist/assets/`) isn't rebuilt: the app
// embeds it when it is compiled (`pnpm run build` once first, as for `cargo build`).
import { spawn } from 'node:child_process';
import { watch } from 'node:fs';
import { fileURLToPath } from 'node:url';

const DASHBOARD = fileURLToPath(new URL('..', import.meta.url));
/** What the page is built from: its sources, and the schemas and help it is generated from */
const SOURCES = ['src', '../crates/atm-common/schema', '../docs/help.md'].map((p) =>
  fileURLToPath(new URL(`../${p}`, import.meta.url)),
);
/** How long to wait for a burst of saves (an editor writes several files) to settle */
const SETTLE_MS = 300;

let building = false;
let again = false;
let timer: ReturnType<typeof setTimeout> | undefined;

function build() {
  if (building) {
    again = true;
    return;
  }
  building = true;
  const started = Date.now();
  const run = spawn('pnpm', ['run', '--silent', 'build:page'], {
    cwd: DASHBOARD,
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  run.on('exit', (code) => {
    building = false;
    console.log(
      code === 0 ? `watch:page: built in ${Date.now() - started} ms` : `watch:page: the build failed (${code})`,
    );
    if (again) {
      again = false;
      build();
    }
  });
}

function changed() {
  clearTimeout(timer);
  timer = setTimeout(build, SETTLE_MS);
}

for (const source of SOURCES) watch(source, { recursive: true }, changed);
console.log('watch:page: watching the dashboard; a change builds the page again');
build();
