// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

// `navigator.locks` (the Web Locks API) isn't in Deno's own ambient
// `Navigator` type yet, so type it locally rather than expanding the
// public lib.deno.*.d.ts surface as a side effect of this test.
declare global {
  interface Navigator {
    readonly locks: {
      request<T>(
        name: string,
        optionsOrCallback:
          | { mode?: "shared" | "exclusive" }
          | ((lock: { name: string; mode: string }) => T),
        callback?: (lock: { name: string; mode: string }) => T,
      ): Promise<T>;
      query(): {
        held: Array<{ name: string; mode: string; clientId: string }>;
        pending: Array<{ name: string; mode: string; clientId: string }>;
      };
    };
  }
}

Deno.test(async function locksRequestDefaultsToExclusive() {
  const result = await navigator.locks.request(
    "web-locks-test-basic",
    (lock) => {
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
    (lock) => {
      assertEquals(lock.mode, "shared");
    },
  );
});

Deno.test(async function locksQueryShape() {
  await navigator.locks.request(
    "web-locks-test-query",
    { mode: "shared" },
    (_lock) => {
      const { held, pending } = navigator.locks.query();
      const entry = held.find((l) => l.name === "web-locks-test-query");
      assertEquals(entry?.mode, "shared");
      assertEquals(typeof entry?.clientId, "string");
      assertEquals(Array.isArray(pending), true);
    },
  );
});
