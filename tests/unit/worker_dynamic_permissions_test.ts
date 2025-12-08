// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, assertThrows } from "./test_util.ts";

Deno.test(
  { permissions: { read: true } },
  async function workerDynamicPermissionsUpdate() {
    const { promise, resolve } = Promise.withResolvers<string>();
    const testFile = import.meta.resolve(
      "../testdata/workers/env_read_check_worker.js",
    );

    const worker = new Worker(
      import.meta.resolve("../testdata/workers/dynamic_permissions_worker.js"),
      { type: "module", deno: { permissions: { read: false } } },
    );

    worker.onmessage = ({ data }) => {
      resolve(data.results.read[testFile]);
    };

    assertThrows(() => {
      worker.updatePermissions({
        write: [testFile],
      });
    });

    worker.updatePermissions({
      read: [testFile],
    });

    worker.postMessage({
      type: "checkPermissions",
      permissions: { read: [testFile] },
    });

    const result = await promise;
    assertEquals(result, "granted");

    worker.terminate();
  },
);
