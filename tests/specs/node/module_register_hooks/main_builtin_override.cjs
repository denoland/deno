const assert = require("assert");
const { registerHooks } = require("module");

// Test load hook that overrides a builtin module by changing
// the format from "builtin" to "commonjs" with custom source.
let loadCalls = 0;
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url === "node:util" && context.format === "builtin") {
      loadCalls++;
      return {
        source: "module.exports = { customUtil: true }",
        format: "commonjs",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

const util = require("node:util");
assert.strictEqual(util.customUtil, true);
assert.strictEqual(util.inspect, undefined);

// Repeated requires (both prefixed and bare) must hit the cached module,
// not re-run the hook or produce a fresh exports object.
const util2 = require("node:util");
const util3 = require("util");
assert.strictEqual(util2, util);
assert.strictEqual(util3, util);
assert.strictEqual(loadCalls, 1);

console.log("builtin override works");
