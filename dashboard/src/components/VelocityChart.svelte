<script lang="ts">
  import { formatWhen } from '../lib/format';
  import { dashboard } from '../lib/state.svelte';
  import { velocity } from '../lib/velocity';

  // Velocity (T-323): tasks done per closed sprint, the last ten in order, with the average as a dashed line. One
  // series, in the accent colour; each bar names its sprint, and its tooltip what was carried over and added. The
  // sprint table above is the table view.
  const data = $derived(velocity(dashboard.sprints, dashboard.history, dashboard.vocab));
  const max = $derived(Math.max(1, ...data.sprints.map((s) => s.done)));
  const height = (n: number) => `${(n / max) * 100}%`;
  const tip = (s: (typeof data.sprints)[number]) =>
    [
      `${s.id}: ${s.done} done`,
      s.carried ? `${s.carried} carried over` : '',
      s.added ? `${s.added} added mid-sprint` : '',
      s.end ? `closed ${formatWhen(Date.parse(s.end), dashboard.loadedAt)}` : '',
    ]
      .filter(Boolean)
      .join(' · ');
  const average = $derived(Math.round(data.average * 10) / 10);
</script>

{#if data.sprints.length >= 2}
  <section class="velocity" id="velocity" aria-label="Velocity">
    <h3 class="burndown-title">
      Velocity <span class="velocity-sub"
        >tasks done per sprint, the last {data.sprints.length} closed · average {average}</span
      >
    </h3>
    <div
      class="velocity-plot"
      role="img"
      aria-label="Tasks done per sprint: {data.sprints.map((s) => `${s.id} ${s.done}`).join(', ')}; average {average}"
    >
      <div class="velocity-average" style:bottom={height(data.average)} aria-hidden="true">
        <span>avg {average}</span>
      </div>
      {#each data.sprints as s (s.id)}
        <div class="velocity-column" data-sprint={s.id} title={tip(s)}>
          <span class="velocity-bar" style:height={height(s.done)}></span>
          <span class="velocity-value" style:bottom={height(s.done)}>{s.done}</span>
        </div>
      {/each}
    </div>
    <div class="velocity-names" aria-hidden="true">
      {#each data.sprints as s (s.id)}<span title={s.id}>{s.id}</span>{/each}
    </div>
  </section>
{/if}
