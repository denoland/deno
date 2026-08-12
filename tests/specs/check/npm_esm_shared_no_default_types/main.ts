// Regression test for https://github.com/denoland/deno/issues/35162.
//
// The npm package mirrors `playwright`: a single shared `.d.ts` (referenced by
// the `types` condition) that only re-exports named bindings with no `export
// default`, alongside an `import` condition pointing at an ESM `.js` file.
// Because the `.d.ts` has no real default export, `import pkg from "..."` must
// be typed via the CommonJS synthetic default (the module namespace), matching
// `tsc` under NodeNext and Deno <= 2.8.2. Treating the `.d.ts` as ESM instead
// would fail with `TS1192: Module ... has no default export`.
import playwright from "npm:@denotest/esm-package-no-default-export-types";

const result: string = playwright.firefox.launch();
console.log(result);
