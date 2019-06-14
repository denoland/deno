// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/*
SharedQueue Binary Layout
+-------------------------------+-------------------------------+
|                        NUM_RECORDS (32)                       |
+---------------------------------------------------------------+
|                        NUM_SHIFTED_OFF (32)                   |
+---------------------------------------------------------------+
|                        HEAD (32)                              |
+---------------------------------------------------------------+
|                        OFFSETS (32)                           |
+---------------------------------------------------------------+
|                        RECORD_ENDS (*MAX_RECORDS)           ...
+---------------------------------------------------------------+
|                        RECORDS (*MAX_RECORDS)               ...
+---------------------------------------------------------------+
 */

/* eslint-disable @typescript-eslint/no-use-before-define */

(window => {
  const GLOBAL_NAMESPACE = "Deno";
  const CORE_NAMESPACE = "core";
  const MAX_RECORDS = 100;
  const INDEX_NUM_RECORDS = 0;
  const INDEX_NUM_SHIFTED_OFF = 1;
  const INDEX_HEAD = 2;
  const INDEX_OFFSETS = 3;
  const INDEX_RECORDS = 3 + MAX_RECORDS;
  const HEAD_INIT = 4 * INDEX_RECORDS;

  const Deno = window[GLOBAL_NAMESPACE];
  const core = Deno[CORE_NAMESPACE];

  let sharedBytes;
  let shared32;
  let initialized = false;

  function maybeInit() {
    if (!initialized) {
      init();
      initialized = true;
    }
  }

  function init() {
    let shared = Deno.core.shared;
    assert(shared.byteLength > 0);
    assert(sharedBytes == null);
    assert(shared32 == null);
    sharedBytes = new Uint8Array(shared);
    shared32 = new Int32Array(shared);
    // Callers should not call Deno.core.recv, use setAsyncHandler.
    Deno.core.recv(handleAsyncMsgFromRust);
  }

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  function reset() {
    maybeInit();
    shared32[INDEX_NUM_RECORDS] = 0;
    shared32[INDEX_NUM_SHIFTED_OFF] = 0;
    shared32[INDEX_HEAD] = HEAD_INIT;
  }

  function head() {
    maybeInit();
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
    if (end > shared32.byteLength || index >= MAX_RECORDS) {
      // console.log("shared_queue.js push fail");
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

    assert(off != null);
    assert(end != null);
    return sharedBytes.subarray(off, end);
  }

  let asyncHandler;
  function setAsyncHandler(cb) {
    maybeInit();
    assert(asyncHandler == null);
    asyncHandler = cb;
  }

  function handleAsyncMsgFromRust(buf) {
    if (buf) {
      asyncHandler(buf);
    } else {
      while ((buf = shift()) != null) {
        asyncHandler(buf);
      }
    }
  }

  function dispatch(control, zeroCopy = null) {
    maybeInit();
    // First try to push control to shared.
    const success = push(control);
    // If successful, don't use first argument of core.send.
    const arg0 = success ? null : control;
    return window.Deno.core.send(arg0, zeroCopy);
  }

  const denoCore = {
    setAsyncHandler,
    dispatch,
    sharedQueue: {
      MAX_RECORDS,
      head,
      numRecords,
      size,
      push,
      reset,
      shift
    }
  };

  assert(window[GLOBAL_NAMESPACE] != null);
  assert(window[GLOBAL_NAMESPACE][CORE_NAMESPACE] != null);
  Object.assign(core, denoCore);
})(this);
