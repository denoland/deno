const assert = require("assert");
const { registerHooks } = require("module");

// Test LIFO chaining: hook2 runs first, then hook1 via nextLoad
const hook1 = registerHooks({
  load(url, context, nextLoad) {
    const result = nextLoad(url, context);
    if (url.includes("empty.js")) {
      assert.strictEqual(result.source, "");
      return {
        source: 'exports.value = "from hook1"',
        format: "commonjs",
      };
    }
    return result;
  },
});

const hook2 = registerHooks({
  load(url, context, nextLoad) {
    const result = nextLoad(url, context);
    if (url.includes("empty.js")) {
      // hook2 runs first (LIFO), nextLoad gives hook1's result
      assert.strictEqual(result.source, 'exports.value = "from hook1"');
      return {
        source: 'exports.value = "from hook2"',
        format: "commonjs",
      };
    }
    return result;
  },
});

const mod = require("./empty.js");
assert.strictEqual(mod.value, "from hook2");
console.log("chained hooks work");

hook1.deregister();
hook2.deregister();
