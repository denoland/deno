// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(async function locksRequestDefaultsToExclusive() {
  const result = await navigator.locks.request(
    "web-locks-test-basic",
    async (lock) => {
      assertEquals(lock.name, "web-locks-test-basic");
      assertEquals(lock.mode, "exclusive");
      return 42;
    },
  );
  assertEquals(result, 42);
});

Deno.test(async function locksRequestSharedMode() {
  await navigator.locks.request(
    "web-locks-test-shared",
    { mode: "shared" },
    async (lock) => {
      assertEquals(lock.mode, "shared");
    },
  );
});

Deno.test(async function locksQueryShape() {
  await navigator.locks.request(
    "web-locks-test-query",
    { mode: "shared" },
    async (_lock) => {
      const { held, pending } = navigator.locks.query();
      const entry = held.find((l) => l.name === "web-locks-test-query");
      assertEquals(entry?.mode, "shared");
      assertEquals(typeof entry?.clientId, "string");
      assertEquals(Array.isArray(pending), true);
    },
  );
});
