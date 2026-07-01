// `loose` is a named export of the inner module but is NOT a
// property of the member the wrapper re-exports. The entry module
// re-exports the wrapper, so `loose` must not be advertised through
// the recursive re-export chain either.
import { loose } from "npm:@denotest/cjs-module-exports-require-member-nested";

console.log(loose);
