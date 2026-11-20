import { describe, expect, it } from 'vitest';
import { commitRef, pullRef, repoLinks, repositoryOf, submoduleCommitUrl } from '../../src/lib/repo';

describe('submoduleCommitUrl (ATM-676, audit D3)', () => {
  it('links a submodule commit as its host does, without a trailing .git', () => {
    for (const [repository, url] of [
      ['https://github.com/acme/lib', 'https://github.com/acme/lib/commit/abc'],
      ['https://github.com/acme/lib.git', 'https://github.com/acme/lib/commit/abc'],
      ['https://github.com/acme/lib/', 'https://github.com/acme/lib/commit/abc'],
      ['https://gitlab.com/acme/group/lib.git', 'https://gitlab.com/acme/group/lib/-/commit/abc'],
      ['https://codeberg.org/acme/lib', 'https://codeberg.org/acme/lib/commit/abc'],
    ]) {
      expect(submoduleCommitUrl(repository, 'abc'), repository).toBe(url);
    }
  });

  it('no link for an address that isn’t http(s), a host it builds no links for, or none', () => {
    for (const repository of [
      'javascript:alert(1)//github.com/a/b',
      'data:text/html,x',
      'git@github.com:acme/lib.git',
      '../lib',
      'https://example.com/acme/lib',
      '',
      null,
      undefined,
    ]) {
      expect(submoduleCommitUrl(repository, 'abc'), String(repository)).toBeNull();
    }
  });
});

const github = { repository: 'https://github.com/acme/demo' };

describe('repositoryOf', () => {
  it('an http(s) address, without a trailing slash; anything else is no address', () => {
    expect(repositoryOf({ repository: 'https://github.com/acme/demo/' })).toBe('https://github.com/acme/demo');
    expect(repositoryOf({ repository: 'git@github.com:acme/demo.git' })).toBeNull();
    expect(repositoryOf({ repository: 42 })).toBeNull();
    expect(repositoryOf(undefined)).toBeNull();
  });
});

describe('repoLinks', () => {
  it('GitHub: commits and pull requests', () => {
    const links = repoLinks('https://github.com/acme/demo')!;
    expect(links.commit('abc123')).toBe('https://github.com/acme/demo/commit/abc123');
    expect(links.pull(12)).toBe('https://github.com/acme/demo/pull/12');
  });

  it('GitLab: commits and merge requests, also in nested groups', () => {
    const links = repoLinks('https://gitlab.com/acme/demo')!;
    expect(links.commit('abc123')).toBe('https://gitlab.com/acme/demo/-/commit/abc123');
    expect(links.pull(12)).toBe('https://gitlab.com/acme/demo/-/merge_requests/12');
    expect(repoLinks('https://gitlab.com/acme/team/demo')!.pull(3)).toBe(
      'https://gitlab.com/acme/team/demo/-/merge_requests/3',
    );
  });

  it('Bitbucket: commits and pull requests', () => {
    const links = repoLinks('https://bitbucket.org/acme/demo')!;
    expect(links.commit('abc123')).toBe('https://bitbucket.org/acme/demo/commits/abc123');
    expect(links.pull(12)).toBe('https://bitbucket.org/acme/demo/pull-requests/12');
  });

  it('Gitea, Forgejo and Codeberg: commits and pulls', () => {
    const links = repoLinks('https://codeberg.org/acme/demo')!;
    expect(links.commit('abc123')).toBe('https://codeberg.org/acme/demo/commit/abc123');
    expect(links.pull(12)).toBe('https://codeberg.org/acme/demo/pulls/12');
    expect(repoLinks('https://forgejo.acme.dev/acme/demo')!.pull(1)).toBe('https://forgejo.acme.dev/acme/demo/pulls/1');
  });

  it('self-hosted servers named after the software are recognised', () => {
    expect(repoLinks('https://gitlab.acme.com/acme/demo')!.pull(4)).toBe(
      'https://gitlab.acme.com/acme/demo/-/merge_requests/4',
    );
    expect(repoLinks('https://github.acme.com/acme/demo')!.pull(4)).toBe('https://github.acme.com/acme/demo/pull/4');
  });

  it('meta.repository_type names the kind of any other server, and wins over the host name', () => {
    expect(repoLinks('https://git.acme.com/acme/demo')).toBeNull();
    expect(repoLinks('https://git.acme.com/acme/demo', 'gitlab')!.commit('abc')).toBe(
      'https://git.acme.com/acme/demo/-/commit/abc',
    );
    expect(repoLinks('https://git.acme.com/acme/demo', 'Bitbucket')!.pull(2)).toBe(
      'https://git.acme.com/acme/demo/pull-requests/2',
    );
    expect(repoLinks('https://git.acme.com/acme/demo', 'svn')).toBeNull();
  });

  it('an address that is not a repository gets no links', () => {
    expect(repoLinks('https://github.com/acme')).toBeNull();
    expect(repoLinks('https://github.com/acme/demo/tree/main')).toBeNull();
    expect(repoLinks('https://bitbucket.org/acme')).toBeNull();
    expect(repoLinks('https://example.com/acme/demo')).toBeNull();
    expect(repoLinks('not a url')).toBeNull();
  });
});

describe('commitRef and pullRef', () => {
  it('a link with a known host, plain text with another, a hint without an address', () => {
    expect(commitRef(github, 'abc')).toEqual({ kind: 'link', href: 'https://github.com/acme/demo/commit/abc' });
    expect(commitRef({ repository: 'https://gitlab.com/acme/demo' }, 'abc')).toEqual({
      kind: 'link',
      href: 'https://gitlab.com/acme/demo/-/commit/abc',
    });
    expect(commitRef({ repository: 'https://example.com/acme/demo' }, 'abc')).toEqual({ kind: 'plain' });
    expect(commitRef(undefined, 'abc')).toEqual({ kind: 'hint' });
  });

  it('meta.repository_type is used for a self-hosted server', () => {
    const meta = { repository: 'https://git.acme.com/acme/demo', repository_type: 'gitea' };
    expect(pullRef(meta, { number: 9 })).toEqual({ kind: 'link', href: 'https://git.acme.com/acme/demo/pulls/9' });
  });

  it('a PR links to the address atm recorded with it, else one built from the repository', () => {
    expect(pullRef(undefined, { number: 5, url: 'https://github.com/acme/demo/pull/5' })).toEqual({
      kind: 'link',
      href: 'https://github.com/acme/demo/pull/5',
    });
    expect(pullRef(github, { number: 7 })).toEqual({ kind: 'link', href: 'https://github.com/acme/demo/pull/7' });
    expect(pullRef(undefined, { number: 7 })).toEqual({ kind: 'hint' });
  });
});
