<script lang="ts">
  // A project's logo (p/<id>/logo, which api/projects lists), or its monogram: the initials on a colour picked from
  // its id. The box has its size from the start, so nothing moves while the image loads; an image that fails to load
  // gives way to the monogram. Decorative: the project's name is always next to it.
  import { inApp } from '../api/http';
  import { imageSource } from '../api/images';
  import { idHash, monogram } from '../projects';
  import { PHASE_PALETTE } from '../theme';

  let { project, size = 16 }: { project: { id: string; name: string; logo?: string | null }; size?: number } = $props();

  let failed = $state<string | null>(null);
  // In the atm app the logo comes through its commands, as a blob URL (api/images.ts); in the browser, as it is
  let loaded = $state<{ logo: string; source: string | null } | null>(null);
  $effect(() => {
    const logo = project.logo;
    if (!logo || !inApp) return;
    void imageSource(logo).then((source) => (loaded = { logo, source }));
  });
  const source = $derived(
    !project.logo ? null : !inApp ? project.logo : loaded?.logo === project.logo ? loaded.source : null,
  );
  const src = $derived(failed !== project.logo ? source : null);
  const color = $derived(PHASE_PALETTE[idHash(project.id) % PHASE_PALETTE.length]);
</script>

<span
  class="project-logo"
  class:monogram={!src}
  style:width="{size}px"
  style:height="{size}px"
  style:--monogram-color={color}
  style:font-size="{Math.round(size * 0.45)}px"
  data-initials={src ? null : monogram(project.name)}
  aria-hidden="true"
>
  <!-- The initials are drawn by CSS (::before), so they stay out of the text of the option or card they sit in -->
  {#if src}
    <img {src} alt="" width={size} height={size} onerror={() => (failed = project.logo ?? null)} />
  {/if}
</span>
