// Regression test for #25311: when a CJS main file is
//   module.exports = require("./inner").IDENT;
// the static CJS analyzer fails to surface the inner module's named
// exports, so `import { gql }` would throw "does not provide an export
// named 'gql'". The fallback in libs/resolver/cjs/analyzer detects this
// pattern and surfaces the inner specifier as a re-export.
import {
  disableFragmentWarnings,
  gql,
  resetCaches,
} from "npm:@denotest/cjs-module-exports-require-member";

console.log(gql());
console.log(resetCaches());
console.log(disableFragmentWarnings());
