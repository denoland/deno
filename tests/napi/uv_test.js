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
