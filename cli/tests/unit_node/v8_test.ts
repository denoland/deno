// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  cachedDataVersionTag,
  getHeapStatistics,
  setFlagsFromString,
} from "node:v8";
import { assertEquals } from "../../../test_util/std/assert/mod.ts";

// https://github.com/nodejs/node/blob/a2bbe5ff216bc28f8dac1c36a8750025a93c3827/test/parallel/test-v8-version-tag.js#L6
Deno.test({
  name: "cachedDataVersionTag success",
  fn() {
    const tag = cachedDataVersionTag();
    assertEquals(typeof tag, "number");
    assertEquals(cachedDataVersionTag(), tag);
  },
});

// https://github.com/nodejs/node/blob/a2bbe5ff216bc28f8dac1c36a8750025a93c3827/test/parallel/test-v8-stats.js#L6
Deno.test({
  name: "getHeapStatistics success",
  fn() {
    const s = getHeapStatistics();
    const keys = [
      "does_zap_garbage",
      "external_memory",
      "heap_size_limit",
      "malloced_memory",
      "number_of_detached_contexts",
      "number_of_native_contexts",
      "peak_malloced_memory",
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
    setFlagsFromString("--allow_natives_syntax");
  },
});
