import { fireEvent, render, screen } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import { version as compmdVersion } from 'virtual:atm-compmd';
import AboutDialog from '../../src/components/AboutDialog.svelte';
import { pluginInfo } from '../../src/lib/plugin-info';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

const overlay = () => document.querySelector('#about-overlay')!;

describe('AboutDialog', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    dashboard.closeDialog();
  });

  it('is closed until opened, then shows the plugin facts', () => {
    render(AboutDialog);
    expect(overlay()).not.toHaveClass('open');
    dashboard.openDialog('about');
    flushSync();
    expect(overlay()).toHaveClass('open');
    const dialog = screen.getByRole('dialog', { name: 'About' });
    expect(document.querySelector('.about-name')).toHaveTextContent(/^Agentic Task Manager \(atm\)$/);
    expect(dialog).toHaveTextContent(pluginInfo.description);
    expect(dialog).toHaveTextContent(`Version ${pluginInfo.version}`);
    expect(dialog).toHaveTextContent('Author Hakan Serce');
    expect(dialog).toHaveTextContent('Licence MIT');
    expect(screen.getByRole('link', { name: pluginInfo.repository })).toHaveAttribute('target', '_blank');
    expect(location.hash).toBe('#about');
  });

  it('names what renders Markdown: CompMD, with its version, licence and site', () => {
    render(AboutDialog);
    dashboard.openDialog('about');
    flushSync();
    const dialog = screen.getByRole('dialog', { name: 'About' });
    expect(dialog).toHaveTextContent(`Rendering CompMD ${compmdVersion} (MIT)`);
    expect(document.querySelector('#about-compmd')).toHaveTextContent(
      new RegExp(`^CompMD ${compmdVersion} \\(MIT\\)$`),
    );
    const link = screen.getByRole('link', { name: 'CompMD' });
    expect(link).toHaveAttribute('href', 'https://compmd.hakanserce.com/');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    expect(dialog).not.toHaveTextContent('Mermaid');
  });

  it("shows the project's vocabularies, short: statuses, phases and types, the defaults or meta's lists (T-255)", () => {
    render(AboutDialog);
    dashboard.openDialog('about');
    flushSync();
    const vocab = () => document.querySelector('#about-vocab')!;
    expect(vocab()).toHaveTextContent('In Review (review)');
    expect(vocab()).toHaveTextContent('Numbered, without names');
    // Priorities and the workflow are on the Project tab
    expect(vocab()).not.toHaveTextContent('Priorities');

    loadIntoDashboard({
      ...standard,
      meta: {
        statuses: [
          { id: 'todo', label: 'Backlog', kind: 'todo' },
          { id: 'doing', label: 'Doing', kind: 'active' },
          { id: 'shipped', label: 'Shipped', kind: 'done' },
        ],
        phases: [{ id: 1, name: 'Core' }],
        types: [{ id: 'spike', label: 'Spike' }],
        workflow: { type: 'pull-request' },
      },
    });
    flushSync();
    expect(vocab()).toHaveTextContent('Statuses Backlog (todo) · Doing (active) · Shipped (done)');
    expect(vocab()).toHaveTextContent('Phases 1 Core');
    expect(vocab()).toHaveTextContent('Types Spike');
    expect(vocab()).not.toHaveTextContent('Workflow');
  });

  it('Project details closes the dialog and opens the Project tab (T-255)', async () => {
    render(AboutDialog);
    dashboard.openDialog('about');
    flushSync();
    await fireEvent.click(screen.getByRole('button', { name: 'Project details' }));
    expect(dashboard.dialog).toBeNull();
    expect(dashboard.tab).toBe('project');
    expect(location.hash).toContain('view=project');
    dashboard.selectTab('kanban');
  });

  it("heads the project's section with its name and logo, and leaves it out away from a project (T-217)", () => {
    const projects = [{ id: 'p1', name: 'Shop', logo: 'p/p1/logo' }];
    Object.assign(dashboard, { projects, route: { page: 'project', id: 'p1' } });
    render(AboutDialog);
    dashboard.openDialog('about');
    flushSync();
    const heading = () => document.querySelector('#about-project');
    expect(heading()).toHaveTextContent(new RegExp(`^${dashboard.projectName}$`)); // as the title bar names it
    expect(heading()!.querySelector('.project-logo img')).toHaveAttribute('src', 'p/p1/logo');
    expect(document.querySelector('#about-vocab')).not.toBeNull();

    // The overview: the project state still holds the last project, so the section would be stale
    dashboard.route = { page: 'overview' };
    flushSync();
    expect(heading()).toBeNull();
    expect(document.querySelector('#about-vocab')).toBeNull();
    expect(screen.getByRole('dialog', { name: 'About' })).toHaveTextContent(`Version ${pluginInfo.version}`);
    Object.assign(dashboard, { projects: [], route: { page: 'project', id: '' } });
  });

  it('closes with the close button, Escape and the backdrop, and drops the hash flag', async () => {
    render(AboutDialog);
    for (const close of [
      () => fireEvent.click(screen.getByRole('button', { name: 'Close' })),
      () => fireEvent.keyDown(document, { key: 'Escape' }),
      () => fireEvent.click(overlay()),
    ]) {
      dashboard.openDialog('about');
      flushSync();
      await close();
      expect(dashboard.dialog).toBeNull();
      expect(location.hash).not.toContain('about');
    }
  });

  it('moves focus to the close button, and back to the opener when closed', async () => {
    const opener = document.body.appendChild(document.createElement('button'));
    opener.focus();
    render(AboutDialog);
    dashboard.openDialog('about');
    flushSync();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
    dashboard.closeDialog();
    flushSync();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});
