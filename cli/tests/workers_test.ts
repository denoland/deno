// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Requires to be run with `--allow-net` flag

import {
  assert,
  assertEquals,
  assertThrows,
  fail,
} from "../../std/testing/asserts.ts";
import { deferred } from "../../std/async/deferred.ts";

Deno.test({
  name: "worker terminate",
  fn: async function (): Promise<void> {
    const promise = deferred();

    const jsWorker = new Worker(
      new URL("workers/test_worker.js", import.meta.url).href,
      { type: "module" },
    );
    const tsWorker = new Worker(
      new URL("workers/test_worker.ts", import.meta.url).href,
      { type: "module", name: "tsWorker" },
    );

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
  },
});

Deno.test({
  name: "worker from data url",
  async fn() {
    const promise = deferred();
    const tsWorker = new Worker(
      "data:application/typescript;base64,aWYgKHNlbGYubmFtZSAhPT0gInRzV29ya2VyIikgewogIHRocm93IEVycm9yKGBJbnZhbGlkIHdvcmtlciBuYW1lOiAke3NlbGYubmFtZX0sIGV4cGVjdGVkIHRzV29ya2VyYCk7Cn0KCm9ubWVzc2FnZSA9IGZ1bmN0aW9uIChlKTogdm9pZCB7CiAgcG9zdE1lc3NhZ2UoZS5kYXRhKTsKICBjbG9zZSgpOwp9Owo=",
      { type: "module", name: "tsWorker" },
    );

    tsWorker.onmessage = (e): void => {
      assertEquals(e.data, "Hello World");
      promise.resolve();
    };

    tsWorker.postMessage("Hello World");

    await promise;
    tsWorker.terminate();
  },
});

Deno.test({
  name: "worker nested",
  fn: async function (): Promise<void> {
    const promise = deferred();

    const nestedWorker = new Worker(
      new URL("workers/nested_worker.js", import.meta.url).href,
      { type: "module", name: "nested" },
    );

    nestedWorker.onmessage = (e): void => {
      assert(e.data.type !== "error");
      promise.resolve();
    };

    nestedWorker.postMessage("Hello World");
    await promise;
    nestedWorker.terminate();
  },
});

Deno.test({
  name: "worker throws when executing",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const throwingWorker = new Worker(
      new URL("workers/throwing_worker.js", import.meta.url).href,
      { type: "module" },
    );

    // deno-lint-ignore no-explicit-any
    throwingWorker.onerror = (e: any): void => {
      e.preventDefault();
      assert(/Uncaught Error: Thrown error/.test(e.message));
      promise.resolve();
    };

    await promise;
    throwingWorker.terminate();
  },
});

Deno.test({
  name: "worker globals",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const w = new Worker(
      new URL("workers/worker_globals.ts", import.meta.url).href,
      { type: "module" },
    );
    w.onmessage = (e): void => {
      assertEquals(e.data, "true, true, true");
      promise.resolve();
    };
    w.postMessage("Hello, world!");
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "worker fetch API",
  fn: async function (): Promise<void> {
    const promise = deferred();

    const fetchingWorker = new Worker(
      new URL("workers/fetching_worker.js", import.meta.url).href,
      { type: "module" },
    );

    // deno-lint-ignore no-explicit-any
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
  },
});

Deno.test({
  name: "worker terminate busy loop",
  fn: async function (): Promise<void> {
    const promise = deferred();

    const busyWorker = new Worker(
      new URL("workers/busy_worker.js", import.meta.url).href,
      { type: "module" },
    );

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
  },
});

Deno.test({
  name: "worker race condition",
  fn: async function (): Promise<void> {
    // See issue for details
    // https://github.com/denoland/deno/issues/4080
    const promise = deferred();

    const racyWorker = new Worker(
      new URL("workers/racy_worker.js", import.meta.url).href,
      { type: "module" },
    );

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
  },
});

Deno.test({
  name: "worker is event listener",
  fn: async function (): Promise<void> {
    let messageHandlersCalled = 0;
    let errorHandlersCalled = 0;

    const promise1 = deferred();
    const promise2 = deferred();

    const worker = new Worker(
      new URL("workers/event_worker.js", import.meta.url).href,
      { type: "module" },
    );

    worker.onmessage = (_e: Event): void => {
      messageHandlersCalled++;
    };
    worker.addEventListener("message", (_e: Event) => {
      messageHandlersCalled++;
    });
    worker.addEventListener("message", (_e: Event) => {
      messageHandlersCalled++;
      promise1.resolve();
    });

    worker.onerror = (e): void => {
      errorHandlersCalled++;
      e.preventDefault();
    };
    worker.addEventListener("error", (_e: Event) => {
      errorHandlersCalled++;
    });
    worker.addEventListener("error", (_e: Event) => {
      errorHandlersCalled++;
      promise2.resolve();
    });

    worker.postMessage("ping");
    await promise1;
    assertEquals(messageHandlersCalled, 3);

    worker.postMessage("boom");
    await promise2;
    assertEquals(errorHandlersCalled, 3);
    worker.terminate();
  },
});

Deno.test({
  name: "worker scope is event listener",
  fn: async function (): Promise<void> {
    const promise1 = deferred();

    const worker = new Worker(
      new URL("workers/event_worker_scope.js", import.meta.url).href,
      { type: "module" },
    );

    worker.onmessage = (e: MessageEvent): void => {
      const { messageHandlersCalled, errorHandlersCalled } = e.data;
      assertEquals(messageHandlersCalled, 4);
      assertEquals(errorHandlersCalled, 4);
      promise1.resolve();
    };

    worker.onerror = (_e): void => {
      throw new Error("unreachable");
    };

    worker.postMessage("boom");
    worker.postMessage("ping");
    await promise1;
    worker.terminate();
  },
});

Deno.test({
  name: "worker with Deno namespace",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const promise2 = deferred();

    const regularWorker = new Worker(
      new URL("workers/non_deno_worker.js", import.meta.url).href,
      { type: "module" },
    );
    const denoWorker = new Worker(
      new URL("workers/deno_worker.ts", import.meta.url).href,
      {
        type: "module",
        deno: {
          namespace: true,
          permissions: "inherit",
        },
      },
    );

    regularWorker.onmessage = (e): void => {
      assertEquals(e.data, "Hello World");
      regularWorker.terminate();
      promise.resolve();
    };

    denoWorker.onmessage = (e): void => {
      assertEquals(e.data, "Hello World");
      denoWorker.terminate();
      promise2.resolve();
    };

    regularWorker.postMessage("Hello World");
    await promise;
    denoWorker.postMessage("Hello World");
    await promise2;
  },
});

Deno.test({
  name: "worker with crypto in scope",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const w = new Worker(
      new URL("workers/worker_crypto.js", import.meta.url).href,
      { type: "module" },
    );
    w.onmessage = (e): void => {
      assertEquals(e.data, true);
      promise.resolve();
    };
    w.postMessage(null);
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "Worker event handler order",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const w = new Worker(
      new URL("workers/test_worker.ts", import.meta.url).href,
      { type: "module", name: "tsWorker" },
    );
    const arr: number[] = [];
    w.addEventListener("message", () => arr.push(1));
    w.onmessage = (e): void => {
      arr.push(2);
    };
    w.addEventListener("message", () => arr.push(3));
    w.addEventListener("message", () => {
      assertEquals(arr, [1, 2, 3]);
      promise.resolve();
    });
    w.postMessage("Hello World");
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "Worker immediate close",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const w = new Worker(
      new URL("./workers/immediately_close_worker.js", import.meta.url).href,
      { type: "module" },
    );
    setTimeout(() => {
      promise.resolve();
    }, 1000);
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "Worker post undefined",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const worker = new Worker(
      new URL("./worker_post_undefined.ts", import.meta.url).href,
      { type: "module" },
    );

    const handleWorkerMessage = (e: MessageEvent): void => {
      console.log("main <- worker:", e.data);
      worker.terminate();
      promise.resolve();
    };

    worker.addEventListener("messageerror", () => console.log("message error"));
    worker.addEventListener("error", () => console.log("error"));
    worker.addEventListener("message", handleWorkerMessage);

    console.log("\npost from parent");
    worker.postMessage(undefined);
    await promise;
  },
});

Deno.test("Worker inherits permissions", async function () {
  const promise = deferred();
  const worker = new Worker(
    new URL("./workers/read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "inherit",
      },
    },
  );

  worker.onmessage = ({ data: hasPermission }) => {
    assert(hasPermission);
    promise.resolve();
  };

  worker.postMessage(null);

  await promise;
  worker.terminate();
});

Deno.test("Worker limit children permissions", async function () {
  const promise = deferred();
  const worker = new Worker(
    new URL("./workers/read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: {
          read: false,
        },
      },
    },
  );

  worker.onmessage = ({ data: hasPermission }) => {
    assert(!hasPermission);
    promise.resolve();
  };

  worker.postMessage(null);

  await promise;
  worker.terminate();
});

Deno.test("Worker limit children permissions granularly", async function () {
  const promise = deferred();
  const worker = new Worker(
    new URL("./workers/read_check_granular_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: {
          read: [
            new URL("./workers/read_check_worker.js", import.meta.url),
          ],
        },
      },
    },
  );

  //Routes are relative to the spawned worker location
  const routes = [
    { permission: false, route: "read_check_granular_worker.js" },
    { permission: true, route: "read_check_worker.js" },
  ];

  let checked = 0;
  worker.onmessage = ({ data }) => {
    checked++;
    assertEquals(data.hasPermission, routes[data.index].permission);
    routes.shift();
    if (checked === routes.length) {
      promise.resolve();
    }
  };

  routes.forEach(({ route }, index) =>
    worker.postMessage({
      index,
      route,
    })
  );

  await promise;
  worker.terminate();
});

Deno.test("Nested worker limit children permissions", async function () {
  const promise = deferred();

  /** This worker has read permissions but doesn't grant them to its children */
  const worker = new Worker(
    new URL("./workers/parent_read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "inherit",
      },
    },
  );

  worker.onmessage = ({ data }) => {
    assert(data.parentHasPermission);
    assert(!data.childHasPermission);
    promise.resolve();
  };

  worker.postMessage(null);

  await promise;
  worker.terminate();
});

Deno.test("Nested worker limit children permissions granularly", async function () {
  const promise = deferred();

  /** This worker has read permissions but doesn't grant them to its children */
  const worker = new Worker(
    new URL("./workers/parent_read_check_granular_worker.js", import.meta.url)
      .href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: {
          read: [
            new URL("./workers/read_check_granular_worker.js", import.meta.url),
          ],
        },
      },
    },
  );

  //Routes are relative to the spawned worker location
  const routes = [
    {
      childHasPermission: false,
      parentHasPermission: true,
      route: "read_check_granular_worker.js",
    },
    {
      childHasPermission: false,
      parentHasPermission: false,
      route: "read_check_worker.js",
    },
  ];

  let checked = 0;
  worker.onmessage = ({ data }) => {
    checked++;
    assertEquals(
      data.childHasPermission,
      routes[data.index].childHasPermission,
    );
    assertEquals(
      data.parentHasPermission,
      routes[data.index].parentHasPermission,
    );
    if (checked === routes.length) {
      promise.resolve();
    }
  };

  // Index needed cause requests will be handled asynchronously
  routes.forEach(({ route }, index) =>
    worker.postMessage({
      index,
      route,
    })
  );

  await promise;
  worker.terminate();
});

// This test relies on env permissions not being granted on main thread
Deno.test("Worker initialization throws on worker permissions greater than parent thread permissions", function () {
  assertThrows(
    () => {
      const worker = new Worker(
        new URL("./workers/deno_worker.ts", import.meta.url).href,
        {
          type: "module",
          deno: {
            namespace: true,
            permissions: {
              env: true,
            },
          },
        },
      );
      worker.terminate();
    },
    Deno.errors.PermissionDenied,
    "Can't escalate parent thread permissions",
  );
});

Deno.test("Worker with disabled permissions", async function () {
  const promise = deferred();

  const worker = new Worker(
    new URL("./workers/no_permissions_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "none",
      },
    },
  );

  worker.onmessage = ({ data: sandboxed }) => {
    assert(sandboxed);
    promise.resolve();
  };

  worker.postMessage(null);
  await promise;
  worker.terminate();
});

Deno.test({
  name: "worker location",
  fn: async function (): Promise<void> {
    const promise = deferred();
    const workerModuleHref =
      new URL("subdir/worker_location.ts", import.meta.url).href;
    const w = new Worker(workerModuleHref, { type: "module" });
    w.onmessage = (e): void => {
      assertEquals(e.data, workerModuleHref);
      promise.resolve();
    };
    w.postMessage("Hello, world!");
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "worker with relative specifier",
  fn: async function (): Promise<void> {
    // TODO(nayeemrmn): Add `Location` and `location` to `dlint`'s globals.
    // deno-lint-ignore no-undef
    assertEquals(location.href, "http://127.0.0.1:4545/cli/tests/");
    const promise = deferred();
    const w = new Worker(
      "./workers/test_worker.ts",
      { type: "module", name: "tsWorker" },
    );
    w.onmessage = (e): void => {
      assertEquals(e.data, "Hello, world!");
      promise.resolve();
    };
    w.postMessage("Hello, world!");
    await promise;
    w.terminate();
  },
});
