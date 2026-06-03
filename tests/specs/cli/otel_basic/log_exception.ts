// Copyright 2018-2026 the Deno authors. MIT license.

const error = new Error("test error");
error.stack = "Error: test error\n    at file:///test.ts:1:1";
console.error(error);
console.log("plain log");
console.error(error, "with extra context");
console.error("string error", "with extra arg");
