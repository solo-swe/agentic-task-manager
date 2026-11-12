<script lang="ts">
  import { dashboard } from '../state.svelte';
  import SettingsPanel from '../shell/SettingsPanel.svelte';
  import Dialog from '../ui/Dialog.svelte';

  // The atm app's Settings (T-517, docs/app-desktop-ui-design.md §6): a sheet over the window, from the menu's
  // Settings… (⌘,; File ▸ Settings… on Windows and Linux) and the title bar's ⋯. The browser's ⋯ popup shows the same
  // controls (SettingsPanel)
</script>

<Dialog id="settings" title="Settings" open={dashboard.settingsOpen} onclose={() => (dashboard.settingsOpen = false)}>
  <div class="settings-menu settings-sheet" id="settings-sheet">
    <SettingsPanel />
  </div>
</Dialog>

<style>
  .settings-sheet {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin-top: var(--space-6);
  }
  :global(.detail-panel.settings) {
    max-width: 420px;
  }
</style>
