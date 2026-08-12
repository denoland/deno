// Regression test for https://github.com/denoland/deno/issues/35697
// The deferred subgraph contains an async (TLA) module whose resume
// microtask is queued while the graph is still evaluating. The sibling
// CJS module then require()s an ES module, which used to drain
// microtasks mid-evaluation and permanently stall the graph's
// evaluation promise.
import defer * as a from "./a.ts";
import "./b.cjs";
console.log("Hello World");
a;
