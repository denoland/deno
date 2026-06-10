import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "virtual:greet") {
      return { url: "file:///virtual_greet.js", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "file:///virtual_greet.js") {
      return {
        source: 'export const msg = "hello from hooks";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const { msg } = await import("virtual:greet");
console.log(msg);

hook.deregister();
console.log("done");
