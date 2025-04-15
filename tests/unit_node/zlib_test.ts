// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "@std/assert";
import { fromFileUrl, relative } from "@std/path";
import {
  BrotliCompress,
  brotliCompress,
  brotliCompressSync,
  BrotliDecompress,
  brotliDecompress,
  brotliDecompressSync,
  constants,
  crc32,
  createBrotliCompress,
  createBrotliDecompress,
  createDeflate,
  gzip,
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
  const decompressed: Buffer = await new Promise((resolve) =>
    brotliDecompress(compressed, (_, res) => {
      return resolve(res);
    })
  );
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
    new Promise<void>((r) => stream.on("close", r)),
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
  const compressed = brotliCompressSync(new DataView(buf.buffer));
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

// https://github.com/denoland/deno/issues/24572
Deno.test("Brotli quality 10 doesn't panic", () => {
  const e = brotliCompressSync("abc", {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 10,
    },
  });
  assertEquals(
    new Uint8Array(e.buffer),
    new Uint8Array([11, 1, 128, 97, 98, 99, 3]),
  );
});

Deno.test(
  "zlib compression with dataview",
  () => {
    const buf = Buffer.from("hello world");
    const compressed = gzipSync(new DataView(buf.buffer));
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

Deno.test("createBrotliCompress params", async () => {
  const compress = createBrotliCompress({
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
    },
  });

  const input = new Uint8Array(10000);
  for (let i = 0; i < input.length; i++) {
    input[i] = Math.random() * 256;
  }
  const output = await buffer(
    Readable.from([input])
      .pipe(compress)
      .pipe(createBrotliDecompress()),
  );
  assertEquals(output.length, input.length);
});

Deno.test("gzip() and gzipSync() accept ArrayBuffer", async () => {
  const deffered = Promise.withResolvers<void>();
  const buf = new ArrayBuffer(0);
  let output: Buffer;
  gzip(buf, (_err, data) => {
    output = data;
    deffered.resolve();
  });
  await deffered.promise;
  assert(output! instanceof Buffer);
  const outputSync = gzipSync(buf);
  assert(outputSync instanceof Buffer);
});

Deno.test("crc32()", () => {
  assertEquals(crc32("hello world"), 222957957);
  // @ts-expect-error: passing an object
  assertThrows(() => crc32({}), TypeError);
});

Deno.test("BrotliCompress", async () => {
  const deffered = Promise.withResolvers<void>();
  // @ts-ignore: BrotliCompress is not typed
  const brotliCompress = new BrotliCompress();
  // @ts-ignore: BrotliDecompress is not typed
  const brotliDecompress = new BrotliDecompress();

  brotliCompress.pipe(brotliDecompress);

  let data = "";
  brotliDecompress.on("data", (v: Buffer) => {
    data += v.toString();
  });

  brotliDecompress.on("end", () => {
    deffered.resolve();
  });

  brotliCompress.write("hello");
  brotliCompress.end();

  await deffered.promise;
  assertEquals(data, "hello");
});
