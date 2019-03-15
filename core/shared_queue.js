// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
(window => {
  const MAX_RECORDS = 100;
  const INDEX_NUM_RECORDS = 0;
  const INDEX_NUM_SHIFTED_OFF = 1;
  const INDEX_HEAD = 2;
  const INDEX_OFFSETS = 3;
  const INDEX_RECORDS = 3 + MAX_RECORDS;
  const HEAD_INIT = 4 * INDEX_RECORDS;

  let sharedBytes = null;
  let shared32 = null;

  if (!window["Deno"]) {
    window["Deno"] = {};
  }

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  function reset() {
    shared32[INDEX_NUM_RECORDS] = 0;
    shared32[INDEX_NUM_SHIFTED_OFF] = 0;
    shared32[INDEX_HEAD] = HEAD_INIT;
  }

  function head() {
    return shared32[INDEX_HEAD];
  }

  function numRecords() {
    return shared32[INDEX_NUM_RECORDS];
  }

  function size() {
    return shared32[INDEX_NUM_RECORDS] - shared32[INDEX_NUM_SHIFTED_OFF];
  }

  function setEnd(index, end) {
    shared32[INDEX_OFFSETS + index] = end;
  }

  function getEnd(index) {
    if (index < numRecords()) {
      return shared32[INDEX_OFFSETS + index];
    } else {
      return null;
    }
  }

  function getOffset(index) {
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

  function push(buf) {
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

  /// Returns null if empty.
  function shift() {
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

    return sharedBytes.subarray(off, end);
  }

  let asyncHandler = null;
  function setAsyncHandler(cb) {
    assert(asyncHandler == null);
    asyncHandler = cb;
  }

  function handleAsyncMsgFromRust() {
    let buf;
    while ((buf = shift()) != null) {
      asyncHandler(buf);
    }
  }

  function init(shared) {
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
})(this);
