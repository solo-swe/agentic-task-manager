<script lang="ts">
  import { DOC_STYLES, MODE_LABELS, MODE_PREFS, THEMES, type ModePref } from '../lib/appearance';
  import type { IconName } from '../lib/icons';
  import { dashboard } from '../lib/state.svelte';
  import Icon from './Icon.svelte';

  // The ⋯ menu at the title bar's right end (T-323): auto-refresh, mode and theme, and on a project's page, resetting
  // its saved view. The reload button stays beside it.
  const MODE_ICONS: Record<ModePref, IconName> = { system: 'monitor', light: 'sun', dark: 'moon' };
  const current = $derived(THEMES.find((t) => t.id === dashboard.theme) ?? THEMES[0]);
  const lastLoaded = $derived(
    new Date(dashboard.loadedAt).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
  let open = $state(false);
  let root: HTMLElement;

  // Like the legacy dashboard, the interval only applies on "change" (blur/enter), not on every keystroke
  function onIntervalChange(e: Event) {
    dashboard.setAutoRefreshSeconds(parseInt((e.currentTarget as HTMLInputElement).value) || 5);
  }
</script>

<svelte:window
  onclick={(e) => open && !e.composedPath().includes(root) && (open = false)}
  onkeydown={(e) => e.key === 'Escape' && (open = false)}
/>

<div class="menu-anchor settings" bind:this={root}>
  <button
    class="menu-trigger settings-btn"
    id="settings-btn"
    aria-haspopup="true"
    aria-expanded={open}
    aria-label="Settings"
    title="Auto-refresh, mode and theme"
    onclick={() => (open = !open)}><Icon name="ellipsis-vertical" /></button
  >
  {#if open}
    <div class="menu settings-menu" id="settings-menu" role="dialog" aria-label="Settings">
      <div class="menu-label">Auto-refresh</div>
      <div class="settings-row">
        <label class="auto-refresh-toggle" title="Auto-refresh tasks.json"
          ><input
            type="checkbox"
            id="auto-refresh-check"
            checked={dashboard.autoRefresh}
            onchange={(e) => dashboard.setAutoRefresh(e.currentTarget.checked)}
          /><span class="toggle-track"></span> On</label
        >
        <label class="refresh-interval"
          >every <input
            type="number"
            class="auto-refresh-interval"
            id="auto-refresh-interval"
            value={dashboard.autoRefreshSeconds}
            min="1"
            max="300"
            title="Interval in seconds"
            onchange={onIntervalChange}
          /> s</label
        >
      </div>
      <div class="menu-note" id="last-loaded">Last loaded {lastLoaded}</div>
      <div class="menu-label">Mode</div>
      <div class="mode-switch" role="group" aria-label="Color mode" id="mode-control">
        {#each MODE_PREFS as pref (pref)}
          <button
            class="mode-btn"
            class:active={dashboard.modePref === pref}
            data-mode={pref}
            aria-pressed={dashboard.modePref === pref}
            onclick={() => dashboard.setModePref(pref)}
            ><Icon name={MODE_ICONS[pref]} size={14} /> {MODE_LABELS[pref]}</button
          >
        {/each}
      </div>
      <label class="menu-label" for="theme-select">Theme</label>
      <!-- A native select: compact, and the keyboard (arrows, letters) works as everywhere else. Its arrow is our
           chevron: the browser's own differs between renders and platforms -->
      <div class="theme-picker">
        <span class="theme-swatch" aria-hidden="true">
          {#each current.swatch as c (c)}<span style="background:{c}"></span>{/each}
        </span>
        <span class="select-chevron"><Icon name="chevron-down" size={14} /></span>
        <select
          id="theme-select"
          value={current.id}
          onchange={(e) => dashboard.setTheme((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each THEMES as theme, i (theme.id)}
            <!-- A separator line where the themes that draw the board begin -->
            {#if i > 0 && theme.group !== THEMES[i - 1].group}<hr />{/if}
            <option value={theme.id}>{theme.name}</option>
          {/each}
        </select>
      </div>
      <!-- How the document viewer's page looks (T-404): the theme's page, or a style of its own -->
      <label class="menu-label" for="doc-style-select">Document style</label>
      <div class="theme-picker doc-style-picker">
        <!-- A sample in the style's type, where the theme has its swatch -->
        <span class="doc-style-sample" data-doc-style={dashboard.docStyle} aria-hidden="true">Aa</span>
        <span class="select-chevron"><Icon name="chevron-down" size={14} /></span>
        <select
          id="doc-style-select"
          value={dashboard.docStyle}
          onchange={(e) => dashboard.setDocStyle((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each DOC_STYLES as style (style.id)}<option value={style.id}>{style.name}</option>{/each}
        </select>
      </div>
      <!-- Forgets the project's saved view (filters, search, tab, each tab's options and zoom): how this page looks,
           so here rather than next to Clear filters -->
      {#if dashboard.route.page === 'project' && dashboard.loaded}
        <div class="menu-label">This project</div>
        <button
          class="panel-button"
          id="reset-view"
          title="Forget this project's saved filters, search, tab, options and zoom"
          onclick={() => {
            dashboard.resetView();
            open = false;
          }}>Reset this project's view</button
        >
      {/if}
    </div>
  {/if}
</div>
