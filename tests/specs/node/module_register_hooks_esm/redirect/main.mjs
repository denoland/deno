import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

// A non-file:/non-node: redirect target so the JS end-of-chain cannot read
// source synchronously and returns { source: null }. This forces the load
// loop to fall through to Rust default loading with the redirect URL, which
// is the path this test exercises: Rust fetches source from the redirect
// target while the module keeps "original.mjs" as its identity.
const redirected =
  "data:text/javascript,export%20const%20value%20%3D%20%22REDIRECTED%22%3B";

// A load hook that delegates source loading for "original.mjs" to a different
// URL via nextLoad(newUrl), then falls through to default loading. The source
// must come from the redirect target while the module keeps the original
// specifier as its identity (matching Node's hook semantics).
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
