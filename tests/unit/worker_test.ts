// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

// Requires to be run with `--allow-net` flag

import { assert, assertEquals, assertMatch, assertThrows } from "@std/assert";
import { toFileUrl } from "@std/path/to-file-url";

function resolveWorker(worker: string): string {
  return import.meta.resolve(`../testdata/workers/${worker}`);
}

Deno.test(
  { permissions: { read: true } },
  function utimeSyncFileSuccess() {
    const w = new Worker(
      resolveWorker("worker_types.ts"),
      { type: "module" },
    );
    assert(w);
    w.terminate();
  },
);

Deno.test({
  name: "worker terminate",
  fn: async function () {
    const jsWorker = new Worker(
      resolveWorker("test_worker.js"),
      { type: "module" },
    );
    const tsWorker = new Worker(
      resolveWorker("test_worker.ts"),
      { type: "module", name: "tsWorker" },
    );

    const deferred1 = Promise.withResolvers<string>();
    jsWorker.onmessage = (e) => {
      deferred1.resolve(e.data);
    };

    const deferred2 = Promise.withResolvers<string>();
    tsWorker.onmessage = (e) => {
      deferred2.resolve(e.data);
    };

    jsWorker.postMessage("Hello World");
    assertEquals(await deferred1.promise, "Hello World");
    tsWorker.postMessage("Hello World");
    assertEquals(await deferred2.promise, "Hello World");
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

    const { promise, resolve } = Promise.withResolvers<string>();
    tsWorker.onmessage = (e) => {
      resolve(e.data);
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
      resolveWorker("nested_worker.js"),
      { type: "module", name: "nested" },
    );

    // deno-lint-ignore no-explicit-any
    const { promise, resolve } = Promise.withResolvers<any>();
    nestedWorker.onmessage = (e) => {
      resolve(e.data);
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
      resolveWorker("throwing_worker.js"),
      { type: "module" },
    );

    const { promise, resolve } = Promise.withResolvers<string>();
    // deno-lint-ignore no-explicit-any
    throwingWorker.onerror = (e: any) => {
      e.preventDefault();
      resolve(e.message);
    };

    assertMatch(
      await promise as string,
      /Uncaught \(in promise\) Error: Thrown error/,
    );
    throwingWorker.terminate();
  },
});

Deno.test({
  name: "worker globals",
  fn: async function () {
    const workerOptions: WorkerOptions = { type: "module" };
    const w = new Worker(
      resolveWorker("worker_globals.ts"),
      workerOptions,
    );

    const { promise, resolve } = Promise.withResolvers<string>();
    w.onmessage = (e) => {
      resolve(e.data);
    };

    w.postMessage("Hello, world!");
    assertEquals(await promise, "true, true, true, true");
    w.terminate();
  },
});

Deno.test({
  name: "worker navigator",
  fn: async function () {
    const workerOptions: WorkerOptions = { type: "module" };
    const w = new Worker(
      resolveWorker("worker_navigator.ts"),
      workerOptions,
    );

    const { promise, resolve } = Promise.withResolvers<string>();
    w.onmessage = (e) => {
      resolve(e.data);
    };

    w.postMessage("Hello, world!");
    assertEquals(await promise, "string, object, string, number");
    w.terminate();
  },
});

Deno.test({
  name: "worker fetch API",
  fn: async function () {
    const fetchingWorker = new Worker(
      resolveWorker("fetching_worker.js"),
      { type: "module" },
    );

    const { promise, resolve, reject } = Promise.withResolvers<string>();
    // deno-lint-ignore no-explicit-any
    fetchingWorker.onerror = (e: any) => {
      e.preventDefault();
      reject(e.message);
    };
    // Defer promise.resolve() to allow worker to shut down
    fetchingWorker.onmessage = (e) => {
      resolve(e.data);
    };

    assertEquals(await promise, "Done!");
    fetchingWorker.terminate();
  },
});

Deno.test({
  name: "worker terminate busy loop",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<number>();

    const busyWorker = new Worker(
      resolveWorker("busy_worker.js"),
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
          resolve(testResult);
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
    const { promise, resolve } = Promise.withResolvers<void>();

    const racyWorker = new Worker(
      resolveWorker("racy_worker.js"),
      { type: "module" },
    );

    racyWorker.onmessage = (_e) => {
      setTimeout(() => {
        resolve();
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

    const deferred1 = Promise.withResolvers<void>();
    const deferred2 = Promise.withResolvers<void>();

    const worker = new Worker(
      resolveWorker("event_worker.js"),
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
      deferred1.resolve();
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
      deferred2.resolve();
    });

    worker.postMessage("ping");
    await deferred1.promise;
    assertEquals(messageHandlersCalled, 3);

    worker.postMessage("boom");
    await deferred2.promise;
    assertEquals(errorHandlersCalled, 3);
    worker.terminate();
  },
});

Deno.test({
  name: "worker scope is event listener",
  fn: async function () {
    const worker = new Worker(
      resolveWorker("event_worker_scope.js"),
      { type: "module" },
    );

    // deno-lint-ignore no-explicit-any
    const { promise, resolve } = Promise.withResolvers<any>();
    worker.onmessage = (e: MessageEvent) => {
      resolve(e.data);
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
    const denoWorker = new Worker(
      resolveWorker("deno_worker.ts"),
      { type: "module", deno: { permissions: "inherit" } },
    );

    const { promise, resolve } = Promise.withResolvers<string>();
    denoWorker.onmessage = (e) => {
      denoWorker.terminate();
      resolve(e.data);
    };

    denoWorker.postMessage("Hello World");
    assertEquals(await promise, "Hello World");
  },
});

Deno.test({
  name: "worker with crypto in scope",
  fn: async function () {
    const w = new Worker(
      resolveWorker("worker_crypto.js"),
      { type: "module" },
    );

    const { promise, resolve } = Promise.withResolvers<boolean>();
    w.onmessage = (e) => {
      resolve(e.data);
    };

    w.postMessage(null);
    assertEquals(await promise, true);
    w.terminate();
  },
});

Deno.test({
  name: "Worker event handler order",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<void>();
    const w = new Worker(
      resolveWorker("test_worker.ts"),
      { type: "module", name: "tsWorker" },
    );
    const arr: number[] = [];
    w.addEventListener("message", () => arr.push(1));
    w.onmessage = (_e) => {
      arr.push(2);
    };
    w.addEventListener("message", () => arr.push(3));
    w.addEventListener("message", () => {
      resolve();
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
    const { promise, resolve } = Promise.withResolvers<void>();
    const w = new Worker(
      resolveWorker("immediately_close_worker.js"),
      { type: "module" },
    );
    setTimeout(() => {
      resolve();
    }, 1000);
    await promise;
    w.terminate();
  },
});

Deno.test({
  name: "Worker post undefined",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<void>();
    const worker = new Worker(
      resolveWorker("post_undefined.ts"),
      { type: "module" },
    );

    const handleWorkerMessage = (e: MessageEvent) => {
      console.log("main <- worker:", e.data);
      worker.terminate();
      resolve();
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
    resolveWorker("read_check_worker.js"),
    { type: "module", deno: { permissions: "inherit" } },
  );

  const { promise, resolve } = Promise.withResolvers<boolean>();
  worker.onmessage = (e) => {
    resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, true);
  worker.terminate();
});

Deno.test("Worker limit children permissions", async function () {
  const worker = new Worker(
    resolveWorker("read_check_worker.js"),
    { type: "module", deno: { permissions: { read: false } } },
  );

  const { promise, resolve } = Promise.withResolvers<boolean>();
  worker.onmessage = (e) => {
    resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, false);
  worker.terminate();
});

function setupReadCheckGranularWorkerTest() {
  const tempDir = Deno.realPathSync(Deno.makeTempDirSync());
  const initialPath = Deno.env.get("PATH")!;
  const initialCwd = Deno.cwd();
  Deno.chdir(tempDir);
  const envSep = Deno.build.os === "windows" ? ";" : ":";
  Deno.env.set("PATH", initialPath + envSep + tempDir);

  // create executables that will be resolved when doing `which`
  const ext = Deno.build.os === "windows" ? ".exe" : "";
  Deno.copyFileSync(Deno.execPath(), tempDir + "/bar" + ext);

  return {
    tempDir,
    runFooFilePath: tempDir + "/foo" + ext,
    [Symbol.dispose]() {
      Deno.removeSync(tempDir, { recursive: true });
      Deno.env.set("PATH", initialPath);
      Deno.chdir(initialCwd);
    },
  };
}

Deno.test("Worker limit children permissions granularly", async function () {
  const ctx = setupReadCheckGranularWorkerTest();
  const workerUrl = resolveWorker("read_check_granular_worker.js");
  const worker = new Worker(
    workerUrl,
    {
      type: "module",
      deno: {
        permissions: {
          env: ["foo"],
          net: ["foo", "bar:8000"],
          ffi: [new URL("foo", workerUrl), "bar"],
          read: [new URL("foo", workerUrl), "bar", ctx.tempDir],
          run: [
            toFileUrl(ctx.runFooFilePath),
            "bar",
            "./baz",
            "unresolved-exec",
          ],
          write: [new URL("foo", workerUrl), "bar"],
        },
      },
    },
  );
  // deno-lint-ignore no-explicit-any
  const { promise, resolve } = Promise.withResolvers<any>();
  worker.onmessage = ({ data }) => resolve(data);
  assertEquals(await promise, {
    envGlobal: "prompt",
    envFoo: "granted",
    envAbsent: "prompt",
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
    runFooPath: "granted",
    runBar: "granted",
    runBaz: "granted",
    runUnresolved: "prompt", // unresolved binaries remain as "prompt"
    runAbsent: "prompt",
    writeGlobal: "prompt",
    writeFoo: "granted",
    writeBar: "granted",
    writeAbsent: "prompt",
  });
  worker.terminate();
});

Deno.test("Nested worker limit children permissions", async function () {
  const _cleanup = setupReadCheckGranularWorkerTest();
  /** This worker has permissions but doesn't grant them to its children */
  const worker = new Worker(
    resolveWorker("parent_read_check_worker.js"),
    { type: "module", deno: { permissions: "inherit" } },
  );
  // deno-lint-ignore no-explicit-any
  const { promise, resolve } = Promise.withResolvers<any>();
  worker.onmessage = ({ data }) => resolve(data);
  assertEquals(await promise, {
    envGlobal: "prompt",
    envFoo: "prompt",
    envAbsent: "prompt",
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
    runFooPath: "prompt",
    runBar: "prompt",
    runBaz: "prompt",
    runUnresolved: "prompt",
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
          resolveWorker("deno_worker.ts"),
          { type: "module", deno: { permissions: { env: true } } },
        );
        worker.terminate();
      },
      Deno.errors.NotCapable,
      "Can't escalate parent thread permissions",
    );
  },
});

Deno.test("Worker with disabled permissions", async function () {
  const worker = new Worker(
    resolveWorker("no_permissions_worker.js"),
    { type: "module", deno: { permissions: "none" } },
  );

  const { promise, resolve } = Promise.withResolvers<boolean>();
  worker.onmessage = (e) => {
    resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, true);
  worker.terminate();
});

Deno.test("Worker permissions are not inherited with empty permission object", async function () {
  const worker = new Worker(
    resolveWorker("permission_echo.js"),
    { type: "module", deno: { permissions: {} } },
  );

  // deno-lint-ignore no-explicit-any
  const { promise, resolve } = Promise.withResolvers<any>();
  worker.onmessage = (e) => {
    resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, {
    env: "prompt",
    net: "prompt",
    ffi: "prompt",
    read: "prompt",
    run: "prompt",
    write: "prompt",
  });
  worker.terminate();
});

Deno.test("Worker permissions are not inherited with single specified permission", async function () {
  const worker = new Worker(
    resolveWorker("permission_echo.js"),
    { type: "module", deno: { permissions: { net: true } } },
  );

  // deno-lint-ignore no-explicit-any
  const { promise, resolve } = Promise.withResolvers<any>();
  worker.onmessage = (e) => {
    resolve(e.data);
  };

  worker.postMessage(null);
  assertEquals(await promise, {
    env: "prompt",
    net: "granted",
    ffi: "prompt",
    read: "prompt",
    run: "prompt",
    write: "prompt",
  });
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
    '(deno.permissions.env) invalid value: string "foo", expected "inherit" or boolean or string[]',
  );
});

Deno.test({
  name: "worker location",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<string>();
    const workerModuleHref = resolveWorker("worker_location.ts");
    const w = new Worker(workerModuleHref, { type: "module" });
    w.onmessage = (e) => {
      resolve(e.data);
    };
    w.postMessage("Hello, world!");
    assertEquals(await promise, `${workerModuleHref}, true`);
    w.terminate();
  },
});

Deno.test({
  name: "Worker with top-level-await",
  fn: async function () {
    const { promise, resolve, reject } = Promise.withResolvers<void>();
    const worker = new Worker(
      resolveWorker("worker_with_top_level_await.ts"),
      { type: "module" },
    );
    worker.onmessage = (e) => {
      if (e.data == "ready") {
        worker.postMessage("trigger worker handler");
      } else if (e.data == "triggered worker handler") {
        resolve();
      } else {
        reject(new Error("Handler didn't run during top-level delay."));
      }
    };
    await promise;
    worker.terminate();
  },
});

Deno.test({
  name: "Worker with native HTTP",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<void>();
    const worker = new Worker(
      resolveWorker("http_worker.js"),
      { type: "module", deno: { permissions: "inherit" } },
    );
    worker.onmessage = () => {
      resolve();
    };
    await promise;

    assert(worker);
    const response = await fetch("http://localhost:4506");
    assert(await response.bytes());
    worker.terminate();
  },
});

Deno.test({
  name: "structured cloning postMessage",
  fn: async function () {
    const worker = new Worker(
      resolveWorker("worker_structured_cloning.ts"),
      { type: "module" },
    );

    // deno-lint-ignore no-explicit-any
    const { promise, resolve } = Promise.withResolvers<any>();
    worker.onmessage = (e) => {
      resolve(e.data);
    };

    worker.postMessage("START");
    const data = await promise;
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
    const { promise, resolve } = Promise.withResolvers<string>();
    w.onmessage = (e) => {
      resolve(e.data);
    };
    w.postMessage("Hello, world!");
    assertEquals(await promise, "Hello, world!");
    w.terminate();
  },
});

Deno.test({
  name: "worker SharedArrayBuffer",
  fn: async function () {
    const { promise, resolve } = Promise.withResolvers<void>();
    const workerOptions: WorkerOptions = { type: "module" };
    const w = new Worker(
      resolveWorker("shared_array_buffer.ts"),
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
        resolve();
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
      resolveWorker("message_port.ts"),
      { type: "module" },
    );
    const channel = new MessageChannel();

    // deno-lint-ignore no-explicit-any
    const deferred1 = Promise.withResolvers<any>();
    const deferred2 = Promise.withResolvers<boolean>();
    const deferred3 = Promise.withResolvers<boolean>();
    const result = Promise.withResolvers<void>();
    worker.onmessage = (e) => {
      deferred1.resolve([e.data, e.ports.length]);
      const port1 = e.ports[0];
      port1.onmessage = (e) => {
        deferred2.resolve(e.data);
        port1.close();
        worker.postMessage("3", [channel.port1]);
      };
      port1.postMessage("2");
    };
    channel.port2.onmessage = (e) => {
      deferred3.resolve(e.data);
      channel.port2.close();
      result.resolve();
    };

    assertEquals(await deferred1.promise, ["1", 1]);
    assertEquals(await deferred2.promise, true);
    assertEquals(await deferred3.promise, true);
    await result.promise;
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
      { type: "module", name: "tsWorker" },
    );

    w.postMessage(null);

    // deno-lint-ignore no-explicit-any
    const { promise, resolve } = Promise.withResolvers<any>();
    w.onmessage = function (evt) {
      resolve(evt.data);
    };

    assertEquals(
      Object.keys(
        await promise as unknown as Record<string, number>,
      ),
      ["rss", "heapTotal", "heapUsed", "external"],
    );
    w.terminate();
  },
});
