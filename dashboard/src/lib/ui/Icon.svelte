<script lang="ts" module>
  // Lucide's icons (https://lucide.dev, ISC licence) from its package (T-432), as data: each icon is a list of SVG
  // elements with their attributes, imported by name so only these are bundled, and drawn here as elements, never as
  // markup. The dashboard makes no external requests. A new icon is one more import and entry here.
  //
  // `lucide` rather than its Svelte package: the same icons, but a component per icon made the page 14 kB larger
  // (5 kB gzipped); this way about 3 kB more than the embedded copies (2 kB gzipped).
  import {
    Bot,
    Calendar,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Copy,
    EllipsisVertical,
    ExternalLink,
    FileText,
    Folder,
    GitBranch,
    Info,
    LayoutGrid,
    List,
    ListTodo,
    MessageSquare,
    Monitor,
    Moon,
    Network,
    Palette,
    Paperclip,
    Pause,
    Play,
    Radio,
    RefreshCw,
    RotateCcwClock,
    Search,
    SlidersHorizontal,
    SquareKanban,
    Sun,
    TrendingUp,
    User,
    X,
    type IconNode,
  } from 'lucide';

  /** The dashboard's icons by the names it uses (Lucide's, but `history`, which Lucide now calls rotate-ccw-clock) */
  export const ICONS = {
    bot: Bot,
    calendar: Calendar,
    check: Check,
    'chevron-down': ChevronDown,
    'chevron-left': ChevronLeft,
    'chevron-right': ChevronRight,
    copy: Copy,
    'ellipsis-vertical': EllipsisVertical,
    'external-link': ExternalLink,
    'file-text': FileText,
    folder: Folder,
    'git-branch': GitBranch,
    history: RotateCcwClock,
    info: Info,
    'layout-grid': LayoutGrid,
    list: List,
    'list-todo': ListTodo,
    'message-square': MessageSquare,
    monitor: Monitor,
    moon: Moon,
    network: Network,
    palette: Palette,
    paperclip: Paperclip,
    pause: Pause,
    play: Play,
    radio: Radio,
    'refresh-cw': RefreshCw,
    search: Search,
    'sliders-horizontal': SlidersHorizontal,
    'square-kanban': SquareKanban,
    sun: Sun,
    'trending-up': TrendingUp,
    user: User,
    x: X,
  } satisfies Record<string, IconNode>;

  export type IconName = keyof typeof ICONS;

  /** Icons drawn filled, as the embedded copies were */
  const FILLED: ReadonlySet<IconName> = new Set(['play']);
</script>

<script lang="ts">
  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<svg
  class="icon icon-{name}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={FILLED.has(name) ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each ICONS[name] as [tag, attrs], i (i)}
    <svelte:element this={tag} {...attrs} />
  {/each}
</svg>
