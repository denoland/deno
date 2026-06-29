// Copyright 2018-2026 the Deno authors. MIT license.

// Benchmarks exercising the hot paths optimized in ext/web/06_streams.js:
//   - Write request queue (writableStreamAddWriteRequest /
//     writableStreamMarkFirstWriteRequestInFlight)
//   - BYOB pending pull-into queue and read-into requests
//   - Default reader read fast path (cached stream[_state])

const chunk = new Uint8Array(64);

// Many enqueue/read pairs on a default ReadableStream. Exercises the default
// reader read path (stream[_state] cached) and the underlying
// _readRequests/_queue handling.
Deno.bench("readable_stream_default_read", { n: 1e4 }, async () => {
  const stream = new ReadableStream({
    start(controller) {
      for (let i = 0; i < 16; i++) controller.enqueue(chunk);
      controller.close();
    },
  });
  const reader = stream.getReader();
  while (true) {
    const { done } = await reader.read();
    if (done) break;
  }
});

// Many concurrent writes drain through the _writeRequests queue. Before the
// Queue conversion each completion was ArrayPrototypeShift (O(n) per write).
Deno.bench("writable_stream_many_writes", { n: 5e3 }, async () => {
  const stream = new WritableStream({ write() {} });
  const writer = stream.getWriter();
  const promises = [];
  for (let i = 0; i < 32; i++) promises.push(writer.write(chunk));
  await Promise.all(promises);
  await writer.close();
});

// Stress the BYOB pull-into pipeline: many pending pull-intos are queued and
// then satisfied as bytes arrive. Exercises _pendingPullIntos enqueue/peek/
// dequeue and _readIntoRequests handling.
Deno.bench("readable_byte_stream_byob_read", { n: 2e3 }, async () => {
  const stream = new ReadableStream({
    type: "bytes",
    pull(controller) {
      controller.enqueue(new Uint8Array(64));
    },
  });
  const reader = stream.getReader({ mode: "byob" });
  for (let i = 0; i < 16; i++) {
    await reader.read(new Uint8Array(64));
  }
  reader.releaseLock();
});

// Pipe-through with a TransformStream. Exercises both the readable and
// writable sides plus the default controller enqueue path on every chunk.
Deno.bench("transform_stream_pipe_through", { n: 1e3 }, async () => {
  const src = new ReadableStream({
    start(controller) {
      for (let i = 0; i < 64; i++) controller.enqueue(chunk);
      controller.close();
    },
  });
  const passthrough = new TransformStream();
  const piped = src.pipeThrough(passthrough);
  const reader = piped.getReader();
  while (true) {
    const { done } = await reader.read();
    if (done) break;
  }
});
