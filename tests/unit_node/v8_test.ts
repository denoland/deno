// Copyright 2018-2026 the Deno authors. MIT license.
import * as v8 from "node:v8";
import { assert, assertEquals } from "@std/assert";

// https://github.com/nodejs/node/blob/a2bbe5ff216bc28f8dac1c36a8750025a93c3827/test/parallel/test-v8-version-tag.js#L6
Deno.test({
  name: "cachedDataVersionTag success",
  fn() {
    const tag = v8.cachedDataVersionTag();
    assertEquals(typeof tag, "number");
    assertEquals(v8.cachedDataVersionTag(), tag);
  },
});

// https://github.com/nodejs/node/blob/a2bbe5ff216bc28f8dac1c36a8750025a93c3827/test/parallel/test-v8-stats.js#L6
Deno.test({
  name: "getHeapStatistics success",
  fn() {
    const s = v8.getHeapStatistics();
    const keys = [
      "does_zap_garbage",
      "external_memory",
      "heap_size_limit",
      "malloced_memory",
      "number_of_detached_contexts",
      "number_of_native_contexts",
      "peak_malloced_memory",
      "total_allocated_bytes",
      "total_available_size",
      "total_global_handles_size",
      "total_heap_size",
      "total_heap_size_executable",
      "total_physical_size",
      "used_global_handles_size",
      "used_heap_size",
    ];
    assertEquals(Object.keys(s).sort(), keys);
    for (const k of keys) {
      assertEquals(
        typeof (s as unknown as Record<string, unknown>)[k],
        "number",
      );
    }
  },
});

Deno.test({
  name: "setFlagsFromString",
  fn() {
    v8.setFlagsFromString("--allow_natives_syntax");
  },
});

Deno.test({
  name: "serialize deserialize",
  fn() {
    const s = v8.serialize({ a: 1 });
    const d = v8.deserialize(s);
    assertEquals(d, { a: 1 });
  },
});

Deno.test({
  name: "writeHeapSnapshot requires write permission",
  permissions: { write: false },
  fn() {
    try {
      v8.writeHeapSnapshot("test.heapsnapshot");
      throw new Error("Expected to throw");
    } catch (e: unknown) {
      // node:fs operations convert NotCapable to EACCES
      const err = e as NodeJS.ErrnoException;
      assert(err.code === "EACCES", `Expected EACCES, got ${err.code}`);
    }
  },
});
