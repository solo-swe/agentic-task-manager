// The route parameters with a fixed set of values (T-429): a project's tab (/projects/<id>/[tab=view]) and the All
// projects page's tabs but the project cards, which are /projects ([tab=overview]: /board, /progress, /agents).
// SvelteKit takes each as a Standard Schema of the parameter's text. SvelteKit's build loads this module on its own,
// so it imports nothing: the lists are lib/tabs.ts VIEWS and lib/address.ts OVERVIEW_TABS, which
// tests/unit/address.test.ts checks them against.
import type { ParamMatcher } from '@sveltejs/kit/params';

export const PROJECT_TABS = ['kanban', 'list', 'graph', 'progress', 'agents', 'project'];
export const OVERVIEW_PATHS = ['board', 'progress', 'agents'];

function oneOf(name: string, values: readonly string[]): ParamMatcher<string> {
  return {
    '~standard': {
      version: 1,
      vendor: 'atm',
      validate: (value) =>
        typeof value === 'string' && values.includes(value) ? { value } : { issues: [{ message: `not a ${name}` }] },
    },
  };
}

export const params = {
  view: oneOf('project tab', PROJECT_TABS),
  overview: oneOf('overview tab', OVERVIEW_PATHS),
};
