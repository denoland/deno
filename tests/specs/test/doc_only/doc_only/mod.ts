/**
 * ```ts
 * import { assertEquals } from "@std/assert/equals";
 *
 * assertEquals(42, 40 + 2);
 * ```
 */
Deno.test("unreachable", function () {
  throw new Error(
    "modules that don't end with _test are scanned for documentation tests only should not be executed",
  );
});
