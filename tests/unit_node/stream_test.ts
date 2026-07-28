// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "@std/assert";
import { fromFileUrl, relative } from "@std/path";
import { finished, pipeline } from "node:stream/promises";
import {
  Duplex,
  finished as finishedCallback,
  getDefaultHighWaterMark,
  promises,
  Readable,
  Stream,
  Writable,
} from "node:stream";
import { TextEncoderStream } from "node:stream/web";
import { createReadStream, createWriteStream } from "node:fs";
import { EventEmitter } from "node:events";

Deno.test("stream/promises pipeline", async () => {
  const filePath = relative(
    Deno.cwd(),
    fromFileUrl(new URL("./testdata/lorem_ipsum.txt", import.meta.url)),
  );
  const input = createReadStream(filePath);
  const output = createWriteStream("lorem_ipsum.txt.copy");

  await pipeline(input, output);

  const content = Deno.readTextFileSync("lorem_ipsum.txt.copy");
  assert(content.startsWith("Lorem ipsum dolor sit amet"));
  try {
    Deno.removeSync("lorem_ipsum.txt.copy");
  } catch {
    // pass
  }
});

Deno.test("stream getDefaultHighWaterMark", () => {
  assertEquals(
    getDefaultHighWaterMark(false),
    Deno.build.os === "windows" ? 16 * 1024 : 64 * 1024,
  );
  assertEquals(getDefaultHighWaterMark(true), 16);
});

Deno.test("stream is an instance of EventEmitter", () => {
  const stream = new Stream();
  assert(stream instanceof EventEmitter);
});

Deno.test("finished on web streams", async () => {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue("asd");
      controller.close();
    },
  });
  const promise = finished(stream as unknown as NodeJS.ReadableStream);
  for await (const chunk of stream) {
    assertEquals(chunk, "asd");
  }
  await promise;
});

Deno.test("finished cleanup removes web stream abort listener", async () => {
  let abortListener: EventListener | undefined;
  let removeCount = 0;
  const signal = {
    aborted: false,
    reason: undefined,
    addEventListener(_type: string, listener: EventListener) {
      abortListener = listener;
    },
    removeEventListener(_type: string, listener: EventListener) {
      assertEquals(listener, abortListener);
      removeCount++;
    },
  } as unknown as AbortSignal;
  let streamController!: ReadableStreamDefaultController;
  const stream = new ReadableStream({
    start(controller) {
      streamController = controller;
    },
  });
  let callbackCount = 0;

  const cleanup = finishedCallback(
    stream as unknown as NodeJS.ReadableStream,
    { signal },
    () => callbackCount++,
  );
  cleanup();
  cleanup();

  assertEquals(removeCount, 1);
  abortListener?.(new Event("abort"));
  streamController.close();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assertEquals(callbackCount, 0);
});

Deno.test("finished promise cleanup works for web streams", async () => {
  let abortListener: EventListener | undefined;
  let removeCount = 0;
  const signal = {
    aborted: false,
    reason: undefined,
    addEventListener(_type: string, listener: EventListener) {
      abortListener = listener;
    },
    removeEventListener(_type: string, listener: EventListener) {
      assertEquals(listener, abortListener);
      removeCount++;
    },
  } as unknown as AbortSignal;
  let streamController!: ReadableStreamDefaultController;
  const stream = new ReadableStream({
    start(controller) {
      streamController = controller;
    },
  });
  const completion = finished(
    stream as unknown as NodeJS.ReadableStream,
    { cleanup: true, signal },
  );

  streamController.close();
  await completion;
  assertEquals(removeCount, 1);
});

// https://github.com/denoland/deno/issues/28905
Deno.test("Writable toWeb", async () => {
  const nodeWritable = new Writable({
    write(_chunk, _encoding, callback) {
      // Simulate the issue by delaying the callback slightly
      setTimeout(() => {
        callback();
      }, 10);
    },
  });

  const webWritable = Writable.toWeb(nodeWritable);

  const source = ["line1", "line2", "line3"];
  const readable = ReadableStream.from(source);

  await readable
    // @ts-ignore wrong types
    .pipeThrough(new TextEncoderStream())
    // @ts-ignore wrong types
    .pipeTo(webWritable);

  await finished(nodeWritable);
});

Deno.test("Duplex fromWeb handles readable errors", async () => {
  let errorController!: ReadableStreamDefaultController;
  const readable = new ReadableStream({
    start(controller) {
      errorController = controller;
    },
  });
  const writable = new WritableStream({
    write() {
      // no-op
    },
  });

  const duplex = Duplex.fromWeb({ readable, writable });
  const errorPromise = new Promise<Error>((resolve) => {
    duplex.once("error", resolve);
  });

  errorController.error(new Error("Network error"));

  const error = await errorPromise;
  assertEquals(error.message, "Network error");
});

Deno.test("Writable toWeb abort handles destroy context", async () => {
  const nodeWritable = new Writable({
    write(_chunk, _encoding, callback) {
      callback();
    },
  });
  const webWritable = Writable.toWeb(nodeWritable);

  await webWritable.abort(new Error("abort"));
  assert(nodeWritable.destroyed);
});

Deno.test("Writable fromWeb writev handles write rejection", async () => {
  const writable = Writable.fromWeb(
    new WritableStream({
      write(chunk) {
        if (String(chunk) === "fail") {
          throw new Error("Writable write failed");
        }
      },
    }),
  );

  const errorPromise = new Promise<Error>((resolve) => {
    writable.once("error", resolve);
  });
  const closePromise = new Promise<void>((resolve) => {
    writable.once("close", resolve);
  });

  writable.cork();
  writable.write("ok");
  writable.write("fail");
  writable.uncork();

  const error = await errorPromise;
  assertEquals(error.message, "Writable write failed");
  await closePromise;
});

Deno.test("Duplex fromWeb writev handles write rejection", async () => {
  const duplex = Duplex.fromWeb({
    readable: new ReadableStream(),
    writable: new WritableStream({
      write(chunk) {
        if (String(chunk) === "fail") {
          throw new Error("Duplex write failed");
        }
      },
    }),
  });

  const errorPromise = new Promise<Error>((resolve) => {
    duplex.once("error", resolve);
  });
  const closePromise = new Promise<void>((resolve) => {
    duplex.once("close", resolve);
  });

  duplex.cork();
  duplex.write("ok");
  duplex.write("fail");
  duplex.uncork();

  const error = await errorPromise;
  assertEquals(error.message, "Duplex write failed");
  await closePromise;
});

// https://github.com/denoland/deno/issues/36275
Deno.test("Readable toWeb applies backpressure", async () => {
  const CHUNK_SIZE = 16 * 1024;
  const TOTAL_CHUNKS = 64;

  async function assertBackpressure(options?: { type: "bytes" }) {
    let produced = 0;
    const readable = new Readable({
      // One chunk fills the queue, so the source must stall right after it.
      highWaterMark: CHUNK_SIZE,
      read() {
        produced++;
        this.push(
          produced > TOTAL_CHUNKS ? null : new Uint8Array(CHUNK_SIZE),
        );
      },
    });

    // @ts-ignore `@types/node` types this parameter as `{ strategy }` only.
    const stream = Readable.toWeb(readable, options) as ReadableStream<
      Uint8Array
    >;
    const reader = stream.getReader();

    // Give the source plenty of turns to run away while nothing is consuming.
    for (let i = 0; i < 20; i++) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    assert(
      produced <= 8,
      `source ignored backpressure: produced ${produced} chunks without a read`,
    );

    let consumed = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      consumed += value.byteLength;
    }
    assertEquals(consumed, CHUNK_SIZE * TOTAL_CHUNKS);
  }

  await assertBackpressure();
  await assertBackpressure({ type: "bytes" });
});

// String chunks (a non-objectMode Readable with an encoding set) have no
// `byteLength`; they must still queue rather than error the stream.
Deno.test("Readable toWeb handles string chunks", async () => {
  const readable = new Readable({
    encoding: "utf8",
    read() {
      this.push("hello");
      this.push("world");
      this.push(null);
    },
  });
  const stream = Readable.toWeb(readable) as ReadableStream<string>;

  // Let the chunks land in the queue before reading, so that the strategy's
  // size algorithm actually runs.
  for (let i = 0; i < 5; i++) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  const reader = stream.getReader();
  const chunks: string[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  assertEquals(chunks, ["hello", "world"]);
});

// https://github.com/denoland/deno/issues/30423
Deno.test("exported `promises` from node:stream works", async () => {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue("asd");
      controller.close();
    },
  });
  const promise = promises.finished(stream as unknown as NodeJS.ReadableStream);
  for await (const chunk of stream) {
    assertEquals(chunk, "asd");
  }
  await promise;
});
