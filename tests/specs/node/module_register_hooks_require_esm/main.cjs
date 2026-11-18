const { registerHooks } = require("node:module");
const mode = process.argv[2];

if (mode === "resolve" || mode === "resolve-reentrant") {
  registerHooks({
    resolve(specifier, context, nextResolve) {
      if (mode === "resolve-reentrant" && specifier === "../types/t.js") {
        console.log(require("./local.mjs").default);
      }
      return nextResolve(specifier, context);
    },
  });
} else {
  registerHooks({
    load(url, context, nextLoad) {
      if (
        mode === "replace" &&
        url.endsWith("/node_modules/esm-pkg/src/index.js")
      ) {
        return {
          format: "module",
          shortCircuit: true,
          source: 'export { default } from "../types/replacement.js";',
        };
      }
      if (mode === "replace-local" && url.endsWith("/local.mjs")) {
        return {
          format: "module",
          shortCircuit: true,
          source: 'export { default } from "./replacement.mjs";',
        };
      }
      return nextLoad(url, context);
    },
  });
}

console.log(
  require(mode === "replace-local" ? "./local.mjs" : "esm-pkg").default,
);
