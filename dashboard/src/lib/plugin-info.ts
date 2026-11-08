import info from 'virtual:atm-plugin-info';

/** atm's name, author, licence and links, from Cargo.toml at build time (see build/plugin-info.ts). */
export interface PluginInfo {
  /** The short name, `atm` */
  name: string;
  /** "Agentic Task Manager" */
  fullName: string;
  description: string;
  version: string;
  author: string;
  license: string;
  repository: string;
  /** Copyright year: the year of the newest release */
  year: string;
}

export const pluginInfo: PluginInfo = info;

/** "© 2026 Hakan Serce · MIT License" */
export function copyright({ year, author, license }: PluginInfo): string {
  return `© ${year} ${author} · ${license} License`;
}
