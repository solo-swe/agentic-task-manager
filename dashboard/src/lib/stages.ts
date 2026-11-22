// Stages of Done (DM-4.3, dashboard D-4.2): a done task is Done (not in a PR yet), In PR (`pr`, not merged) or Merged
// (`merged_at`). Stages are derived, never statuses: they don't affect dependencies or the columns.
import type { KanbanCard } from './kanban';
import type { Task } from './types';
import { safeUrl } from './url';

export type Stage = 'done' | 'in_pr' | 'merged';
export const STAGES: Stage[] = ['done', 'in_pr', 'merged'];
export const STAGE_LABELS: Record<Stage, string> = { done: 'Done', in_pr: 'In PR', merged: 'Merged' };

/** The task's PR, if it records a usable one */
export function prOf(task: Task): { number: number; url?: string } | undefined {
  const pr = task.pr;
  return pr && typeof pr === 'object' && typeof pr.number === 'number' ? pr : undefined;
}

/** Whether the task records a stage beyond Done: a PR or a merge */
export function hasStage(task: Task): boolean {
  return !!prOf(task) || !!task.merged_at;
}

/** A done task's stage, as atm says it (`derived.stage`, ATM-719: every task the dashboard has comes with atm's
 * values, archived ones and a worktree's copies too); undefined for a task that isn't done */
export function stageOf(task: Task): Stage | undefined {
  const served = task.derived?.stage;
  return served && (STAGES as string[]).includes(served) ? (served as Stage) : undefined;
}

export interface DoneSection {
  /** "in_pr:12", "done" or "merged" */
  id: string;
  stage: Stage;
  label: string;
  /** The PR's link, for an In PR section */
  url?: string;
  cards: KanbanCard[];
}

/**
 * The Done column of a pull-request project, in sections: In PR (one per PR, the newest PR first), Done (not pushed
 * yet), Merged. Empty sections are left out; cards keep the column's order within each section.
 */
export function doneSections(cards: KanbanCard[]): DoneSection[] {
  const prs = new Map<number, DoneSection>();
  const done: KanbanCard[] = [];
  const merged: KanbanCard[] = [];
  for (const card of cards) {
    const stage = stageOf(card.task) ?? 'done';
    if (stage === 'merged') merged.push(card);
    else if (stage === 'in_pr') {
      const pr = prOf(card.task)!;
      if (!prs.has(pr.number))
        prs.set(pr.number, {
          id: `in_pr:${pr.number}`,
          stage,
          label: `In PR #${pr.number}`,
          url: safeUrl(pr.url) ?? undefined,
          cards: [],
        });
      prs.get(pr.number)!.cards.push(card);
    } else done.push(card);
  }
  const sections = [...prs.values()].sort((a, b) => Number(b.id.slice(6)) - Number(a.id.slice(6)));
  if (done.length) sections.push({ id: 'done', stage: 'done', label: 'Done', cards: done });
  if (merged.length) sections.push({ id: 'merged', stage: 'merged', label: 'Merged', cards: merged });
  return sections;
}
