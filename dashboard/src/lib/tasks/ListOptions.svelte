<script lang="ts">
  // The List's options (T-323): its groups and its order. In the scope bar, or above the list without sprints
  import type { ListSort } from '../filters';
  import type { ListGroupBy } from '../list-groups';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  const groupBy = $derived(dashboard.listGroups);
  const severalSprints = $derived(dashboard.scope.kind === 'all' || dashboard.scope.kind === 'several');
  const anyLabel = $derived(dashboard.base.some((t) => Array.isArray(t.labels) && t.labels.length > 0));
  const anyComponent = $derived(dashboard.base.some((t) => Array.isArray(t.components) && t.components.length > 0));
</script>

<!-- Drawn by the page, not the platform (.select-box, app.css), so it looks the same everywhere (T-281) -->
<label class="kanban-option"
  >Group <span class="select-box"
    ><select
      id="list-group"
      value={groupBy}
      onchange={(e) => (dashboard.listGroupBy = e.currentTarget.value as ListGroupBy)}
    >
      {#if severalSprints}<option value="sprint">By sprint</option>{/if}
      {#if anyLabel}<option value="label">By label</option>{/if}
      {#if anyComponent}<option value="component">By component</option>{/if}
      <option value="none">None</option>
    </select><span class="select-chevron"><Icon name="chevron-down" size={12} /></span></span
  ></label
>
<label class="kanban-option"
  >Order <span class="select-box"
    ><select
      id="list-sort"
      value={dashboard.listSort}
      onchange={(e) => (dashboard.listSort = e.currentTarget.value as ListSort)}
    >
      <option value="file">As in the file</option>
      <option value="priority">By priority</option>
    </select><span class="select-chevron"><Icon name="chevron-down" size={12} /></span></span
  ></label
>
