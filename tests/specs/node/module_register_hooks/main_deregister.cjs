const assert = require("assert");
const { registerHooks } = require("module");

// Test that deregister removes hooks
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.includes("empty.js")) {
      return {
        source: 'module.exports = "hooked"',
        format: "commonjs",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

// First require uses hook
const result1 = require("./empty.js");
assert.strictEqual(result1, "hooked");

// Deregister and clear cache
hook.deregister();
delete require.cache[require.resolve("./empty.js")];

// Second require should not use hook
const result2 = require("./empty.js");
assert.notStrictEqual(result2, "hooked");
console.log("deregister works");
