import { browserFetch } from './api/http';
import { loadClaims, type ClaimMap } from './claims';
import {
  DEFAULT_PROJECT_NAME,
  loadArchive,
  loadProjects,
  loadTasks,
  newBuildReady,
  serverBuild,
  type LoadedTasks,
  type ProjectEntry,
  type Validators,
} from './load';
import { compareVersions } from './changelog';
import { projectComponents } from './components';
import { getPref, setPref, storageAvailable } from './prefs';
import { dashboardBuild, dashboardVersion } from './release-notes';
import { isModePref, knownTheme, resolveMode, type Mode, type ModePref } from './appearance';
import {
  emptyFilters,
  FILTER_GROUPS,
  filtersFrom,
  filterTasks,
  searchTasks,
  sprintSelection as appliedSprintSelection,
  toggleFilter,
  type FilterGroup,
  type Filters,
  type ListSort,
} from './filters';
import { parseDialogHash, withDialogHash, type DialogId } from './dialogs';
import { hasViewHash, parseViewHash, withViewHash, type ViewId } from './tabs';
import { isOverviewTab, parseOverviewTab, routeFor, withProjectHash, type OverviewTab, type Route } from './routes';
import { formatHash, hashParams } from './hash';
import type { BoardScope } from './allboard';
import type { BurndownRange } from './burndown';
import type { ActivityRange } from './insights';
import type { ViewWindow } from './time-window';
import type { Task } from './types';
import type { ListGroupBy } from './list-groups';
import { loadProjectSources, type ProjectSource } from './overview-data';
import { leftovers as leftoversOf, scopeOf } from './scope';
import { parseSprintHash, withSprintHash } from './sprint-hash';
import { sprintModel } from './sprints';
import { DEFAULT_VOCABULARY, resolveVocabulary, type Vocabulary } from './vocab';
import { loadWorktrees, mergeWorktrees, type Worktree, type WorktreesResult } from './worktrees';
import { clearView, defaultView, projectKey, readView, sanitizeView, writeView, type SavedView } from './view-store';

const RELOAD_SPIN_MS = 400;
/** A filter group with nothing selected ("All"), for the groups the Progress tab doesn't apply */
const NO_VALUES = emptyFilters().status;

function savedOverviewTab(): OverviewTab {
  const tab = getPref('overviewTab', 'projects');
  return isOverviewTab(tab) ? tab : 'projects';
}

/** Two Sprint filter selections hold the same values, in any order */
function sameSelection(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((v) => b.includes(v));
}

function releaseNotesUnseen(): boolean {
  const version = dashboardVersion();
  const lastSeen = getPref('lastSeenVersion', '');
  return !!version && storageAvailable() && (lastSeen === '' || compareVersions(version, lastSeen) > 0);
}

export class DashboardState {
  /** The projects the server lists (api/projects) */
  projects = $state.raw<ProjectEntry[]>([]);
  /** True once the project list has loaded */
  projectsLoaded = $state(false);
  /** The page shown: a project, the overview, how to add a project, or an unknown project (lib/routes.ts) */
  route = $state.raw<Route>({ page: 'project', id: '' });
  /** The project whose files are loaded, by id; null until one is shown */
  projectId = $state<string | null>(null);
  /** The server holds a newer dashboard than this page (after a plugin update): a bar offers to reload */
  newVersionReady = $state(false);
  /** The overview shows only active projects (open work, or activity lately); remembered per browser */
  overviewActiveOnly = $state(getPref('overviewActiveOnly', true));
  /** The overview's tab, project cards or the board (T-259): "#view=board" in a link, else the last one chosen */
  overviewTab = $state<OverviewTab>(parseOverviewTab(location.hash) ?? savedOverviewTab());
  /** The All projects board in a lane per project (T-260); remembered per browser */
  boardGroupByProject = $state(getPref('boardGroupByProject', false));
  /** The All projects board's search (T-261), over every project's tasks through api/search; remembered per browser */
  boardQuery = $state(getPref('boardQuery', ''));
  /** The All projects board: the active sprints' tasks (the default) or every task (T-340); remembered per browser */
  boardScope = $state<BoardScope>(getPref('boardScope', 'active') === 'all' ? 'all' : 'active');
  /** Every listed project's tasks and claims, for the overview's views across projects (Agents T-333, Activity T-328,
   * Progress T-329): loaded while the overview is shown, refreshed with it */
  projectSources = $state.raw<ProjectSource[]>([]);
  projectSourcesLoaded = $state(false);
  /** The overview's Agents timeline: its zoomed range and Live, apart from a project's (not saved) */
  overviewAgents = $state<{ agentsWindow: ViewWindow | null; agentsLive: boolean }>({
    agentsWindow: null,
    agentsLive: false,
  });
  /** The overview's Activity period, apart from a project's (not saved) */
  overviewActivity = $state<{ activityRange: ActivityRange }>({ activityRange: { preset: 'all' } });
  tasks = $state.raw<Task[]>([]);
  // Never mutated, only replaced on (re)load, so a plain Map in $state.raw is reactive enough
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  taskMap = $state.raw<ReadonlyMap<string, Task>>(new Map());
  projectName = $state(DEFAULT_PROJECT_NAME);
  /** tasks.json's meta, as loaded */
  meta = $state.raw<LoadedTasks['meta']>(undefined);
  /** The project's vocabularies (statuses with kinds, types, priorities, phase names), defaults where meta has none */
  vocab = $state.raw<Vocabulary>(DEFAULT_VOCABULARY);
  /** Live claims by task ID (who works on what), from the optional claims.json; empty without it. */
  // Replaced on every (re)load, never mutated (like taskMap)
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  claims = $state.raw<ClaimMap>(new Map());
  /** The branch a task is shown from, when one of the project's linked git worktrees has it further along (T-179) */
  // Replaced on every (re)load, never mutated (like taskMap)
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  branches = $state.raw<ReadonlyMap<string, string>>(new Map());
  /** When the data was last loaded; "12 m ago" labels are relative to it. */
  loadedAt = $state(Date.now());
  /** True once tasks.json has loaded successfully at least once. */
  loaded = $state(false);
  /** Message from a failed initial load; shown instead of the task grid. */
  loadError = $state<string | null>(null);
  reloading = $state(false);

  /** tasks.json's size (characters), for the archive notice */
  tasksBytes = $state(0);
  /**
   * tasks-archive.json's tasks (DM-5), loaded only once a view needs history (D-5.2: wantArchive); empty until then,
   * and when there is no archive. A task in both files is shown from tasks.json (DM-5.2).
   */
  archive = $state.raw<Task[]>([]);
  archiveLoaded = $state(false);
  /** Search, the lists and the graph include archived tasks ("Include archived") */
  includeArchived = $state(false);
  /** tasks.json's tasks, then the archived ones it doesn't have: the history views use these */
  history = $derived(
    this.archive.length ? [...this.tasks, ...this.archive.filter((t) => !this.taskMap.has(t.id))] : this.tasks,
  );
  /** The project's sprints and which sprint each task is in (lib/sprints.ts); read from phases until it is migrated */
  sprints = $derived(sprintModel(this.meta, this.history, this.vocab));
  /** The project's components (meta.components), in their order; tasks name them by ID (T-323) */
  components = $derived(projectComponents(this.meta));
  /** Open tasks left in closed sprints, which no loop takes until they are moved (lib/scope.ts, T-323) */
  leftovers = $derived(leftoversOf(this.tasks, this.sprints, this.vocab));
  // Archived tasks by ID, for dependency stubs in the detail panel
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  archiveMap = $derived<ReadonlyMap<string, Task>>(new Map(this.archive.map((t) => [t.id, t])));
  /** How many tasks are archived: meta.archived (written by atm archive), else the loaded archive's size */
  archivedCount = $derived(
    typeof this.meta?.archived === 'number' ? this.meta.archived : this.archiveLoaded ? this.archive.length : 0,
  );

  filters = $state.raw<Filters>(emptyFilters());
  /** Keyword search over ID, name, description and type; part of the saved view */
  query = $state('');
  /** Tasks matching the search, before the phase/type/status filters (the filter counts are based on these) */
  /** The tasks the lists, the board and the graph start from: tasks.json's, plus the archive's with Include archived */
  base = $derived(this.includeArchived ? this.history : this.tasks);
  searched = $derived(searchTasks(this.base, this.query));
  filtered = $derived(filterTasks(this.searched, this.filters, this.vocab, this.sprints));
  /** The Sprint filter as it applies: "the active sprint" selects nothing while no sprint is active */
  sprintSelection = $derived(appliedSprintSelection(this.filters.sprint, this.sprints));
  /** What the Kanban, List and Graph show, by sprint (the scope bar) */
  scope = $derived(scopeOf(this.sprints, this.sprintSelection));
  /** Filters other than Sprint, or a search, narrow the board within its sprints (the scope bar says "filtered") */
  narrowed = $derived(
    this.query.trim() !== '' || FILTER_GROUPS.some((g) => g !== 'sprint' && this.filters[g].size > 0),
  );
  /** The tasks matching the search and the filters but Sprint, which scopes only the Kanban, List and Graph (T-323) */
  unscoped = $derived(filterTasks(this.searched, { ...this.filters, sprint: NO_VALUES }, this.vocab, this.sprints));
  /**
   * The Progress tab's tasks: the history (archived tasks too, once loaded) matching the search and filters, except
   * the status and stage filters. Progress *is* the split by status, so those would make it meaningless (T-239).
   * Nor the Sprint filter: the tab shows every sprint and focuses one itself (T-323).
   */
  historyFiltered = $derived(
    filterTasks(
      searchTasks(this.history, this.query),
      { ...this.filters, sprint: NO_VALUES, status: NO_VALUES, stage: NO_VALUES },
      this.vocab,
      this.sprints,
    ),
  );

  /** Task shown in the detail panel; the panel keeps its last content while closed. */
  detailId = $state<string | null>(null);
  detailOpen = $state(false);

  /** The selected view tab; mirrored in the URL hash ("#view=kanban") */
  tab = $state<ViewId>(parseViewHash(location.hash));
  kanbanGroupBySprint = $state(false);
  kanbanShowSkipped = $state(false);
  /** The Kanban done column shows every card, not just the most recent */
  kanbanShowAllDone = $state(false);
  /** The List tab's order: as in the file, or by priority */
  listSort = $state<ListSort>('file');
  /** The List tab's groups, and whether its closed sprints are open (part of the saved view) */
  listGroupBy = $state<ListGroupBy>('sprint');
  listShowClosed = $state(false);
  /** The List's groups as they apply: by sprint only where it shows more than one sprint */
  listGroups = $derived<ListGroupBy>(
    this.listGroupBy === 'sprint' && this.scope.kind !== 'all' && this.scope.kind !== 'several'
      ? 'none'
      : this.listGroupBy,
  );
  /** The sprint the Progress tab focuses on (its ID, BACKLOG, or ACTIVE: the active one); null for all of them */
  progressSprint = $state<string | null>(defaultView().progressSprint);
  /** The filter panel's Sprint group lists the closed sprints too (part of the saved view) */
  filterClosedSprints = $state(false);
  /** The Progress tab's burndown range (part of the saved view) */
  progressRange = $state<BurndownRange>('sprint');
  /** Tasks outside the search and filters: left out of the graph, or greyed out in it */
  graphFiltered = $state<'hide' | 'dim'>('hide');
  /** The graph greys out done tasks, whatever the filters (part of the saved view) */
  graphDimDone = $state(false);
  /** The graph keeps the matching tasks fitted; panning or zooming turns it off (part of the saved view) */
  graphAutoFit = $state(true);
  /** What auto-fit keeps fitted, set by the fit buttons (part of the saved view) */
  graphFitTarget = $state<'all' | 'matches'>('matches');
  /** The graph's zoom and position once the user moved it; null while it is the automatic view */
  graphView = $state.raw<{ k: number; x: number; y: number } | null>(null);
  /** The Agents tab's zoomed time range; null shows everything */
  agentsWindow = $state.raw<ViewWindow | null>(null);
  /** The Agents timeline runs up to now and moves with time */
  agentsLive = $state(false);
  /** The period the Activity panel shows */
  activityRange = $state.raw<ActivityRange>({ preset: 'all' });
  /** Where this project's view is saved; set once the first load says which project this is */
  viewKey = $state<string | null>(null);

  whatsNewOpen = $state(false);
  /** The open About or Help dialog; mirrored in the URL hash ("#about") so it can be linked */
  dialog = $state<DialogId | null>(parseDialogHash(location.hash));
  /** "New" on the version until this version's release notes are opened once (FR-1.5). Without working storage
   * it could never be cleared, so it is not shown at all. */
  releaseNotesUnseen = $state(releaseNotesUnseen());

  // Light/dark mode and theme, per browser (FR-2.3, FR-3.2). "System" follows the OS setting, live.
  modePref = $state<ModePref>(
    isModePref(getPref('mode', 'system')) ? (getPref('mode', 'system') as ModePref) : 'system',
  );
  theme = $state(knownTheme(getPref('theme', 'default')));
  systemPrefersLight = $state(prefersLight());
  mode = $derived<Mode>(resolveMode(this.modePref, this.systemPrefersLight));

  // On by default (FR-6.1); the user's choice is remembered per browser (FR-6.2)
  autoRefresh = $state(getPref('autoRefresh', true));
  autoRefreshSeconds = $state(getPref('autoRefreshSeconds', 5));

  // The last tasks.json's ETag / Last-Modified: polls ask whether it changed, and skip the work when it hasn't
  #validators: Validators | undefined;
  #projectsValidators: Validators | undefined;
  #pendingNeedsYou = false;
  /** #sprint= in the URL (syncSprintHash): not until the project's view is restored, then replaced once, then pushed */
  #sprintHash: 'pending' | 'restored' | 'synced' = 'pending';
  #pendingTask: string | null = null;
  // The build the server last said it holds, and the one dismissed in this page (for when storage is unavailable)
  #serverBuild: string | null = null;
  #dismissedBuild = '';
  #archiveValidators: Validators | undefined;
  #archiveWanted = false;
  // The main worktree's tasks as loaded, and the project's worktrees: `tasks` is the two merged
  #mainTasks: Task[] = [];
  #worktrees: Worktree[] = [];
  #worktreesValidators: Validators | undefined;

  /**
   * A view needs history (D-5.2): load the archive now, and keep it current with the polls from then on. Failures
   * are logged only; the views then show tasks.json alone.
   */
  wantArchive() {
    if (this.#archiveWanted) return;
    this.#archiveWanted = true;
    void this.refreshArchive().catch((e) => console.error('Loading the archive failed:', e));
  }

  setIncludeArchived(on: boolean) {
    this.includeArchived = on;
    if (on) this.wantArchive();
  }

  private async refreshArchive() {
    if (!this.projectId) return;
    const result = await loadArchive(
      this.projectId,
      browserFetch,
      this.archiveLoaded ? this.#archiveValidators : undefined,
    );
    if (!result.unchanged) {
      this.#archiveValidators = result.validators;
      this.archive = result.tasks;
    }
    this.archiveLoaded = true;
  }

  async load() {
    try {
      await this.fetchAndApply();
      this.loadError = null;
    } catch (e) {
      this.loadError = (e as Error).message;
    }
  }

  /** Re-fetch the project list and the project's files. Failures keep the current data (logged only). */
  async reload() {
    this.reloading = true;
    try {
      await this.fetchAndApply();
      this.loadError = null;
    } catch (e) {
      console.error('Reload failed:', e);
    }
    setTimeout(() => (this.reloading = false), RELOAD_SPIN_MS);
  }

  /** Re-fetch the project list and decide the page from the URL; returns the project to load, or null for a page
   * without one (the overview, how to add a project, an unknown project). */
  private async refreshProjects(): Promise<string | null> {
    const result = await loadProjects(browserFetch, this.#projectsValidators);
    if (!result.unchanged) {
      this.#projectsValidators = result.validators;
      this.projects = result.projects;
    }
    this.projectsLoaded = true;
    this.route = routeFor(this.projects, location.hash);
    if (this.route.page !== 'project') {
      // A task's details belong to its project's page (Back from a task opened on the overview)
      this.detailOpen = false;
      return null;
    }
    if (this.route.id !== this.projectId) this.switchProject(this.route.id);
    // The project page wasn't shown, so nothing followed a #view= in the link that opened it
    if (hasViewHash(location.hash)) this.tab = parseViewHash(location.hash);
    return this.route.id;
  }

  /** Forget the shown project's data and view state, so the next load brings in `id`'s files and saved view. */
  private switchProject(id: string) {
    this.projectId = id;
    this.loaded = false;
    this.loadError = null;
    this.#validators = undefined;
    this.#archiveValidators = undefined;
    this.#archiveWanted = false;
    this.archive = [];
    this.archiveLoaded = false;
    this.includeArchived = false;
    this.tasks = [];
    // Replaced whole, never mutated (like on a load)
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    this.taskMap = new Map();
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    this.claims = new Map();
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    this.branches = new Map();
    this.#mainTasks = [];
    this.#worktrees = [];
    this.#worktreesValidators = undefined;
    this.meta = undefined;
    this.vocab = DEFAULT_VOCABULARY;
    this.projectName = DEFAULT_PROJECT_NAME;
    this.detailOpen = false;
    this.detailId = null;
    this.viewKey = null;
    this.#sprintHash = 'pending';
  }

  /** Show project `id`, or the overview for null (a new history entry, so Back returns). With `needsYou`, the project
   * opens on the Kanban board filtered to the tasks in review and blocked; with `task`, with that task's details open;
   * with `view`, on that tab. */
  openProject(
    id: string | null,
    { needsYou = false, task, view }: { needsYou?: boolean; task?: string; view?: ViewId } = {},
  ) {
    // Another project's sprints are not this one's: its own saved Sprint filter applies
    let hash = withSprintHash(withProjectHash(location.hash, id), null);
    if (id !== null && needsYou) {
      hash = withViewHash(hash, 'kanban');
      this.#pendingNeedsYou = true;
    } else if (id !== null && view) hash = withViewHash(hash, view);
    if (id !== null && task) this.#pendingTask = task;
    location.hash = hash;
  }

  /** The URL changed (a link, Back): pick up the dialog, and the page when it names another one. */
  onHashChange() {
    this.dialog = parseDialogHash(location.hash);
    // An overview tab from the URL (Back, a link) is the last one shown, so it is remembered too
    const overviewTab = parseOverviewTab(location.hash);
    if (overviewTab) {
      this.overviewTab = overviewTab;
      setPref('overviewTab', overviewTab);
    }
    const route = routeFor(this.projects, location.hash);
    if (JSON.stringify(route) !== JSON.stringify(this.route)) {
      void this.reload();
      return;
    }
    // Back, or a link to another sprint of the project shown (T-323)
    const linked = this.#sprintHash === 'pending' ? null : parseSprintHash(location.hash, this.sprints);
    if (linked && !sameSelection(linked, [...this.filters.sprint])) this.setSprintScope(linked);
  }

  /** Whether the server now holds a newer dashboard than this page (the plugin was updated): offer a reload, unless
   * the user dismissed this build already (in this tab, another, or before a reload). A failed check changes nothing. */
  private async checkServerBuild() {
    const server = await serverBuild();
    if (server === null) return;
    this.#serverBuild = server;
    const dismissed = getPref('dismissedBuild', '') || this.#dismissedBuild;
    this.newVersionReady = newBuildReady(dashboardBuild(), server, dismissed);
  }

  /** "Not now": the bar stays away for this build, also after a reload; only another build brings it back */
  dismissNewVersion() {
    if (this.#serverBuild) {
      this.#dismissedBuild = this.#serverBuild;
      setPref('dismissedBuild', this.#serverBuild);
    }
    this.newVersionReady = false;
  }

  private async fetchAndApply() {
    const project = await this.refreshProjects();
    void this.checkServerBuild(); // the server answered: ask which dashboard it holds now
    if (project === null) {
      if (this.route.page === 'overview') await this.refreshProjectSources();
      this.loadedAt = Date.now();
      return;
    }
    const [result, claims, trees] = await Promise.all([
      loadTasks(project, browserFetch, this.loaded ? this.#validators : undefined),
      loadClaims(project),
      // Worktrees are extra: a failure leaves the board on the main worktree's tasks (logged only)
      loadWorktrees(project, browserFetch, this.#worktreesValidators).catch((e): WorktreesResult => {
        console.error('Loading the worktrees failed:', e);
        return { unchanged: true };
      }),
      this.#archiveWanted
        ? this.refreshArchive().catch((e) => console.error('Reloading the archive failed:', e))
        : undefined,
    ]);
    if (!trees.unchanged) {
      this.#worktreesValidators = trees.validators;
      this.#worktrees = trees.worktrees;
    }
    if (result.unchanged) {
      // tasks.json is as it was: keep the tasks (no re-parse, no re-render); claims and the clock still move
      this.claims = claims;
      if (!trees.unchanged) this.mergeWorktrees();
      this.loadedAt = Date.now();
    } else {
      this.#validators = result.validators;
      this.apply(result.loaded, claims);
    }
    if (!this.loaded) this.restoreView();
    if (this.#pendingNeedsYou) {
      // Opened from an overview card's "needs you": only what waits for a review or a decision
      this.#pendingNeedsYou = false;
      const status = this.vocab.statuses.filter((s) => s.kind === 'review' || s.kind === 'blocked').map((s) => s.id);
      const none = Object.fromEntries(FILTER_GROUPS.map((g) => [g, []])) as unknown as Record<FilterGroup, string[]>;
      this.filters = filtersFrom({ ...none, status });
    }
    this.loaded = true;
    if (this.#pendingTask) {
      // Opened from an overview card's "needs you" list: that task's details
      this.showDetail(this.#pendingTask);
      this.#pendingTask = null;
    }
  }

  /** Select an overview tab: remembered per browser, and in the URL so Back returns to the other */
  selectOverviewTab(tab: OverviewTab) {
    setPref('overviewTab', tab);
    if (tab !== this.overviewTab) {
      const params = hashParams(location.hash);
      params.set('view', tab);
      location.hash = formatHash(params);
    }
    this.overviewTab = tab;
  }

  /** Every listed project's tasks and claims, for the overview's views across projects; a failure keeps what was
   * there */
  private async refreshProjectSources() {
    this.projectSources = await loadProjectSources(this.projects, this.projectSources);
    this.projectSourcesLoaded = true;
  }

  setBoardGroupByProject(on: boolean) {
    this.boardGroupByProject = on;
    setPref('boardGroupByProject', on);
  }

  setBoardScope(scope: BoardScope) {
    this.boardScope = scope;
    setPref('boardScope', scope);
  }

  setBoardQuery(query: string) {
    this.boardQuery = query;
    setPref('boardQuery', query);
  }

  setOverviewActiveOnly(on: boolean) {
    this.overviewActiveOnly = on;
    setPref('overviewActiveOnly', on);
  }

  setAutoRefresh(on: boolean) {
    this.autoRefresh = on;
    setPref('autoRefresh', on);
  }

  setModePref(pref: ModePref) {
    this.modePref = pref;
    setPref('mode', pref);
  }

  setTheme(id: string) {
    this.theme = knownTheme(id);
    setPref('theme', this.theme);
  }

  setAutoRefreshSeconds(seconds: number) {
    this.autoRefreshSeconds = seconds;
    setPref('autoRefreshSeconds', seconds);
  }

  toggleFilter(group: FilterGroup, value: string) {
    // A Sprint pick replaces "the active sprint" kept while none is active: it isn't shown, so it isn't kept either
    const filters = group === 'sprint' ? { ...this.filters, sprint: this.sprintSelection } : this.filters;
    this.filters = toggleFilter(filters, group, value);
  }

  /** No filters and no search */
  clearFilters() {
    this.filters = emptyFilters();
    this.query = '';
  }

  /** The scope bar's choice: exactly these sprints (ACTIVE, a sprint's ID, BACKLOG), or none for all tasks */
  setSprintScope(values: string[]) {
    // A fresh set replaces the old one (filters is $state.raw): nothing mutates it
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    this.filters = { ...this.filters, sprint: new Set(values) };
  }

  /** The other filters and the search cleared, the sprints kept (the scope bar's "filtered ✕") */
  clearNarrowing() {
    this.filters = { ...emptyFilters(), sprint: this.filters.sprint };
    this.query = '';
  }

  showDetail(id: string) {
    if (!this.taskMap.has(id) && !this.archiveMap.has(id)) return;
    this.detailId = id;
    this.detailOpen = true;
  }

  closeDetail() {
    this.detailOpen = false;
  }

  openWhatsNew() {
    this.whatsNewOpen = true;
    const version = dashboardVersion();
    if (version) setPref('lastSeenVersion', version);
    this.releaseNotesUnseen = false;
  }

  closeWhatsNew() {
    this.whatsNewOpen = false;
  }

  openDialog(id: DialogId) {
    this.dialog = id;
    location.hash = withDialogHash(location.hash, id);
  }

  /** Closes the dialog without a new history entry, so Back doesn't reopen it. */
  closeDialog() {
    this.dialog = null;
    history.replaceState(null, '', withDialogHash(location.hash, null));
  }

  selectTab(tab: ViewId) {
    if (tab !== this.tab) location.hash = withViewHash(location.hash, tab);
    this.tab = tab;
  }

  /** The current view, as saved per project. */
  currentView(): SavedView {
    return {
      filters: Object.fromEntries(FILTER_GROUPS.map((g) => [g, [...this.filters[g]]])) as SavedView['filters'],
      version: defaultView().version,
      query: this.query,
      tab: this.tab,
      kanban: {
        groupBySprint: this.kanbanGroupBySprint,
        showSkipped: this.kanbanShowSkipped,
        showAllDone: this.kanbanShowAllDone,
      },
      listSort: this.listSort,
      listGroupBy: this.listGroupBy,
      listShowClosed: this.listShowClosed,
      progressSprint: this.progressSprint,
      filterClosedSprints: this.filterClosedSprints,
      progressRange: this.progressRange,
      graphFiltered: this.graphFiltered,
      graphDimDone: this.graphDimDone,
      graphAutoFit: this.graphAutoFit,
      graphFitTarget: this.graphFitTarget,
      graph: this.graphView,
      agents: this.agentsWindow,
      agentsLive: this.agentsLive,
      activity: this.activityRange,
    };
  }

  saveView() {
    const view = this.currentView(); // read first, so an $effect calling this tracks every field
    if (this.viewKey) writeView(this.viewKey, view);
  }

  /** Forget this project's saved view and return to the defaults. */
  resetView() {
    if (this.viewKey) clearView(this.viewKey);
    this.applyView(defaultView(), true);
  }

  /** After the first load: bring back this project's view. A #view= in the URL wins over the saved tab. */
  private restoreView() {
    // The server's project id is meta.id (or stands in for it), so views saved before 0.15.0 are found again
    this.viewKey = projectKey(this.projectId ? { id: this.projectId } : this.meta);
    this.applyView(
      sanitizeView(readView(this.viewKey), this.tasks, this.vocab, this.sprints),
      !hasViewHash(location.hash),
    );
    // A #sprint= in the link wins over the saved Sprint filter (T-323)
    const linked = parseSprintHash(location.hash, this.sprints);
    if (linked) this.setSprintScope(linked);
    this.#sprintHash = 'restored';
  }

  /**
   * The Sprint filter into the URL (#sprint=, T-323), so the address shares the sprint shown and Back returns to the
   * previous one; App runs it on every change. The first write after a project's view is restored replaces the entry.
   */
  syncSprintHash() {
    const selection = [...this.filters.sprint];
    if (this.#sprintHash === 'pending' || this.route.page !== 'project' || !this.sprints.sprints.length) return;
    const linked = parseSprintHash(location.hash, this.sprints);
    if (linked && sameSelection(linked, selection)) {
      this.#sprintHash = 'synced';
      return;
    }
    const hash = withSprintHash(location.hash, selection);
    if (this.#sprintHash === 'restored') history.replaceState(null, '', hash);
    else location.hash = hash;
    this.#sprintHash = 'synced';
  }

  private applyView(view: SavedView, includeTab: boolean) {
    this.filters = filtersFrom(view.filters);
    this.query = view.query;
    this.kanbanGroupBySprint = view.kanban.groupBySprint;
    this.kanbanShowSkipped = view.kanban.showSkipped;
    this.kanbanShowAllDone = view.kanban.showAllDone;
    this.listSort = view.listSort;
    this.listGroupBy = view.listGroupBy;
    this.listShowClosed = view.listShowClosed;
    this.progressSprint = view.progressSprint;
    this.filterClosedSprints = view.filterClosedSprints;
    this.progressRange = view.progressRange;
    this.graphFiltered = view.graphFiltered;
    this.graphDimDone = view.graphDimDone;
    this.graphAutoFit = view.graphAutoFit;
    this.graphFitTarget = view.graphFitTarget;
    this.graphView = view.graph;
    this.agentsWindow = view.agents;
    this.agentsLive = view.agentsLive;
    this.activityRange = view.activity;
    if (includeTab && view.tab !== this.tab) {
      history.replaceState(null, '', withViewHash(location.hash, view.tab));
      this.tab = view.tab;
    }
  }

  private apply(loaded: LoadedTasks, claims: ClaimMap) {
    this.tasksBytes = loaded.bytes;
    this.meta = loaded.meta;
    this.vocab = resolveVocabulary(loaded.meta);
    this.#mainTasks = loaded.tasks;
    this.tasks = loaded.tasks;
    this.taskMap = loaded.taskMap;
    this.mergeWorktrees();
    this.projectName = loaded.projectName;
    this.claims = claims;
    this.loadedAt = Date.now();
  }

  /** Show each task from the worktree where it got furthest (lib/worktrees.ts) */
  private mergeWorktrees() {
    const merged = mergeWorktrees(this.#mainTasks, this.#worktrees, this.vocab);
    if (merged.tasks !== this.tasks) {
      this.tasks = merged.tasks;
      // eslint-disable-next-line svelte/prefer-svelte-reactivity
      this.taskMap = new Map(merged.tasks.map((t) => [t.id, t]));
    }
    if (merged.branches.size || this.branches.size) this.branches = merged.branches;
  }
}

function prefersLight(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: light)').matches;
}

export const dashboard = new DashboardState();
