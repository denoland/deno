// Copyright 2018-2026 the Deno authors. MIT license.
// Mocking a CommonJS module required by a CommonJS importer: named exports are
// applied onto the default export, the default cache behavior gives a fresh
// exports object on every require, and restore() brings the original back.
"use strict";

const assert = require("node:assert/strict");
const { join } = require("node:path");
const { pathToFileURL } = require("node:url");
const { mock } = require("node:test");

const fixture = join(__dirname, "basic-cjs.cjs");

const original = require(fixture);
assert.strictEqual(original.string, "original cjs string");
assert.strictEqual(original.fn, undefined);

const ctx = mock.module(pathToFileURL(fixture), {
  namedExports: {
    fn() {
      return 42;
    },
  },
});

const mocked = require(fixture);
assert.notStrictEqual(original, mocked);
assert.strictEqual(mocked.string, undefined);
assert.strictEqual(mocked.fn(), 42);

// Not cached by default: every require is a fresh exports object.
assert.notStrictEqual(mocked, require(fixture));

ctx.restore();

const restored = require(fixture);
assert.strictEqual(restored.string, "original cjs string");
assert.strictEqual(restored.fn, undefined);

console.log("cjs ok");
