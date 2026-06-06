// Negative test for the heuristic introduced in #28071.
//
// The npm package has an `import` condition in its `exports` map, which
// otherwise flips its `.d.ts` to ESM. But the `.d.ts` itself uses `export =`
// (TypeScript-only CJS syntax), so `compute_is_script` returns true and the
// `is_script == Some(true)` guard keeps the `.d.ts` classified as CJS.
import value from "npm:@denotest/esm-package-cjs-types";

const s: string = value.foo();
console.log(s);
