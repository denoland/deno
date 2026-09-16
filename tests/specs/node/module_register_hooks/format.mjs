import assert from "node:assert";
import { createRequire, registerHooks } from "node:module";

const require = createRequire(import.meta.url);
const seen = [];
registerHooks({
  load(url, context, nextLoad) {
    const result = nextLoad(url, context);
    if (url.endsWith("format-cjs.cjs")) {
      assert.strictEqual(context.format, "commonjs");
      assert.strictEqual(result.format, "commonjs");
      seen.push("commonjs");
    } else if (url.endsWith("format-esm.mjs")) {
      assert.strictEqual(context.format, "module");
      assert.strictEqual(result.format, "module");
      seen.push("module");
    }
    return result;
  },
});

assert.strictEqual(require("./format-cjs.cjs"), "cjs");
assert.strictEqual((await import("./format-esm.mjs")).default, "esm");
assert.deepStrictEqual(seen.sort(), ["commonjs", "module"]);
console.log("load hook format works");
