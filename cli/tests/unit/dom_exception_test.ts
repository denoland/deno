import { assertEquals, assertStringIncludes, unitTest } from "./test_util.ts";

unitTest(function customInspectFunction() {
  const blob = new DOMException("test");
  assertEquals(
    Deno.inspect(blob),
    `DOMException: test`,
  );
  assertStringIncludes(Deno.inspect(DOMException.prototype), "DOMException");
});
