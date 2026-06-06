// Regression test for https://github.com/denoland/deno/issues/28071.
//
// The npm package mimics `@rollup/plugin-replace`: its `exports` map has a
// shared bare `types` condition that resolves to a `.d.ts` file using
// `export default function`, with an `import` condition pointing at an
// ESM-mode `.js` file. The default import should resolve to the function
// type, not the module namespace.
import replace from "npm:@denotest/esm-shared-default-types";

const result: string = replace({ delimiters: ["<", ">"] });
console.log(result);
