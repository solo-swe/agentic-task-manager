// Links to a task's commit and pull request, built from the project's repository address (meta.repository, set by
// atm init from origin or with `atm repository set`), never stored per task. GitHub, GitLab, Bitbucket and Gitea
// (Codeberg, Forgejo) are recognised by their host name; a self-hosted server whose name doesn't say gets its kind from
// meta.repository_type. Other hosts show plain text, and without an address a click explains how to set one.

import { safeUrl } from './url';

type RepoMeta = { repository?: unknown; repository_type?: unknown } | undefined;

/** Where a commit or PR reference goes: a link, plain text (a host we can't build links for), or a hint to set the
 * repository's address */
export type RepoRef = { kind: 'link'; href: string } | { kind: 'plain' } | { kind: 'hint' };

export const REPOSITORY_HINT =
  "Set the repository's address to link commits and pull requests: `atm repository set <url>` (or ask your agent).";

/** The repository's web address from meta, without a trailing slash; null when it isn't set */
export function repositoryOf(meta: RepoMeta): string | null {
  return safeUrl(meta?.repository)?.replace(/\/+$/, '') ?? null;
}

export type HostKind = 'github' | 'gitlab' | 'bitbucket' | 'gitea';
const KINDS: HostKind[] = ['github', 'gitlab', 'bitbucket', 'gitea'];

/** Commit, pull (merge) request and file paths, after the repository's address */
const PATHS: Record<HostKind, { commit: string; pull: string; file: string }> = {
  github: { commit: 'commit', pull: 'pull', file: 'blob' },
  gitlab: { commit: '-/commit', pull: '-/merge_requests', file: '-/blob' },
  bitbucket: { commit: 'commits', pull: 'pull-requests', file: 'src' },
  gitea: { commit: 'commit', pull: 'pulls', file: 'src/commit' },
};

/** The host's name as a person says it, for "Open on …" */
export const HOST_NAMES: Record<HostKind, string> = {
  github: 'GitHub',
  gitlab: 'GitLab',
  bitbucket: 'Bitbucket',
  gitea: 'Gitea',
};

/** A line or a range of lines, as each host's file view marks them */
function linesFragment(kind: HostKind, lines?: [number, number]): string {
  if (!lines) return '';
  const [start, end] = lines;
  if (kind === 'bitbucket') return start === end ? `#lines-${start}` : `#lines-${start}:${end}`;
  if (kind === 'gitlab') return start === end ? `#L${start}` : `#L${start}-${end}`;
  return start === end ? `#L${start}` : `#L${start}-L${end}`;
}

/** The host's kind from its name: the public services, and servers named after the software (gitlab.acme.com) */
function kindOfHost(host: string): HostKind | null {
  if (host === 'github.com' || host === 'www.github.com' || /^github\./.test(host)) return 'github';
  if (host === 'gitlab.com' || /^gitlab\./.test(host)) return 'gitlab';
  if (host === 'bitbucket.org' || /^bitbucket\./.test(host)) return 'bitbucket';
  if (host === 'codeberg.org' || /^(gitea|forgejo)\./.test(host)) return 'gitea';
  return null;
}

/** Link builders for a repository address, or null for a host we don't know. `type` (meta.repository_type) names
 * the kind of a self-hosted server and wins over the host name. */
export function repoLinks(
  repository: string,
  type?: unknown,
): {
  kind: HostKind;
  commit: (sha: string) => string;
  pull: (n: number) => string;
  file: (sha: string, path: string, lines?: [number, number]) => string;
} | null {
  let url: URL;
  try {
    url = new URL(repository);
  } catch {
    return null;
  }
  const segments = url.pathname.split('/').filter(Boolean);
  const hinted = typeof type === 'string' ? KINDS.find((k) => k === type.toLowerCase()) : undefined;
  const kind = hinted ?? kindOfHost(url.hostname.toLowerCase());
  // A repository is an owner and a name; GitLab nests groups, so it may have more
  if (!kind || segments.length < 2 || (kind !== 'gitlab' && segments.length !== 2)) return null;
  const paths = PATHS[kind];
  return {
    kind,
    commit: (sha) => `${repository}/${paths.commit}/${encodeURIComponent(sha)}`,
    pull: (n) => `${repository}/${paths.pull}/${n}`,
    file: (sha, path, lines) =>
      `${repository}/${paths.file}/${encodeURIComponent(sha)}/${path.split('/').map(encodeURIComponent).join('/')}` +
      linesFragment(kind, lines),
  };
}

/** Where a file of the project is in the repository's web view, at `commit`, with its lines marked; null when the
 * repository has no address or its host isn't one atm builds links for */
export function fileRef(
  meta: RepoMeta,
  commit: string,
  path: string,
  lines?: [number, number],
): { href: string; host: string } | null {
  const repository = repositoryOf(meta);
  const links = repository ? repoLinks(repository, meta?.repository_type) : null;
  return links ? { href: links.file(commit, path, lines), host: HOST_NAMES[links.kind] } : null;
}

function refFor(meta: RepoMeta, build: (links: NonNullable<ReturnType<typeof repoLinks>>) => string): RepoRef {
  const repository = repositoryOf(meta);
  if (!repository) return { kind: 'hint' };
  const links = repoLinks(repository, meta?.repository_type);
  return links ? { kind: 'link', href: build(links) } : { kind: 'plain' };
}

export function commitRef(meta: RepoMeta, sha: string): RepoRef {
  return refFor(meta, (links) => links.commit(sha));
}

/** A PR's link: the address atm recorded with it (`atm pr --url`), else one built from the repository */
export function pullRef(meta: RepoMeta, pr: { number: number; url?: string }): RepoRef {
  const recorded = safeUrl(pr.url);
  if (recorded) return { kind: 'link', href: recorded };
  return refFor(meta, (links) => links.pull(pr.number));
}
