// @vitest-environment node
// Static checks for invariants of docs/security-architecture.md that no behaviour test can show (T-234), and a check
// that every test the document names exists, so the table can't drift from the suites.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const REPO = fileURLToPath(new URL('../../../', import.meta.url));
const filesIn = (dir: string, ext: string): string[] =>
  readdirSync(join(REPO, dir), { withFileTypes: true }).flatMap((d) =>
    d.isDirectory() ? filesIn(join(dir, d.name), ext) : d.name.endsWith(ext) ? [join(dir, d.name)] : [],
  );
const read = (file: string) => readFileSync(join(REPO, file), 'utf8');

describe('security invariants (docs/security-architecture.md)', () => {
  it('I5: atm never runs a shell', () => {
    const sources = filesIn('crates', '.rs');
    expect(sources.length).toBeGreaterThan(10);
    // A shell by name anywhere a process is started or given its arguments
    const offending = sources.flatMap((file) =>
      read(file)
        .split('\n')
        .map((line, i) => ({ line, at: `${file}:${i + 1}` }))
        .filter(({ line }) => /Command::new\(\s*"(sh|bash|zsh|cmd|cmd\.exe|powershell|pwsh)"/.test(line))
        .map(({ at, line }) => `${at}: ${line.trim()}`),
    );
    expect(offending).toEqual([]);
  });

  it('I24: it starts only git, itself (the claim’s watcher, the dashboard server) and the browser opener (T-446, T-449)', () => {
    // Every place that starts a process, and the program it starts; the shell's start time comes from sysinfo, not ps
    const starts = filesIn('crates', '.rs').flatMap((file) =>
      [...read(file).matchAll(/Command::new\(\s*([^\s,)]+)/g)].map(
        (call) => `${file.replace(/^crates\//, '')}: ${call[1]}`,
      ),
    );
    expect(starts.sort()).toEqual(
      [
        'atm-cli/src/commands/claim.rs: own_binary',
        'atm-cli/src/commands/maintenance.rs: own_binary',
        'atm-cli/src/commands/maintenance.rs: opener',
        'atm-db/src/git.rs: "git"',
        'atm-vcs/src/lib.rs: "git"',
      ].sort(),
    );
    // The watcher and the server are started with atm's own path, never one from the command line or a task; the
    // browser opener is `open` or `xdg-open`, with the URL atm made
    for (const file of ['claim.rs', 'maintenance.rs']) {
      expect(read(`crates/atm-cli/src/commands/${file}`)).toContain('let own_binary = std::env::current_exe()');
    }
    expect(read('crates/atm-cli/src/commands/maintenance.rs')).toMatch(
      /let opener = if cfg!\(target_os = "macos"\) \{\s*"open"\s*\} else \{\s*"xdg-open"\s*\};/,
    );
  });

  it('I15: {@html} only in the bundled logo, never on data (the icons are components since T-432)', () => {
    const using = filesIn('dashboard/src', '.svelte').filter((file) => read(file).includes('{@html'));
    expect(using.sort()).toEqual(['dashboard/src/lib/ui/Logo.svelte']);
  });

  it('every test file the invariants table names exists', () => {
    const doc = read('docs/security-architecture.md');
    const rows = doc.split('\n').filter((line) => /^\| I\d+ \|/.test(line));
    expect(rows.length).toBeGreaterThanOrEqual(21);
    // Test files by name, and scenario files by path (a bare `tasks.json` in a row is data, not a test)
    const named = rows.flatMap((row) =>
      [...row.matchAll(/`([\w./-]+\.(?:test|spec)\.(?:ts|mjs)|[\w.-]+\/[\w./-]+\.json)`/g)].map((m) => m[1]),
    );
    expect(named.length).toBeGreaterThan(5);
    const known = [...filesIn('dashboard/tests', ''), ...filesIn('tests/scripts', '')];
    const missing = named.filter(
      (name) => !existsSync(join(REPO, name)) && !known.some((f) => f.endsWith(`/${name}`) || f.endsWith(name)),
    );
    expect(missing).toEqual([]);
  });
});
