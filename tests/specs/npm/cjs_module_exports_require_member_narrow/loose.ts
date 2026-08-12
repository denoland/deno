// `loose` is a named export of the inner module but is NOT a
// property of the member that the wrapper re-exports. The wrapper
// must not advertise it, so `deno check` should fail here.
import { loose } from "npm:@denotest/cjs-module-exports-require-member-narrow";

console.log(loose);
