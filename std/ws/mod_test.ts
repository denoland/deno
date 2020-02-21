const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createSecKey } from "./mod.ts";

// cargo run -- --seed=86 -A testing/runner.ts --exclude "**/testdata" ws/mod_test.ts
test(function testCreateSecKey(): void {
  const secKey = createSecKey();
  assertEquals(atob(secKey).length, 16);
});
