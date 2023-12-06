// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertObjectMatch,
} from "../../../test_util/std/assert/mod.ts";
import { fromFileUrl, relative } from "../../../test_util/std/path/mod.ts";
import * as workerThreads from "node:worker_threads";
import { EventEmitter, once } from "node:events";

Deno.test("[node/worker_threads] BroadcastChannel is exported", () => {
  assertEquals<unknown>(workerThreads.BroadcastChannel, BroadcastChannel);
});

Deno.test("[node/worker_threads] MessageChannel are MessagePort are exported", () => {
  assertEquals<unknown>(workerThreads.MessageChannel, MessageChannel);
  assertEquals<unknown>(workerThreads.MessagePort, MessagePort);
});

Deno.test({
  name: "[worker_threads] isMainThread",
  fn() {
    assertEquals(workerThreads.isMainThread, true);
  },
});

Deno.test({
  name: "[worker_threads] threadId",
  fn() {
    assertEquals(workerThreads.threadId, 0);
  },
});

Deno.test({
  name: "[worker_threads] resourceLimits",
  fn() {
    assertObjectMatch(workerThreads.resourceLimits, {});
  },
});

Deno.test({
  name: "[worker_threads] parentPort",
  fn() {
    assertEquals(workerThreads.parentPort, null);
  },
});

Deno.test({
  name: "[worker_threads] workerData",
  fn() {
    assertEquals(workerThreads.workerData, null);
  },
});

Deno.test({
  name: "[worker_threads] setEnvironmentData / getEnvironmentData",
  fn() {
    workerThreads.setEnvironmentData("test", "test");
    assertEquals(workerThreads.getEnvironmentData("test"), "test");
  },
});

Deno.test({
  name: "[worker_threads] Worker threadId",
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
  name: "[worker_threads] Worker basics",
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
  name: "[worker_threads] Worker eval",
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
  name: "[worker_threads] worker thread with type module",
  fn() {
    const worker = new workerThreads.Worker(
      new URL("./testdata/worker_module/index.js", import.meta.url),
    );
    worker.terminate();
  },
});

Deno.test({
  name: "[worker_threads] inheritances",
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
  name: "[worker_threads] Worker workerData",
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
  name: "[worker_threads] Worker with relative path",
  async fn() {
    const worker = new workerThreads.Worker(relative(
      Deno.cwd(),
      fromFileUrl(new URL("./testdata/worker_threads.mjs", import.meta.url)),
    ));
    worker.postMessage("Hello, how are you my thread?");
    assertEquals((await once(worker, "message"))[0], "I'm fine!");
    worker.terminate();
  },
});
