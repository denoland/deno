// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @deno-types="./streams.d.ts"
import {
  createReadStream,
  createWriteStream,
  ReadStream,
  WriteStream,
} from "./streams.mjs";
import { assertEquals } from "../../../testing/asserts.ts";
import { Buffer } from "../../buffer.ts";
import * as path from "../../path/mod.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

// Need to wait for file processing to complete within each test to prevent false negatives.
async function waiter(
  stream: ReadStream | WriteStream,
  interval = 100,
  maxCount = 50,
) {
  for (let i = maxCount; i > 0; i--) {
    await new Promise((resolve) => setTimeout(resolve, interval));
    if (stream.destroyed) return true;
  }
  return false;
}

Deno.test({
  name: "[node/fs.ReadStream] Read a chunk of data using 'new ReadStream()'",
  async fn() {
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    const readable = new ReadStream(testData);

    let data: Uint8Array;
    readable.on("data", (chunk: Uint8Array) => {
      data = chunk;
    });

    readable.on("close", () => {
      assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name:
    "[node/fs.createReadStream] Read a chunk of data using 'new createReadStream()'",
  async fn() {
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    const readable = new createReadStream(testData);

    let data: Uint8Array;
    readable.on("data", (chunk: Uint8Array) => {
      data = chunk;
    });

    readable.on("close", () => {
      assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name: "[node/fs.createReadStream] Read given amount of data",
  async fn() {
    const readable = createReadStream(testData);

    const data: (Uint8Array | null)[] = [];
    readable.on("readable", function () {
      data.push(readable.read(5));
      data.push(readable.read());
    });

    readable.on("close", () => {
      assertEquals(new TextDecoder().decode(data[0] as Uint8Array), "hello");
      assertEquals(new TextDecoder().decode(data[1] as Uint8Array), " world");
      assertEquals(data[2], null);
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name: "[node/fs.createReadStream] Handling of read position",
  async fn() {
    const readable = createReadStream(testData, {
      highWaterMark: 3,
      start: 1,
      end: 9,
    });

    const data: (Uint8Array | null)[] = [];
    readable.on("readable", function () {
      data.push(readable.read(4));
      data.push(readable.read(1));
    });

    readable.on("close", () => {
      assertEquals(data[0], null);
      assertEquals(new TextDecoder().decode(data[1] as Uint8Array), "e");
      assertEquals(new TextDecoder().decode(data[2] as Uint8Array), "llo ");
      assertEquals(new TextDecoder().decode(data[3] as Uint8Array), "w");
      assertEquals(new TextDecoder().decode(data[4] as Uint8Array), "orl");
      assertEquals(data[5], null);
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name: "[node/fs.createReadStream] Specify the path as a Buffer",
  async fn() {
    const readable = createReadStream(Buffer.from(testData));

    let data: Uint8Array;
    readable.on("data", (chunk: Uint8Array) => {
      data = chunk;
    });

    readable.on("close", () => {
      assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name: "[node/fs.createReadStream] Destroy the stream with an error",
  async fn() {
    const readable = createReadStream(testData);

    const data: (Uint8Array | null)[] = [];
    readable.on("readable", function () {
      data.push(readable.read(5));
      readable.destroy(Error("destroy has been called."));
    });

    readable.on("close", () => {
      assertEquals(new TextDecoder().decode(data[0] as Uint8Array), "hello");
      assertEquals(data.length, 1);
    });

    readable.on("error", (err: Error) => {
      assertEquals(err.name, "Error");
      assertEquals(err.message, "destroy has been called.");
    });

    assertEquals(await waiter(readable), true);
  },
});

Deno.test({
  name: "[node/fs.WriteStream] Write data using 'WriteStream()'",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    const writable = WriteStream(tempFile);

    writable.write("hello world");
    writable.end("\n");

    writable.on("close", () => {
      const data = Deno.readFileSync(tempFile);
      Deno.removeSync(tempFile);
      assertEquals(new TextDecoder("utf-8").decode(data), "hello world\n");
    });

    assertEquals(await waiter(writable), true);
  },
});

Deno.test({
  name: "[node/fs.WriteStream] Write data using 'new WriteStream()'",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    const writable = new WriteStream(tempFile);

    writable.write("hello world");
    writable.end("\n");

    writable.on("close", () => {
      const data = Deno.readFileSync(tempFile);
      Deno.removeSync(tempFile);
      assertEquals(new TextDecoder("utf-8").decode(data), "hello world\n");
    });

    assertEquals(await waiter(writable), true);
  },
});

Deno.test({
  name:
    "[node/fs.createWriteStream] Write data using 'new createWriteStream()'",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    // deno-lint-ignore ban-ts-comment
    // @ts-ignore
    const writable = new createWriteStream(tempFile);

    writable.write("hello world");
    writable.end("\n");

    writable.on("close", () => {
      const data = Deno.readFileSync(tempFile);
      Deno.removeSync(tempFile);
      assertEquals(new TextDecoder("utf-8").decode(data), "hello world\n");
    });

    assertEquals(await waiter(writable), true);
  },
});

Deno.test({
  name: "[node/fs.createWriteStream] Specify the path as a Buffer",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const writable = createWriteStream(Buffer.from(tempFile));

    writable.write("hello world");
    writable.end("\n");

    writable.on("close", () => {
      const data = Deno.readFileSync(tempFile);
      Deno.removeSync(tempFile);
      assertEquals(new TextDecoder("utf-8").decode(data), "hello world\n");
    });

    assertEquals(await waiter(writable), true);
  },
});

Deno.test({
  name: "[node/fs.createWriteStream] Destroy the stream with an error",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const writable = createWriteStream(tempFile);

    writable.write("hello world");
    writable.destroy(Error("destroy has been called."));

    writable.on("close", () => {
      const data = Deno.readFileSync(tempFile);
      Deno.removeSync(tempFile);
      assertEquals(new TextDecoder("utf-8").decode(data), "");
    });

    writable.on("error", (err: Error) => {
      assertEquals(err.name, "Error");
      assertEquals(err.message, "destroy has been called.");
    });

    assertEquals(await waiter(writable), true);
  },
});
