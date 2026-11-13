<script lang="ts">
  import { inApp } from '../api/http';
  import { DOC_STYLES, MODE_LABELS, MODE_PREFS, THEMES, type ModePref } from '../appearance';
  import type { IconName } from '../ui/Icon.svelte';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  // The settings (T-323): auto-refresh, mode and theme, the document style, and on a project's page, resetting its
  // saved view; in the atm app also the icon's badge. The browser shows them in the ⋯ popup (SettingsMenu), the app
  // in a sheet (SettingsSheet, T-517): the same controls, once
  const MODE_ICONS: Record<ModePref, IconName> = { system: 'monitor', light: 'sun', dark: 'moon' };
  const current = $derived(THEMES.find((t) => t.id === dashboard.theme) ?? THEMES[0]);
  const lastLoaded = $derived(
    new Date(dashboard.loadedAt).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
  // Like the legacy dashboard, the interval only applies on "change" (blur/enter), not on every keystroke
  function onIntervalChange(e: Event) {
    dashboard.setAutoRefreshSeconds(parseInt((e.currentTarget as HTMLInputElement).value) || 5);
  }
</script>

<!-- In the atm app the page reacts to changes (T-506): nothing to set -->
{#if !dashboard.live}
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
{/if}
<div class="menu-note" id="last-loaded">
  {#if dashboard.live}Updates as tasks change ·
  {/if}Last loaded {lastLoaded}
</div>
<div class="menu-label">Mode</div>
<div class="mode-switch" role="group" aria-label="Color mode" id="mode-control">
  {#each MODE_PREFS as pref (pref)}
    <button
      class="mode-btn"
      class:active={dashboard.modePref === pref}
      data-mode={pref}
      aria-pressed={dashboard.modePref === pref}
      onclick={() => dashboard.setModePref(pref)}><Icon name={MODE_ICONS[pref]} size={14} /> {MODE_LABELS[pref]}</button
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
<!-- What needs you, from the app: the count on its icon (T-518) and a notification when it goes up (T-520) -->
{#if inApp}
  <div class="menu-label">What needs you</div>
  <div class="settings-row">
    <label class="auto-refresh-toggle" title="The number of tasks that need you, on the app's icon"
      ><input
        type="checkbox"
        id="needs-you-badge-check"
        checked={dashboard.needsYouBadge}
        onchange={(e) => dashboard.setNeedsYouBadge(e.currentTarget.checked)}
      /><span class="toggle-track"></span> On the app’s icon</label
    >
  </div>
  <div class="settings-row">
    <label class="auto-refresh-toggle" title="A notification when a task in a project starts needing you"
      ><input
        type="checkbox"
        id="needs-you-notify-check"
        checked={dashboard.needsYouNotify}
        onchange={(e) => dashboard.setNeedsYouNotify(e.currentTarget.checked)}
      /><span class="toggle-track"></span> As notifications</label
    >
  </div>
{/if}
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
      dashboard.settingsOpen = false;
    }}>Reset this project's view</button
  >
{/if}
