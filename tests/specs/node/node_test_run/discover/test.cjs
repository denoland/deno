"use strict";
const test = require("node:test");

// Discovered by run({ cwd }) default discovery because the basename is test.cjs.
test("discovered test");
