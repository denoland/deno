import { test, assertEqual, assert } from "./test_util.ts";

test(function version() {
  assert(typeof Deno.version.deno === "string");
  assert(typeof Deno.version.v8 === "string");
  assert(typeof Deno.version.typescript === "string");
});
