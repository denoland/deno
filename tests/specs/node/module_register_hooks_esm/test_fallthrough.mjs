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

// This should fall through to default resolution
const { helper } = await import("./helper.mjs");
console.log("real:", helper);

hook.deregister();
console.log("fallthrough works");
