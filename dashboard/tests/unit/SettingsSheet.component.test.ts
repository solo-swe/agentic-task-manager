import { fireEvent, render, screen } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SettingsSheet from '../../src/lib/dialogs/SettingsSheet.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// The sheet is the app's (T-517): the panel shows the app's own settings there
vi.mock('../../src/lib/api/http', async (original) => ({
  ...(await original<typeof import('../../src/lib/api/http')>()),
  inApp: true,
}));

const overlay = () => document.querySelector('#settings-overlay')!;

describe('SettingsSheet', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    dashboard.settingsOpen = false;
    dashboard.setNeedsYouBadge(true);
  });

  it('opens with settingsOpen (the menu’s Settings…, the ⋯) and closes back', () => {
    render(SettingsSheet);
    expect(overlay()).not.toHaveClass('open');
    dashboard.settingsOpen = true;
    flushSync();
    expect(overlay()).toHaveClass('open');
    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    flushSync();
    expect(dashboard.settingsOpen).toBe(false);
    expect(overlay()).not.toHaveClass('open');
  });

  it('holds the ⋯ popup’s settings, laid out as a pane, and the badge’s switch', () => {
    dashboard.settingsOpen = true;
    render(SettingsSheet);
    const sheet = document.querySelector('#settings-sheet')!;
    expect(sheet.querySelector('#mode-control')).not.toBeNull();
    expect(sheet.querySelector('#theme-select')).not.toBeNull();
    expect(sheet.querySelector('#doc-style-select')).not.toBeNull();
    const badge = sheet.querySelector<HTMLInputElement>('#needs-you-badge-check')!;
    expect(badge.checked).toBe(true);
    fireEvent.click(badge);
    flushSync();
    expect(dashboard.needsYouBadge).toBe(false);
  });

  it('resets the project’s view and closes', () => {
    dashboard.settingsOpen = true;
    render(SettingsSheet);
    fireEvent.click(screen.getByRole('button', { name: "Reset this project's view" }));
    flushSync();
    expect(dashboard.settingsOpen).toBe(false);
  });
});
