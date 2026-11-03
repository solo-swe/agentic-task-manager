// The CompMD runtime the details panel loads from atm serve's asset folder (docs/compmd-rendering-design.md §2, §3c),
// built by vite.compmd.config.ts into plugin/assets/compmd/<version>/: one module, with the processor's WebAssembly
// beside it, and @compmd/vite's `request` (the packs compmd.config.json lists, ambient, with their policy) and
// `sandbox` (where its frames and the packs' built modules are, with their hashes). Only what src/lib/compmd.ts uses
// (CompmdModule there).
export { init, OpenDocument } from '@compmd/processor';
export { createSandbox } from '@compmd/sandbox';
export { request, sandbox } from 'virtual:compmd';
