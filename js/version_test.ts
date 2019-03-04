import { test, assert } from "./test_util.ts";

test(function version() {
  const pattern = /^\d+\.\d+\.\d+$/;
  assert(pattern.test(Deno.version.deno));
  assert(pattern.test(Deno.version.v8));
  assert(pattern.test(Deno.version.typescript));
});

test(function versionGnArgs() {
  assert(Deno.version.gnArgs.length > 100);
});
