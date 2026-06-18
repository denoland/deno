"use strict";
const test = require("node:test");

// Never resolves; the run-level AbortSignal must terminate this and report it
// as a failed test.
test("hangs forever", () => new Promise(() => {}));
