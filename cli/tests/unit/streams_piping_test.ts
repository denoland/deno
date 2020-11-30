// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertThrowsAsync } from "../../../std/testing/asserts.ts";
import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(function streamPipeLocks() {
  const rs = new ReadableStream();
  const ws = new WritableStream();

  assertEquals(rs.locked, false);
  assertEquals(ws.locked, false);

  rs.pipeTo(ws);

  assert(rs.locked);
  assert(ws.locked);
});

unitTest(async function streamPipeFinishUnlocks() {
  const rs = new ReadableStream({
    start(controller: ReadableStreamDefaultController): void {
      controller.close();
    },
  });
  const ws = new WritableStream();

  await rs.pipeTo(ws);
  assertEquals(rs.locked, false);
  assertEquals(ws.locked, false);
});

unitTest(async function streamPipeReadableStreamLocked() {
  const rs = new ReadableStream();
  const ws = new WritableStream();

  rs.getReader();

  await assertThrowsAsync(async () => {
    await rs.pipeTo(ws);
  }, TypeError);
});

unitTest(async function streamPipeReadableStreamLocked() {
  const rs = new ReadableStream();
  const ws = new WritableStream();

  ws.getWriter();

  await assertThrowsAsync(async () => {
    await rs.pipeTo(ws);
  }, TypeError);
});

unitTest(async function streamPipeLotsOfChunks() {
  const CHUNKS = 10;

  const rs = new ReadableStream<number>({
    start(c: ReadableStreamDefaultController): void {
      for (let i = 0; i < CHUNKS; ++i) {
        c.enqueue(i);
      }
      c.close();
    },
  });

  const written: Array<string | number> = [];
  const ws = new WritableStream(
    {
      write(chunk: number): void {
        written.push(chunk);
      },
      close(): void {
        written.push("closed");
      },
    },
    new CountQueuingStrategy({ highWaterMark: CHUNKS }),
  );

  await rs.pipeTo(ws);
  const targetValues = [];
  for (let i = 0; i < CHUNKS; ++i) {
    targetValues.push(i);
  }
  targetValues.push("closed");

  assertEquals(written, targetValues, "the correct values must be written");

  // Ensure both readable and writable are closed by the time the pipe finishes.
  await Promise.all([rs.getReader().closed, ws.getWriter().closed]);
});

for (const preventAbort of [true, false]) {
  unitTest(function undefinedRejectionFromPull() {
    const rs = new ReadableStream({
      pull(): Promise<void> {
        return Promise.reject(undefined);
      },
    });

    return rs.pipeTo(new WritableStream(), { preventAbort }).then(
      () => {
        throw new Error("pipeTo promise should be rejected");
      },
      (value) =>
        assertEquals(value, undefined, "rejection value should be undefined"),
    );
  });
}

for (const preventCancel of [true, false]) {
  unitTest(function undefinedRejectionWithPreventCancel() {
    const rs = new ReadableStream({
      pull(controller: ReadableStreamDefaultController<number>): void {
        controller.enqueue(0);
      },
    });

    const ws = new WritableStream({
      write(): Promise<void> {
        return Promise.reject(undefined);
      },
    });

    return rs.pipeTo(ws, { preventCancel }).then(
      () => {
        throw new Error("pipeTo promise should be rejected");
      },
      (value) =>
        assertEquals(value, undefined, "rejection value should be undefined"),
    );
  });
}
