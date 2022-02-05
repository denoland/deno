// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Requires to be run with `--allow-net` flag

import {
  assert,
  assertEquals,
  assertMatch,
  assertThrows,
} from "../../../../test_util/std/testing/asserts.ts";
import { deferred } from "../../../../test_util/std/async/deferred.ts";

Deno.test({
  name: "worker terminate",
  fn: async function () {
    const jsWorker = new Worker(
      new URL("test_worker.js", import.meta.url).href,
      { type: "module" },
    );
    const tsWorker = new Worker(
      new URL("test_worker.ts", import.meta.url).href,
      { type: "module", name: "tsWorker" },
    );

    const promise1 = deferred();
    jsWorker.onmessage = (e) => {
      promise1.resolve(e.data);
    };

    const promise2 = deferred();
    tsWorker.onmessage = (e) => {
      promise2.resolve(e.data);
    };

    jsWorker.postMessage("Hello World");
    assertEquals(await promise1, "Hello World");
    tsWorker.postMessage("Hello World");
    assertEquals(await promise2, "Hello World");
    tsWorker.terminate();
    jsWorker.terminate();
  },
});

Deno.test({
  name: "worker from data url",
  async fn() {
    const tsWorker = new Worker(
      "data:application/typescript;base64,aWYgKHNlbGYubmFtZSAhPT0gInRzV29ya2VyIikgewogIHRocm93IEVycm9yKGBJbnZhbGlkIHdvcmtlciBuYW1lOiAke3NlbGYubmFtZX0sIGV4cGVjdGVkIHRzV29ya2VyYCk7Cn0KCm9ubWVzc2FnZSA9IGZ1bmN0aW9uIChlKTogdm9pZCB7CiAgcG9zdE1lc3NhZ2UoZS5kYXRhKTsKICBjbG9zZSgpOwp9Owo=",
      { type: "module", name: "tsWorker" },
    );

    const promise = deferred();
    tsWorker.onmessage = (e) => {
      promise.resolve(e.data);
    };

    tsWorker.postMessage("Hello World");
    assertEquals(await promise, "Hello World");
    tsWorker.terminate();
  },
});

Deno.test({
  name: "worker nested",
  fn: async function () {
    const nestedWorker = new Worker(
      new URL("nested_worker.js", import.meta.url).href,
      { type: "module", name: "nested" },
    );

    const promise = deferred();
    nestedWorker.onmessage = (e) => {
      promise.resolve(e.data);
    };

    nestedWorker.postMessage("Hello World");
    assertEquals(await promise, { type: "msg", text: "Hello World" });
    nestedWorker.terminate();
  },
});

Deno.test({
  name: "worker throws when executing",
  fn: async function () {
    const throwingWorker = new Worker(
      new URL("throwing_worker.js", import.meta.url).href,
      { type: "module" },
    );

    const promise = deferred();
    // deno-lint-ignore no-explicit-any
    throwingWorker.onerror = (e: any) => {
      e.preventDefault();
      promise.resolve(e.message);
    };

    assertMatch(await promise as string, /Uncaught Error: Thrown error/);
    throwingWorker.terminate();
  },
});

Deno.test({
  name: "worker globals",
  fn: async function () {
    const workerOptions: WorkerOptions = { type: "module" };
    const w = new Worker(
      new URL("worker_globals.ts", import.meta.url).href,
      workerOptions,
    );

    const promise = deferred();
    w.onmessage = (e) => {
      promise.resolve(e.data);
    };

    w.postMessage("Hello, world!");
    assertEquals(await promise, "true, true, true, true");
    w.terminate();
  },
});

Deno.test({
  name: "worker fetch API",
  fn: async function () {
    const fetchingWorker = new Worker(
      new URL("fetching_worker.js", import.meta.url).href,
      { type: "module" },
    );

    const promise = deferred();
    // deno-lint-ignore no-explicit-any
    fetchingWorker.onerror = (e: any) => {
      e.preventDefault();
      promise.reject(e.message);
    };
    // Defer promise.resolve() to allow worker to shut down
    fetchingWorker.onmessage = (e) => {
      promise.resolve(e.data);
    };

    assertEquals(await promise, "Done!");
    fetchingWorker.terminate();
  },
});

Deno.test({
  name: "worker terminate busy loop",
  fn: async function () {
    const promise = deferred();

    const busyWorker = new Worker(
      new URL("busy_worker.js", import.meta.url),
      { type: "module" },
    );

    let testResult = 0;

    busyWorker.onmessage = (e) => {
      testResult = e.data;
      if (testResult >= 10000) {
        busyWorker.terminate();
        busyWorker.onmessage = (_e) => {
          throw new Error("unreachable");
        };
        setTimeout(() => {
          promise.resolve(testResult);
        }, 100);
      }
    };

    busyWorker.postMessage("ping");
    assertEquals(await promise, 10000);
  },
});

Deno.test({
  name: "worker race condition",
  fn: async function () {
    // See issue for details
    // https://github.com/denoland/deno/issues/4080
    const promise = deferred();

    const racyWorker = new Worker(
      new URL("racy_worker.js", import.meta.url),
      { type: "module" },
    );

    racyWorker.onmessage = (_e) => {
      setTimeout(() => {
        promise.resolve();
      }, 100);
    };

    racyWorker.postMessage("START");
    await promise;
  },
});

Deno.test({
  name: "worker is event listener",
  fn: async function () {
    let messageHandlersCalled = 0;
    let errorHandlersCalled = 0;

    const promise1 = deferred();
    const promise2 = deferred();

    const worker = new Worker(
      new URL("event_worker.js", import.meta.url),
      { type: "module" },
    );

    worker.onmessage = (_e: Event) => {
      messageHandlersCalled++;
    };
    worker.addEventListener("message", (_e: Event) => {
      messageHandlersCalled++;
    });
    worker.addEventListener("message", (_e: Event) => {
      messageHandlersCalled++;
      promise1.resolve();
    });

    worker.onerror = (e) => {
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
  fn: async function () {
    const worker = new Worker(
      new URL("event_worker_scope.js", import.meta.url),
      { type: "module" },
    );

    const promise = deferred();
    worker.onmessage = (e: MessageEvent) => {
      promise.resolve(e.data);
    };
    worker.onerror = (_e) => {
      throw new Error("unreachable");
    };

    worker.postMessage("boom");
    worker.postMessage("ping");
    assertEquals(await promise, {
      messageHandlersCalled: 4,
      errorHandlersCalled: 4,
    });
    worker.terminate();
  },
});

Deno.test({
  name: "worker with Deno namespace",
  fn: async function () {
    const regularWorker = new Worker(
      new URL("non_deno_worker.js", import.meta.url),
      { type: "module" },
    );
    const denoWorker = new Worker(
      new URL("deno_worker.ts", import.meta.url),
      {
        type: "module",
        deno: {
          namespace: true,
          permissions: "inherit",
        },
      },
    );

    const promise1 = deferred();
    regularWorker.onmessage = (e) => {
      regularWorker.terminate();
      promise1.resolve(e.data);
    };

    const promise2 = deferred();
    denoWorker.onmessage = (e) => {
      denoWorker.terminate();
      promise2.resolve(e.data);
    };

    regularWorker.postMessage("Hello World");
    assertEquals(await promise1, "Hello World");
    denoWorker.postMessage("Hello World");
    assertEquals(await promise2, "Hello World");
  },
});

Deno.test({
  name: "worker with crypto in scope",
  fn: async function () {
    const w = new Worker(
      new URL("worker_crypto.js", import.meta.url).href,
      { type: "module" },
    );

    const promise = deferred();
    w.onmessage = (e) => {
      promise.resolve(e.data);
    };

    w.postMessage(null);
    assertEquals(await promise, true);
    w.terminate();
  },
});

Deno.test({
  name: "Worker event handler order",
  fn: async function () {
    const promise = deferred();
    const w = new Worker(
      new URL("test_worker.ts", import.meta.url).href,
      { type: "module", name: "tsWorker" },
    );
    const arr: number[] = [];
    w.addEventListener("message", () => arr.push(1));
    w.onmessage = (_e) => {
      arr.push(2);
    };
    w.addEventListener("message", () => arr.push(3));
    w.addEventListener("message", () => {
      promise.resolve();
    });
    w.postMessage("Hello World");
    await promise;
    assertEquals(arr, [1, 2, 3]);
    w.terminate();
  },
});

Deno.test({
  name: "Worker immediate close",
  fn: async function () {
    const promise = deferred();
    const w = new Worker(
      new URL("./immediately_close_worker.js", import.meta.url).href,
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
  fn: async function () {
    const promise = deferred();
    const worker = new Worker(
      new URL("./post_undefined.ts", import.meta.url).href,
      { type: "module" },
    );

    const handleWorkerMessage = (e: MessageEvent) => {
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
  const worker = new Worker(
    new URL("./read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "inherit",
      },
    },
  );

  const promise = deferred();
  worker.onmessage = (e) => {
    promise.resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, true);
  worker.terminate();
});

Deno.test("Worker limit children permissions", async function () {
  const worker = new Worker(
    new URL("./read_check_worker.js", import.meta.url).href,
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

  const promise = deferred();
  worker.onmessage = (e) => {
    promise.resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, false);
  worker.terminate();
});

Deno.test("Worker limit children permissions granularly", async function () {
  const worker = new Worker(
    new URL("./read_check_granular_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: {
          env: ["foo"],
          hrtime: true,
          net: ["foo", "bar:8000"],
          ffi: [new URL("foo", import.meta.url), "bar"],
          read: [new URL("foo", import.meta.url), "bar"],
          run: [new URL("foo", import.meta.url), "bar", "./baz"],
          write: [new URL("foo", import.meta.url), "bar"],
        },
      },
    },
  );
  const promise = deferred();
  worker.onmessage = ({ data }) => promise.resolve(data);
  assertEquals(await promise, {
    envGlobal: "prompt",
    envFoo: "granted",
    envAbsent: "prompt",
    hrtime: "granted",
    netGlobal: "prompt",
    netFoo: "granted",
    netFoo8000: "granted",
    netBar: "prompt",
    netBar8000: "granted",
    ffiGlobal: "prompt",
    ffiFoo: "granted",
    ffiBar: "granted",
    ffiAbsent: "prompt",
    readGlobal: "prompt",
    readFoo: "granted",
    readBar: "granted",
    readAbsent: "prompt",
    runGlobal: "prompt",
    runFoo: "granted",
    runBar: "granted",
    runBaz: "granted",
    runAbsent: "prompt",
    writeGlobal: "prompt",
    writeFoo: "granted",
    writeBar: "granted",
    writeAbsent: "prompt",
  });
  worker.terminate();
});

Deno.test("Nested worker limit children permissions", async function () {
  /** This worker has permissions but doesn't grant them to its children */
  const worker = new Worker(
    new URL("./parent_read_check_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "inherit",
      },
    },
  );
  const promise = deferred();
  worker.onmessage = ({ data }) => promise.resolve(data);
  assertEquals(await promise, {
    envGlobal: "prompt",
    envFoo: "prompt",
    envAbsent: "prompt",
    hrtime: "prompt",
    netGlobal: "prompt",
    netFoo: "prompt",
    netFoo8000: "prompt",
    netBar: "prompt",
    netBar8000: "prompt",
    ffiGlobal: "prompt",
    ffiFoo: "prompt",
    ffiBar: "prompt",
    ffiAbsent: "prompt",
    readGlobal: "prompt",
    readFoo: "prompt",
    readBar: "prompt",
    readAbsent: "prompt",
    runGlobal: "prompt",
    runFoo: "prompt",
    runBar: "prompt",
    runBaz: "prompt",
    runAbsent: "prompt",
    writeGlobal: "prompt",
    writeFoo: "prompt",
    writeBar: "prompt",
    writeAbsent: "prompt",
  });
  worker.terminate();
});

// This test relies on env permissions not being granted on main thread
Deno.test({
  name:
    "Worker initialization throws on worker permissions greater than parent thread permissions",
  permissions: { env: false },
  fn: function () {
    assertThrows(
      () => {
        const worker = new Worker(
          new URL("./deno_worker.ts", import.meta.url).href,
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
  },
});

Deno.test("Worker with disabled permissions", async function () {
  const worker = new Worker(
    new URL("./no_permissions_worker.js", import.meta.url).href,
    {
      type: "module",
      deno: {
        namespace: true,
        permissions: "none",
      },
    },
  );

  const promise = deferred();
  worker.onmessage = (e) => {
    promise.resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, true);
  worker.terminate();
});

Deno.test("Worker with invalid permission arg", function () {
  assertThrows(
    () =>
      new Worker(`data:,close();`, {
        type: "module",
        // @ts-expect-error invalid env value
        deno: { permissions: { env: "foo" } },
      }),
    TypeError,
    'Error parsing args: (deno.permissions.env) invalid value: string "foo", expected "inherit" or boolean or string[]',
  );
});

Deno.test({
  name: "worker location",
  fn: async function () {
    const promise = deferred();
    const workerModuleHref =
      new URL("worker_location.ts", import.meta.url).href;
    const w = new Worker(workerModuleHref, { type: "module" });
    w.onmessage = (e) => {
      promise.resolve(e.data);
    };
    w.postMessage("Hello, world!");
    assertEquals(await promise, `${workerModuleHref}, true`);
    w.terminate();
  },
});

Deno.test({
  name: "worker with relative specifier",
  fn: async function () {
    assertEquals(location.href, "http://127.0.0.1:4545/");
    const w = new Worker(
      "./workers/test_worker.ts",
      { type: "module", name: "tsWorker" },
    );
    const promise = deferred();
    w.onmessage = (e) => {
      promise.resolve(e.data);
    };
    w.postMessage("Hello, world!");
    assertEquals(await promise, "Hello, world!");
    w.terminate();
  },
});

Deno.test({
  name: "Worker with top-level-await",
  fn: async function () {
    const result = deferred();
    const worker = new Worker(
      new URL("worker_with_top_level_await.ts", import.meta.url).href,
      { type: "module" },
    );
    worker.onmessage = (e) => {
      if (e.data == "ready") {
        worker.postMessage("trigger worker handler");
      } else if (e.data == "triggered worker handler") {
        result.resolve();
      } else {
        result.reject(new Error("Handler didn't run during top-level delay."));
      }
    };
    await result;
    worker.terminate();
  },
});

Deno.test({
  name: "Worker with native HTTP",
  fn: async function () {
    const result = deferred();
    const worker = new Worker(
      new URL(
        "./http_worker.js",
        import.meta.url,
      ).href,
      {
        type: "module",
        deno: {
          namespace: true,
          permissions: "inherit",
        },
      },
    );
    worker.onmessage = () => {
      result.resolve();
    };
    await result;

    assert(worker);
    const response = await fetch("http://localhost:4506");
    assert(await response.arrayBuffer());
    worker.terminate();
  },
});

Deno.test({
  name: "structured cloning postMessage",
  fn: async function () {
    const worker = new Worker(
      new URL("worker_structured_cloning.ts", import.meta.url).href,
      { type: "module" },
    );

    const result = deferred();
    worker.onmessage = (e) => {
      result.resolve(e.data);
    };

    worker.postMessage("START");
    // deno-lint-ignore no-explicit-any
    const data = await result as any;
    // self field should reference itself (circular ref)
    assert(data === data.self);
    // fields a and b refer to the same array
    assertEquals(data.a, ["a", true, 432]);
    assertEquals(data.b, ["a", true, 432]);
    data.b[0] = "b";
    data.a[2] += 5;
    assertEquals(data.a, ["b", true, 437]);
    assertEquals(data.b, ["b", true, 437]);
    // c is a set
    const len = data.c.size;
    data.c.add(1); // This value is already in the set.
    data.c.add(2);
    assertEquals(len + 1, data.c.size);
    worker.terminate();
  },
});

Deno.test({
  name: "worker with relative specifier",
  fn: async function () {
    assertEquals(location.href, "http://127.0.0.1:4545/");
    const w = new Worker(
      "./workers/test_worker.ts",
      { type: "module", name: "tsWorker" },
    );
    const promise = deferred();
    w.onmessage = (e) => {
      promise.resolve(e.data);
    };
    w.postMessage("Hello, world!");
    assertEquals(await promise, "Hello, world!");
    w.terminate();
  },
});

Deno.test({
  name: "worker SharedArrayBuffer",
  fn: async function () {
    const promise = deferred();
    const workerOptions: WorkerOptions = { type: "module" };
    const w = new Worker(
      new URL("shared_array_buffer.ts", import.meta.url).href,
      workerOptions,
    );
    const sab1 = new SharedArrayBuffer(1);
    const sab2 = new SharedArrayBuffer(1);
    const bytes1 = new Uint8Array(sab1);
    const bytes2 = new Uint8Array(sab2);
    assertEquals(bytes1[0], 0);
    assertEquals(bytes2[0], 0);
    w.onmessage = () => {
      w.postMessage([sab1, sab2]);
      w.onmessage = () => {
        promise.resolve();
      };
    };
    await promise;
    assertEquals(bytes1[0], 1);
    assertEquals(bytes2[0], 2);
    w.terminate();
  },
});

Deno.test({
  name: "Send MessagePorts from / to workers",
  fn: async function () {
    const worker = new Worker(
      new URL("message_port.ts", import.meta.url).href,
      { type: "module" },
    );
    const channel = new MessageChannel();

    const promise1 = deferred();
    const promise2 = deferred();
    const promise3 = deferred();
    const result = deferred();
    worker.onmessage = (e) => {
      promise1.resolve([e.data, e.ports.length]);
      const port1 = e.ports[0];
      port1.onmessage = (e) => {
        promise2.resolve(e.data);
        port1.close();
        worker.postMessage("3", [channel.port1]);
      };
      port1.postMessage("2");
    };
    channel.port2.onmessage = (e) => {
      promise3.resolve(e.data);
      channel.port2.close();
      result.resolve();
    };

    assertEquals(await promise1, ["1", 1]);
    assertEquals(await promise2, true);
    assertEquals(await promise3, true);
    await result;
    worker.terminate();
  },
});

Deno.test({
  name: "worker Deno.memoryUsage",
  fn: async function () {
    const w = new Worker(
      /**
       * Source code
       * self.onmessage = function() {self.postMessage(Deno.memoryUsage())}
       */
      "data:application/typescript;base64,c2VsZi5vbm1lc3NhZ2UgPSBmdW5jdGlvbigpIHtzZWxmLnBvc3RNZXNzYWdlKERlbm8ubWVtb3J5VXNhZ2UoKSl9",
      { type: "module", name: "tsWorker", deno: true },
    );

    w.postMessage(null);

    const memoryUsagePromise = deferred();
    w.onmessage = function (evt) {
      memoryUsagePromise.resolve(evt.data);
    };

    assertEquals(
      Object.keys(
        await memoryUsagePromise as unknown as Record<string, number>,
      ),
      ["rss", "heapTotal", "heapUsed", "external"],
    );
    w.terminate();
  },
});
