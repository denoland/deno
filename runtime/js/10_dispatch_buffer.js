// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ////////////////////////////// General async handling //////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  // General Async response handling
  let nextRequestId = 1;
  const promiseTable = {};

  function opAsync(opName, opRequestBuilder, opResultParser) {
    // Make sure requests of this type are handled by the asyncHandler
    // The asyncHandler's role is to call the "promiseTable[requestId]" function
    core.setAsyncHandlerByName(opName, (bufUi8, _) => {
      const [requestId, result, error] = opResultParser(bufUi8, true);
      if (error !== null) {
        promiseTable[requestId][1](error);
      } else {
        promiseTable[requestId][0](result);
      }
      delete promiseTable[requestId];
    });

    const requestId = nextRequestId++;

    // Create and store promise
    const promise = new Promise((resolve, reject) => {
      promiseTable[requestId] = [resolve, reject];
    });

    // Synchronously dispatch async request
    core.dispatchByName(opName, ...opRequestBuilder(requestId));

    // Wait for async response
    return promise;
  }

  function opSync(opName, opRequestBuilder, opResultParser) {
    const rawResult = core.dispatchByName(opName, ...opRequestBuilder());

    const [_, result, error] = opResultParser(rawResult, false);
    if (error !== null) throw error;
    return result;
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  /////////////////////////////////// Error handling /////////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  function handleError(className, message) {
    const [ErrorClass, args] = core.getErrorClassAndArgs(className);
    if (!ErrorClass) {
      return new Error(
        `Unregistered error class: "${className}"\n  ${message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
      );
    }
    return new ErrorClass(message, ...args);
  }

  ////////////////////////////////////////////////////////////////////////////////////////////
  ///////////////////////////////// Buffer ops handling //////////////////////////////////////
  ////////////////////////////////////////////////////////////////////////////////////////////

  const scratchBytes = new ArrayBuffer(3 * 4);
  const scratchView = new DataView(
    scratchBytes,
    scratchBytes.byteOffset,
    scratchBytes.byteLength,
  );

  function bufferOpBuildRequest(requestId, argument, zeroCopy) {
    scratchView.setBigUint64(0, BigInt(requestId), true);
    scratchView.setUint32(8, argument, true);
    return [scratchView, ...zeroCopy];
  }

  function bufferOpParseResult(bufUi8, isCopyNeeded) {
    // Decode header value from ui8 buffer
    const headerByteLength = 4 * 4;
    assert(bufUi8.byteLength >= headerByteLength);
    assert(bufUi8.byteLength % 4 == 0);
    const view = new DataView(
      bufUi8.buffer,
      bufUi8.byteOffset + bufUi8.byteLength - headerByteLength,
      headerByteLength,
    );

    const requestId = Number(view.getBigUint64(0, true));
    const status = view.getUint32(8, true);
    const result = view.getUint32(12, true);

    // Error handling
    if (status !== 0) {
      const className = core.decode(bufUi8.subarray(0, result));
      const message = core.decode(bufUi8.subarray(result, -headerByteLength))
        .trim();

      return [requestId, null, handleError(className, message)];
    }

    if (bufUi8.byteLength === headerByteLength) {
      return [requestId, result, null];
    }

    // Rest of response buffer is passed as reference or as a copy
    let respBuffer = null;
    if (isCopyNeeded) {
      respBuffer = bufUi8.slice(0, result); // Copy part of the response array (if sent through shared array buf)
    } else {
      respBuffer = bufUi8.subarray(0, result); // Create view on existing array (if sent through overflow)
    }

    return [requestId, respBuffer, null];
  }

  function bufferOpAsync(opName, argument = 0, ...zeroCopy) {
    return opAsync(
      opName,
      (requestId) => bufferOpBuildRequest(requestId, argument, zeroCopy),
      bufferOpParseResult,
    );
  }

  function bufferOpSync(opName, argument = 0, ...zeroCopy) {
    return opSync(
      opName,
      () => bufferOpBuildRequest(0, argument, zeroCopy),
      bufferOpParseResult,
    );
  }

  window.__bootstrap.dispatchBuffer = {
    bufferOpSync,
    bufferOpAsync,
  };
})(this);
