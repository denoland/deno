import { register } from "node:module";

// Regression test for #34004 — `vite build` failed with
// `Unsupported scheme "node" for module "node:path"` because rollup's
// `node-entry.js` does a static `import { ... } from 'node:path'` and the
// runtime routed that load through the user `module.register()` hook
// bridge instead of the runtime's built-in module map.
register("./hooks-basic.mjs", import.meta.url);

// Dynamic import of a module that statically re-imports a node: builtin.
const { joinName } = await import("./uses_node_path.mjs");
console.log(joinName);
