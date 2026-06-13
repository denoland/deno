// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertThrows } from "@std/assert";
import { fromFileUrl, relative } from "@std/path";
import { randomBytes } from "node:crypto";
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
  createGunzip,
  createGzip,
  createInflate,
  createZstdCompress,
  createZstdDecompress,
  deflateSync,
  gunzip,
  gzip,
  gzipSync,
  unzipSync,
  zstdCompressSync,
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
    assertThrows(() =>
      createDeflate({
        // @ts-expect-error: passing non-int flush value
        flush: "",
      }), TypeError);
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

// https://github.com/denoland/deno/issues/24572
Deno.test("Brotli quality 10 doesn't panic", () => {
  const e = brotliCompressSync("abc", {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 10,
    },
  });
  assertEquals(
    new Uint8Array(e.buffer, e.byteOffset, e.byteLength),
    new Uint8Array([11, 1, 128, 97, 98, 99, 3]),
  );
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

Deno.test("crc32 doesn't overflow", () => {
  let checksum = 0;
  checksum = crc32(Buffer.from("H4sIAAAAAAAACg==", "base64"), checksum);
  checksum = crc32("aaa", checksum);
  assertEquals(checksum, 1466848669);
});

Deno.test("crc32 large input", () => {
  let checkSum = 0xffffffff;
  for (let i = 0; i < 2 ** 16; i++) {
    checkSum = crc32("", checkSum);
  }
  assertEquals(checkSum, 0xffffffff);
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

Deno.test("ERR_BUFFER_TOO_LARGE works correctly", () => {
  assertThrows(
    () => {
      deflateSync(randomBytes(1024), {
        maxOutputLength: 1,
      });
    },
    "Cannot create a Buffer larger than 1 bytes",
  );
});

// https://github.com/denoland/deno/issues/30829
Deno.test("gunzip doesn't cause stack overflow with 64MiB data", async () => {
  const data = Buffer.alloc(64 * 1024 * 1024);
  const compressed = gzipSync(data);

  const { promise, resolve, reject } = Promise.withResolvers<void>();

  gunzip(compressed, (err, result) => {
    if (err) {
      reject(err);
      return;
    }
    if (!result) {
      reject(new Error("expected gunzip to return a Buffer"));
      return;
    }
    if (result.length !== data.length) {
      reject(
        new Error(`expected ${data.length} bytes, got ${result.length}`),
      );
      return;
    }
    resolve();
  });

  await promise;
});

// Every compression/decompression backend whose native handle writes the
// post-write avail_out/avail_in pair back into `_writeState`. Decoders are
// paired with valid compressed input so the write reaches the result buffer
// instead of bailing out on a decode error.
const raw = new Uint8Array(16).fill(0x61);
const writeStateCases: [() => unknown, Uint8Array][] = [
  [createDeflate, raw],
  [createGzip, raw],
  [createGunzip, gzipSync(raw)],
  [createBrotliCompress, raw],
  [createBrotliDecompress, brotliCompressSync(raw)],
  [createZstdCompress, raw],
  [createZstdDecompress, zstdCompressSync(raw)],
];

// The native zlib/brotli/zstd bindings must not retain a pointer into the JS
// `_writeState` buffer across calls. The result buffer is passed per write and
// bounds checked, so detaching it (e.g. via a structuredClone transfer) before
// a write is harmless rather than writing into a freed backing store.
Deno.test("zlib writeSync does not write through a detached _writeState", () => {
  const output = new Uint8Array(128);

  // Each of these would crash or corrupt the heap before the fix. With the
  // buffer detached the result write must be a no-op; the only thing asserted
  // is that the process survives and nothing lands in the freed backing store.
  for (const [factory, input] of writeStateCases) {
    // deno-lint-ignore no-explicit-any
    const stream = factory() as any;
    const handle = stream._handle;
    const state = stream._writeState;
    // Detach the underlying ArrayBuffer, freeing the backing store.
    structuredClone(state.buffer, { transfer: [state.buffer] });
    assertEquals(state.buffer.byteLength, 0);
    handle.writeSync(
      0,
      input,
      0,
      input.length,
      output,
      0,
      output.length,
      state,
    );
    assertEquals(state.buffer.byteLength, 0);
  }
});

// Guards the other direction: with a live buffer the per-write result must
// actually be written. A regression that dropped or misrouted the argument
// would silently leave `_writeState` untouched and pass the detach test above.
Deno.test("zlib writeSync populates _writeState per call", () => {
  const SENTINEL = 0xffffffff;
  const input = new Uint8Array(32).fill(0x61);
  const output = new Uint8Array(128);

  // Decoders need valid input to reach the result write, so only the encoders
  // are exercised here; the shared native path covers the decoders.
  for (
    const factory of [
      createDeflate,
      createGzip,
      createBrotliCompress,
      createZstdCompress,
    ]
  ) {
    // deno-lint-ignore no-explicit-any
    const stream = factory() as any;
    const handle = stream._handle;
    const state = stream._writeState;
    state[0] = SENTINEL;
    state[1] = SENTINEL;
    // flush 0 means "process / no flush" across zlib, brotli and zstd.
    handle.writeSync(
      0,
      input,
      0,
      input.length,
      output,
      0,
      output.length,
      state,
    );
    // avail_out and avail_in were written, replacing the sentinel, and stay
    // within the bounds of the output and input buffers.
    assert(state[0] !== SENTINEL, "avail_out was not written");
    assert(state[1] !== SENTINEL, "avail_in was not written");
    assert(state[0] <= output.length, "avail_out out of range");
    assert(state[1] <= input.length, "avail_in out of range");
  }
});

// Same regression as the writeSync detach test above, but for the async
// `handle.write` op. That op is a separately duplicated native function which
// also writes the result pair back into `_writeState`, so it carries its own
// copy of the bounds check and needs its own coverage. The op is invoked
// directly (rather than through the stream) so the detach is observed against
// the freed backing store before any other state mutates. brotli/zstd run
// `processCallback` synchronously from the op, which then reads the now-detached
// state and throws downstream; that throw is unrelated to the native result
// write under test and is swallowed. Surviving the call with the buffer still
// detached is the assertion.
Deno.test("zlib write does not write through a detached _writeState", () => {
  const output = new Uint8Array(128);

  for (const [factory, input] of writeStateCases) {
    // deno-lint-ignore no-explicit-any
    const stream = factory() as any;
    const handle = stream._handle;
    const state = stream._writeState;
    // Detach the underlying ArrayBuffer, freeing the backing store.
    structuredClone(state.buffer, { transfer: [state.buffer] });
    assertEquals(state.buffer.byteLength, 0);
    try {
      handle.write(
        0,
        input,
        0,
        input.length,
        output,
        0,
        output.length,
        state,
      );
    } catch {
      // Expected for the engines whose op runs processCallback synchronously:
      // it reads the detached state and throws. The native result write (the
      // thing under test) already happened as a no-op before the callback.
    }
    assertEquals(state.buffer.byteLength, 0);
  }
});

// pngjs's `sync-inflate.js` binds directly to `inflate._handle.writeSync` and
// invokes it with seven arguments (no per-call `_writeState`), relying on the
// pre-#35043 contract where the result buffer was registered at init time. The
// per-call argument is now optional so that signature keeps working (#35185).
Deno.test("zlib writeSync accepts the pre-#35043 seven-argument signature", () => {
  const chunk = Buffer.from([
    0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01,
  ]);
  const out = Buffer.allocUnsafe(1024);

  // deno-lint-ignore no-explicit-any
  const inflate = createInflate() as any;

  // Must not throw `expected typed ArrayBufferView` for the missing 8th arg.
  inflate._handle.writeSync(
    constants.Z_FINISH,
    chunk,
    0,
    chunk.length,
    out,
    0,
    out.length,
  );
});
