import { loadTestLibrary, assert, assertEquals } from "./common.js";

const strings = loadTestLibrary();

Deno.test("napi strings.test_utf8()", function () {
  assertEquals(strings.test_utf8("Hello"), "Hello");
});

