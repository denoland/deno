const assert = require("node:assert/strict");
const { registerHooks } = require("node:module");

const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "redirected") {
      return {
        url: new URL("./entry.js", context.parentURL).href,
        shortCircuit: true,
      };
    }
    return nextResolve(specifier, context);
  },
});

const specifier = process.argv[2] === "redirect" ? "redirected" : "./entry.js";
assert.strictEqual(require(specifier).value, "loaded dependency");
console.log("required ES module and its dependency");
hook.deregister();
