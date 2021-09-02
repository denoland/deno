/**
 * ```ts
 * import "./mod.ts";
 * ```
 */
Deno.test("unreachable", function () {
  throw new Error(
    "modules that don't end with _test are scanned for documentation tests only should not be executed",
  );
});
