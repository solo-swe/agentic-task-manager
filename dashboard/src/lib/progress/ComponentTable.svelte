<script lang="ts">
  import { componentProgress } from '../component-progress';
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';

  // The Progress tab's "By component" table (T-323): the tasks the tab shows (the focused sprint's, or all), by
  // component. A row filters by its component: the board views and this tab then show only its tasks; again to undo.
  let { tasks, scope }: { tasks: readonly Task[]; scope: string } = $props();

  const rows = $derived(componentProgress(tasks, dashboard.components, dashboard.vocab));
  const selected = $derived(dashboard.filters.component);
  const pct = (r: { done: number; total: number; skipped: number }) =>
    r.total ? Math.round((r.done / r.total) * 100) : r.skipped ? 100 : 0;

  function select(id: string) {
    if (id) dashboard.toggleFilter('component', id);
  }
  function onKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      select(id);
    }
  }
</script>

{#if dashboard.components.length}
  <section class="component-progress" id="component-progress" aria-label="By component">
    <h3 class="burndown-title">By component · <span class="burndown-focus">{scope}</span></h3>
    <p class="progress-basis">
      A task in several components counts in each. <span class="phase-table-hint"
        >Click a component to filter by it.</span
      >
    </p>
    <table class="phase-table component-table" id="component-table">
      <thead>
        <tr>
          <th>Component</th>
          <th>Done</th>
          <th class="phase-table-progress">Progress</th>
          <th>Remaining</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as row (row.id)}
          <tr
            data-component={row.id}
            class:selected={selected.has(row.id)}
            class:selectable={row.id !== ''}
            tabindex={row.id ? 0 : undefined}
            aria-selected={row.id ? selected.has(row.id) : undefined}
            onclick={() => select(row.id)}
            onkeydown={(e) => onKeydown(e, row.id)}
          >
            <td class="phase-cell"
              >{#if row.id}<span class="badge component-chip">{row.label}</span>{:else}<span class="phase-name"
                  >{row.label}</span
                >{/if}</td
            >
            <td class="num"
              >{row.done}/{row.total}{#if row.skipped}<span class="skipped-note">&nbsp;· {row.skipped} skipped</span
                >{/if}</td
            >
            <td class="phase-table-progress">
              <div class="progress-bar">
                <div class="progress-fill" style:width="{pct(row)}%"></div>
              </div>
            </td>
            <td class="num">
              {row.open}{#if row.blocked}<span class="phase-blocked">{row.blocked} blocked</span
                >{/if}{#if row.inReview}<span class="phase-blocked phase-in-review">{row.inReview} in review</span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}
