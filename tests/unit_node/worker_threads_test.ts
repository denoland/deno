// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertObjectMatch,
  assertThrows,
  fail,
} from "@std/assert";
import { fromFileUrl, relative, SEPARATOR } from "@std/path";
import * as workerThreads from "node:worker_threads";
import { EventEmitter, once } from "node:events";
import process from "node:process";

Deno.test("[node/worker_threads] BroadcastChannel is exported", () => {
  assertEquals<unknown>(workerThreads.BroadcastChannel, BroadcastChannel);
});

Deno.test("[node/worker_threads] MessageChannel are MessagePort are exported", () => {
  assert(workerThreads.MessageChannel);
  assertEquals<unknown>(workerThreads.MessagePort, MessagePort);
});

Deno.test({
  name: "[node/worker_threads] isMainThread",
  fn() {
    assertEquals(workerThreads.isMainThread, true);
  },
});

Deno.test({
  name: "[node/worker_threads] threadId",
  fn() {
    assertEquals(workerThreads.threadId, 0);
  },
});

Deno.test({
  name: "[node/worker_threads] resourceLimits",
  fn() {
    assertObjectMatch(workerThreads.resourceLimits, {});
  },
});

Deno.test({
  name: "[node/worker_threads] parentPort",
  fn() {
    assertEquals(workerThreads.parentPort, null);
  },
});

Deno.test({
  name: "[node/worker_threads] workerData",
  fn() {
    assertEquals(workerThreads.workerData, null);
  },
});

Deno.test({
  name: "[node/worker_threads] setEnvironmentData / getEnvironmentData",
  fn() {
    workerThreads.setEnvironmentData("test", "test");
    assertEquals(workerThreads.getEnvironmentData("test"), "test");
  },
});

Deno.test({
  name: "[node/worker_threads] Worker threadId",
  async fn() {
    const worker = new workerThreads.Worker(
      new URL("./testdata/worker_threads.mjs", import.meta.url),
    );
    worker.postMessage("Hello, how are you my thread?");
    await once(worker, "message");
    const message = await once(worker, "message");
    assertEquals(message[0].threadId, 1);
    worker.terminate();

    const worker1 = new workerThreads.Worker(
      new URL("./testdata/worker_threads.mjs", import.meta.url),
    );
    worker1.postMessage("Hello, how are you my thread?");
    await once(worker1, "message");
    assertEquals((await once(worker1, "message"))[0].threadId, 2);
    worker1.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] Worker basics",
  async fn() {
    workerThreads.setEnvironmentData("test", "test");
    workerThreads.setEnvironmentData(1, {
      test: "random",
      random: "test",
    });
    const { port1 } = new MessageChannel();
    const worker = new workerThreads.Worker(
      new URL("./testdata/worker_threads.mjs", import.meta.url),
      {
        workerData: ["hey", true, false, 2, port1],
        // deno-lint-ignore no-explicit-any
        transferList: [port1 as any],
      },
    );
    worker.postMessage("Hello, how are you my thread?");
    assertEquals((await once(worker, "message"))[0], "I'm fine!");
    const data = (await once(worker, "message"))[0];
    // data.threadId can be 1 when this test is run individually
    if (data.threadId === 1) data.threadId = 3;
    assertObjectMatch(data, {
      isMainThread: false,
      threadId: 3,
      workerData: ["hey", true, false, 2],
      envData: ["test", { test: "random", random: "test" }],
    });
    worker.terminate();
  },
  sanitizeResources: false,
});

Deno.test({
  name: "[node/worker_threads] Worker eval",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      parentPort.postMessage("It works!");
      `,
      {
        eval: true,
      },
    );
    assertEquals((await once(worker, "message"))[0], "It works!");
    worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] worker thread with type module",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          new URL("./testdata/worker_module/index.js", import.meta.url),
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] worker thread in nested module",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          new URL("./testdata/worker_module/nested/index.js", import.meta.url),
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] .cjs worker file within module",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          new URL("./testdata/worker_module/cjs-file.cjs", import.meta.url),
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] relativ path string",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          "./tests/unit_node/testdata/worker_module/index.js",
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] utf-8 path string",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          "./tests/unit_node/testdata/worker_module/βάρβαροι.js",
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] utf-8 path URL",
  async fn() {
    function p() {
      return new Promise<workerThreads.Worker>((resolve, reject) => {
        const worker = new workerThreads.Worker(
          new URL(
            "./testdata/worker_module/βάρβαροι.js",
            import.meta.url,
          ),
        );
        worker.on("error", (e) => reject(e.message));
        worker.on("message", () => resolve(worker));
      });
    }
    await p();
  },
});

Deno.test({
  name: "[node/worker_threads] throws on relativ path without leading dot",
  fn() {
    assertThrows(
      () => {
        new workerThreads.Worker(
          "tests/unit_node/testdata/worker_module/index.js",
        );
      },
    );
  },
});

Deno.test({
  name: "[node/worker_threads] throws on unsupported URL protcol",
  fn() {
    assertThrows(
      () => {
        new workerThreads.Worker(new URL("https://example.com"));
      },
    );
  },
});

Deno.test({
  name: "[node/worker_threads] throws on non-existend file",
  fn() {
    assertThrows(
      () => {
        new workerThreads.Worker(new URL("file://very/unlikely"));
      },
    );
  },
});

Deno.test({
  name: "[node/worker_threads] inheritances",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { EventEmitter } from "node:events";
      import { parentPort } from "node:worker_threads";
      parentPort.postMessage(parentPort instanceof EventTarget);
      await new Promise(resolve => setTimeout(resolve, 100));
      parentPort.postMessage(parentPort instanceof EventEmitter);
      `,
      {
        eval: true,
      },
    );
    assertEquals((await once(worker, "message"))[0], true);
    assertEquals((await once(worker, "message"))[0], false);
    assert(worker instanceof EventEmitter);
    assert(!(worker instanceof EventTarget));
    worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] Worker workerData",
  async fn() {
    const worker = new workerThreads.Worker(
      new URL("./testdata/worker_threads.mjs", import.meta.url),
      {
        workerData: null,
      },
    );
    worker.postMessage("Hello, how are you my thread?");
    await once(worker, "message");
    assertEquals((await once(worker, "message"))[0].workerData, null);
    worker.terminate();

    const worker1 = new workerThreads.Worker(
      new URL("./testdata/worker_threads.mjs", import.meta.url),
    );
    worker1.postMessage("Hello, how are you my thread?");
    await once(worker1, "message");
    assertEquals((await once(worker1, "message"))[0].workerData, undefined);
    worker1.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] Worker with relative path",
  async fn() {
    const worker = new workerThreads.Worker(
      `.${SEPARATOR}` + relative(
        Deno.cwd(),
        fromFileUrl(new URL("./testdata/worker_threads.mjs", import.meta.url)),
      ),
    );
    worker.postMessage("Hello, how are you my thread?");
    assertEquals((await once(worker, "message"))[0], "I'm fine!");
    worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] unref",
  async fn() {
    const timeout = setTimeout(() => fail("Test timed out"), 60_000);
    const child = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "import { Worker } from 'node:worker_threads'; new Worker('setTimeout(() => {}, 1_000_000)', {eval:true}).unref();",
      ],
    }).spawn();
    await child.status;
    clearTimeout(timeout);
  },
});

Deno.test({
  name: "[node/worker_threads] SharedArrayBuffer",
  async fn() {
    const sab = new SharedArrayBuffer(Uint8Array.BYTES_PER_ELEMENT);
    const uint = new Uint8Array(sab);
    const worker = new workerThreads.Worker(
      new URL("./testdata/worker_threads2.mjs", import.meta.url),
      {
        workerData: { sharedArrayBuffer: sab },
      },
    );
    worker.postMessage("Hello");
    if ((await once(worker, "message"))[0] != "Hello") throw new Error();
    await new Promise((resolve) => setTimeout(resolve, 100));
    worker.terminate();
    if (uint[0] != 1) throw new Error();
  },
  sanitizeResources: false,
});

Deno.test({
  name: "[node/worker_threads] Worker workerData with MessagePort",
  async fn() {
    const { port1: mainPort, port2: workerPort } = new workerThreads
      .MessageChannel();
    const deferred = Promise.withResolvers<void>();
    const worker = new workerThreads.Worker(
      `
      import {
        isMainThread,
        MessageChannel,
        parentPort,
        receiveMessageOnPort,
        Worker,
        workerData,
      } from "node:worker_threads";
      parentPort.on("message", (msg) => {
        /* console.log("message from main", msg); */
        parentPort.postMessage("Hello from worker on parentPort!");
        workerData.workerPort.postMessage("Hello from worker on workerPort!");
      });
      `,
      {
        eval: true,
        workerData: { workerPort },
        transferList: [workerPort],
      },
    );

    worker.on("message", (data) => {
      assertEquals(data, "Hello from worker on parentPort!");
      // TODO(bartlomieju): it would be better to use `mainPort.on("message")`,
      // but we currently don't support it.
      // https://github.com/denoland/deno/issues/22951
      // Wait a bit so the message can arrive.
      setTimeout(() => {
        const msg = workerThreads.receiveMessageOnPort(mainPort)!.message;
        assertEquals(msg, "Hello from worker on workerPort!");
        deferred.resolve();
      }, 500);
    });

    worker.postMessage("Hello from parent");
    await deferred.promise;
    await worker.terminate();
    mainPort.close();
  },
});

// Regression test for https://github.com/denoland/deno/issues/23362
Deno.test("[node/worker_threads] receiveMessageOnPort works if there's pending read", function () {
  const { port1, port2 } = new workerThreads.MessageChannel();
  const { port1: port3, port2: port4 } = new workerThreads.MessageChannel();
  const { port1: port5, port2: port6 } = new workerThreads.MessageChannel();

  const message1 = { hello: "world" };
  const message2 = { foo: "bar" };

  assertEquals(workerThreads.receiveMessageOnPort(port2), undefined);
  port2.start();
  port4.start();
  port6.start();

  port1.postMessage(message1);
  port1.postMessage(message2);
  port3.postMessage(message1);
  port3.postMessage(message2);
  port5.postMessage(message1);
  port5.postMessage(message2);
  assertEquals(workerThreads.receiveMessageOnPort(port2), {
    message: message1,
  });
  assertEquals(workerThreads.receiveMessageOnPort(port2), {
    message: message2,
  });
  assertEquals(workerThreads.receiveMessageOnPort(port4), {
    message: message1,
  });
  assertEquals(workerThreads.receiveMessageOnPort(port4), {
    message: message2,
  });
  assertEquals(workerThreads.receiveMessageOnPort(port6), {
    message: message1,
  });
  assertEquals(workerThreads.receiveMessageOnPort(port6), {
    message: message2,
  });
  port1.close();
  port2.close();
  port3.close();
  port4.close();
  port5.close();
  port6.close();
});

Deno.test({
  name: "[node/worker_threads] Worker env",
  async fn() {
    const deferred = Promise.withResolvers<void>();
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      import process from "node:process";
      parentPort.postMessage(process.env.TEST_ENV);
      `,
      {
        eval: true,
        env: { TEST_ENV: "test" },
      },
    );

    worker.on("message", (data) => {
      assertEquals(data, "test");
      deferred.resolve();
    });

    await deferred.promise;
    await worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] Worker env using process.env",
  async fn() {
    const deferred = Promise.withResolvers<void>();
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      import process from "node:process";
      parentPort.postMessage("ok");
      `,
      {
        eval: true,
        // Make sure this doesn't throw `DataCloneError`.
        // See https://github.com/denoland/deno/issues/23522.
        env: process.env,
      },
    );

    worker.on("message", (data) => {
      assertEquals(data, "ok");
      deferred.resolve();
    });

    await deferred.promise;
    await worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] Returns terminate promise with exit code",
  async fn() {
    const deferred = Promise.withResolvers<void>();
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      parentPort.postMessage("ok");
      `,
      {
        eval: true,
      },
    );

    worker.on("message", (data) => {
      assertEquals(data, "ok");
      deferred.resolve();
    });

    await deferred.promise;
    const promise = worker.terminate();
    assertEquals(typeof promise.then, "function");
    assertEquals(await promise, 0);
  },
});

Deno.test({
  name:
    "[node/worker_threads] MessagePort.on all message listeners are invoked",
  async fn() {
    const output: string[] = [];
    const deferred = Promise.withResolvers<void>();
    const { port1, port2 } = new workerThreads.MessageChannel();
    port1.on("message", (msg) => output.push(msg));
    port1.on("message", (msg) => output.push(msg + 2));
    port1.on("message", (msg) => {
      output.push(msg + 3);
      deferred.resolve();
    });
    port2.postMessage("hi!");
    await deferred.promise;
    assertEquals(output, ["hi!", "hi!2", "hi!3"]);
    port2.close();
    port1.close();
  },
});

// Test for https://github.com/denoland/deno/issues/23854
Deno.test({
  name: "[node/worker_threads] MessagePort.addListener is present",
  async fn() {
    const channel = new workerThreads.MessageChannel();
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      parentPort.addListener("message", message => {
        if (message.foo) {
          const success = typeof message.foo.bar.addListener === "function";
          parentPort.postMessage(success ? "it works" : "it doesn't work")
        }
      })
      `,
      {
        eval: true,
      },
    );
    worker.postMessage({ foo: { bar: channel.port1 } }, [channel.port1]);

    assertEquals((await once(worker, "message"))[0], "it works");
    worker.terminate();
    channel.port1.close();
    channel.port2.close();
  },
});

Deno.test({
  name: "[node/worker_threads] Emits online event",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      const p = Promise.withResolvers();
      let ok = false;
      parentPort.on("message", () => {
        ok = true;
        p.resolve();
      });
      await Promise.race([p.promise, new Promise(resolve => setTimeout(resolve, 20000))]);
      if (ok) {
        parentPort.postMessage("ok");
      } else {
        parentPort.postMessage("timed out");
      }
      `,
      {
        eval: true,
      },
    );
    worker.on("online", () => {
      worker.postMessage("ok");
    });
    assertEquals((await once(worker, "message"))[0], "ok");
    worker.terminate();
  },
});

Deno.test({
  name: "[node/worker_threads] receiveMessageOnPort doesn't exit receive loop",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { parentPort, receiveMessageOnPort } from "node:worker_threads";
      parentPort.on("message", (msg) => {
        const port = msg.port;
        port.on("message", (msg2) => {
          if (msg2 === "c") {
            port.postMessage("done");
            port.unref();
            parentPort.unref();
          }
        });
        parentPort.postMessage("ready");
        const msg2 = receiveMessageOnPort(port);
      });
      `,
      { eval: true },
    );

    const { port1, port2 } = new workerThreads.MessageChannel();

    worker.postMessage({ port: port2 }, [port2]);

    const done = Promise.withResolvers<boolean>();

    port1.on("message", (msg) => {
      assertEquals(msg, "done");
      worker.unref();
      port1.close();
      done.resolve(true);
    });
    worker.on("message", (msg) => {
      assertEquals(msg, "ready");
      port1.postMessage("a");
      port1.postMessage("b");
      port1.postMessage("c");
    });

    const timeout = setTimeout(() => {
      fail("Test timed out");
    }, 20_000);
    try {
      const result = await done.promise;
      assertEquals(result, true);
    } finally {
      clearTimeout(timeout);
    }
  },
});

Deno.test({
  name: "[node/worker_threads] MessagePort.unref doesn't exit receive loop",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { parentPort } from "node:worker_threads";
      const assertEquals = (a, b) => {
        if (a !== b) {
          throw new Error();
        }
      };
      let state = 0;
      parentPort.on("message", (msg) => {
        const port = msg.port;
        const expect = ["a", "b", "c"];
        port.on("message", (msg2) => {
          assertEquals(msg2, expect[state++]);
          if (msg2 === "c") {
            port.postMessage({ type: "done", got: msg2 });
            parentPort.unref();
          }
        });
        port.unref();
        parentPort.postMessage("ready");
      });
      `,
      { eval: true },
    );

    const { port1, port2 } = new workerThreads.MessageChannel();

    const done = Promise.withResolvers<boolean>();

    port1.on("message", (msg) => {
      assertEquals(msg.type, "done");
      assertEquals(msg.got, "c");
      worker.unref();
      port1.close();
      done.resolve(true);
    });
    worker.on("message", (msg) => {
      assertEquals(msg, "ready");
      port1.postMessage("a");
      port1.postMessage("b");
      port1.postMessage("c");
    });
    worker.postMessage({ port: port2 }, [port2]);

    const timeout = setTimeout(() => {
      fail("Test timed out");
    }, 20_000);
    try {
      const result = await done.promise;
      assertEquals(result, true);
    } finally {
      clearTimeout(timeout);
    }
  },
});

Deno.test({
  name: "[node/worker_threads] npm:piscina wait loop hang regression",
  async fn() {
    const worker = new workerThreads.Worker(
      `
      import { assert, assertEquals } from "@std/assert";
      import { parentPort, receiveMessageOnPort } from "node:worker_threads";

      assert(parentPort !== null);

      let currentTasks = 0;
      let lastSeen = 0;

      parentPort.on("message", (msg) => {
        (async () => {
          assert(typeof msg === "object" && msg !== null);
          assert(msg.buf !== undefined);
          assert(msg.port !== undefined);
          const { buf, port } = msg;
          port.postMessage("ready");
          port.on("message", (msg) => onMessage(msg, buf, port));
          atomicsWaitLoop(buf, port);
        })();
      });

      function onMessage(msg, buf, port) {
        currentTasks++;
        (async () => {
          assert(msg.taskName !== undefined);
          port.postMessage({ type: "response", taskName: msg.taskName });
          currentTasks--;
          atomicsWaitLoop(buf, port);
        })();
      }

      function atomicsWaitLoop(buf, port) {
        while (currentTasks === 0) {
          Atomics.wait(buf, 0, lastSeen);
          lastSeen = Atomics.load(buf, 0);
          let task;
          while ((task = receiveMessageOnPort(port)) !== undefined) {
            onMessage(task.message, buf, port);
          }
        }
      }
      `,
      { eval: true },
    );

    const sab = new SharedArrayBuffer(4);
    const buf = new Int32Array(sab);
    const { port1, port2 } = new workerThreads.MessageChannel();

    const done = Promise.withResolvers<boolean>();

    port1.unref();

    worker.postMessage({
      type: "init",
      buf,
      port: port2,
    }, [port2]);

    let count = 0;
    port1.on("message", (msg) => {
      if (count++ === 0) {
        assertEquals(msg, "ready");
      } else {
        assertEquals(msg.type, "response");
        port1.close();
        done.resolve(true);
      }
    });

    port1.postMessage({
      taskName: "doThing",
    });

    Atomics.add(buf, 0, 1);
    Atomics.notify(buf, 0, 1);

    worker.unref();

    const result = await done.promise;
    assertEquals(result, true);
  },
});
