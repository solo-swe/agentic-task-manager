import { render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import LoadError from '../../src/lib/shell/LoadError.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import { loadIntoDashboard } from './components.helpers';

// Shown instead of the views when nothing could be loaded at all; a later failure keeps the data shown
describe('LoadError', () => {
  beforeEach(() => loadIntoDashboard({ tasks: [] }));

  it.each([
    ['loading, no error yet', false, null, false],
    ['loaded, no error', true, null, false],
    ['loaded, then a reload failed', true, 'HTTP 500', false],
    ['never loaded, and failed', false, 'HTTP 502', true],
    ['never loaded, failed with an empty message', false, '', true],
  ])('%s', (_, loaded, loadError, shown) => {
    Object.assign(dashboard, { loaded, loadError });
    const { container } = render(LoadError);
    const alert = container.querySelector('#load-error');
    expect(alert !== null).toBe(shown);
    if (shown) {
      expect(alert).toHaveAttribute('role', 'alert');
      expect(alert).toHaveTextContent('Failed to load tasks.json');
      expect(alert).toHaveTextContent(`${loadError}The dashboard is served by atm.`);
    }
  });
});
