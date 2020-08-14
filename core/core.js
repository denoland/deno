// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
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

((window) => {
  const MAX_RECORDS = 100;
  const INDEX_NUM_RECORDS = 0;
  const INDEX_NUM_SHIFTED_OFF = 1;
  const INDEX_HEAD = 2;
  const INDEX_OFFSETS = 3;
  const INDEX_RECORDS = INDEX_OFFSETS + 2 * MAX_RECORDS;
  const HEAD_INIT = 4 * INDEX_RECORDS;

  // Available on start due to bindings.
  const core = window.Deno.core;
  const { recv, send } = core;

  let sharedBytes;
  let shared32;

  let asyncHandlers;

  let initialized = false;
  let opsCache = {};
  const errorMap = {};

  function maybeInit() {
    if (!initialized) {
      init();
      initialized = true;
    }
  }

  function init() {
    const shared = core.shared;
    assert(shared.byteLength > 0);
    assert(sharedBytes == null);
    assert(shared32 == null);
    sharedBytes = new Uint8Array(shared);
    shared32 = new Int32Array(shared);
    asyncHandlers = [];
    // Callers should not call core.recv, use setAsyncHandler.
    recv(handleAsyncMsgFromRust);
  }

  function ops() {
    // op id 0 is a special value to retrieve the map of registered ops.
    const opsMapBytes = send(0, new Uint8Array([]));
    const opsMapJson = String.fromCharCode.apply(null, opsMapBytes);
    opsCache = JSON.parse(opsMapJson);
    return { ...opsCache };
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

  function setMeta(index, end, opId) {
    shared32[INDEX_OFFSETS + 2 * index] = end;
    shared32[INDEX_OFFSETS + 2 * index + 1] = opId;
  }

  function getMeta(index) {
    if (index < numRecords()) {
      const buf = shared32[INDEX_OFFSETS + 2 * index];
      const opId = shared32[INDEX_OFFSETS + 2 * index + 1];
      return [opId, buf];
    } else {
      return null;
    }
  }

  function getOffset(index) {
    if (index < numRecords()) {
      if (index == 0) {
        return HEAD_INIT;
      } else {
        const prevEnd = shared32[INDEX_OFFSETS + 2 * (index - 1)];
        return (prevEnd + 3) & ~3;
      }
    } else {
      return null;
    }
  }

  function push(opId, buf) {
    const off = head();
    const end = off + buf.byteLength;
    const alignedEnd = (end + 3) & ~3;
    const index = numRecords();
    if (alignedEnd > shared32.byteLength || index >= MAX_RECORDS) {
      // console.log("shared_queue.js push fail");
      return false;
    }
    setMeta(index, end, opId);
    assert(alignedEnd % 4 === 0);
    assert(end - off == buf.byteLength);
    sharedBytes.set(buf, off);
    shared32[INDEX_NUM_RECORDS] += 1;
    shared32[INDEX_HEAD] = alignedEnd;
    return true;
  }

  /// Returns null if empty.
  function shift() {
    const i = shared32[INDEX_NUM_SHIFTED_OFF];
    if (size() == 0) {
      assert(i == 0);
      return null;
    }

    const off = getOffset(i);
    const [opId, end] = getMeta(i);

    if (size() > 1) {
      shared32[INDEX_NUM_SHIFTED_OFF] += 1;
    } else {
      reset();
    }

    assert(off != null);
    assert(end != null);
    const buf = sharedBytes.subarray(off, end);
    return [opId, buf];
  }

  function setAsyncHandler(opId, cb) {
    maybeInit();
    assert(opId != null);
    asyncHandlers[opId] = cb;
  }

  function handleAsyncMsgFromRust(opId, buf) {
    if (buf) {
      // This is the overflow_response case of deno::Isolate::poll().
      asyncHandlers[opId](buf);
    } else {
      while (true) {
        const opIdBuf = shift();
        if (opIdBuf == null) {
          break;
        }
        assert(asyncHandlers[opIdBuf[0]] != null);
        asyncHandlers[opIdBuf[0]](opIdBuf[1]);
      }
    }
  }

  function dispatch(opName, control, ...zeroCopy) {
    return send(opsCache[opName], control, ...zeroCopy);
  }

  function registerErrorClass(errorName, errorClass) {
    if (typeof errorMap[errorName] !== "undefined") {
      throw new TypeError(`Error class for "${errorName}" already registered`);
    }
    errorMap[errorName] = errorClass;
  }

  function getErrorClass(errorName) {
    const errorClass = errorMap[errorName];
    assert(errorClass);
    return errorClass;
  }

  Object.assign(window.Deno.core, {
    setAsyncHandler,
    dispatch: send,
    dispatchByName: dispatch,
    ops,
    registerErrorClass,
    getErrorClass,
    // sharedQueue is private but exposed for testing.
    sharedQueue: {
      MAX_RECORDS,
      head,
      numRecords,
      size,
      push,
      reset,
      shift,
    },
  });
})(this);
