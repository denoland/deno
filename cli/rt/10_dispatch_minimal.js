// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const util = window.__bootstrap.util;

  // Using an object without a prototype because `Map` was causing GC problems.
  const promiseTableMin = Object.create(null);

  // Note it's important that promiseId starts at 1 instead of 0, because sync
  // messages are indicated with promiseId 0. If we ever add wrap around logic for
  // overflows, this should be taken into account.
  let _nextPromiseId = 1;

  const decoder = new TextDecoder();

  function nextPromiseId() {
    return _nextPromiseId++;
  }

  function recordFromBufMinimal(ui8) {
    const header = ui8.subarray(0, 12);
    const buf32 = new Int32Array(
      header.buffer,
      header.byteOffset,
      header.byteLength / 4,
    );
    const promiseId = buf32[0];
    const arg = buf32[1];
    const result = buf32[2];
    let err;

    if (arg < 0) {
      const codeLen = result;
      const codeAndMessage = decoder.decode(ui8.subarray(12));
      const errorCode = codeAndMessage.slice(0, codeLen);
      const message = codeAndMessage.slice(codeLen);
      err = { kind: errorCode, message };
    } else if (ui8.length != 12) {
      throw new TypeError("Malformed response message");
    }

    return {
      promiseId,
      arg,
      result,
      err,
    };
  }

  function unwrapResponse(res) {
    if (res.err != null) {
      throw new (core.getErrorClass(res.err.kind))(res.err.message);
    }
    return res.result;
  }

  const scratch32 = new Int32Array(3);
  const scratchBytes = new Uint8Array(
    scratch32.buffer,
    scratch32.byteOffset,
    scratch32.byteLength,
  );
  util.assert(scratchBytes.byteLength === scratch32.length * 4);

  function asyncMsgFromRust(ui8) {
    const record = recordFromBufMinimal(ui8);
    const { promiseId } = record;
    const promise = promiseTableMin[promiseId];
    delete promiseTableMin[promiseId];
    util.assert(promise);
    promise.resolve(record);
  }

  async function sendAsync(
    opName,
    arg,
    zeroCopy,
  ) {
    const promiseId = nextPromiseId(); // AKA cmdId
    scratch32[0] = promiseId;
    scratch32[1] = arg;
    scratch32[2] = 0; // result
    const promise = util.createResolvable();
    const buf = core.dispatchByName(opName, scratchBytes, zeroCopy);
    if (buf != null) {
      const record = recordFromBufMinimal(buf);
      // Sync result.
      promise.resolve(record);
    } else {
      // Async result.
      promiseTableMin[promiseId] = promise;
    }

    const res = await promise;
    return unwrapResponse(res);
  }

  function sendSync(
    opName,
    arg,
    zeroCopy,
  ) {
    scratch32[0] = 0; // promiseId 0 indicates sync
    scratch32[1] = arg;
    const res = core.dispatchByName(opName, scratchBytes, zeroCopy);
    const resRecord = recordFromBufMinimal(res);
    return unwrapResponse(resRecord);
  }

  window.__bootstrap.dispatchMinimal = {
    asyncMsgFromRust,
    sendSync,
    sendAsync,
  };
})(this);
