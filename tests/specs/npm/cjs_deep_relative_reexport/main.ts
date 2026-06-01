// Regression test for https://github.com/denoland/deno/issues/29910.
// The package's main entry re-exports from a module two directories up
// using a forward-slash relative specifier (`require("../../types")`).
// On Windows this previously failed with `[ERR_MODULE_NOT_FOUND] Cannot
// find module 'types'` because joining the backslash base path with the
// forward-slash specifier confused the lexical path normalizer.
import {
  greet,
  IMAGE_TYPE,
} from "npm:@denotest/cjs-deep-relative-reexport@1.0.0";

console.log(greet());
console.log(IMAGE_TYPE);
