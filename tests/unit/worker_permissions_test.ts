// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(
  { permissions: { env: true, read: true } },
  async function workerEnvArrayPermissions() {
    const { promise, resolve } = Promise.withResolvers<boolean[]>();

    const worker = new Worker(
      import.meta.resolve(
        "../testdata/workers/env_read_check_worker.js",
      ),
      { type: "module", deno: { permissions: { env: ["test", "OTHER"] } } },
    );

    worker.onmessage = ({ data }) => {
      resolve(data.permissions);
    };

    worker.postMessage({
      names: ["test", "TEST", "asdf", "OTHER"],
    });

    const permissions = await promise;
    worker.terminate();

    if (Deno.build.os === "windows") {
      // windows ignores case
      assertEquals(permissions, [true, true, false, true]);
    } else {
      assertEquals(permissions, [true, false, false, true]);
    }
  },
);
