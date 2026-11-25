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
  import { projectDocuments } from './lib/project-documents';
  import { page } from '$app/state';
  import { addressOf, reviewOf } from './lib/address';
  import AppTitleBar from './lib/shell/AppTitleBar.svelte';
  import ReviewsTab from './lib/shell/ReviewsTab.svelte';
  import NewTabPage from './lib/shell/NewTabPage.svelte';
  import { reviewWindow } from './lib/reviews/window.svelte';
  import SettingsMenu from './lib/shell/SettingsMenu.svelte';

  // A project's address naming a review (/projects/<id>/reviews/CR-7) shows that review alone, over the whole window
  const reviewing = $derived(dashboard.route.page === 'project' && !!reviewOf(addressOf(page.url)).review);

  // The document viewer's files (ATM-571): the shown project's working copy; on the Omniboard, that of the project
  // whose task is open over it (T-411)
  const documentProject = $derived(
    dashboard.route.page === 'project' || dashboard.peeking ? dashboard.projectId : null,
  );
  const documents = $derived(documentProject ? projectDocuments(documentProject, () => dashboard.meta) : null);
  // Asked for again on the dashboard's refresh in the browser; in the app (T-506), each time the dashboard reloads
  const documentRefresh = $derived({
    every: dashboard.polls ? dashboard.autoRefreshSeconds : null,
    reloadedAt: dashboard.live ? dashboard.loadedAt : 0,
  });

  // The first load is the layout's (routes/+layout.svelte): in the app, once the address of the tab it starts on is in
  onMount(() => {
    // "System" mode follows the OS setting while the page is open
    const query = matchMedia('(prefers-color-scheme: light)');
    const onChange = () => (dashboard.systemPrefersLight = query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  });

  $effect(() => applyAppearance(document.documentElement, dashboard.theme, dashboard.mode, dashboard.docSize));

  // Remember the view per project (FR-5): every change to filters, tab, panel or board options is saved
  $effect(() => dashboard.saveView());
  // The Sprint filter in the address (?sprint=, T-323)
  $effect(() => dashboard.syncSprintParam());
</script>

<!-- A review's tab, at the title bar's right end: the ⋯ settings, as every tab has (ATM-691) -->
{#snippet reviewEnd()}<SettingsMenu />{/snippet}

<UpdateNotice />
{#if reviewing && inApp}
  <!-- A review's page in the app (ATM-574): a tab of its own, under the window's sidebar and title bar as every tab -->
  <Sidebar />
  <AppTitleBar title={reviewWindow.title || dashboard.projectName} end={reviewEnd} />
  <main class="review-tab-main" id="app-main"><ReviewsTab /></main>
{:else if dashboard.route.page === 'new'}
  <!-- The new tab (ATM-755): its chooser of scope and item, under the window's sidebar and title bar -->
  {#if inApp}<Sidebar /><AppTitleBar title="New Tab" end={reviewEnd} />{/if}
  <main class="new-tab-main" id="app-main"><NewTabPage /></main>
{:else if reviewing}
  <!-- A review's page in the browser (ATM-574): the whole window -->
  <div class="review-window" id="review-window">
    <main class="review-window-main" id="app-main"><ReviewsTab /></main>
  </div>
{:else}
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
{/if}
<LoadError />
<DetailPanel />
<DocumentViewer
  source={documents}
  ref={dashboard.docRef}
  refresh={documentRefresh}
  docStyle={dashboard.docStyle}
  onback={() => dashboard.documentBack()}
  onclose={() => dashboard.closeDocument()}
  onopen={(ref) => dashboard.openDocument(ref)}
/>
<WhatsNew />
<AboutDialog />
<AgentsDialog />
<HelpDialog />
<!-- The app's Settings, a sheet (T-517); the browser's are the ⋯ popup -->
{#if inApp}<SettingsSheet />{/if}

<style>
  .review-window {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  /* Under the app's title bar, out of the page's padding (body in app.css): the rest of the window */
  .review-tab-main {
    height: calc(100vh - 40px);
    margin: 0 calc(-1 * var(--space-9)) calc(-1 * var(--space-9));
    min-height: 0;
  }
  .review-window-main {
    flex: 1;
    min-height: 0;
  }
  /* The new tab's chooser, under the app's title bar */
  .new-tab-main {
    min-height: calc(100vh - 40px);
  }
</style>
