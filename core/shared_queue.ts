// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

interface Window {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  Deno: any;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare var Deno: { _sharedQueue: any };

declare var libdeno: {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  print(...args: any): void;
  recv(handler: () => void): void;
  shared: ArrayBuffer;
};

declare var globalThis: Window;

((window: Window) => {
  const MAX_RECORDS = 100;
  const INDEX_NUM_RECORDS = 0;
  const INDEX_NUM_SHIFTED_OFF = 1;
  const INDEX_HEAD = 2;
  const INDEX_OFFSETS = 3;
  const INDEX_RECORDS = 3 + MAX_RECORDS;
  const HEAD_INIT = 4 * INDEX_RECORDS;

  let sharedBytes: Uint8Array;
  let shared32: Int32Array;

  if (!window["Deno"]) {
    window["Deno"] = {};
  }

  function assert(cond: boolean): void {
    if (!cond) {
      throw Error("assert");
    }
  }

  function reset(): void {
    shared32[INDEX_NUM_RECORDS] = 0;
    shared32[INDEX_NUM_SHIFTED_OFF] = 0;
    shared32[INDEX_HEAD] = HEAD_INIT;
  }

  function head(): number {
    return shared32[INDEX_HEAD];
  }

  function numRecords(): number {
    return shared32[INDEX_NUM_RECORDS];
  }

  function size(): number {
    return shared32[INDEX_NUM_RECORDS] - shared32[INDEX_NUM_SHIFTED_OFF];
  }

  function setEnd(index: number, end: number): void {
    shared32[INDEX_OFFSETS + index] = end;
  }

  function getEnd(index: number): number | null {
    if (index < numRecords()) {
      return shared32[INDEX_OFFSETS + index];
    } else {
      return null;
    }
  }

  function getOffset(index: number): number | null {
    if (index < numRecords()) {
      if (index == 0) {
        return HEAD_INIT;
      } else {
        return shared32[INDEX_OFFSETS + index - 1];
      }
    } else {
      return null;
    }
  }

  function push(buf: Uint8Array): boolean {
    let off = head();
    let end = off + buf.byteLength;
    let index = numRecords();
    if (end > shared32.byteLength) {
      console.log("shared_queue.ts push fail");
      return false;
    }
    setEnd(index, end);
    assert(end - off == buf.byteLength);
    sharedBytes.set(buf, off);
    shared32[INDEX_NUM_RECORDS] += 1;
    shared32[INDEX_HEAD] = end;
    return true;
  }

  // Returns null if empty.
  function shift(): Uint8Array | null {
    let i = shared32[INDEX_NUM_SHIFTED_OFF];
    if (size() == 0) {
      assert(i == 0);
      return null;
    }

    let off = getOffset(i);
    let end = getEnd(i);

    if (size() > 1) {
      shared32[INDEX_NUM_SHIFTED_OFF] += 1;
    } else {
      reset();
    }

    assert(off != null);
    assert(end != null);

    return sharedBytes.subarray(off!, end!);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let asyncHandler: (...args: any[]) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function setAsyncHandler(cb: (...args: any[]) => void): void {
    assert(asyncHandler == null);
    asyncHandler = cb;
  }

  function handleAsyncMsgFromRust(): void {
    let buf;
    while ((buf = shift()) != null) {
      asyncHandler(buf);
    }
  }

  function init(shared: ArrayBuffer): void {
    assert(shared.byteLength > 0);
    assert(sharedBytes == null);
    assert(shared32 == null);
    sharedBytes = new Uint8Array(shared);
    shared32 = new Int32Array(shared);
    // Callers should not call libdeno.recv, use setAsyncHandler.
    libdeno.recv(handleAsyncMsgFromRust);
  }

  window.Deno._setAsyncHandler = setAsyncHandler;
  window.Deno._sharedQueue = {
    head,
    numRecords,
    size,
    push,
    shift
  };

  init(libdeno.shared);
})(globalThis);
