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
(window => {
  function debug(...msg) {
    if (!window) {
      // TODO: never reach. we need macro like debug! for rust...
      console.log("js:shared_queue:", ...msg);
    }
  }
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

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  function reset() {
    debug("reset");
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

  function numShiftedOff() {
    return shared32[INDEX_NUM_SHIFTED_OFF];
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
      if (index === 0) {
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
    if (end > shared32.byteLength || size() >= MAX_RECORDS) {
      debug("push fail");
      return false;
    }
    setEnd(index, end);
    assert(end - off === buf.byteLength);
    sharedBytes.set(buf, off);
    shared32[INDEX_NUM_RECORDS] += 1;
    shared32[INDEX_HEAD] = end;
    debug(`push: num_records=${numRecords()}, index_head=${head()}`);
    return true;
  }

  /// Returns null if empty.
  function shift() {
    let i = shared32[INDEX_NUM_SHIFTED_OFF];
    debug(`shift:begin: num_records=${numRecords()}, shifted_off=${i}`);
    if (size() === 0) {
      assert(i === 0);
      return null;
    }

    let off = getOffset(i);
    let end = getEnd(i);

    debug(`shift:going off=${off}, end=${end}, ablen=${sharedBytes.length}`);
    if (size() > 1) {
      debug(`shift:incr size=${size()}, num_shifted_off=${numShiftedOff()}`);
      shared32[INDEX_NUM_SHIFTED_OFF] += 1;
      debug(`shift:done num_shifted_off=${numShiftedOff()}`);
    } else {
      debug(`shift: size=${size()}. will reset`);
      reset();
    }

    assert(off != null);
    assert(end != null);
    debug(
      `shift: num_records=${numRecords()}, num_shifted_off=${numShiftedOff()}`
    );
    return sharedBytes.subarray(off, end);
  }

  let asyncHandler;

  function setAsyncHandler(cb) {
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

  function init(shared) {
    assert(shared.byteLength > 0);
    assert(sharedBytes == null);
    assert(shared32 == null);
    debug(`init: ${shared.byteLength}`);
    sharedBytes = new Uint8Array(shared);
    shared32 = new Int32Array(shared);
    // Callers should not call Deno.core.recv, use setAsyncHandler.
    window.Deno.core.recv(handleAsyncMsgFromRust);
  }

  function dispatch(control, zeroCopy = null) {
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

  init(Deno.core.shared);
})(globalThis);
