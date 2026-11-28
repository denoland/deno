const { registerHooks } = require("node:module");

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "./t.js" && context.importAttributes.type === "text") {
      return { url: "http://localhost:4545/welcome.ts", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url.endsWith("/node_modules/esm-pkg/src/index.js")) {
      const nested = "./node_modules/esm-pkg/types/raw-import.mjs";
      console.log(require(nested).default);
    }
    return nextLoad(url, context);
  },
});

console.log(require("esm-pkg").default);
