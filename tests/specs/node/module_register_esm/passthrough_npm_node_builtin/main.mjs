import { register } from "node:module";

// Regression test: with `resolve_active` flipped on (a passthrough hook),
// bare specifiers inside an npm package that resolve to a node: builtin
// (here `import m2 from "module"` and `await import("module")` ->
// `node:module`) must still be served from the built-in module map.
// Previously the async-resolve future would short-circuit to
// `take_lazy_esm_source` after the lazy source had been consumed, then fall
// through to the file loader and surface as "Unsupported scheme \"node\" for
// module \"node:module\"".
register("../hooks-passthrough.mjs", import.meta.url);

await import("npm:@denotest/builtin-module-module");
console.log("done");
