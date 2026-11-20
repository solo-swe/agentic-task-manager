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
  import { afterNavigate, beforeNavigate, goto, replaceState } from '$app/navigation';
  import { page } from '$app/state';
  import App from '../App.svelte';
  import { addressOf, legacyParams } from '../lib/address';
  import { watchChanges } from '../lib/api/changes';
  import { inApp } from '../lib/api/http';
  import { listenForLinks } from '../lib/api/links';
  import { listenForMenu, reportMenuState } from '../lib/api/menu';
  import { openUrl, reportNeedsYou, restoredTabs, saveTabs } from '../lib/api/shell';
  import { appTabs } from '../lib/app-tabs.svelte';
  import { sendExternalLink } from '../lib/external-links';
  import { menuStateOf, runMenuAction } from '../lib/menu-actions';
  import {
    arrived,
    back as goBackInTab,
    currentAddress,
    forward as goForwardInTab,
    navigate,
    originOf,
    useRouter,
    useTabHistory,
    type EntryState,
  } from '../lib/navigation';
  import { newTabAddress } from '../lib/new-tab-clicks';
  import { inBackground, tabActions, wantsNewTab } from '../lib/tab-actions';

  const tabs = tabActions();

  /** In the app, before the click reaches what would open it here: an external link opens in the user's browser
   * (T-551: the window opens none itself), any click on it; ⌘-click, Ctrl-click or a middle click on a task or a
   * document opens it in a new tab (T-515) */
  function openInNewTab(e: MouseEvent) {
    if (!inApp || sendExternalLink(e, openUrl) || !wantsNewTab(e)) return;
    const address = newTabAddress(e.target as Element | null, currentAddress());
    if (!address) return;
    e.preventDefault();
    e.stopPropagation();
    // In the background, right of this tab; ⇧⌘ brings it forward (ATM-698)
    tabs.open(address, inBackground(e));
  }
  import { dashboard } from '../lib/state.svelte';

  let { children }: { children: Snippet } = $props();

  useRouter({
    go: (address, { replace, state }) =>
      // The addresses are lib/address.ts's, paths of this app at the server's root
      // eslint-disable-next-line svelte/no-navigation-without-resolve
      void goto(address, { replace, state, reset: false }),
    state: () => page.state as EntryState,
    back: (steps) => history.go(-steps),
  });
  // In the app each title-bar tab has its own history (ATM-694): navigate, Back and Forward go through it
  if (inApp) useTabHistory(appTabs);

  /** In the app, the mouse's back and forward buttons walk the active tab's history (the page's own is one entry) */
  function mouseBackForward(e: MouseEvent) {
    if (!inApp || (e.button !== 3 && e.button !== 4)) return;
    e.preventDefault();
    if (e.button === 3) goBackInTab();
    else goForwardInTab();
  }

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
      // As the app starts, the window's tabs as they were when it last quit (T-518): the active one is shown, and a
      // link the app was started with opens in a new tab of them
      void restoredTabs().then((restored) => {
        const back = appTabs.start(addressOf(location), restored);
        if (back) navigate(back, { replace: true });
        void listenForLinks(
          // The link the app was started with is where its first tab starts (ATM-694: no Back to the app's root)
          (address) => (back ? tabs.open(address) : navigate(address, { replace: true })),
          (address) => tabs.open(address),
        ).catch((e) => console.error('Deep links:', e));
      });
      void listenForMenu((action) => runMenuAction(action, dashboard, undefined, tabs)).catch((e) =>
        console.error('The menu:', e),
      );
    }
  });

  // In the atm app, what the menu shows checked follows the page (T-512)
  $effect(() => {
    if (inApp) void reportMenuState(menuStateOf(dashboard));
  });

  // In the atm app, the window's tabs are kept for the next start as they change (T-518)
  $effect(() => {
    if (inApp && appTabs.tabs.length > 0) void saveTabs(appTabs.kept);
  });

  // In the atm app, what needs the user: the workspace's count on the app's icon and a notification when it goes up,
  // each unless Settings turns it off (T-518, T-520). Told again whenever the workspace's projects change; the app
  // reads the counts itself
  $effect(() => {
    if (!inApp || !dashboard.projectsLoaded) return;
    void dashboard.projects;
    void reportNeedsYou(dashboard.workspace, dashboard.needsYouBadge, dashboard.needsYouNotify);
  });

  // In the atm app the page reacts to changes (T-506): the app says when the workspace's list or the open project's
  // data changed, and the page reloads them (a workspace switch or another project ends these subscriptions and makes
  // new ones). A change its watcher missed shows when the window regains focus.
  $effect(() => {
    if (!dashboard.live) return;
    return watchChanges(dashboard.workspace, dashboard.projectId, () => void dashboard.reload());
  });

  // A link the router followed itself (an anchor's plain click) pushed an entry: it remembers the address it was
  // reached from, as navigate's entries do (ATM-686)
  let leaving: { address: string; state: EntryState } | null = null;
  beforeNavigate(({ type, to, cancel }) => {
    // In the app, a link the router would follow itself is a step in the tab's history instead (ATM-694): the page's
    // own history stays one entry deep
    if (inApp && type === 'link' && to?.url && to.url.origin === location.origin) {
      cancel();
      navigate(addressOf(to.url));
      return;
    }
    leaving = type === 'link' ? { address: currentAddress(), state: page.state as EntryState } : null;
  });

  afterNavigate(({ type }) => {
    arrived();
    if (type === 'link' && leaving && leaving.address !== currentAddress() && !(page.state as EntryState).atmFrom)
      replaceState('', { ...page.state, ...originOf(currentAddress(), leaving.address, leaving.state) });
    leaving = null;
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
  onclickcapture={openInNewTab}
  onauxclickcapture={openInNewTab}
  onmouseup={mouseBackForward}
/>

<App />
{@render children()}
