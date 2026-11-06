// Deterministically generates the large fixtures:
// - large-90.json: a wide graph (90 tasks, 40 done, 1 in progress), as used by the graph and visual specs.
// - large-500.json: 500 tasks for the performance checks (300 done with start/complete times, 1 in progress).
// Run: node tests/fixtures/generate-large.mjs && pnpm exec prettier --write tests/fixtures
import { writeFileSync } from 'node:fs';

const TYPES = ['feature', 'test', 'infra', 'docs', 'refactor', 'bugfix', 'polish'];
const id = (n) => 'T-' + String(n).padStart(3, '0');
const pad = (n) => String(n).padStart(2, '0');

function generate(count, doneCount, { timed = false } = {}) {
  const tasks = [];
  for (let i = 1; i <= count; i++) {
    const deps = new Set();
    if (i > 1) deps.add(id(Math.max(1, i - 1 - (i % 4))));
    if (i > 6 && i % 3 === 0) deps.add(id(i - 5));
    const status = i <= doneCount ? 'done' : i === doneCount + 1 ? 'in_progress' : 'todo';
    const day = pad(1 + (Math.floor(i / 8) % 28));
    const updated = status === 'todo' ? '2026-04-01T00:00:00Z' : `2026-04-${day}T${pad(i % 24)}:00:00Z`;
    tasks.push({
      id: id(i),
      name: `Task number ${i} with a reasonably long name`,
      phase: Math.floor(i / 15),
      type: TYPES[i % TYPES.length],
      description: `Generated task ${i}.`,
      requirements: [],
      use_cases: [],
      status,
      dependencies: [...deps],
      files: [],
      created_at: '2026-04-01T00:00:00Z',
      updated_at: updated,
      ...(timed && status !== 'todo'
        ? {
            started_at: `2026-04-${day}T${pad(i % 24)}:00:00Z`,
            ...(status === 'done' ? { completed_at: `2026-04-${day}T${pad(i % 24)}:${pad(5 + (i % 10))}:00Z` } : {}),
          }
        : {}),
    });
  }
  return tasks;
}

const write = (file, project, tasks) =>
  writeFileSync(new URL(file, import.meta.url), JSON.stringify({ meta: { project }, tasks }, null, 2) + '\n');

write('./large-90.json', 'Large Project', generate(90, 40));
write('./large-500.json', 'Performance Project', generate(500, 300, { timed: true }));
