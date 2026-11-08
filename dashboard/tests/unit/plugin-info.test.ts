// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { readPluginInfo } from '../../build/plugin-info.ts';
import { CARGO_TOML, CHANGELOG, readVersion } from '../../build/workspace.ts';
import { copyright } from '../../src/lib/plugin-info.ts';

const cargoToml = `[workspace]
members = ["crates/*"]

[workspace.package]
version = "1.2.0"
description = "Tasks for agents."
authors = ["Ada Lovelace <ada@example.com>"]
edition = "2024"
license = "MIT"
repository = "https://github.com/example/atm"
publish = false

[workspace.dependencies]
version = "not this one"
`;
const changelog =
  '# Changelog\n\n## [Unreleased]\n\n## [1.2.0] - 2027-01-05\n### Added\n- x\n\n## [1.1.0] - 2026-12-01\n';

describe('readPluginInfo', () => {
  it("takes the version, author, licence and links from Cargo.toml's [workspace.package]", () => {
    expect(readPluginInfo(cargoToml, changelog)).toEqual({
      name: 'atm',
      fullName: 'Agentic Task Manager',
      description: 'Tasks for agents.',
      version: '1.2.0',
      author: 'Ada Lovelace',
      license: 'MIT',
      repository: 'https://github.com/example/atm',
      year: '2027',
    });
  });

  it('takes the year from the newest dated release, skipping Unreleased', () => {
    expect(readPluginInfo(cargoToml, changelog).year).toBe('2027');
  });

  it('reads a Cargo.toml with CRLF line ends (a Windows checkout)', () => {
    expect(readPluginInfo(cargoToml.replace(/\n/g, '\r\n'), changelog).version).toBe('1.2.0');
  });

  it('throws without a dated release, or without a field it shows', () => {
    expect(() => readPluginInfo(cargoToml, '## [Unreleased]\n')).toThrow(/No dated release/);
    expect(() => readPluginInfo(cargoToml.replace(/^license.*\n/m, ''), changelog)).toThrow(/has no license/);
    expect(() => readPluginInfo(cargoToml.replace(/^authors.*\n/m, ''), changelog)).toThrow(/has no authors/);
  });

  it('reads the real Cargo.toml and changelog', () => {
    const info = readPluginInfo(readFileSync(CARGO_TOML, 'utf8'), readFileSync(CHANGELOG, 'utf8'));
    expect(info.version).toBe(readVersion());
    expect(info.author).toBe('Hakan Serce');
    expect(info.repository).toBe('https://github.com/solo-swe/agentic-task-manager');
    expect(info.year).toMatch(/^\d{4}$/);
  });
});

describe('copyright', () => {
  it('reads "© year author · licence License"', () => {
    expect(copyright(readPluginInfo(cargoToml, changelog))).toBe('© 2027 Ada Lovelace · MIT License');
  });
});
