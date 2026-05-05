import { register, registerHooks } from "node:module";

// Register async hook FIRST
register("./hooks-ordering-async.mjs", import.meta.url);

// Allow hook module to load
await new Promise((resolve) => setTimeout(resolve, 50));

// Register sync hook SECOND - but sync hooks run BEFORE async hooks
// per Node.js spec, regardless of registration order.
// The sync hook intercepts "virtual:order-test" and short-circuits,
// so the async hook never sees it.
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "virtual:order-test") {
      return { url: "file:///order_sync.js", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "file:///order_sync.js") {
      return {
        source: 'export const source = "sync";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const { source } = await import("virtual:order-test");
// Should be "sync" because sync hooks (registerHooks) run before async (register)
console.log("source:", source);
