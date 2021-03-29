// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
"use strict";

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

  ////////////////////////////////////////////////////////////////////////////////////////////
  ///////////////////////////////////////// Dispatch /////////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  const dispatch = send;
  const dispatchByName = (opName, control, ...zeroCopy) =>
    dispatch(opsCache[opName], control, ...zeroCopy);

  ////////////////////////////////////////////////////////////////////////////////////////////
  //////////////////////////////////// Shared array buffer ///////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  let sharedBytes;
  let shared32;

  let opsCache = {};

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
    const opsMapBytes = send(0);
    const opsMapJson = String.fromCharCode.apply(null, opsMapBytes);
    opsCache = JSON.parse(opsMapJson);
    return { ...opsCache };
  }

  function assert(cond) {
    if (!cond) {
      throw new Error("assert");
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

  function setMeta(index, end, opId) {
    shared32[INDEX_OFFSETS + 2 * index] = end;
    shared32[INDEX_OFFSETS + 2 * index + 1] = opId;
  }

  function getMeta(index) {
    if (index >= numRecords()) {
      return null;
    }
    const buf = shared32[INDEX_OFFSETS + 2 * index];
    const opId = shared32[INDEX_OFFSETS + 2 * index + 1];
    return [opId, buf];
  }

  function getOffset(index) {
    if (index >= numRecords()) {
      return null;
    }
    if (index == 0) {
      return HEAD_INIT;
    }
    const prevEnd = shared32[INDEX_OFFSETS + 2 * (index - 1)];
    return (prevEnd + 3) & ~3;
  }

  function push(opId, buf) {
    const off = head();
    const end = off + buf.byteLength;
    const alignedEnd = (end + 3) & ~3;
    const index = numRecords();
    const shouldNotPush = alignedEnd > shared32.byteLength ||
      index >= MAX_RECORDS;
    if (shouldNotPush) {
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

  ////////////////////////////////////////////////////////////////////////////////////////////
  ////////////////////////////////////// Error handling //////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  const errorMap = {};

  function registerErrorClass(errorName, className, args) {
    if (typeof errorMap[errorName] !== "undefined") {
      throw new TypeError(`Error class for "${errorName}" already registered`);
    }
    errorMap[errorName] = [className, args ?? []];
  }

  function handleError(className, message) {
    if (typeof errorMap[className] === "undefined") {
      return new Error(
        `Unregistered error class: "${className}"\n` +
          `  ${message}\n` +
          `  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
      );
    }

    const [ErrorClass, args] = errorMap[className];
    return new ErrorClass(message, ...args);
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ////////////////////////////////////// Async handling //////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  let asyncHandlers = [];

  function setAsyncHandler(opId, cb) {
    assert(opId != null);
    asyncHandlers[opId] = cb;
  }

  function handleAsyncMsgFromRust() {
    while (true) {
      const opIdBuf = shift();
      if (opIdBuf == null) {
        break;
      }
      assert(asyncHandlers[opIdBuf[0]] != null);
      asyncHandlers[opIdBuf[0]](opIdBuf[1], true);
    }

    for (let i = 0; i < arguments.length; i += 2) {
      asyncHandlers[arguments[i]](arguments[i + 1], false);
    }
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ///////////////////////////// General sync & async ops handling ////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  let nextRequestId = 1;
  const promiseTable = {};

  function asyncHandle(u8Array, isCopyNeeded, opResultParser) {
    const [requestId, result, error] = opResultParser(u8Array, isCopyNeeded);
    if (error !== null) {
      promiseTable[requestId][1](error);
    } else {
      promiseTable[requestId][0](result);
    }
    delete promiseTable[requestId];
  }

  function opAsync(opName, opRequestBuilder, opResultParser) {
    const opId = opsCache[opName];
    // Make sure requests of this type are handled by the asyncHandler
    // The asyncHandler's role is to call the "promiseTable[requestId]" function
    if (typeof asyncHandlers[opId] === "undefined") {
      asyncHandlers[opId] = (buffer, isCopyNeeded) =>
        asyncHandle(buffer, isCopyNeeded, opResultParser);
    }

    const requestId = nextRequestId++;

    // Create and store promise
    const promise = new Promise((resolve, reject) => {
      promiseTable[requestId] = [resolve, reject];
    });

    // Synchronously dispatch async request
    core.dispatch(opId, ...opRequestBuilder(requestId));

    // Wait for async response
    return promise;
  }

  function opSync(opName, opRequestBuilder, opResultParser) {
    const opId = opsCache[opName];
    const u8Array = core.dispatch(opId, ...opRequestBuilder());

    const [_, result, error] = opResultParser(u8Array, false);
    if (error !== null) throw error;
    return result;
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ///////////////////////////////////// Bin ops handling /////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  const binRequestHeaderByteLength = 8 + 4;
  const scratchBuffer = new ArrayBuffer(binRequestHeaderByteLength);
  const scratchView = new DataView(scratchBuffer);

  function binOpBuildRequest(requestId, argument, zeroCopy) {
    scratchView.setBigUint64(0, BigInt(requestId), true);
    scratchView.setUint32(8, argument, true);
    return [scratchView, ...zeroCopy];
  }

  function binOpParseResult(u8Array, isCopyNeeded) {
    // Decode header value from u8Array
    const headerByteLength = 8 + 2 * 4;
    assert(u8Array.byteLength >= headerByteLength);
    assert(u8Array.byteLength % 4 == 0);
    const view = new DataView(
      u8Array.buffer,
      u8Array.byteOffset + u8Array.byteLength - headerByteLength,
      headerByteLength,
    );

    const requestId = Number(view.getBigUint64(0, true));
    const status = view.getUint32(8, true);
    const result = view.getUint32(12, true);

    // Error handling
    if (status !== 0) {
      const className = core.decode(u8Array.subarray(0, result));
      const message = core.decode(u8Array.subarray(result, -headerByteLength))
        .trim();

      return [requestId, null, handleError(className, message)];
    }

    if (u8Array.byteLength === headerByteLength) {
      return [requestId, result, null];
    }

    // Rest of response buffer is passed as reference or as a copy
    let respBuffer = null;
    if (isCopyNeeded) {
      // Copy part of the response array (if sent through shared array buf)
      respBuffer = u8Array.slice(0, result);
    } else {
      // Create view on existing array (if sent through overflow)
      respBuffer = u8Array.subarray(0, result);
    }

    return [requestId, respBuffer, null];
  }

  function binOpAsync(opName, argument = 0, ...zeroCopy) {
    return opAsync(
      opName,
      (requestId) => binOpBuildRequest(requestId, argument, zeroCopy),
      binOpParseResult,
    );
  }

  function binOpSync(opName, argument = 0, ...zeroCopy) {
    return opSync(
      opName,
      () => binOpBuildRequest(0, argument, zeroCopy),
      binOpParseResult,
    );
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ///////////////////////////////////// Json ops handling ////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  const jsonRequestHeaderLength = 8;

  function jsonOpBuildRequest(requestId, argument, zeroCopy) {
    const u8Array = core.encode(
      "\0".repeat(jsonRequestHeaderLength) + JSON.stringify(argument),
    );
    new DataView(u8Array.buffer).setBigUint64(0, BigInt(requestId), true);
    return [u8Array, ...zeroCopy];
  }

  function jsonOpParseResult(u8Array, _) {
    const data = JSON.parse(core.decode(u8Array));

    if ("err" in data) {
      return [
        data.requestId,
        null,
        handleError(data.err.className, data.err.message),
      ];
    }

    return [data.requestId, data.ok, null];
  }

  function jsonOpAsync(opName, argument = null, ...zeroCopy) {
    return opAsync(
      opName,
      (requestId) => jsonOpBuildRequest(requestId, argument, zeroCopy),
      jsonOpParseResult,
    );
  }

  function jsonOpSync(opName, argument = null, ...zeroCopy) {
    return opSync(
      opName,
      () => [core.encode(JSON.stringify(argument)), ...zeroCopy],
      jsonOpParseResult,
    );
  }

  function resources() {
    return jsonOpSync("op_resources");
  }
  function close(rid) {
    return jsonOpSync("op_close", { rid });
  }

  Object.assign(window.Deno.core, {
    jsonOpAsync,
    jsonOpSync,
    binOpAsync,
    binOpSync,
    dispatch,
    dispatchByName,
    ops,
    close,
    resources,
    registerErrorClass,
    sharedQueueInit: init,
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
    // setAsyncHandler is private but exposed for testing.
    setAsyncHandler,
  });
})(this);
