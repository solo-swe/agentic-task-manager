declare module 'virtual:atm-changelog' {
  const entries: import('./lib/changelog').ChangelogEntry[];
  export default entries;
}

declare module 'virtual:atm-help' {
  const blocks: import('./lib/markdown').Block[];
  export default blocks;
}

declare module 'virtual:atm-compmd' {
  /** The CompMD version the build ships in plugin/assets/compmd/<version>/ */
  export const version: string;
}

declare module 'virtual:atm-plugin-info' {
  const info: import('./lib/plugin-info').PluginInfo;
  export default info;
}
