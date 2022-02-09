import { assertEquals, loadTestLibrary } from "./common.js";

const strings = loadTestLibrary();

Deno.test("napi string utf8", function () {
  assertEquals(strings.test_utf8(""), "");
  assertEquals(strings.test_utf8("🦕"), "🦕");
});

Deno.test("napi string", function () {
  assertEquals(strings.test_utf16(""), "");
  assertEquals(strings.test_utf16("🦕"), "🦕");
});
