const assert = require("assert");
const { registerHooks } = require("module");

// Test load hook that overrides a builtin module by changing
// the format from "builtin" to "commonjs" with custom source.
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url === "node:util" && context.format === "builtin") {
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
console.log("builtin override works");
