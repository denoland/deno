// Copyright 2018-2026 the Deno authors. MIT license.
// The legacy `exports` option bundles the default export (its `default` key)
// and the named exports (its remaining keys) into a single object. It works for
// both ESM and CJS modules, observed from both module systems.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mock } from "node:test";

const require = createRequire(import.meta.url);

// exports without a `default` key behaves like namedExports.
const noDefault = mock.module("./basic-esm.mjs", {
  exports: {
    fn() {
      return 42;
    },
  },
});
{
  const esm = await import("./basic-esm.mjs");
  assert.strictEqual(esm.fn(), 42);
  assert.strictEqual(esm.string, undefined);
}
noDefault.restore();

// exports with a `default` key: ESM module keeps default and named independent.
const esmExports = mock.module("./basic-esm.mjs", {
  exports: {
    default: { mocked: true },
    val1: "mock value",
  },
});
{
  const esm = await import("./basic-esm.mjs");
  assert.deepStrictEqual(esm.default, { mocked: true });
  assert.strictEqual(esm.val1, "mock value");
}
esmExports.restore();

// exports with a `default` key for a CJS module: named exports are applied onto
// the default export, and the default is returned as module.exports.
const defaultExport = { val1: 5, val2: 3 };
const cjsExports = mock.module("./basic-cjs.cjs", {
  exports: {
    default: defaultExport,
    val1: "mock value",
  },
});
{
  const cjs = require("./basic-cjs.cjs");
  assert.strictEqual(cjs, defaultExport);
  assert.strictEqual(cjs.val1, "mock value");
  assert.strictEqual(cjs.val2, 3);
}
cjsExports.restore();

console.log("exports ok");
