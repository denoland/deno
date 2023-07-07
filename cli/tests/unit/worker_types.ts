// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test(
  { permissions: { read: true } },
  function utimeSyncFileSuccess() {
    const w = new Worker(
      import.meta.resolve("../testdata/workers/worker_types.ts"),
      { type: "module" },
    );
    assert(w);
    w.terminate();
  },
);
