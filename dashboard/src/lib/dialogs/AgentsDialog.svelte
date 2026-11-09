<script lang="ts">
  // Connect your agents (T-458): in the atm app, a button per harness atm knows (Claude Code for now) that puts this
  // version's skills where it reads the user's, the same code as `atm agents install`, and says what it wrote where,
  // and what it left alone. It runs only when clicked: opening the dialog only reads the status. In the browser
  // (atm serve never writes into the user's folders) it gives the command to run instead.
  import { agentsStatus, connectAgents, connected, type Connected, type HarnessStatus } from '../api/agents';
  import { inApp } from '../api/http';
  import { dashboard } from '../state.svelte';
  import Dialog from '../ui/Dialog.svelte';

  const open = $derived(dashboard.dialog === 'agents');
  let harnesses = $state.raw<HarnessStatus[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let result = $state.raw<{ harness: string; connected: Connected } | null>(null);

  const message = (e: unknown) => (e && typeof e === 'object' && 'message' in e ? String(e.message) : String(e));

  async function refresh() {
    try {
      harnesses = await agentsStatus();
      error = null;
    } catch (e) {
      error = message(e);
    }
  }

  // Opening it reads the status; nothing is written until a click
  $effect(() => {
    if (open && inApp) void refresh();
  });

  async function connect(harness: string) {
    busy = harness;
    try {
      result = { harness, connected: await connectAgents(harness) };
      error = null;
      await refresh();
    } catch (e) {
      error = message(e);
    } finally {
      busy = null;
    }
  }

  const count = (status: HarnessStatus, state: string) => status.skills.filter((s) => s.state === state).length;
</script>

<Dialog id="agents" title="Connect your agents" {open} onclose={() => dashboard.closeDialog()}>
  {#if !inApp}
    <p class="agents-intro">
      Let your AI agents use atm: run <code>atm agents install</code> in a terminal. It puts atm's skills where your
      agent harness reads them (Claude Code: <code>~/.claude/skills</code>); <code>atm agents status</code> shows what is
      installed.
    </p>
  {:else}
    <p class="agents-intro">
      atm's skills let your AI agents use atm. Connecting a harness puts this version's skills where it reads yours, as
      <code>atm agents install</code> does; nothing happens until you click.
    </p>
    {#if error}<p class="agents-error" id="agents-error" role="alert">{error}</p>{/if}
    {#if harnesses}
      <ul class="agents-list" id="agents-list">
        {#each harnesses as status (status.harness)}
          <li class="agents-harness" data-harness={status.harness}>
            <div class="agents-harness-head">
              <span class="agents-harness-name">{status.label}</span>
              <span class="agents-harness-state" data-state={connected(status) ? 'connected' : 'not-connected'}>
                {#if connected(status)}Connected{:else if !status.detected}Not found on this Mac{:else}Not connected{/if}
              </span>
            </div>
            <p class="agents-harness-detail">
              {status.skills.length} skills in <code>{status.folder}</code>{#if count(status, 'stale')}, {count(
                  status,
                  'stale',
                )} from another version{/if}{#if count(status, "not atm's")}, {count(status, "not atm's")} not atm's{/if}
            </p>
            <button
              class="agents-connect"
              data-harness={status.harness}
              disabled={busy !== null}
              onclick={() => connect(status.harness)}
              >{busy === status.harness
                ? 'Connecting…'
                : connected(status)
                  ? `Connect ${status.label} again`
                  : `Connect ${status.label}`}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
    {#if result}
      <div class="agents-result" id="agents-result">
        <p>Wrote {result.connected.written.length} skill{result.connected.written.length === 1 ? '' : 's'}:</p>
        <ul>
          {#each result.connected.written as w (w.path)}
            <li><code>{w.path}</code> ({w.action}, {w.how === 'link' ? 'linked' : 'copied'})</li>
          {/each}
        </ul>
        {#if result.connected.refused.length}
          <p>Left alone (atm didn't put them there):</p>
          <ul class="agents-refused">
            {#each result.connected.refused as r (r.path)}<li><code>{r.path}</code>: {r.reason}</li>{/each}
          </ul>
        {/if}
      </div>
    {/if}
  {/if}
</Dialog>
