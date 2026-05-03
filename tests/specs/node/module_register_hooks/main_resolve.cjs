const assert = require("assert");
const { registerHooks } = require("module");

// Test resolve hook with short-circuit
const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "virtual-module") {
      return { url: "file:///virtual.js", shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "file:///virtual.js") {
      return {
        source: 'module.exports = "hello from virtual"',
        format: "commonjs",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const result = require("virtual-module");
assert.strictEqual(result, "hello from virtual");
console.log("resolve hook works");

hook.deregister();
