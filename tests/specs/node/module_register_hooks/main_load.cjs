const assert = require("assert");
const { registerHooks } = require("module");

// Test load hook that transforms source
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.includes("empty.js")) {
      return {
        source: 'module.exports = "modified"',
        format: "commonjs",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const result = require("./empty.js");
assert.strictEqual(result, "modified");
console.log("load hook works");

hook.deregister();
