<script lang="ts" module>
  /** The image types the viewer shows (atm-docs' `DOC_KINDS`): a binary file of one of these shows its images */
  const IMAGE = /\.(png|jpe?g|gif|webp|svg)$/i;

  /** Whether a review's binary file is an image it can show (ATM-689) */
  export function isImage(path: string): boolean {
    return IMAGE.test(path);
  }
</script>

<script lang="ts">
  import { imageSource } from '../api/images';

  // One side of a review's image file (ATM-689): the image as its revision has it, read through the files route (in the
  // app, its command, as a data: URL), under its label (Before, After); why not, when it can't be read
  let { url, label }: { url: string; label: string } = $props();

  let source = $state<string | null>(null);
  let failed = $state(false);
  $effect(() => {
    const asked = url;
    source = null;
    failed = false;
    void imageSource(asked).then((s) => {
      if (asked !== url) return;
      if (s) source = s;
      else failed = true;
    });
  });
</script>

<figure class="review-image" data-side={label.toLowerCase()}>
  <figcaption>{label}</figcaption>
  {#if source}
    <img src={source} alt="{label}: the image" onerror={() => (failed = true)} hidden={failed} />
  {/if}
  {#if failed}<p class="review-image-failed">This image can't be shown.</p>{/if}
</figure>

<style>
  .review-image {
    align-items: flex-start;
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    min-width: 0;
  }
  figcaption {
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
  }
  img {
    /* A checkerboard behind it, so a transparent image's edges show, as image viewers do */
    background: repeating-conic-gradient(var(--surface2) 0% 25%, var(--bg) 0% 50%) 50% / 16px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    max-height: 70vh;
    max-width: 100%;
    object-fit: contain;
  }
  .review-image-failed {
    color: var(--text-muted);
    font-size: var(--text-sm);
    margin: 0;
  }
</style>
