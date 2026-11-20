<script lang="ts">
  // The + beside a line that starts a comment on it (ATM-650): the source view's (`DiffCode`) and the rendered page's
  // (`DocumentPage`), one button. Pressing it picks the line (Shift extends the pick); held while its comment is
  // written. Where it sits is its host's: `left` and `top` in the host's row, centred on `top`.
  let {
    line,
    held = false,
    left,
    top = '50%',
    onpress,
  }: {
    /** The line it comments on, for its label */
    line: number | string;
    /** Kept shown: its line is picked */
    held?: boolean;
    left: string;
    top?: string;
    onpress: (e: MouseEvent) => void;
  } = $props();
</script>

<button
  class="diff-plus"
  class:held
  tabindex="-1"
  aria-label="Comment on line {line}"
  style:left
  style:top
  onmousedown={onpress}>+</button
>

<style>
  .diff-plus {
    pointer-events: auto;
    position: absolute;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--space-7);
    height: var(--space-7);
    max-height: 100%;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--on-accent);
    font: inherit;
    font-weight: 700;
    font-size: var(--text-sm);
    line-height: 1;
    cursor: pointer;
    user-select: none;
  }
  .diff-plus:not(.held) {
    opacity: 0.85;
  }
</style>
