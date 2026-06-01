// Regression test for #16708: graphql-tag@2's main entry is
//   module.exports = require("./lib/graphql-tag.umd.js").gql;
// where the inner module is a UMD bundle that attaches its named
// exports inside the factory IIFE through a namespace alias. The
// member's attached names can't be narrowed statically, so the
// analyzer falls back to re-exporting the inner module wholesale and
// `import { gql }` resolves (matching Node).
import {
  disableFragmentWarnings,
  gql,
  resetCaches,
} from "npm:@denotest/cjs-module-exports-require-member-umd";

console.log(gql());
console.log(resetCaches());
console.log(disableFragmentWarnings());
