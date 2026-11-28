const { registerHooks } = require("node:module");

registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("/local.mjs")) {
      return {
        format: "module",
        shortCircuit: true,
        source: 'import "http://localhost:4545/welcome.ts";',
      };
    }
    return nextLoad(url, context);
  },
});

const entry = "./local.mjs";
require(entry);
