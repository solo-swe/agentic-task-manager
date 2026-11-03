declare module 'virtual:atm-changelog' {
  const entries: import('./lib/changelog').ChangelogEntry[];
  export default entries;
}

declare module 'virtual:atm-help' {
  /** docs/help.md, as written */
  const text: string;
  export default text;
}

declare module 'virtual:atm-compmd' {
  /** The CompMD version the build ships in plugin/assets/compmd/<version>/ */
  export const version: string;
}

declare module 'virtual:atm-plugin-info' {
  const info: import('./lib/plugin-info').PluginInfo;
  export default info;
}
