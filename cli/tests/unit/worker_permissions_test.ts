// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals, deferred } from "./test_util.ts";

Deno.test(
  { permissions: { env: true, read: true } },
  async function workerEnvArrayPermissions() {
    const promise = deferred<boolean[]>();

    const worker = new Worker(
      new URL(
        "../testdata/workers/env_read_check_worker.js",
        import.meta.url,
      ).href,
      {
        type: "module",
        deno: {
          namespace: true,
          permissions: {
            env: ["test", "OTHER"],
          },
        },
      },
    );

    worker.onmessage = ({ data }) => {
      promise.resolve(data.permissions);
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
