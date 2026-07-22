// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  fail,
} from "./test_util.ts";

// `resourceForReadableStream` is registered on the internals object only when
// `ext:deno_web/06_streams.js` first evaluates, which is now lazy. Touch the
// lazy `ReadableStream` global so the polyfill loads before we read it below.
const { ReadableStream: _ReadableStream } = globalThis;
const {
  core,
  resourceForReadableStream,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

const LOREM =
  "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";

// Hello world, with optional close
function helloWorldStream(
  close?: boolean,
  cancelResolve?: (value: unknown) => void,
) {
  return new ReadableStream({
    start(controller) {
      controller.enqueue("hello, world");
      if (close == true) {
        controller.close();
      }
    },
    cancel(reason) {
      if (cancelResolve != undefined) {
        cancelResolve(reason);
      }
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

// Long stream with Lorem Ipsum text.
function longAsyncStream(cancelResolve?: (value: unknown) => void) {
  let currentTimeout: NodeJS.Timeout | undefined = undefined;
  return new ReadableStream({
    async start(controller) {
      for (let i = 0; i < 100; i++) {
        await new Promise((r) => currentTimeout = setTimeout(r, 1));
        currentTimeout = undefined;
        controller.enqueue(LOREM);
      }
      controller.close();
    },
    cancel(reason) {
      if (cancelResolve != undefined) {
        cancelResolve(reason);
      }
      if (currentTimeout !== undefined) {
        clearTimeout(currentTimeout);
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

function largePacketStream(packetSize: number, count: number) {
  return new ReadableStream({
    pull(controller) {
      if (count-- > 0) {
        const buffer = new Uint8Array(packetSize);
        for (let i = 0; i < 256; i++) {
          buffer[i * (packetSize / 256)] = i;
        }
        controller.enqueue(buffer);
      } else {
        controller.close();
      }
    },
  });
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

// Try to blow up any recursive reads.
function veryLongTinyPacketStream(length: number) {
  return new ReadableStream({
    start(controller) {
      for (let i = 0; i < length; i++) {
        controller.enqueue(new Uint8Array([1]));
      }
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
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 12);
  core.close(rid);
});

// Close the stream after reading everything
Deno.test(async function readableStreamClose() {
  const cancel = Promise.withResolvers();
  const rid = resourceForReadableStream(
    helloWorldStream(false, cancel.resolve),
  );
  const buffer = new Uint8Array(1024);
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 12);
  core.close(rid);
  assertEquals(await cancel.promise, "resource closed");
});

// Close the stream without reading everything
Deno.test(async function readableStreamClosePartialRead() {
  const cancel = Promise.withResolvers();
  const rid = resourceForReadableStream(
    helloWorldStream(false, cancel.resolve),
  );
  const buffer = new Uint8Array(5);
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 5);
  core.close(rid);
  assertEquals(await cancel.promise, "resource closed");
});

// Close the stream without reading anything
Deno.test(async function readableStreamCloseWithoutRead() {
  const cancel = Promise.withResolvers();
  const rid = resourceForReadableStream(
    helloWorldStream(false, cancel.resolve),
  );
  core.close(rid);
  assertEquals(await cancel.promise, "resource closed");
});

// Close the stream without reading anything
Deno.test(async function readableStreamCloseWithoutRead2() {
  const cancel = Promise.withResolvers();
  const rid = resourceForReadableStream(longAsyncStream(cancel.resolve));
  core.close(rid);
  assertEquals(await cancel.promise, "resource closed");
});

// Regression test for https://github.com/denoland/deno/issues/35807: teeing a
// byte stream, reading one branch with a BYOB reader while the other branch is
// drained by a default reader. When the source closes, the tee reads the source
// with its BYOB reader; readableStreamClose() does not run a BYOB reader's
// read-into close steps, so without the fix the pending BYOB read on the first
// branch (and the default drain on the other) would hang forever.
Deno.test(async function readableByteStreamTeeByobReadResolvesOnSourceClose() {
  function makeByteStream(chunks: number[][]) {
    let i = 0;
    return new ReadableStream({
      type: "bytes",
      pull(c) {
        if (i < chunks.length) c.enqueue(new Uint8Array(chunks[i++]));
        else c.close();
      },
    });
  }
  async function collect(s: ReadableStream<Uint8Array>) {
    const out = [];
    for await (const c of s) out.push(c);
    return out;
  }

  const [a, b] = makeByteStream([[1, 2, 3], [4, 5, 6]]).tee();
  const bDone = collect(b);
  const reader = a.getReader({ mode: "byob" });
  assertEquals(
    (await reader.read(new Uint8Array(3))).value,
    new Uint8Array([1, 2, 3]),
  );
  assertEquals(
    (await reader.read(new Uint8Array(3))).value,
    new Uint8Array([4, 5, 6]),
  );
  // Previously this read hung ("Top-level await promise never resolved").
  const r3 = await reader.read(new Uint8Array(3));
  assertEquals(r3.done, true);
  assertEquals((await bDone).length, 2);
});

Deno.test(async function readableStreamPartial() {
  const rid = resourceForReadableStream(helloWorldStream());
  const buffer = new Uint8Array(5);
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 5);
  const buffer2 = new Uint8Array(1024);
  const nread2 = await core.read(rid, buffer2);
  assertEquals(nread2, 7);
  core.close(rid);
});

Deno.test(async function readableStreamLongReadAll() {
  const rid = resourceForReadableStream(longStream());
  const buffer = await core.readAll(rid);
  assertEquals(buffer.length, LOREM.length * 4);
  core.close(rid);
});

Deno.test(async function readableStreamLongAsyncReadAll() {
  const rid = resourceForReadableStream(longAsyncStream());
  const buffer = await core.readAll(rid);
  assertEquals(buffer.length, LOREM.length * 100);
  core.close(rid);
});

Deno.test(async function readableStreamVeryLongReadAll() {
  const rid = resourceForReadableStream(veryLongTinyPacketStream(1_000_000));
  const buffer = await core.readAll(rid);
  assertEquals(buffer.length, 1_000_000);
  core.close(rid);
});

Deno.test(async function readableStreamLongByPiece() {
  const rid = resourceForReadableStream(longStream());
  let total = 0;
  for (let i = 0; i < 100; i++) {
    const length = await core.read(rid, new Uint8Array(16));
    total += length;
    if (length == 0) {
      break;
    }
  }
  assertEquals(total, LOREM.length * 4);
  core.close(rid);
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
    let nread;
    try {
      nread = await core.read(rid, new Uint8Array(16));
    } catch (_) {
      fail("Should not have thrown");
    }
    assertEquals(12, nread);
    try {
      await core.read(rid, new Uint8Array(1));
      fail();
    } catch (e) {
      assertEquals((e as Error).message, `Uh oh (${type})!`);
    }
    core.close(rid);
  });
}

Deno.test(async function readableStreamEmptyOnStart() {
  const rid = resourceForReadableStream(emptyStream(true));
  const buffer = new Uint8Array(1024);
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 0);
  core.close(rid);
});

Deno.test(async function readableStreamEmptyOnPull() {
  const rid = resourceForReadableStream(emptyStream(false));
  const buffer = new Uint8Array(1024);
  const nread = await core.read(rid, buffer);
  assertEquals(nread, 0);
  core.close(rid);
});

Deno.test(async function readableStreamEmptyReadAll() {
  const rid = resourceForReadableStream(emptyStream(false));
  const buffer = await core.readAll(rid);
  assertEquals(buffer.length, 0);
  core.close(rid);
});

Deno.test(async function readableStreamWithEmptyChunk() {
  const rid = resourceForReadableStream(emptyChunkStream());
  const buffer = await core.readAll(rid);
  assertEquals(buffer, new Uint8Array([1, 2]));
  core.close(rid);
});

Deno.test(async function readableStreamWithEmptyChunkOneByOne() {
  const rid = resourceForReadableStream(emptyChunkStream());
  assertEquals(1, await core.read(rid, new Uint8Array(1)));
  assertEquals(1, await core.read(rid, new Uint8Array(1)));
  assertEquals(0, await core.read(rid, new Uint8Array(1)));
  core.close(rid);
});

// Ensure that we correctly transmit all the sub-chunks of the larger chunks.
Deno.test(async function readableStreamReadSmallerChunks() {
  const packetSize = 16 * 1024;
  const rid = resourceForReadableStream(largePacketStream(packetSize, 1));
  const buffer = new Uint8Array(packetSize);
  for (let i = 0; i < packetSize / 1024; i++) {
    await core.read(rid, buffer.subarray(i * 1024, i * 1024 + 1024));
  }
  for (let i = 0; i < 256; i++) {
    assertEquals(
      i,
      buffer[i * (packetSize / 256)],
      `at index ${i * (packetSize / 256)}`,
    );
  }
  core.close(rid);
});

Deno.test(async function readableStreamLargePackets() {
  const packetSize = 128 * 1024;
  const rid = resourceForReadableStream(largePacketStream(packetSize, 1024));
  for (let i = 0; i < 1024; i++) {
    const buffer = new Uint8Array(packetSize);
    assertEquals(packetSize, await core.read(rid, buffer));
    for (let i = 0; i < 256; i++) {
      assertEquals(
        i,
        buffer[i * (packetSize / 256)],
        `at index ${i * (packetSize / 256)}`,
      );
    }
  }
  assertEquals(0, await core.read(rid, new Uint8Array(1)));
  core.close(rid);
});

Deno.test(async function readableStreamVeryLargePackets() {
  // 1024 packets of 1MB
  const rid = resourceForReadableStream(largePacketStream(1024 * 1024, 1024));
  let total = 0;
  // Read 96kB up to 12,288 times (96kB is not an even multiple of the 1MB packet size to test this)
  const readCounts: Record<number, number> = {};
  for (let i = 0; i < 12 * 1024; i++) {
    const nread = await core.read(rid, new Uint8Array(96 * 1024));
    total += nread;
    readCounts[nread] = (readCounts[nread] || 0) + 1;
    if (nread == 0) {
      break;
    }
  }
  assertEquals({ 0: 1, 65536: 1024, 98304: 10 * 1024 }, readCounts);
  assertEquals(total, 1024 * 1024 * 1024);
  core.close(rid);
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
        await core.read(rid, buffer);
      }
      if (action == "Throw") {
        try {
          const buffer = new Uint8Array(1);
          assertEquals(1, await core.read(rid, buffer));
          fail();
        } catch (e) {
          // We expect this to be thrown
          assertEquals((e as Error).message, "Expected error!");
        }
      } else {
        const buffer = new Uint8Array(1);
        assertEquals(0, await core.read(rid, buffer));
      }
    } finally {
      core.close(rid);
    }
  });
}

// 1024 is the size of the internal packet buffer -- we want to make sure we fill the internal pipe fully.
for (const packetCount of [1, 1024]) {
  Deno.test(`readableStreamWithAggressiveResourceClose_${packetCount}`, async function () {
    let first = true;
    const { promise, resolve } = Promise.withResolvers();
    const rid = resourceForReadableStream(
      new ReadableStream({
        pull(controller) {
          if (first) {
            // We queue this up and then immediately close the resource (not the reader)
            for (let i = 0; i < packetCount; i++) {
              controller.enqueue(new Uint8Array(1));
            }
            core.close(rid);
            // This doesn't throw, even though the resource is closed
            controller.enqueue(new Uint8Array(1));
            first = false;
          }
        },
        cancel(reason) {
          resolve(reason);
        },
      }),
    );
    try {
      for (let i = 0; i < packetCount; i++) {
        await core.read(rid, new Uint8Array(1));
      }
      fail();
    } catch (e) {
      assertEquals((e as Error).message, "operation canceled");
    }
    assertEquals(await promise, "resource closed");
  });
}

Deno.test(async function compressionStreamWritableMayBeAborted() {
  await Promise.all([
    new CompressionStream("gzip").writable.getWriter().abort(),
    new CompressionStream("deflate").writable.getWriter().abort(),
    new CompressionStream("deflate-raw").writable.getWriter().abort(),
    new CompressionStream("brotli").writable.getWriter().abort(),
  ]);
});

Deno.test(async function compressionStreamReadableMayBeCancelled() {
  await Promise.all([
    new CompressionStream("gzip").readable.getReader().cancel(),
    new CompressionStream("deflate").readable.getReader().cancel(),
    new CompressionStream("deflate-raw").readable.getReader().cancel(),
    new CompressionStream("brotli").readable.getReader().cancel(),
  ]);
});

Deno.test(async function decompressionStreamWritableMayBeAborted() {
  await Promise.all([
    new DecompressionStream("gzip").writable.getWriter().abort(),
    new DecompressionStream("deflate").writable.getWriter().abort(),
    new DecompressionStream("deflate-raw").writable.getWriter().abort(),
    new DecompressionStream("brotli").writable.getWriter().abort(),
  ]);
});

Deno.test(async function decompressionStreamReadableMayBeCancelled() {
  await Promise.all([
    new DecompressionStream("gzip").readable.getReader().cancel(),
    new DecompressionStream("deflate").readable.getReader().cancel(),
    new DecompressionStream("deflate-raw").readable.getReader().cancel(),
    new DecompressionStream("brotli").readable.getReader().cancel(),
  ]);
});

Deno.test(async function decompressionStreamValidGzipDoesNotThrow() {
  const cs = new CompressionStream("gzip");
  const ds = new DecompressionStream("gzip");
  cs.readable.pipeThrough(ds);
  const writer = cs.writable.getWriter();
  await writer.write(new Uint8Array([1]));
  writer.releaseLock();
  await cs.writable.close();
  let result = new Uint8Array();
  for await (const chunk of ds.readable.values()) {
    result = new Uint8Array([...result, ...chunk]);
  }
  assertEquals(result, new Uint8Array([1]));
});

Deno.test(async function decompressionStreamValidBrotliDoesNotThrow() {
  const cs = new CompressionStream("brotli");
  const ds = new DecompressionStream("brotli");
  cs.readable.pipeThrough(ds);
  const writer = cs.writable.getWriter();
  await writer.write(new Uint8Array([1]));
  writer.releaseLock();
  await cs.writable.close();
  let result = new Uint8Array();
  for await (const chunk of ds.readable.values()) {
    result = new Uint8Array([...result, ...chunk]);
  }
  assertEquals(result, new Uint8Array([1]));
});

Deno.test(async function brotliCompressionDecompressionRoundTrip() {
  const original = new TextEncoder().encode(LOREM);
  const cs = new CompressionStream("brotli");
  const ds = new DecompressionStream("brotli");
  cs.readable.pipeThrough(ds);
  const writer = cs.writable.getWriter();
  await writer.write(original);
  writer.releaseLock();
  await cs.writable.close();
  let result = new Uint8Array();
  for await (const chunk of ds.readable.values()) {
    result = new Uint8Array([...result, ...chunk]);
  }
  assertEquals(result, original);
});

Deno.test(async function decompressionStreamInvalidGzipStillReported() {
  await assertRejects(
    async () => {
      await new DecompressionStream("gzip").writable.close();
    },
    TypeError,
    "corrupt gzip stream does not have a matching checksum",
  );
});

Deno.test(async function readableStreamFromString() {
  // A string is a sync iterable; ReadableStream.from() should yield its
  // characters one at a time (matches the WHATWG streams spec and Node).
  // @ts-expect-error: TS lib types exclude primitives, but they work at runtime
  const stream: ReadableStream<string> = ReadableStream.from("hi");
  const reader = stream.getReader();
  assertEquals(await reader.read(), { value: "h", done: false });
  assertEquals(await reader.read(), { value: "i", done: false });
  assertEquals(await reader.read(), { value: undefined, done: true });
});

Deno.test(function readableStreamFromNonIterableThrows() {
  for (const value of [null, undefined, 42, true]) {
    assertThrows(
      // @ts-expect-error: non-iterable values are not acceptable
      () => ReadableStream.from(value),
      TypeError,
      "Failed to execute 'ReadableStream.from': Argument 1 can not be converted to async iterable.",
    );
  }
});

Deno.test(async function readableStreamFromWithStringThrows() {
  const serverPort = 4592;
  const upstreamServerPort = 4593;

  const stopSignal = new AbortController();
  const promise = Promise.withResolvers();
  // Response transforming server that crashes with an uncaught AbortError.
  function startServer() {
    Deno.serve({ port: serverPort, signal: stopSignal.signal }, async (req) => {
      const upstreamResponse = await fetch(
        `http://localhost:${upstreamServerPort}`,
        req,
      );

      // Use a TransformStream to convert the response body to uppercase.
      const transformStream = new TransformStream({
        transform(chunk, controller) {
          const decoder = new TextDecoder();
          const encoder = new TextEncoder();
          const chunk2 = encoder.encode(decoder.decode(chunk).toUpperCase());
          controller.enqueue(chunk2);
        },
      });

      upstreamResponse.body?.pipeTo(transformStream.writable).catch(() => {});

      return new Response(transformStream.readable);
    });
  }

  // ==== THE ISSUE IS NOT IN THE CODE BELOW ====

  // Upstream server that sends a response with a body that never ends.
  // This is not where the error happens (it handlers the cancellation correctly).
  function startUpstreamServer() {
    Deno.serve({ port: upstreamServerPort, signal: stopSignal.signal }, (_) => {
      // Create an infinite readable stream that emits 'a'
      let pushTimeout: NodeJS.Timeout | null = null;
      const readableStream = new ReadableStream({
        start(controller) {
          const encoder = new TextEncoder();
          const chunk = encoder.encode("a");

          function push() {
            controller.enqueue(chunk);
            pushTimeout = setTimeout(push, 100);
          }

          push();
        },

        cancel(reason) {
          assertEquals(reason, "resource closed");
          promise.resolve(undefined);
          clearTimeout(pushTimeout!);
        },
      });

      return new Response(readableStream, {
        headers: { "Content-Type": "text/plain" },
      });
    });
  }

  // The client is just there to simulate a client that cancels a request.
  async function startClient() {
    const controller = new AbortController();
    const signal = controller.signal;

    try {
      const response = await fetch(`http://localhost:${serverPort}`, {
        signal,
      });
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("client: failed to get reader from response");
      }

      let received = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        received += value.length;

        if (received >= 5) {
          controller.abort();
          break;
        }
      }
    } catch (_) {
      //
    }
  }

  startUpstreamServer();
  startServer();
  const p = startClient();

  await promise.promise;
  stopSignal.abort();
  await p;
});

Deno.test(async function readableStreamEmittingManyChunks() {
  const code = `
    const serverPort = 4594;
    const stopSignal = new AbortController();
    let count = 0;
    let before = 0;
    let after = 0;

    function startServer() {
      Deno.serve({ port: serverPort, signal: stopSignal.signal }, (_) => {
        return new Response(
          new ReadableStream({
            start() {
              before = Deno.memoryUsage().heapUsed;
            },
            pull(controller) {
              const used = Deno.memoryUsage().heapUsed;

              if (used > after) {
                after = used;
              }

              if (count < 30_000) {
                controller.enqueue(new Uint8Array([0]));
              } else {
                controller.close();
              }

              count += 1;
            },
          }),
        );
      });
    }

    async function startClient() {
      const response = await fetch(\`http://localhost:\${serverPort}\`);
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("client: failed to get reader from response");
      }

      while (true) {
        const { done } = await reader.read();
        if (done) break;
      }
    }

    startServer();
    await startClient();
    stopSignal.abort();
    console.log(\`\${after} / \${before} = \${after / before}\`);
    // This guards against a per-chunk leak: a real leak over 30k chunks grows
    // the heap many-fold, so the threshold only needs to exclude the fixed
    // streaming overhead (~1.8 MB here, constant regardless of chunk count).
    // It's a ratio rather than an absolute so the baseline heap size doesn't
    // need hardcoding — but that makes it sensitive to baseline shrinkage:
    // deferring the node-polyfill foundation out of the startup snapshot
    // lowered \`before\` (~4.4 -> ~3.7 MB) while absolute growth was unchanged,
    // pushing the ratio from ~1.42 to ~1.50. Use 2x, which still flags any real
    // unbounded leak (those are >>2x) and is robust to baseline size.
    if (after / before > 2) {
      Deno.exit(1);
    }
  `;
  // The in-process heap-growth heuristic above is sensitive to GC timing and
  // can transiently exceed the 2x ratio on some platforms (observed on
  // macOS aarch64 in release mode) even without a real leak. A genuine
  // unbounded leak grows >>2x and reproduces on every run, so retry a few
  // times and only fail if every attempt reports a leak. See #35353.
  let codeResult = 1;
  for (let attempt = 0; attempt < 3 && codeResult !== 0; attempt++) {
    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", "-N", "-"],
      stdin: "piped",
    });

    await using child = command.spawn();
    await ReadableStream.from([code])
      .pipeThrough(new TextEncoderStream())
      .pipeTo(child.stdin);

    codeResult = (await child.status).code;
  }

  assertEquals(codeResult, 0, "memory leak");
});

// Regression test for https://github.com/denoland/deno/issues/33476
// `ReadableStreamBYOBRequest.view` is always constructed as a Uint8Array
// (matching whatwg/streams#1367), so its type should be narrowed from
// `ArrayBufferView | null` to `Uint8Array<ArrayBuffer> | null`. The runtime
// check verifies the actual value, and the variable annotation acts as a
// compile-time check via `deno check`.
Deno.test("ReadableStreamBYOBRequest.view is a Uint8Array", async () => {
  let viewIsUint8Array = false;
  const stream = new ReadableStream({
    type: "bytes",
    pull(controller) {
      const byobReq = controller.byobRequest;
      if (byobReq === null) return;
      // Compile-time type check: this assignment must succeed against the
      // narrowed signature.
      const view: Uint8Array<ArrayBuffer> | null = byobReq.view;
      viewIsUint8Array = view instanceof Uint8Array;
      view![0] = 42;
      byobReq.respond(1);
    },
  });
  const reader = stream.getReader({ mode: "byob" });
  const result = await reader.read(new Uint8Array(8));
  reader.releaseLock();
  assertEquals(viewIsUint8Array, true);
  assertEquals(result.value!.byteLength, 1);
  assertEquals(result.value![0], 42);
});

// https://github.com/denoland/deno/issues/22381
// Resource-backed writable streams (e.g. `Deno.connect().writable`,
// `Deno.open().writable`) should accept any ArrayBuffer / ArrayBufferView,
// not only Uint8Array.
Deno.test(
  { permissions: { net: true } },
  async function writableStreamForRidAcceptsAnyArrayBufferView() {
    const listener = Deno.listen({ port: 0, hostname: "127.0.0.1" });
    const port = (listener.addr as Deno.NetAddr).port;

    const serverConnPromise = (async () => {
      const conn = await listener.accept();
      const chunks: Uint8Array[] = [];
      const buf = new Uint8Array(64);
      while (true) {
        const n = await conn.read(buf);
        if (n === null) break;
        chunks.push(buf.slice(0, n));
      }
      conn.close();
      let total = 0;
      for (const c of chunks) total += c.byteLength;
      const out = new Uint8Array(total);
      let off = 0;
      for (const c of chunks) {
        out.set(c, off);
        off += c.byteLength;
      }
      return out;
    })();

    const client = await Deno.connect({ hostname: "127.0.0.1", port });
    const writer = client.writable.getWriter();

    // Uint8Array (existing behavior)
    await writer.write(new Uint8Array([0x01]));
    // Other typed-array views — all backed by raw bytes 0x02..
    const u16 = new Uint16Array(1);
    new Uint8Array(u16.buffer).set([0x02, 0x03]);
    // deno-lint-ignore no-explicit-any
    await writer.write(u16 as any);
    const u32 = new Uint32Array(1);
    new Uint8Array(u32.buffer).set([0x04, 0x05, 0x06, 0x07]);
    // deno-lint-ignore no-explicit-any
    await writer.write(u32 as any);
    const big = new BigUint64Array(1);
    new Uint8Array(big.buffer).set([
      0x08,
      0x09,
      0x0a,
      0x0b,
      0x0c,
      0x0d,
      0x0e,
      0x0f,
    ]);
    // deno-lint-ignore no-explicit-any
    await writer.write(big as any);
    // Bare ArrayBuffer
    const ab = new ArrayBuffer(2);
    new Uint8Array(ab).set([0x10, 0x11]);
    // deno-lint-ignore no-explicit-any
    await writer.write(ab as any);
    // DataView with non-zero byteOffset
    const backing = new ArrayBuffer(8);
    new Uint8Array(backing).set([
      0xaa,
      0xbb,
      0x20,
      0x21,
      0x22,
      0xcc,
      0xdd,
      0xee,
    ]);
    // deno-lint-ignore no-explicit-any
    await writer.write(new DataView(backing, 2, 3) as any);
    await writer.close();

    const got = await serverConnPromise;
    listener.close();
    assertEquals(
      got,
      new Uint8Array([
        0x01,
        0x02,
        0x03,
        0x04,
        0x05,
        0x06,
        0x07,
        0x08,
        0x09,
        0x0a,
        0x0b,
        0x0c,
        0x0d,
        0x0e,
        0x0f,
        0x10,
        0x11,
        0x20,
        0x21,
        0x22,
      ]),
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function writableStreamForRidRejectsNonBufferChunk() {
    const listener = Deno.listen({ port: 0, hostname: "127.0.0.1" });
    const port = (listener.addr as Deno.NetAddr).port;
    const acceptPromise = (async () => {
      const conn = await listener.accept();
      const buf = new Uint8Array(16);
      while ((await conn.read(buf)) !== null) { /* drain */ }
      conn.close();
    })();

    const client = await Deno.connect({ hostname: "127.0.0.1", port });
    const writer = client.writable.getWriter();
    await assertRejects(
      // deno-lint-ignore no-explicit-any
      () => writer.write("not a buffer" as any),
      TypeError,
      "ArrayBuffer or ArrayBufferView",
    );
    // The failed write closes the underlying connection via the sink's
    // controller-error path, so the server's read returns null.
    await acceptPromise;
    listener.close();
  },
);

// When both the source and the sink are resource-backed (here a file readable
// piped into a file writable), `pipeTo` takes the Rust `op_pipe` fast path.
// This exercises byte-exact transfer and the default close behavior.
Deno.test(
  { permissions: { read: true, write: true } },
  async function pipeToResourceBackedFastPath() {
    const input = await Deno.makeTempFile();
    const output = await Deno.makeTempFile();
    try {
      const data = new Uint8Array(1024 * 1024);
      for (let i = 0; i < data.length; i++) data[i] = i % 251;
      await Deno.writeFile(input, data);

      using src = await Deno.open(input, { read: true });
      using dst = await Deno.open(output, { write: true });
      await src.readable.pipeTo(dst.writable);

      assertEquals(await Deno.readFile(output), data);
    } finally {
      await Deno.remove(input);
      await Deno.remove(output);
    }
  },
);

// With `preventClose: true` the resource-backed sink must stay writable after
// the fast-path pipe completes.
Deno.test(
  { permissions: { read: true, write: true } },
  async function pipeToResourceBackedPreventClose() {
    const input = await Deno.makeTempFile();
    const output = await Deno.makeTempFile();
    try {
      await Deno.writeTextFile(input, "hello ");
      using src = await Deno.open(input, { read: true });
      const dst = await Deno.open(output, { write: true });
      await src.readable.pipeTo(dst.writable, { preventClose: true });

      const writer = dst.writable.getWriter();
      await writer.write(new TextEncoder().encode("world"));
      await writer.close();

      assertEquals(await Deno.readTextFile(output), "hello world");
    } finally {
      await Deno.remove(input);
      await Deno.remove(output);
    }
  },
);

// A present-but-never-aborted signal still takes the fast path (exercising the
// cancel-handle wiring) and completes normally, byte-exact.
Deno.test(
  { permissions: { read: true, write: true } },
  async function pipeToResourceBackedSignalNotAborted() {
    const input = await Deno.makeTempFile();
    const output = await Deno.makeTempFile();
    try {
      const data = new Uint8Array(128 * 1024);
      for (let i = 0; i < data.length; i++) data[i] = i % 251;
      await Deno.writeFile(input, data);

      using src = await Deno.open(input, { read: true });
      using dst = await Deno.open(output, { write: true });
      const ac = new AbortController();
      await src.readable.pipeTo(dst.writable, { signal: ac.signal });

      assertEquals(await Deno.readFile(output), data);
    } finally {
      await Deno.remove(input);
      await Deno.remove(output);
    }
  },
);

// Aborting the signal mid-pump must unblock `op_pipe` (the source never
// produces data, so without cancellation the pipe would hang forever), reject
// with the abort reason, and tear both ends down.
Deno.test(
  { permissions: { net: true, read: true, write: true } },
  async function pipeToResourceBackedAbortSignal() {
    const listener = Deno.listen({ port: 0 });
    const connectPromise = Deno.connect({ port: listener.addr.port });
    const server = await listener.accept();
    const client = await connectPromise;
    listener.close();

    // The server never writes, so the resource-backed read blocks; only the
    // abort signal (via op_pipe cancellation) can unblock it.
    const outPath = await Deno.makeTempFile();
    const out = await Deno.open(outPath, { write: true });

    const ac = new AbortController();
    const piped = client.readable.pipeTo(out.writable, { signal: ac.signal });
    const timer = setTimeout(() => ac.abort(), 50);

    const error = await assertRejects(() => piped, DOMException);
    assertEquals(error.name, "AbortError");

    clearTimeout(timer);
    // The abort cancelled `client.readable` (closing the conn) and aborted
    // `out.writable` (closing the file); only the server remains open.
    server.close();
    await Deno.remove(outPath);
  },
);

// A real (non-abort) resource error mid-pump must surface through
// `annotateResourceStreamError` — the bad-resource message is rewritten to a
// stream-level one — and still tear both ends down. Here the sink's underlying
// resource is closed out from under the pipe, so `op_pipe` fails with a
// bad-resource error rather than an abort.
Deno.test(
  { permissions: { read: true, write: true } },
  async function pipeToResourceBackedSinkError() {
    const input = await Deno.makeTempFile();
    const output = await Deno.makeTempFile();
    try {
      await Deno.writeTextFile(input, "some data to pipe");
      using src = await Deno.open(input, { read: true });
      const dst = await Deno.open(output, { write: true });
      // Close the sink's underlying resource, leaving its writable stream in the
      // "writable" state so the fast path is still taken.
      dst.close();

      const error = await assertRejects(() =>
        src.readable.pipeTo(dst.writable)
      );
      assert(
        (error as Error).message.includes(
          "The stream's underlying resource was closed or consumed",
        ),
        `unexpected error message: ${(error as Error).message}`,
      );
    } finally {
      await Deno.remove(input);
      await Deno.remove(output);
    }
  },
);

// With `preventCancel: true`, an abort mid-pump must tear down the sink but
// leave the source uncancelled, so its underlying connection stays open and
// readable. This locks in the flag handling in the fast path's `catch` block.
Deno.test(
  { permissions: { net: true, read: true, write: true } },
  async function pipeToResourceBackedPreventCancel() {
    const listener = Deno.listen({ port: 0 });
    const connectPromise = Deno.connect({ port: listener.addr.port });
    const server = await listener.accept();
    const client = await connectPromise;
    listener.close();

    const outPath = await Deno.makeTempFile();
    const out = await Deno.open(outPath, { write: true });

    const ac = new AbortController();
    // The server never writes, so the pump blocks on the read; only the abort
    // (via op_pipe cancellation) unblocks it.
    const piped = client.readable.pipeTo(out.writable, {
      signal: ac.signal,
      preventCancel: true,
    });
    const timer = setTimeout(() => ac.abort(), 50);

    const error = await assertRejects(() => piped, DOMException);
    assertEquals(error.name, "AbortError");
    clearTimeout(timer);

    // `preventCancel` kept the source open: the server can still send bytes and
    // the client receives them over the same connection.
    await server.write(new TextEncoder().encode("still open"));
    const reader = client.readable.getReader({ mode: "byob" });
    const { value } = await reader.read(new Uint8Array(16));
    assertEquals(new TextDecoder().decode(value), "still open");
    reader.releaseLock();

    client.close();
    server.close();
    await Deno.remove(outPath);
  },
);
