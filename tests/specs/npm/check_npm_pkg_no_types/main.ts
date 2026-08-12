// Regression test for https://github.com/denoland/deno/issues/23507
//
// An npm package that ships no type declarations (no "types"/"typings" entry,
// no adjacent declaration files and no corresponding `@types` package) should
// be treated as untyped when type checking instead of producing a hard
// "Failed resolving types" error. This previously failed when using a "bring
// your own node_modules" (byonm) setup.
import * as multipleExports from "npm:@denotest/cjs-multiple-exports@1.0.0";
import noTypesCjs from "npm:@denotest/no-types-cjs@1.0.0";

console.log(multipleExports);
console.log(noTypesCjs());
