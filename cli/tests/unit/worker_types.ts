// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test(
  { permissions: { read: true } },
  function utimeSyncFileSuccess() {
    const w = new Worker(
      new URL("../testdata/workers/worker_types.ts", import.meta.url).href,
      { type: "module" },
    );
    assert(w);
    w.terminate();
  },
);
