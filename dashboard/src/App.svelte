<script lang="ts">
  import { onMount } from 'svelte';
  import { applyAppearance } from './lib/appearance';
  import { dashboard } from './lib/state.svelte';
  import Header from './lib/shell/Header.svelte';
  import ViewPanel from './lib/shell/ViewPanel.svelte';
  import LoadError from './lib/shell/LoadError.svelte';
  import DetailPanel from './lib/details/DetailPanel.svelte';
  import DocumentViewer from './lib/documents/DocumentViewer.svelte';
  import WhatsNew from './lib/dialogs/WhatsNew.svelte';
  import Footer from './lib/shell/Footer.svelte';
  import AboutDialog from './lib/dialogs/AboutDialog.svelte';
  import AgentsDialog from './lib/dialogs/AgentsDialog.svelte';
  import HelpDialog from './lib/dialogs/HelpDialog.svelte';
  import LandingPage from './lib/shell/LandingPage.svelte';
  import UpdateNotice from './lib/shell/UpdateNotice.svelte';
  import Sidebar from './lib/shell/Sidebar.svelte';
  import SettingsSheet from './lib/dialogs/SettingsSheet.svelte';
  import { inApp } from './lib/api/http';

  onMount(() => {
    dashboard.load();
    // "System" mode follows the OS setting while the page is open
    const query = matchMedia('(prefers-color-scheme: light)');
    const onChange = () => (dashboard.systemPrefersLight = query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  });

  $effect(() => applyAppearance(document.documentElement, dashboard.theme, dashboard.mode));

  // Remember the view per project (FR-5): every change to filters, tab, panel or board options is saved
  $effect(() => dashboard.saveView());
  // The Sprint filter in the address (?sprint=, T-323)
  $effect(() => dashboard.syncSprintParam());
</script>

<UpdateNotice />
<!-- The atm app's sidebar (T-515): the workspace's projects, left of the page -->
{#if inApp}<Sidebar />{/if}
<!-- One layout for every page: the title bar, a main area that takes the rest of the window, the status bar -->
<Header />
<main class="app-main" id="app-main">
  {#if dashboard.route.page === 'project'}
    <ViewPanel />
  {:else}
    <LandingPage />
  {/if}
</main>
<Footer />
<LoadError />
<DetailPanel />
<DocumentViewer />
<WhatsNew />
<AboutDialog />
<AgentsDialog />
<HelpDialog />
<!-- The app's Settings, a sheet (T-517); the browser's are the ⋯ popup -->
{#if inApp}<SettingsSheet />{/if}
