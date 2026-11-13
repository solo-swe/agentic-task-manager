<script lang="ts">
  import { Popover } from 'bits-ui';
  import { inApp } from '../api/http';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import SettingsPanel from './SettingsPanel.svelte';

  // The ⋯ button at the title bar's right end (T-323). In the browser it opens the settings (SettingsPanel) in a
  // popup under it; in the atm app, the Settings sheet (T-517), as the menu's Settings… (⌘,) does
</script>

{#snippet trigger(props: Record<string, unknown>)}
  <button
    {...props}
    class="menu-trigger settings-btn"
    id="settings-btn"
    aria-label="Settings"
    title={dashboard.live ? 'Mode and theme' : 'Auto-refresh, mode and theme'}><Icon name="ellipsis-vertical" /></button
  >
{/snippet}

{#if inApp}
  <div class="menu-anchor settings">
    {@render trigger({ 'aria-haspopup': 'dialog', onclick: () => (dashboard.settingsOpen = true) })}
  </div>
{:else}
  <!-- A popover of settings, Bits UI's (T-431): focus moves in and stays while it is open, Escape or a click outside
       closes it, and focus goes back to the button. Placed by CSS under the button -->
  <div class="menu-anchor settings">
    <Popover.Root bind:open={dashboard.settingsOpen}>
      <Popover.Trigger>
        {#snippet child({ props })}{@render trigger(props)}{/snippet}
      </Popover.Trigger>
      <Popover.ContentStatic role="dialog" aria-label="Settings">
        {#snippet child({ props })}
          <div {...props} class="menu settings-menu" id="settings-menu">
            <SettingsPanel />
          </div>
        {/snippet}
      </Popover.ContentStatic>
    </Popover.Root>
  </div>
{/if}
