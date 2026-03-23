// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const uv = loadTestLibrary();

Deno.test({
  name: "napi uv async",
  ignore: true,
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_async((value) => {
        called = true;
        if (value === 5) {
          resolve();
        }
      });
    });
    assertEquals(called, true);
  },
});

Deno.test({
  name: "napi uv async keeps event loop alive",
  fn: async () => {
    let called = false;
    await new Promise((resolve) => {
      uv.test_uv_async_ref(() => {
        called = true;
        resolve();
      });
    });
    assertEquals(called, true);
  },
});

Deno.test({
  name: "napi uv timer repeating",
  fn: async () => {
    const counts = [];
    await new Promise((resolve) => {
      uv.test_uv_timer((count) => {
        counts.push(count);
        if (count >= 3) {
          resolve();
        }
      });
    });
    assertEquals(counts, [1, 2, 3]);
  },
});

Deno.test({
  name: "napi uv timer ref/unref",
  fn: async () => {
    let result = null;
    await new Promise((resolve) => {
      uv.test_uv_timer_ref_unref((value) => {
        result = value;
        resolve();
      });
    });
    assertEquals(result, "ok");
  },
});

Deno.test({
  name: "napi uv timer set_repeat/get_repeat",
  fn: () => {
    const ok = uv.test_uv_timer_repeat();
    assertEquals(ok, true);
  },
});

Deno.test({
  name: "napi uv idle",
  fn: async () => {
    let count = null;
    await new Promise((resolve) => {
      uv.test_uv_idle((c) => {
        count = c;
        resolve();
      });
    });
    assertEquals(count, 3);
  },
});

Deno.test({
  name: "napi uv check",
  fn: async () => {
    let count = null;
    await new Promise((resolve) => {
      uv.test_uv_check((c) => {
        count = c;
        resolve();
      });
    });
    assertEquals(count, 3);
  },
});

Deno.test({
  name: "napi uv_os_getpid",
  fn: () => {
    const pid = uv.test_uv_os_getpid();
    assertEquals(typeof pid, "number");
    assertEquals(pid > 0, true);
    // Should match Deno.pid
    assertEquals(pid, Deno.pid);
  },
});
