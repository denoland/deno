// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  unitTest,
  assert,
  assertEquals,
  createResolvable,
} from "./test_util.ts";

unitTest(async function workerTerminate(): Promise<void> {
  const promise = createResolvable();

  const jsWorker = new Worker("../../tests/subdir/test_worker.js", {
    type: "module",
  });
  const tsWorker = new Worker("../../tests/subdir/test_worker.ts", {
    type: "module",
    name: "tsWorker",
  });

  tsWorker.onmessage = (e): void => {
    assertEquals(e.data, "Hello World");
    promise.resolve();
  };

  jsWorker.onmessage = (e): void => {
    assertEquals(e.data, "Hello World");
    tsWorker.postMessage("Hello World");
  };

  jsWorker.onerror = (e: Event): void => {
    e.preventDefault();
    jsWorker.postMessage("Hello World");
  };

  jsWorker.postMessage("Hello World");
  await promise;
  tsWorker.terminate();
  jsWorker.terminate();
});

unitTest(async function workerNested(): Promise<void> {
  const promise = createResolvable();

  const nestedWorker = new Worker("../../tests/subdir/nested_worker.js", {
    type: "module",
    name: "nested",
  });

  nestedWorker.onmessage = (e): void => {
    assert(e.data.type !== "error");
    promise.resolve();
  };

  nestedWorker.postMessage("Hello World");
  await promise;
  nestedWorker.terminate();
});

unitTest(async function workerThrowsWhenExecuting(): Promise<void> {
  const promise = createResolvable();
  const throwingWorker = new Worker("../../tests/subdir/throwing_worker.js", {
    type: "module",
  });

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  throwingWorker.onerror = (e: any): void => {
    e.preventDefault();
    assert(/Uncaught Error: Thrown error/.test(e.message));
    promise.resolve();
  };

  await promise;
  throwingWorker.terminate();
});

unitTest(
  {
    perms: { net: true },
  },
  async function workerFetchAPI(): Promise<void> {
    const promise = createResolvable();

    const fetchingWorker = new Worker("../../tests/subdir/fetching_worker.js", {
      type: "module",
    });

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    fetchingWorker.onerror = (e: any): void => {
      e.preventDefault();
      promise.reject(e.message);
    };

    // Defer promise.resolve() to allow worker to shut down
    fetchingWorker.onmessage = (e): void => {
      assert(e.data === "Done!");
      promise.resolve();
    };

    await promise;
    fetchingWorker.terminate();
  }
);

unitTest(async function workerTerminateBusyLoop(): Promise<void> {
  const promise = createResolvable();

  const busyWorker = new Worker("../../tests/subdir/busy_worker.js", {
    type: "module",
  });

  let testResult = 0;

  busyWorker.onmessage = (e): void => {
    testResult = e.data;
    if (testResult >= 10000) {
      busyWorker.terminate();
      busyWorker.onmessage = (_e): void => {
        throw new Error("unreachable");
      };
      setTimeout(() => {
        assertEquals(testResult, 10000);
        promise.resolve();
      }, 100);
    }
  };

  busyWorker.postMessage("ping");
  await promise;
});

unitTest(async function workerRaceCondition(): Promise<void> {
  // See issue for details
  // https://github.com/denoland/deno/issues/4080
  const promise = createResolvable();

  const racyWorker = new Worker("../../tests/subdir/racy_worker.js", {
    type: "module",
  });

  racyWorker.onmessage = (e): void => {
    assertEquals(e.data.buf.length, 999999);
    racyWorker.onmessage = (_e): void => {
      throw new Error("unreachable");
    };
    setTimeout(() => {
      promise.resolve();
    }, 100);
  };

  await promise;
});

unitTest(async function workerWithDenoNamespace(): Promise<void> {
  const promise = createResolvable();

  const denoWorker = new Worker("../../tests/subdir/deno_worker.js", {
    type: "module",
    name: "denoWorker",
    deno: true,
    permissions: {
      read: true,
      write: true,
    },
  });

  denoWorker.onmessage = (e): void => {
    assertEquals(e.data, "Hello World");
    denoWorker.terminate();
    promise.resolve();
  };

  denoWorker.postMessage("Hello World");
  await promise;
});
