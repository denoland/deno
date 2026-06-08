import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

const redirected = new URL("./redirected.mjs", import.meta.url).href;

// A load hook that delegates source loading for "original.mjs" to a different
// real file via nextLoad(newUrl), then falls through to default loading. The
// source must come from the redirect target while the module keeps the
// original specifier as its identity (matching Node's hook semantics).
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("original.mjs")) {
      return nextLoad(redirected, context);
    }
    return nextLoad(url, context);
  },
});

const mod = await import("./original.mjs");
console.log("value:", mod.value);

hook.deregister();
