// The CompMD runtime the details panel loads from atm serve's asset folder (docs/compmd-rendering-design.md §2), built
// by vite.compmd.config.ts into plugin/assets/compmd/<version>/: one module, with the processor's WebAssembly beside
// it. Only what src/lib/compmd.ts uses (CompmdModule there).
export { init, process } from '@compmd/processor';
export { createSandbox } from '@compmd/sandbox';
