<script lang="ts">
  import { dashboard } from '../state.svelte';
</script>

<!-- Shown instead of the views when the project list or tasks.json could not be loaded at all -->
{#if !dashboard.loaded && dashboard.loadError !== null}
  <div
    id="load-error"
    role="alert"
    style="background:var(--surface);border:1px solid var(--red);border-radius:var(--radius-lg);padding:var(--space-9);color:var(--red)"
  >
    <strong>Failed to load tasks.json</strong><br />
    <span style="color:var(--text-muted)"
      >{dashboard.loadError}<br /><br />The dashboard is served by atm. Start it from a project with
      <code style="color:var(--text)">/atm-serve</code>
      (or <code style="color:var(--text)">atm serve --open</code>), and open the address it prints.</span
    >
  </div>
{/if}
