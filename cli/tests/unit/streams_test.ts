// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { fail } from "https://deno.land/std@v0.42.0/testing/asserts.ts";
import { assertEquals, Deferred, deferred } from "./test_util.ts";

const {
  core,
  resourceForReadableStream,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

const LOREM =
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

// Hello world, with optional close
// deno-lint-ignore no-explicit-any
function helloWorldStream(close?: boolean, completion?: Deferred<any>) {
  return new ReadableStream({
    start(controller) {
      controller.enqueue("hello, world");
      if (close == true) {
        controller.close();
      }
    },
    cancel(reason) {
      completion?.resolve(reason);
    },
  }).pipeThrough(new TextEncoderStream());
}

// Hello world, with optional close
function errorStream(type: "string" | "controller" | "TypeError") {
  return new ReadableStream({
    start(controller) {
      controller.enqueue("hello, world");
    },
    pull(controller) {
      if (type == "string") {
        throw "Uh oh (string)!";
      }
      if (type == "TypeError") {
        throw TypeError("Uh oh (TypeError)!");
      }
      controller.error("Uh oh (controller)!");
    },
  }).pipeThrough(new TextEncoderStream());
}

// Long stream with Lorem Ipsum text.
function longStream() {
  return new ReadableStream({
    start(controller) {
      for (let i = 0; i < 4; i++) {
        setTimeout(() => {
          controller.enqueue(LOREM);
          if (i == 3) {
            controller.close();
          }
        }, i * 100);
      }
    },
  }).pipeThrough(new TextEncoderStream());
}

// Empty stream, closes either immediately or on a call to pull.
function emptyStream(onPull: boolean) {
  return new ReadableStream({
    start(controller) {
      if (!onPull) {
        controller.close();
      }
    },
    pull(controller) {
      if (onPull) {
        controller.close();
      }
    },
  }).pipeThrough(new TextEncoderStream());
}

// Include an empty chunk
function emptyChunkStream() {
  return new ReadableStream({
    start(controller) {
      controller.enqueue(new Uint8Array([1]));
      controller.enqueue(new Uint8Array([]));
      controller.enqueue(new Uint8Array([2]));
      controller.close();
    },
  });
}

// Creates a stream with the given number of packets, a configurable delay between packets, and a final
// action (either "Throw" or "Close").
function makeStreamWithCount(
  count: number,
  delay: number,
  action: "Throw" | "Close",
): ReadableStream {
  function doAction(controller: ReadableStreamDefaultController, i: number) {
    if (i == count) {
      if (action == "Throw") {
        controller.error(new Error("Expected error!"));
      } else {
        controller.close();
      }
    } else {
      controller.enqueue(String.fromCharCode("a".charCodeAt(0) + i));

      if (delay == 0) {
        doAction(controller, i + 1);
      } else {
        setTimeout(() => doAction(controller, i + 1), delay);
      }
    }
  }

  return new ReadableStream({
    start(controller) {
      if (delay == 0) {
        doAction(controller, 0);
      } else {
        setTimeout(() => doAction(controller, 0), delay);
      }
    },
  }).pipeThrough(new TextEncoderStream());
}

// Normal stream operation
Deno.test(async function readableStream() {
  const rid = resourceForReadableStream(helloWorldStream());
  const buffer = new Uint8Array(1024);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 12);
  core.ops.op_close(rid);
});

// Close the stream after reading everything
Deno.test(async function readableStreamClose() {
  const cancel = deferred();
  const rid = resourceForReadableStream(helloWorldStream(false, cancel));
  const buffer = new Uint8Array(1024);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 12);
  core.ops.op_close(rid);
  assertEquals(await cancel, undefined);
});

// Close the stream without reading everything
Deno.test(async function readableStreamClosePartialRead() {
  const cancel = deferred();
  const rid = resourceForReadableStream(helloWorldStream(false, cancel));
  const buffer = new Uint8Array(5);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 5);
  core.ops.op_close(rid);
  assertEquals(await cancel, undefined);
});

// Close the stream without reading anything
Deno.test(async function readableStreamCloseWithoutRead() {
  const cancel = deferred();
  const rid = resourceForReadableStream(helloWorldStream(false, cancel));
  core.ops.op_close(rid);
  assertEquals(await cancel, undefined);
});

Deno.test(async function readableStreamPartial() {
  const rid = resourceForReadableStream(helloWorldStream());
  const buffer = new Uint8Array(5);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 5);
  const buffer2 = new Uint8Array(1024);
  const nread2 = await core.ops.op_read(rid, buffer2);
  assertEquals(nread2, 7);
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamLongReadAll() {
  const rid = resourceForReadableStream(longStream());
  const buffer = await core.ops.op_read_all(rid);
  assertEquals(buffer.length, LOREM.length * 4);
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamLongByPiece() {
  const rid = resourceForReadableStream(longStream());
  let total = 0;
  for (let i = 0; i < 100; i++) {
    const length = await core.ops.op_read(rid, new Uint8Array(16));
    total += length;
    if (length == 0) {
      break;
    }
  }
  assertEquals(total, LOREM.length * 4);
  core.ops.op_close(rid);
});

for (
  const type of [
    "string",
    "TypeError",
    "controller",
  ] as ("string" | "TypeError" | "controller")[]
) {
  Deno.test(`readableStreamError_${type}`, async function () {
    const rid = resourceForReadableStream(errorStream(type));
    assertEquals(12, await core.ops.op_read(rid, new Uint8Array(16)));
    try {
      await core.ops.op_read(rid, new Uint8Array(1));
      fail();
    } catch (e) {
      assertEquals(e.message, `Uh oh (${type})!`);
    }
    core.ops.op_close(rid);
  });
}

Deno.test(async function readableStreamEmptyOnStart() {
  const rid = resourceForReadableStream(emptyStream(true));
  const buffer = new Uint8Array(1024);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 0);
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamEmptyOnPull() {
  const rid = resourceForReadableStream(emptyStream(false));
  const buffer = new Uint8Array(1024);
  const nread = await core.ops.op_read(rid, buffer);
  assertEquals(nread, 0);
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamEmptyReadAll() {
  const rid = resourceForReadableStream(emptyStream(false));
  const buffer = await core.ops.op_read_all(rid);
  assertEquals(buffer.length, 0);
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamWithEmptyChunk() {
  const rid = resourceForReadableStream(emptyChunkStream());
  const buffer = await core.ops.op_read_all(rid);
  assertEquals(buffer, new Uint8Array([1, 2]));
  core.ops.op_close(rid);
});

Deno.test(async function readableStreamWithEmptyChunkOneByOne() {
  const rid = resourceForReadableStream(emptyChunkStream());
  assertEquals(1, await core.ops.op_read(rid, new Uint8Array(1)));
  assertEquals(1, await core.ops.op_read(rid, new Uint8Array(1)));
  assertEquals(0, await core.ops.op_read(rid, new Uint8Array(1)));
  core.ops.op_close(rid);
});

for (const count of [0, 1, 2, 3]) {
  for (const delay of [0, 1, 10]) {
    // Creating a stream that errors in start will throw
    if (delay > 0) {
      createStreamTest(count, delay, "Throw");
    }
    createStreamTest(count, delay, "Close");
  }
}

function createStreamTest(
  count: number,
  delay: number,
  action: "Throw" | "Close",
) {
  Deno.test(`streamCount${count}Delay${delay}${action}`, async () => {
    let rid;
    try {
      rid = resourceForReadableStream(
        makeStreamWithCount(count, delay, action),
      );
      for (let i = 0; i < count; i++) {
        const buffer = new Uint8Array(1);
        await core.ops.op_read(rid, buffer);
      }
      if (action == "Throw") {
        try {
          const buffer = new Uint8Array(1);
          assertEquals(1, await core.ops.op_read(rid, buffer));
          fail();
        } catch (e) {
          // We expect this to be thrown
          assertEquals(e.message, "Expected error!");
        }
      } else {
        const buffer = new Uint8Array(1);
        assertEquals(0, await core.ops.op_read(rid, buffer));
      }
    } finally {
      core.ops.op_close(rid);
    }
  });
}
