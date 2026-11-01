// The saved view of each project: filters, tab, panel and board options, remembered between visits (FR-5).
// Per project, not per browser: dashboards of different projects served from the same origin (localhost:8000)
// share localStorage, so each project's view lives under its own key. Theme, mode, auto-refresh and the last-seen
// version stay per browser (prefs.ts).
import { BURNDOWN_RANGES, type BurndownRange } from './burndown';
import { ACTIVE, FILTER_GROUPS, filterValues, type FilterGroup, type ListSort } from './filters';
import { sprintKey } from './forecast';
import { ACTIVITY_PRESETS, type ActivityRange } from './insights';
import { LIST_GROUP_BYS, type ListGroupBy } from './list-groups';
import { getPref, readStored, writeStored } from './prefs';
import { NO_SPRINTS, type SprintModel } from './sprints';
import { DEFAULT_VIEW, VIEWS, type ViewId } from './tabs';
import type { ViewWindow } from './time-window';
import type { Task } from './types';
import { DEFAULT_VOCABULARY, type Vocabulary } from './vocab';

export const VIEW_KEY_PREFIX = 'atm-dashboard:view:';
/** 2: the view opens on the active sprint (T-323); before it, no Sprint filter was the default, not a choice */
const VIEW_VERSION = 2;

export interface SavedView {
  filters: Record<FilterGroup, string[]>;
  version: number;
  /** The keyword search */
  query: string;
  tab: ViewId;
  kanban: { groupBySprint: boolean; showSkipped: boolean; showAllDone: boolean };
  /** The List tab's order */
  listSort: ListSort;
  /** The List tab's groups (T-323): by sprint (where it shows more than one), label, component, or none */
  listGroupBy: ListGroupBy;
  /** Grouped by sprint, the closed sprints are open, not folded into one row */
  listShowClosed: boolean;
  /** The sprint the Progress tab focuses on (its ID, or BACKLOG), or null for all of them */
  progressSprint: string | null;
  /** The filter panel's Sprint group lists the closed sprints too, not just the open ones */
  filterClosedSprints: boolean;
  /** The Progress tab's burndown range: since the (focused or current) sprint started, 14 days, or all time */
  progressRange: BurndownRange;
  /** Tasks outside the search and filters: left out of the graph, or shown greyed out */
  graphFiltered: 'hide' | 'dim';
  /** The graph greys out done tasks, whatever the filters */
  graphDimDone: boolean;
  /** The dependency graph's zoom and position, or null for the automatic initial view (FR-5.7) */
  graph: { k: number; x: number; y: number } | null;
  /** The Agents tab's zoomed time range, or null to show everything */
  agents: ViewWindow | null;
  /** The Agents timeline runs up to now and moves with time */
  agentsLive: boolean;
  /** The period the Activity panel shows */
  activity: ActivityRange;
}

export interface ProjectMeta {
  id?: unknown;
  project?: unknown;
}

/** The key a project's view is stored under: meta.id (written by /atm:create-tasks), else the project name. */
export function projectKey(meta: ProjectMeta | undefined): string {
  const id = typeof meta?.id === 'string' && meta.id ? meta.id : null;
  const name = typeof meta?.project === 'string' && meta.project ? meta.project : null;
  return VIEW_KEY_PREFIX + (id ? 'id:' + id : name ? 'project:' + name : 'default');
}

export function defaultView(): SavedView {
  return {
    // The active sprint first (T-323): with none active, the selection is there but selects nothing
    filters: Object.fromEntries(
      FILTER_GROUPS.map((g) => [g, g === 'sprint' ? [ACTIVE] : []]),
    ) as unknown as SavedView['filters'],
    version: VIEW_VERSION,
    query: '',
    tab: DEFAULT_VIEW,
    // Before 0.12.0 "group by phase" was remembered per browser; it still seeds projects without a saved view
    kanban: { groupBySprint: getPref('kanbanGroupByPhase', false), showSkipped: false, showAllDone: false },
    listSort: 'file',
    listGroupBy: 'sprint',
    listShowClosed: false,
    // The burndown of the active sprint, whichever it is (T-323); all sprints while none is active
    progressSprint: ACTIVE,
    filterClosedSprints: false,
    progressRange: 'sprint',
    graphFiltered: 'hide',
    graphDimDone: false,
    graph: null,
    agents: null,
    agentsLive: false,
    activity: { preset: 'all' },
  };
}

const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
const bool = (value: unknown, fallback: boolean) => (typeof value === 'boolean' ? value : fallback);
const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);

function graphView(value: unknown): SavedView['graph'] {
  const g = value as Record<string, unknown> | null;
  return g && finite(g.k) && g.k > 0 && finite(g.x) && finite(g.y) ? { k: g.k, x: g.x, y: g.y } : null;
}

function agentsWindow(value: unknown): SavedView['agents'] {
  const w = value as Record<string, unknown> | null;
  return w && finite(w.start) && finite(w.end) && w.end > w.start
    ? { start: w.start, end: w.end, followEnd: w.followEnd === true }
    : null;
}

function activityRange(value: unknown): ActivityRange {
  const r = value as Record<string, unknown> | null;
  const preset = (ACTIVITY_PRESETS as readonly unknown[]).includes(r?.preset)
    ? (r!.preset as ActivityRange['preset'])
    : 'all';
  if (preset !== 'custom')
    return finite(r?.offset) && r.offset < 0 ? { preset, offset: Math.round(r.offset) } : { preset };
  return finite(r!.from) && finite(r!.to) && r!.to > r!.from ? { preset, from: r!.from, to: r!.to } : { preset: 'all' };
}

/** The stored view for `key`, with every missing or malformed field replaced by its default. */
export function readView(key: string): SavedView {
  const stored = readStored(key);
  const view = defaultView();
  const filters = (stored.filters ?? {}) as Record<string, unknown>;
  const kanban = (stored.kanban ?? {}) as Record<string, unknown>;
  // Views saved before 0.20.0 also have progressActive ("Active phases only", replaced by folding finished phases,
  // T-241, T-243): ignored
  const sprint = strings(filters.sprint);
  return {
    // Views saved before sprints also have a phase filter: dropped (SR-7.1). Views saved before version 2 without a
    // Sprint filter open on the active sprint, as new ones do.
    filters: Object.fromEntries(
      FILTER_GROUPS.map((g) => [
        g,
        g === 'sprint' && stored.version !== VIEW_VERSION && !sprint.length ? [ACTIVE] : strings(filters[g]),
      ]),
    ) as SavedView['filters'],
    version: VIEW_VERSION,
    query: typeof stored.query === 'string' ? stored.query : '',
    tab: (VIEWS as readonly unknown[]).includes(stored.tab) ? (stored.tab as ViewId) : view.tab,
    kanban: {
      // groupByPhase, its name before sprints (SR-7.3), meant the same
      groupBySprint: bool(kanban.groupBySprint, bool(kanban.groupByPhase, view.kanban.groupBySprint)),
      showSkipped: bool(kanban.showSkipped, view.kanban.showSkipped),
      showAllDone: bool(kanban.showAllDone, view.kanban.showAllDone),
    },
    listSort: stored.listSort === 'priority' ? 'priority' : 'file',
    listGroupBy: (LIST_GROUP_BYS as readonly unknown[]).includes(stored.listGroupBy)
      ? (stored.listGroupBy as ListGroupBy)
      : view.listGroupBy,
    listShowClosed: stored.listShowClosed === true,
    // A focused phase (progressPhase, before sprints) is dropped: phases are no longer a unit of the Progress tab
    // Before version 2, all sprints was the default, not a choice: the active sprint now, as for new views
    progressSprint:
      typeof stored.progressSprint === 'string'
        ? stored.progressSprint
        : stored.version !== VIEW_VERSION
          ? view.progressSprint
          : null,
    // filterFinishedPhases, its name before sprints, meant the same
    filterClosedSprints: stored.filterClosedSprints === true || stored.filterFinishedPhases === true,
    // "phase", the range's name before sprints, is now "sprint"
    progressRange:
      stored.progressRange === 'phase'
        ? 'sprint'
        : (BURNDOWN_RANGES as readonly unknown[]).includes(stored.progressRange)
          ? (stored.progressRange as BurndownRange)
          : view.progressRange,
    graphFiltered: stored.graphFiltered === 'dim' ? 'dim' : 'hide',
    graphDimDone: stored.graphDimDone === true,
    graph: graphView(stored.graph),
    agents: agentsWindow(stored.agents),
    agentsLive: stored.agentsLive === true,
    activity: activityRange(stored.activity),
  };
}

export function writeView(key: string, view: SavedView): void {
  writeStored(key, view);
}

export function clearView(key: string): void {
  writeStored(key, null);
}

/**
 * Drop what the tasks no longer have: filter values for removed sprints, releases, components, types, statuses,
 * priorities or labels, a focused sprint that has no tasks.
 */
export function sanitizeView(
  view: SavedView,
  tasks: Task[],
  vocab: Vocabulary = DEFAULT_VOCABULARY,
  model: SprintModel = NO_SPRINTS,
): SavedView {
  const keep = (group: FilterGroup) => {
    const present = new Set(tasks.flatMap((t) => filterValues(group, t, vocab, model)));
    // The active sprint, whichever it is, stays selected while none is active: it applies again once one starts
    if (group === 'sprint') present.add(ACTIVE);
    return view.filters[group].filter((v) => present.has(v));
  };
  const sprints = new Set(tasks.map((t) => sprintKey(model, t)));
  return {
    ...view,
    filters: Object.fromEntries(FILTER_GROUPS.map((g) => [g, keep(g)])) as SavedView['filters'],
    progressSprint:
      view.progressSprint === ACTIVE || (view.progressSprint !== null && sprints.has(view.progressSprint))
        ? view.progressSprint
        : null,
  };
}
