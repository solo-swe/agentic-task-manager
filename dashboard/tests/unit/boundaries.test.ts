// @vitest-environment node
// The components' boundaries (docs/code-review-design.md §3.1, ATM-571): the document viewer and the review tool
// never import the task manager's state, task model or areas. ESLint's rule says so; this checks the rule refuses
// what it should, on made-up files where those components live.
import { ESLint } from 'eslint';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const eslint = new ESLint({ cwd: fileURLToPath(new URL('../..', import.meta.url)) });

/** The boundary's messages for a file at `filePath` importing `from` */
async function refused(filePath: string, from: string) {
  const [result] = await eslint.lintText(`import * as it from '${from}';\nexport default it;\n`, { filePath });
  return result.messages.filter((m) => m.ruleId === 'no-restricted-imports').map((m) => m.message);
}

describe('the component boundaries (ESLint no-restricted-imports)', () => {
  it('the document viewer imports no dashboard state, task model, task area or task manager source', async () => {
    for (const from of [
      '../state.svelte',
      '../types',
      '../details/DetailPanel.svelte',
      '../kanban/Board.svelte',
      '../project-documents',
    ])
      expect(await refused('src/lib/documents/Probe.ts', from), from).toEqual([
        expect.stringContaining('The document viewer is a component of its own'),
      ]);
    for (const from of ['./source', '../compmd', '../documents', '../ui/Icon.svelte', '../scroll'])
      expect(await refused('src/lib/documents/Probe.ts', from), from).toEqual([]);
  });

  it('the review tool reaches the task areas only through lib/reviews/links', async () => {
    expect(await refused('src/lib/reviews/Probe.ts', '../state.svelte')).toHaveLength(1);
    expect(await refused('src/lib/reviews/Probe.ts', '../types')).toHaveLength(1);
    // Its own types, and the document viewer, are its own to use
    expect(await refused('src/lib/reviews/Probe.ts', './types')).toEqual([]);
    expect(await refused('src/lib/reviews/Probe.ts', '../documents/CompMd.svelte')).toEqual([]);
    expect(await refused('src/lib/reviews/links/Probe.ts', '../../state.svelte')).toEqual([]);
    // The task manager itself may use them
    expect(await refused('src/lib/details/Probe.ts', '../state.svelte')).toEqual([]);
  });
});
