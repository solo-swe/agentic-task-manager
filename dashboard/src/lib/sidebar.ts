// The app's sidebar (ATM-753, docs/sidebar-overhaul-design.md §2): a scope (All projects, or one project) and its
// items, each opening that scope's page in a new tab. What it shows and where an item goes, apart from the component.
import { ALL_ITEMS, formatAddress, ITEMS, type Item } from './address';
import { ALL_ITEM_VIEWS, PROJECT_ITEM_VIEWS } from './tabs';

/** A scope: a project's id, or null for All projects */
export type Scope = string | null;

/** The scope's items: six for a project, four for All (no Inbox or Documents, Hakan CR-135) */
export function itemsOf(scope: Scope): readonly Item[] {
  return scope === null ? ALL_ITEMS : ITEMS;
}

/** Where `item` of `scope` opens, in `workspace`: its first view, nothing else of the page shown (no task, document or
 * filter carried over); null for an item the scope doesn't have */
export function itemAddress(workspace: string, scope: Scope, item: Item): string | null {
  const params = new URLSearchParams();
  if (workspace) params.set('workspace', workspace);
  if (scope === null) {
    const tab = ALL_ITEM_VIEWS[item]?.[0];
    if (!tab) return null;
    params.set('overview', '');
    params.set('view', tab);
  } else {
    params.set('project', scope);
    params.set('view', PROJECT_ITEM_VIEWS[item][0]);
  }
  return formatAddress(params);
}

/** A scope chosen by hand, and on what: the active tab and the scope its page had then */
export interface ScopeChoice {
  scope: Scope;
  tab: string | null;
  pageScope: Scope | undefined;
}

/** The sidebar's scope (§2.1): it follows the active tab's page; a choice by hand holds while the active tab and its
 * page's scope stay what they were when it was made (a switch of tab ends it: the component forgets it then). A page of
 * no scope (the landing page, the new tab) keeps the last one followed */
export function sidebarScope(
  page: Scope | undefined,
  tab: string | null,
  choice: ScopeChoice | null,
  last: Scope,
): Scope {
  if (choice && choice.tab === tab && choice.pageScope === page) return choice.scope;
  return page === undefined ? last : page;
}

/** A scope as the preferences keep it ('' for All projects), and back; a project no longer listed is All */
export const scopePref = (scope: Scope): string => scope ?? '';
export function scopeFromPref(pref: string, projects: readonly { id: string; missing?: boolean }[]): Scope {
  return projects.some((p) => p.id === pref && !p.missing) ? pref : null;
}
