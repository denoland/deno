import { assertEquals, assertStringIncludes } from "./test_util.ts";

Deno.test(function customInspectFunction() {
  const blob = new DOMException("test");
  assertEquals(
    Deno.inspect(blob),
    `DOMException: test`,
  );
  assertStringIncludes(Deno.inspect(DOMException.prototype), "DOMException");
});
