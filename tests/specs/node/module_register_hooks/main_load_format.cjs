const assert = require("assert");
const { registerHooks } = require("module");

// Load hooks receive the format hint Node passes in `context.format`, and a
// pass-through hook keeps the default loading of each format.
const seen = {};
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.includes("/format/")) {
      seen[url.slice(url.lastIndexOf("/") + 1)] = context.format;
    }
    return nextLoad(url, context);
  },
});

assert.strictEqual(require("./format/data.json").value, 42);
assert.strictEqual(require("./format/d.cjs"), "cjs");
assert.strictEqual(require("./format/e.js"), "js");
assert.strictEqual(require("./format/b.ts"), "ts");

import("./format/c.mjs").then((ns) => {
  assert.strictEqual(ns.default, "mjs");
  console.log(JSON.stringify(seen));
  hook.deregister();
});
