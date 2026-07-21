// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "@std/assert";
import { fromFileUrl, relative } from "@std/path";
import { finished, pipeline } from "node:stream/promises";
import {
  Duplex,
  getDefaultHighWaterMark,
  promises,
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
