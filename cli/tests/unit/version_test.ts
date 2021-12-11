import { assert } from "./test_util.ts";

Deno.test(function version() {
  const pattern = /^\d+\.\d+\.\d+/;
  assert(pattern.test(Deno.version.deno));
  assert(pattern.test(Deno.version.v8));
  // Unreleased version of TypeScript now set the version to 0-dev
  assert(
    pattern.test(Deno.version.typescript) ||
      Deno.version.typescript === "0-dev" ||
      Deno.version.typescript === "0-beta" ||
      Deno.version.typescript === "1-rc",
  );
});
