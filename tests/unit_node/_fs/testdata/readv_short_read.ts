// Copyright 2018-2026 the Deno authors. MIT license.

import { closeSync, openSync, readSync, readv, readvSync } from "node:fs";

function viewBytes(view: NodeJS.ArrayBufferView): number[] {
  return [
    ...new Uint8Array(view.buffer, view.byteOffset, view.byteLength),
  ];
}

function makeBuffers() {
  const dataViewBacking = new Uint8Array([255, 0, 0, 254]);
  const uint16Backing = new Uint8Array([253, 252, 0, 0, 0, 0, 251, 250]);
  const buffers = [
    new Uint8Array(0),
    new DataView(dataViewBacking.buffer, 1, 2),
    new Uint8Array(0),
    new Uint16Array(uint16Backing.buffer, 2, 2),
  ];
  return { buffers, dataViewBacking, uint16Backing };
}

function readvAsync(
  fd: number,
  buffers: NodeJS.ArrayBufferView[],
  position?: number,
) {
  let callReturned = false;
  const promise = new Promise<{
    bytesRead: number;
    buffersMatch: boolean;
    callbackWasAsync: boolean;
  }>((resolve, reject) => {
    const callback = (
      err: NodeJS.ErrnoException | null,
      bytesRead: number,
      returnedBuffers: readonly NodeJS.ArrayBufferView[],
    ) => {
      if (err) {
        reject(err);
        return;
      }
      resolve({
        bytesRead,
        buffersMatch: returnedBuffers === buffers,
        callbackWasAsync: callReturned,
      });
    };
    if (position === undefined) {
      readv(fd, buffers, callback);
    } else {
      readv(fd, buffers, position, callback);
    }
  });
  callReturned = true;
  return promise;
}

async function testFileReads() {
  const file = Deno.makeTempFileSync();
  try {
    Deno.writeTextFileSync(file, "abc");

    const syncViews = makeBuffers();
    const syncFd = openSync(file, "r");
    const syncBytesRead = readvSync(syncFd, syncViews.buffers);
    const syncEofBytesRead = readvSync(syncFd, [new Uint8Array(1)]);
    closeSync(syncFd);

    const asyncViews = makeBuffers();
    const asyncFd = openSync(file, "r");
    const asyncResult = await readvAsync(asyncFd, asyncViews.buffers);
    const asyncEofResult = await readvAsync(asyncFd, [new Uint8Array(1)]);
    closeSync(asyncFd);

    Deno.writeTextFileSync(file, "abcdef");
    const syncPositionBuffer = new Uint8Array(2);
    const syncCursorBuffer = new Uint8Array(1);
    const syncPositionFd = openSync(file, "r");
    const syncPositionBytesRead = readvSync(
      syncPositionFd,
      [syncPositionBuffer],
      3,
    );
    readSync(syncPositionFd, syncCursorBuffer, 0, 1, null);
    closeSync(syncPositionFd);

    const asyncPositionBuffer = new Uint8Array(2);
    const asyncCursorBuffer = new Uint8Array(1);
    const asyncPositionFd = openSync(file, "r");
    const asyncPositionResult = await readvAsync(
      asyncPositionFd,
      [asyncPositionBuffer],
      3,
    );
    readSync(asyncPositionFd, asyncCursorBuffer, 0, 1, null);
    closeSync(asyncPositionFd);

    const emptyFd = openSync(file, "r");
    const emptySync = readvSync(emptyFd, [new Uint8Array(0)]);
    const emptyAsync = await readvAsync(emptyFd, [new Uint8Array(0)]);
    closeSync(emptyFd);

    const closedFd = openSync(file, "r");
    closeSync(closedFd);
    let invalidZeroLengthSyncCode;
    try {
      readvSync(closedFd, [new Uint8Array(0)]);
    } catch (error) {
      invalidZeroLengthSyncCode = (error as NodeJS.ErrnoException).code;
    }
    const invalidZeroLengthBuffers = [new Uint8Array(0)];
    let invalidZeroLengthCallReturned = false;
    const invalidZeroLengthPromise = new Promise<{
      code: string | undefined;
      bytesRead: number;
      buffersMatch: boolean;
      callbackWasAsync: boolean;
    }>((resolve) => {
      readv(
        closedFd,
        invalidZeroLengthBuffers,
        (error, bytesRead, returnedBuffers) => {
          resolve({
            code: error?.code,
            bytesRead,
            buffersMatch: returnedBuffers === invalidZeroLengthBuffers,
            callbackWasAsync: invalidZeroLengthCallReturned,
          });
        },
      );
    });
    invalidZeroLengthCallReturned = true;
    const invalidZeroLengthAsync = await invalidZeroLengthPromise;

    const nonNumberPositionFd = openSync(file, "r");
    readSync(nonNumberPositionFd, new Uint8Array(1), 0, 1, null);
    const nonNumberPositionBuffer = new Uint8Array(1);
    const readvSyncWithUnknownPosition = readvSync as unknown as (
      fd: number,
      buffers: NodeJS.ArrayBufferView[],
      position: unknown,
    ) => number;
    readvSyncWithUnknownPosition(
      nonNumberPositionFd,
      [nonNumberPositionBuffer],
      "4",
    );
    closeSync(nonNumberPositionFd);

    const overlappingBuffer = new Uint8Array(2);
    const overlappingFd = openSync(file, "r");
    const overlappingBytesRead = readvSync(overlappingFd, [
      overlappingBuffer,
      overlappingBuffer,
    ]);
    closeSync(overlappingFd);

    return {
      sync: {
        bytesRead: syncBytesRead,
        eofBytesRead: syncEofBytesRead,
        dataViewBacking: [...syncViews.dataViewBacking],
        uint16Backing: [...syncViews.uint16Backing],
      },
      async: {
        ...asyncResult,
        eofBytesRead: asyncEofResult.bytesRead,
        dataViewBacking: [...asyncViews.dataViewBacking],
        uint16Backing: [...asyncViews.uint16Backing],
      },
      positionedSync: {
        bytesRead: syncPositionBytesRead,
        positioned: [...syncPositionBuffer],
        cursor: [...syncCursorBuffer],
      },
      positionedAsync: {
        ...asyncPositionResult,
        positioned: [...asyncPositionBuffer],
        cursor: [...asyncCursorBuffer],
      },
      empty: {
        sync: emptySync,
        async: emptyAsync,
      },
      invalidZeroLength: {
        syncCode: invalidZeroLengthSyncCode,
        async: invalidZeroLengthAsync,
      },
      nonNumberPosition: [...nonNumberPositionBuffer],
      overlapping: {
        bytesRead: overlappingBytesRead,
        buffer: [...overlappingBuffer],
      },
    };
  } finally {
    Deno.removeSync(file);
  }
}

function testPipeSync() {
  const buffers = [new Uint8Array(2), new Uint8Array(4)];
  const bytesRead = readvSync(0, buffers);
  return { bytesRead, buffers: buffers.map(viewBytes) };
}

async function testPipeAsync() {
  const buffers = [new Uint8Array(2), new Uint8Array(4)];
  let timerFired = false;
  setTimeout(() => {
    timerFired = true;
    Deno.stderr.writeSync(new TextEncoder().encode("ready\n"));
  }, 0);
  const result = await readvAsync(0, buffers);
  return { ...result, timerFired, buffers: buffers.map(viewBytes) };
}

const mode = Deno.args[0];
let result;
switch (mode) {
  case "file":
    result = await testFileReads();
    break;
  case "pipe-sync":
    result = testPipeSync();
    break;
  case "pipe-async":
    result = await testPipeAsync();
    break;
  default:
    throw new Error(`Unknown mode: ${mode}`);
}
Deno.stdout.writeSync(
  new TextEncoder().encode(`${JSON.stringify(result)}\n`),
);
