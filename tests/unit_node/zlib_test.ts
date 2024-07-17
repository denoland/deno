// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "@std/assert/mod.ts";
import { fromFileUrl, relative } from "@std/path/mod.ts";
import {
  brotliCompress,
  brotliCompressSync,
  brotliDecompressSync,
  createBrotliCompress,
  createBrotliDecompress,
  createDeflate,
  gzipSync,
  unzipSync,
} from "node:zlib";
import { Buffer } from "node:buffer";
import { createReadStream, createWriteStream } from "node:fs";
import { Readable } from "node:stream";
import { buffer } from "node:stream/consumers";

Deno.test("brotli compression sync", () => {
  const buf = Buffer.from("hello world");
  const compressed = brotliCompressSync(buf);
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("brotli compression async", async () => {
  const buf = Buffer.from("hello world");
  const compressed: Buffer = await new Promise((resolve) =>
    brotliCompress(buf, (_, res) => {
      return resolve(res);
    })
  );
  assertEquals(compressed instanceof Buffer, true);
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("gzip compression sync", () => {
  const buf = Buffer.from("hello world");
  const compressed = gzipSync(buf);
  const decompressed = unzipSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("brotli compression", {
  ignore: true,
}, async () => {
  const promise = Promise.withResolvers<void>();
  const compress = createBrotliCompress();
  const filePath = relative(
    Deno.cwd(),
    fromFileUrl(new URL("./testdata/lorem_ipsum.txt", import.meta.url)),
  );
  const input = createReadStream(filePath);
  const output = createWriteStream("lorem_ipsum.txt.br");

  const stream = input.pipe(compress).pipe(output);

  stream.on("finish", () => {
    const decompress = createBrotliDecompress();
    const input2 = createReadStream("lorem_ipsum.txt.br");
    const output2 = createWriteStream("lorem_ipsum.txt");

    const stream2 = input2.pipe(decompress).pipe(output2);
    stream2.on("close", () => promise.resolve());
  });

  await Promise.all([
    promise.promise,
    new Promise((r) => stream.on("close", r)),
  ]);

  const content = Deno.readTextFileSync("lorem_ipsum.txt");
  assert(content.startsWith("Lorem ipsum dolor sit amet"), content);
  try {
    Deno.removeSync("lorem_ipsum.txt.br");
  } catch {
    // pass
  }
  try {
    Deno.removeSync("lorem_ipsum.txt");
  } catch {
    // pass
  }
});

Deno.test("brotli end-to-end with 4097 bytes", () => {
  const a = "a".repeat(4097);
  const compressed = brotliCompressSync(a);
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), a);
});

Deno.test(
  "zlib create deflate with dictionary",
  async () => {
    const { promise, resolve } = Promise.withResolvers<void>();
    const handle = createDeflate({
      dictionary: Buffer.alloc(0),
    });

    handle.on("close", () => resolve());
    handle.end();
    handle.destroy();

    await promise;
  },
);

Deno.test(
  "zlib flush i32",
  function () {
    const handle = createDeflate({
      // @ts-expect-error: passing non-int flush value
      flush: "",
    });

    handle.end();
    handle.destroy();
  },
);

Deno.test("should work with dataview", () => {
  const buf = Buffer.from("hello world");
  const compressed = brotliCompressSync(
    new DataView(buf.buffer, buf.byteOffset, buf.byteLength),
  );
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("should work with a buffer from an encoded string", () => {
  const encoder = new TextEncoder();
  const buffer = encoder.encode("hello world");
  const buf = Buffer.from(buffer);
  const compressed = brotliCompressSync(buf);
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test(
  "zlib compression with dataview",
  () => {
    const buf = Buffer.from("hello world");
    const compressed = gzipSync(
      new DataView(buf.buffer, buf.byteOffset, buf.byteLength),
    );
    const decompressed = unzipSync(compressed);
    assertEquals(decompressed.toString(), "hello world");
  },
);

Deno.test("zlib compression with an encoded string", () => {
  const encoder = new TextEncoder();
  const buffer = encoder.encode("hello world");
  const compressed = gzipSync(buffer);
  const decompressed = unzipSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("brotli large chunk size", async () => {
  const input = new Uint8Array(1000000);
  for (let i = 0; i < input.length; i++) {
    input[i] = Math.random() * 256;
  }
  const output = await buffer(
    Readable.from([input])
      .pipe(createBrotliCompress())
      .pipe(createBrotliDecompress()),
  );
  assertEquals(output.length, input.length);
});

Deno.test("brotli decompress flush restore size", async () => {
  const input = new Uint8Array(1000000);
  const output = await buffer(
    Readable.from([input])
      .pipe(createBrotliCompress())
      .pipe(createBrotliDecompress()),
  );
  assertEquals(output.length, input.length);
});
