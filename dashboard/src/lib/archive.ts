// The archive notice (D-5.4): atm's own suggestion to archive (atm-cli's archive_hint, DM-5.3), which the API sends
// with the tasks (T-372), so the dashboard and the agents agree on when tasks.json has grown too big. The dashboard
// only words it.
import type { ArchiveHint } from '$generated/types';

function formatSize(bytes: number): string {
  return bytes >= 1_048_576 ? `${(bytes / 1_048_576).toFixed(1)} MB` : `${Math.floor(bytes / 1024)} KB`;
}

/** The footer's notice for atm's hint; null without one (tasks.json isn't over meta.archive's thresholds). */
export function archiveNotice(hint: ArchiveHint | null | undefined): string | null {
  if (!hint || !hint.archivable) return null;
  const n = hint.archivable;
  return `tasks.json is ${formatSize(hint.bytes)}: \`atm archive\` would move ${n} finished task${n === 1 ? '' : 's'} to tasks-archive.json`;
}
