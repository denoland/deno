import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

// Hook that only intercepts "virtual:*", everything else falls through
const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "virtual:test") {
      return { url: "file:///virtual_test.js", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "file:///virtual_test.js") {
      return {
        source: "export const x = 42;",
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

// This should use the hook
const { x } = await import("virtual:test");
console.log("virtual:", x);

// Dynamic import with a non-static specifier that is NOT in the initial
// module graph. This exercises prepare_load for fallthrough imports: hooks
// are active but don't intercept this specifier, so prepare_load must still
// build the graph for it.
const name = "helper2";
const { value } = await import(`./${name}.mjs`);
console.log("dynamic fallthrough:", value);

hook.deregister();
console.log("done");
