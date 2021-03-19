// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test("utimeSyncFileSuccess", function () {
  const w = new Worker(
    new URL("../workers/worker_types.ts", import.meta.url).href,
    { type: "module" },
  );
  assert(w);
  w.terminate();
});
