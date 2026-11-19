<script lang="ts" module>
  // Lucide's icons (https://lucide.dev, ISC licence) from its package (T-432), as data: each icon is a list of SVG
  // elements with their attributes, imported by name so only these are bundled, and drawn here as elements, never as
  // markup. The dashboard makes no external requests. A new icon is one more import and entry here.
  //
  // `lucide` rather than its Svelte package: the same icons, but a component per icon made the page 14 kB larger
  // (5 kB gzipped); this way about 3 kB more than the embedded copies (2 kB gzipped).
  import {
    ArrowLeft,
    Bell,
    Bot,
    Calendar,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronsDownUp,
    ChevronsUpDown,
    CircleCheck,
    Code,
    Columns2,
    Copy,
    EllipsisVertical,
    ExternalLink,
    Eye,
    FileDiff,
    FileText,
    Folder,
    GitBranch,
    GitCommitHorizontal,
    Info,
    LayoutGrid,
    List,
    ListTodo,
    MessageSquare,
    Monitor,
    Moon,
    Network,
    PanelLeft,
    Palette,
    Paperclip,
    Pause,
    Pencil,
    Play,
    Plus,
    Radio,
    RefreshCw,
    Reply,
    RotateCcw,
    RotateCcwClock,
    Rows2,
    Search,
    Send,
    SlidersHorizontal,
    SquareKanban,
    Sun,
    Trash2,
    TrendingUp,
    User,
    WrapText,
    X,
    type IconNode,
  } from 'lucide';

  /** The dashboard's icons by the names it uses (Lucide's, but `history`, which Lucide now calls rotate-ccw-clock) */
  export const ICONS = {
    'arrow-left': ArrowLeft,
    bell: Bell,
    bot: Bot,
    calendar: Calendar,
    check: Check,
    'chevron-down': ChevronDown,
    'chevron-left': ChevronLeft,
    'chevron-right': ChevronRight,
    'chevron-up': ChevronUp,
    'chevrons-down-up': ChevronsDownUp,
    'chevrons-up-down': ChevronsUpDown,
    'circle-check': CircleCheck,
    code: Code,
    'columns-2': Columns2,
    copy: Copy,
    'ellipsis-vertical': EllipsisVertical,
    'external-link': ExternalLink,
    eye: Eye,
    'file-diff': FileDiff,
    'file-text': FileText,
    folder: Folder,
    'git-branch': GitBranch,
    'git-commit': GitCommitHorizontal,
    history: RotateCcwClock,
    info: Info,
    'layout-grid': LayoutGrid,
    list: List,
    'list-todo': ListTodo,
    'message-square': MessageSquare,
    monitor: Monitor,
    moon: Moon,
    network: Network,
    'panel-left': PanelLeft,
    palette: Palette,
    paperclip: Paperclip,
    pause: Pause,
    pencil: Pencil,
    play: Play,
    plus: Plus,
    radio: Radio,
    'refresh-cw': RefreshCw,
    reply: Reply,
    'rotate-ccw': RotateCcw,
    'rows-2': Rows2,
    search: Search,
    send: Send,
    'sliders-horizontal': SlidersHorizontal,
    'square-kanban': SquareKanban,
    sun: Sun,
    'trash-2': Trash2,
    'trending-up': TrendingUp,
    user: User,
    'wrap-text': WrapText,
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
