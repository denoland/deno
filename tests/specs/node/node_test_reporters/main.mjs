import assert from "node:assert";
import { builtinModules, createRequire, isBuiltin } from "node:module";
import reporters, { dot, junit, lcov, spec, tap } from "node:test/reporters";

assert.strictEqual(typeof tap, "function");
assert.strictEqual(typeof dot, "function");
assert.strictEqual(typeof junit, "function");
assert.strictEqual(typeof spec, "function");
assert.strictEqual(typeof lcov, "function");
assert.strictEqual(reporters.tap, tap);

assert.strictEqual(isBuiltin("node:test/reporters"), true);
assert.strictEqual(isBuiltin("test/reporters"), false);
assert(builtinModules.includes("node:test/reporters"));

const require = createRequire(import.meta.url);
assert.strictEqual(require("node:test/reporters").tap, tap);

async function* events() {
  yield { type: "test:plan", data: { nesting: 0, count: 1 } };
  yield { type: "test:start", data: { nesting: 0, name: "sample" } };
  yield {
    type: "test:pass",
    data: { nesting: 0, testNumber: 1, name: "sample" },
  };
}

const output = [];
for await (const chunk of tap(events())) {
  output.push(chunk);
}

assert.deepStrictEqual(output, [
  "TAP version 13\n",
  "1..1\n",
  "# Subtest: sample\n",
  "ok 1 sample\n",
]);

console.log("ok");
