// Regression test: an npm package whose ESM entry (`lib/entry.mjs`)
// imports a deep CJS file in a sibling `dist/` directory, and that CJS
// file require()s another sibling CJS module. Before `--bundle` forced
// bundle-style resolver config, the deep CJS files were skipped from the
// module graph and the compile bundle tripped an esbuild parse error.
import fn from "@denotest/esm-import-deep-cjs";

console.log(fn());
