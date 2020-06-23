// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "./test_util.ts";

function delay(seconds: number): Promise<void> {
  return new Promise<void>((resolve) => {
    setTimeout(() => {
      resolve();
    }, seconds);
  });
}

function readableStreamToArray<R>(
  readable: { getReader(): ReadableStreamDefaultReader<R> },
  reader?: ReadableStreamDefaultReader<R>
): Promise<R[]> {
  if (reader === undefined) {
    reader = readable.getReader();
  }

  const chunks: R[] = [];

  return pump();

  function pump(): Promise<R[]> {
    return reader!.read().then((result) => {
      if (result.done) {
        return chunks;
      }

      chunks.push(result.value);
      return pump();
    });
  }
}

unitTest(function transformStreamConstructedWithTransformFunction() {
  new TransformStream({ transform(): void {} });
});

unitTest(function transformStreamConstructedNoTransform() {
  new TransformStream();
  new TransformStream({});
});

unitTest(function transformStreamIntstancesHaveProperProperties() {
  const ts = new TransformStream({ transform(): void {} });
  const proto = Object.getPrototypeOf(ts);

  const writableStream = Object.getOwnPropertyDescriptor(proto, "writable");
  assert(writableStream !== undefined, "it has a writable property");
  assert(!writableStream.enumerable, "writable should be non-enumerable");
  assertEquals(
    typeof writableStream.get,
    "function",
    "writable should have a getter"
  );
  assertEquals(
    writableStream.set,
    undefined,
    "writable should not have a setter"
  );
  assert(writableStream.configurable, "writable should be configurable");
  assert(
    ts.writable instanceof WritableStream,
    "writable is an instance of WritableStream"
  );
  assert(
    WritableStream.prototype.getWriter.call(ts.writable),
    "writable should pass WritableStream brand check"
  );

  const readableStream = Object.getOwnPropertyDescriptor(proto, "readable");
  assert(readableStream !== undefined, "it has a readable property");
  assert(!readableStream.enumerable, "readable should be non-enumerable");
  assertEquals(
    typeof readableStream.get,
    "function",
    "readable should have a getter"
  );
  assertEquals(
    readableStream.set,
    undefined,
    "readable should not have a setter"
  );
  assert(readableStream.configurable, "readable should be configurable");
  assert(
    ts.readable instanceof ReadableStream,
    "readable is an instance of ReadableStream"
  );
  assertNotEquals(
    ReadableStream.prototype.getReader.call(ts.readable),
    undefined,
    "readable should pass ReadableStream brand check"
  );
});

unitTest(function transformStreamWritableStartsAsWritable() {
  const ts = new TransformStream({ transform(): void {} });

  const writer = ts.writable.getWriter();
  assertEquals(writer.desiredSize, 1, "writer.desiredSize should be 1");
});

unitTest(async function transformStreamReadableCanReadOutOfWritable() {
  const ts = new TransformStream();

  const writer = ts.writable.getWriter();
  writer.write("a");
  assertEquals(
    writer.desiredSize,
    0,
    "writer.desiredSize should be 0 after write()"
  );

  const result = await ts.readable.getReader().read();
  assertEquals(
    result.value,
    "a",
    "result from reading the readable is the same as was written to writable"
  );
  assert(!result.done, "stream should not be done");

  await delay(0);
  assert(writer.desiredSize === 1, "desiredSize should be 1 again");
});

unitTest(async function transformStreamCanReadWhatIsWritten() {
  let c: TransformStreamDefaultController;
  const ts = new TransformStream({
    start(controller: TransformStreamDefaultController): void {
      c = controller;
    },
    transform(chunk: string): void {
      c.enqueue(chunk.toUpperCase());
    },
  });

  const writer = ts.writable.getWriter();
  writer.write("a");

  const result = await ts.readable.getReader().read();
  assertEquals(
    result.value,
    "A",
    "result from reading the readable is the transformation of what was written to writable"
  );
  assert(!result.done, "stream should not be done");
});

unitTest(async function transformStreamCanReadBothChunks() {
  let c: TransformStreamDefaultController;
  const ts = new TransformStream({
    start(controller: TransformStreamDefaultController): void {
      c = controller;
    },
    transform(chunk: string): void {
      c.enqueue(chunk.toUpperCase());
      c.enqueue(chunk.toUpperCase());
    },
  });

  const writer = ts.writable.getWriter();
  writer.write("a");

  const reader = ts.readable.getReader();

  const result1 = await reader.read();
  assertEquals(
    result1.value,
    "A",
    "the first chunk read is the transformation of the single chunk written"
  );
  assert(!result1.done, "stream should not be done");

  const result2 = await reader.read();
  assertEquals(
    result2.value,
    "A",
    "the second chunk read is also the transformation of the single chunk written"
  );
  assert(!result2.done, "stream should not be done");
});

unitTest(async function transformStreamCanReadWhatIsWritten() {
  let c: TransformStreamDefaultController;
  const ts = new TransformStream({
    start(controller: TransformStreamDefaultController): void {
      c = controller;
    },
    transform(chunk: string): Promise<void> {
      return delay(0).then(() => c.enqueue(chunk.toUpperCase()));
    },
  });

  const writer = ts.writable.getWriter();
  writer.write("a");

  const result = await ts.readable.getReader().read();
  assertEquals(
    result.value,
    "A",
    "result from reading the readable is the transformation of what was written to writable"
  );
  assert(!result.done, "stream should not be done");
});

unitTest(async function transformStreamAsyncReadMultipleChunks() {
  let doSecondEnqueue: () => void;
  let returnFromTransform: () => void;
  const ts = new TransformStream({
    transform(
      chunk: string,
      controller: TransformStreamDefaultController
    ): Promise<void> {
      delay(0).then(() => controller.enqueue(chunk.toUpperCase()));
      doSecondEnqueue = (): void => controller.enqueue(chunk.toUpperCase());
      return new Promise((resolve) => {
        returnFromTransform = resolve;
      });
    },
  });

  const reader = ts.readable.getReader();

  const writer = ts.writable.getWriter();
  writer.write("a");

  const result1 = await reader.read();
  assertEquals(
    result1.value,
    "A",
    "the first chunk read is the transformation of the single chunk written"
  );
  assert(!result1.done, "stream should not be done");
  doSecondEnqueue!();

  const result2 = await reader.read();
  assertEquals(
    result2.value,
    "A",
    "the second chunk read is also the transformation of the single chunk written"
  );
  assert(!result2.done, "stream should not be done");
  returnFromTransform!();
});

unitTest(function transformStreamClosingWriteClosesRead() {
  const ts = new TransformStream({ transform(): void {} });

  const writer = ts.writable.getWriter();
  writer.close();

  return Promise.all([writer.closed, ts.readable.getReader().closed]).then(
    undefined
  );
});

unitTest(async function transformStreamCloseWaitAwaitsTransforms() {
  let transformResolve: () => void;
  const transformPromise = new Promise<void>((resolve) => {
    transformResolve = resolve;
  });
  const ts = new TransformStream(
    {
      transform(): Promise<void> {
        return transformPromise;
      },
    },
    undefined,
    { highWaterMark: 1 }
  );

  const writer = ts.writable.getWriter();
  writer.write("a");
  writer.close();

  let rsClosed = false;
  ts.readable.getReader().closed.then(() => {
    rsClosed = true;
  });

  await delay(0);
  assertEquals(rsClosed, false, "readable is not closed after a tick");
  transformResolve!();

  await writer.closed;
  // TODO: Is this expectation correct?
  assertEquals(rsClosed, true, "readable is closed at that point");
});

unitTest(async function transformStreamCloseWriteAfterSyncEnqueues() {
  let c: TransformStreamDefaultController<string>;
  const ts = new TransformStream<string, string>({
    start(controller: TransformStreamDefaultController): void {
      c = controller;
    },
    transform(): Promise<void> {
      c.enqueue("x");
      c.enqueue("y");
      return delay(0);
    },
  });

  const writer = ts.writable.getWriter();
  writer.write("a");
  writer.close();

  const readableChunks = readableStreamToArray(ts.readable);

  await writer.closed;
  const chunks = await readableChunks;
  assertEquals(
    chunks,
    ["x", "y"],
    "both enqueued chunks can be read from the readable"
  );
});

unitTest(async function transformStreamWritableCloseAsyncAfterAsyncEnqueues() {
  let c: TransformStreamDefaultController<string>;
  const ts = new TransformStream<string, string>({
    start(controller: TransformStreamDefaultController<string>): void {
      c = controller;
    },
    transform(): Promise<void> {
      return delay(0)
        .then(() => c.enqueue("x"))
        .then(() => c.enqueue("y"))
        .then(() => delay(0));
    },
  });

  const writer = ts.writable.getWriter();
  writer.write("a");
  writer.close();

  const readableChunks = readableStreamToArray(ts.readable);

  await writer.closed;
  const chunks = await readableChunks;
  assertEquals(
    chunks,
    ["x", "y"],
    "both enqueued chunks can be read from the readable"
  );
});

unitTest(async function transformStreamTransformerMethodsCalledAsMethods() {
  let c: TransformStreamDefaultController<string>;
  const transformer = {
    suffix: "-suffix",

    start(controller: TransformStreamDefaultController<string>): void {
      c = controller;
      c.enqueue("start" + this.suffix);
    },

    transform(chunk: string): void {
      c.enqueue(chunk + this.suffix);
    },

    flush(): void {
      c.enqueue("flushed" + this.suffix);
    },
  };
  const ts = new TransformStream(transformer);

  const writer = ts.writable.getWriter();
  writer.write("a");
  writer.close();

  const readableChunks = readableStreamToArray(ts.readable);

  await writer.closed;
  const chunks = await readableChunks;
  assertEquals(
    chunks,
    ["start-suffix", "a-suffix", "flushed-suffix"],
    "all enqueued chunks have suffixes"
  );
});

unitTest(async function transformStreamMethodsShouldNotBeAppliedOrCalled() {
  function functionWithOverloads(): void {}
  functionWithOverloads.apply = (): void => {
    throw new Error("apply() should not be called");
  };
  functionWithOverloads.call = (): void => {
    throw new Error("call() should not be called");
  };
  const ts = new TransformStream({
    start: functionWithOverloads,
    transform: functionWithOverloads,
    flush: functionWithOverloads,
  });
  const writer = ts.writable.getWriter();
  writer.write("a");
  writer.close();

  await readableStreamToArray(ts.readable);
});

unitTest(async function transformStreamCallTransformSync() {
  let transformCalled = false;
  const ts = new TransformStream(
    {
      transform(): void {
        transformCalled = true;
      },
    },
    undefined,
    { highWaterMark: Infinity }
  );
  // transform() is only called synchronously when there is no backpressure and
  // all microtasks have run.
  await delay(0);
  const writePromise = ts.writable.getWriter().write(undefined);
  assert(transformCalled, "transform() should have been called");
  await writePromise;
});

unitTest(function transformStreamCloseWriteCloesesReadWithNoChunks() {
  const ts = new TransformStream({}, undefined, { highWaterMark: 0 });

  const writer = ts.writable.getWriter();
  writer.close();

  return Promise.all([writer.closed, ts.readable.getReader().closed]).then(
    undefined
  );
});

unitTest(function transformStreamEnqueueThrowsAfterTerminate() {
  new TransformStream({
    start(controller: TransformStreamDefaultController): void {
      controller.terminate();
      assertThrows(() => {
        controller.enqueue(undefined);
      }, TypeError);
    },
  });
});

unitTest(function transformStreamEnqueueThrowsAfterReadableCancel() {
  let controller: TransformStreamDefaultController;
  const ts = new TransformStream({
    start(c: TransformStreamDefaultController): void {
      controller = c;
    },
  });
  const cancelPromise = ts.readable.cancel();
  assertThrows(
    () => controller.enqueue(undefined),
    TypeError,
    undefined,
    "enqueue should throw"
  );
  return cancelPromise;
});

unitTest(function transformStreamSecondTerminateNoOp() {
  new TransformStream({
    start(controller: TransformStreamDefaultController): void {
      controller.terminate();
      controller.terminate();
    },
  });
});

unitTest(async function transformStreamTerminateAfterReadableCancelIsNoop() {
  let controller: TransformStreamDefaultController;
  const ts = new TransformStream({
    start(c: TransformStreamDefaultController): void {
      controller = c;
    },
  });
  const cancelReason = { name: "cancelReason" };
  const cancelPromise = ts.readable.cancel(cancelReason);
  controller!.terminate();
  await cancelPromise;
  try {
    await ts.writable.getWriter().closed;
  } catch (e) {
    assert(e === cancelReason);
    return;
  }
  throw new Error("closed should have rejected");
});

unitTest(async function transformStreamStartCalledOnce() {
  let calls = 0;
  new TransformStream({
    start(): void {
      ++calls;
    },
  });
  await delay(0);
  assertEquals(calls, 1, "start() should have been called exactly once");
});

unitTest(function transformStreamReadableTypeThrows() {
  assertThrows(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => new TransformStream({ readableType: "bytes" as any }),
    RangeError,
    undefined,
    "constructor should throw"
  );
});

unitTest(function transformStreamWirtableTypeThrows() {
  assertThrows(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => new TransformStream({ writableType: "bytes" as any }),
    RangeError,
    undefined,
    "constructor should throw"
  );
});

unitTest(function transformStreamSubclassable() {
  class Subclass extends TransformStream {
    extraFunction(): boolean {
      return true;
    }
  }
  assert(
    Object.getPrototypeOf(Subclass.prototype) === TransformStream.prototype,
    "Subclass.prototype's prototype should be TransformStream.prototype"
  );
  assert(
    Object.getPrototypeOf(Subclass) === TransformStream,
    "Subclass's prototype should be TransformStream"
  );
  const sub = new Subclass();
  assert(
    sub instanceof TransformStream,
    "Subclass object should be an instance of TransformStream"
  );
  assert(
    sub instanceof Subclass,
    "Subclass object should be an instance of Subclass"
  );
  const readableGetter = Object.getOwnPropertyDescriptor(
    TransformStream.prototype,
    "readable"
  )!.get;
  assert(
    readableGetter!.call(sub) === sub.readable,
    "Subclass object should pass brand check"
  );
  assert(
    sub.extraFunction(),
    "extraFunction() should be present on Subclass object"
  );
});
