import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

// Static import so target.mjs is embedded in the compiled binary's VFS. After
// compilation the on-disk copy is removed, so at runtime the only source for
// this module lives in the VFS, reachable only through the Rust default
// loader (not the JS end-of-chain's synchronous real-disk read).
import "./target.mjs";

// A load hook that delegates source loading for "original.mjs" to the embedded
// target via nextLoad(newUrl), then falls through to default loading. In a
// compiled binary the JS end-of-chain cannot read the (now deleted) target
// from disk and returns { source: null }, so the load loop falls through to
// the Rust default loader with the redirect URL. Rust serves the source from
// the VFS while the module keeps "original.mjs" as its identity.
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("original.mjs")) {
      return nextLoad(url.replace("original.mjs", "target.mjs"), context);
    }
    return nextLoad(url, context);
  },
});

// Computed specifier so `deno compile` cannot statically embed original.mjs;
// its load is serviced entirely through the hook chain at runtime.
const spec = "./ori" + "ginal.mjs";
const mod = await import(spec);
console.log("value:", mod.value);

hook.deregister();
