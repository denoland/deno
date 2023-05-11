// deno-fmt-ignore-file
// deno-lint-ignore-file

"use strict";

const Module = require("module");
const assert = require("assert/strict");
const path = require("path");

const file = path.join(__dirname, "..", "fixtures", "run-main.js");
process.argv = [process.argv[0], file];
Module.runMain();

// The required file via `Module.runMain()` sets this global
assert.equal(globalThis.foo, 42);
