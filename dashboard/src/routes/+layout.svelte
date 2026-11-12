<script lang="ts">
  // The dashboard's one layout (T-428), around every page (T-429): the app is here, so it stays as it is when the
  // address moves from one page to another, and the pages under it (the route files beside this one) are its
  // addresses, with nothing of their own to show. lib/address.ts reads and writes them; the router's history entries
  // are the dashboard's own (lib/navigation.ts).
  import '../tokens.css';
  import '../app.css';
  import '../themes/default.css';
  import '../themes/nord.css';
  import '../themes/solarized.css';
  import '../themes/catppuccin.css';
  import '../themes/tokyo-night.css';
  import '../themes/ayu.css';
  import '../themes/night-owl.css';
  import '../themes/gruvbox.css';
  import '../themes/noctis.css';
  import '../themes/sublime-material.css';
  import '../themes/high-contrast.css';
  import '../themes/sticky-notes.css';
  import '../themes/hand-drawn.css';
  import '../themes/doc-styles.css';
  import { onMount, type Snippet } from 'svelte';
  import { afterNavigate, goto } from '$app/navigation';
  import { page } from '$app/state';
  import App from '../App.svelte';
  import { addressOf, legacyParams } from '../lib/address';
  import { watchChanges } from '../lib/api/changes';
  import { inApp } from '../lib/api/http';
  import { listenForLinks } from '../lib/api/links';
  import { listenForMenu, reportMenuState } from '../lib/api/menu';
  import { appTabs } from '../lib/app-tabs.svelte';
  import { menuStateOf, runMenuAction } from '../lib/menu-actions';
  import { arrived, currentAddress, navigate, useRouter, type EntryState } from '../lib/navigation';
  import { tabActions } from '../lib/tab-actions';

  const tabs = tabActions();
  import { dashboard } from '../lib/state.svelte';

  let { children }: { children: Snippet } = $props();

  useRouter({
    go: (address, { replace, state }) =>
      // The addresses are lib/address.ts's, paths of this app at the server's root
      // eslint-disable-next-line svelte/no-navigation-without-resolve
      void goto(address, { replace, state, reset: false }),
    state: () => page.state as EntryState,
  });

  /** An old link (#project=…&view=…), or an address written another way, becomes the page's own address */
  function settle() {
    const shown = location.pathname + location.search;
    if (legacyParams(location.hash) || addressOf(location) !== shown) navigate(addressOf(location), { replace: true });
  }

  onMount(() => {
    settle();
    // In the atm app: the window's tabs, from the page shown (T-514); its atm:// links open their page, the one it was
    // started with here, later ones in a new tab (T-459); its menu's items do what their controls do (T-512)
    if (inApp) {
      appTabs.start(addressOf(location));
      void listenForLinks(
        (address) => navigate(address),
        (address) => tabs.open(address),
      ).catch((e) => console.error('Deep links:', e));
      void listenForMenu((action) => runMenuAction(action, dashboard, undefined, tabs)).catch((e) =>
        console.error('The menu:', e),
      );
    }
  });

  // In the atm app, what the menu shows checked follows the page (T-512)
  $effect(() => {
    if (inApp) void reportMenuState(menuStateOf(dashboard));
  });

  // In the atm app the page reacts to changes (T-506): the app says when the workspace's list or the open project's
  // data changed, and the page reloads them (a workspace switch or another project ends these subscriptions and makes
  // new ones). A change its watcher missed shows when the window regains focus.
  $effect(() => {
    if (!dashboard.live) return;
    return watchChanges(dashboard.workspace, dashboard.projectId, () => void dashboard.reload());
  });

  afterNavigate(({ type }) => {
    arrived();
    // The active tab shows where the page is now (T-514)
    if (inApp) appTabs.follow(currentAddress());
    // The first page is the app's start, which loads it
    if (type !== 'enter') dashboard.onNavigate();
  });
</script>

<!-- An old link typed over the page's address: its page, in place -->
<svelte:window
  onhashchange={() => legacyParams(location.hash) && settle()}
  onfocus={() => dashboard.live && void dashboard.reload()}
/>

<App />
{@render children()}
