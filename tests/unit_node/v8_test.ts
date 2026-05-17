// Copyright 2018-2026 the Deno authors. MIT license.
import * as v8 from "node:v8";
import { assertEquals, assertThrows } from "@std/assert";

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
    assertThrows(() => {
      v8.writeHeapSnapshot("test.heapsnapshot");
    }, Deno.errors.NotCapable);
  },
});

Deno.test({
  name: "queryObjects counts instances by constructor",
  fn() {
    class QueryObjectsTestFixture {}
    const before = v8.queryObjects(QueryObjectsTestFixture, {
      format: "count",
    });
    assertEquals(typeof before, "number");
    const instances = [];
    for (let i = 0; i < 50; i++) {
      instances.push(new QueryObjectsTestFixture());
    }
    const after = v8.queryObjects(QueryObjectsTestFixture, {
      format: "count",
    });
    assertEquals(after - before >= 50, true);

    const summary = v8.queryObjects(QueryObjectsTestFixture, {
      format: "summary",
    });
    assertEquals(Array.isArray(summary), true);
    assertEquals((summary as string[]).length, 1);
    assertEquals(
      (summary as string[])[0].includes("QueryObjectsTestFixture"),
      true,
    );

    // Keep the instances reachable until after the snapshot.
    assertEquals(instances.length, 50);
  },
});

Deno.test({
  name: "queryObjects validates the constructor argument",
  fn() {
    assertThrows(() => {
      // @ts-expect-error testing invalid input
      v8.queryObjects("not a function");
    });
  },
});

Deno.test({
  name: "queryObjects validates the format option",
  fn() {
    class Anything {}
    assertThrows(() => {
      v8.queryObjects(
        Anything,
        // @ts-expect-error testing invalid input
        { format: "bogus" },
      );
    });
  },
});
