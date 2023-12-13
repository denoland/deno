// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../../../test_util/std/assert/mod.ts";
import { fromFileUrl, relative } from "../../../test_util/std/path/mod.ts";
import {
  brotliCompressSync,
  brotliDecompressSync,
  createBrotliCompress,
  createBrotliDecompress,
  createDeflate,
} from "node:zlib";
import { Buffer } from "node:buffer";
import { createReadStream, createWriteStream } from "node:fs";

Deno.test("brotli compression sync", () => {
  const buf = Buffer.from("hello world");
  const compressed = brotliCompressSync(buf);
  const decompressed = brotliDecompressSync(compressed);
  assertEquals(decompressed.toString(), "hello world");
});

Deno.test("brotli compression", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
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

    stream2.on("finish", () => {
      resolve();
    });
  });

  await promise;
  const content = Deno.readTextFileSync("lorem_ipsum.txt");
  assert(content.startsWith("Lorem ipsum dolor sit amet"));
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
  { sanitizeResources: false },
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
  // FIXME: Handle is not closed properly
  { sanitizeResources: false },
  function () {
    const handle = createDeflate({
      // @ts-expect-error: passing non-int flush value
      flush: "",
    });

    handle.end();
    handle.destroy();
  },
);
