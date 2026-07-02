import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "virtual:worker") {
      return { url: "file:///virtual_worker.js", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "file:///virtual_worker.js") {
      return {
        source: 'export const msg = "hooks-in-worker";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const { msg } = await import("virtual:worker");
self.postMessage(msg);
