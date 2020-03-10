import { unitTest, assert } from "./test_util.ts";

unitTest(function version(): void {
  const pattern = /^\d+\.\d+\.\d+/;
  assert(pattern.test(Deno.version.deno));
  assert(pattern.test(Deno.version.v8));
  assert(pattern.test(Deno.version.typescript));
});
