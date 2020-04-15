// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register("$deno$/web/streams/shared-internals.ts", [], function (
  exports_73,
  context_73
) {
  "use strict";
  let objectCloneMemo, sharedArrayBufferSupported_;
  const __moduleName = context_73 && context_73.id;
  function isInteger(value) {
    if (!isFinite(value)) {
      // covers NaN, +Infinity and -Infinity
      return false;
    }
    const absValue = Math.abs(value);
    return Math.floor(absValue) === absValue;
  }
  exports_73("isInteger", isInteger);
  function isFiniteNonNegativeNumber(value) {
    if (!(typeof value === "number" && isFinite(value))) {
      // covers NaN, +Infinity and -Infinity
      return false;
    }
    return value >= 0;
  }
  exports_73("isFiniteNonNegativeNumber", isFiniteNonNegativeNumber);
  function isAbortSignal(signal) {
    if (typeof signal !== "object" || signal === null) {
      return false;
    }
    try {
      // TODO
      // calling signal.aborted() probably isn't the right way to perform this test
      // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L41
      signal.aborted();
      return true;
    } catch (err) {
      return false;
    }
  }
  exports_73("isAbortSignal", isAbortSignal);
  function invokeOrNoop(o, p, args) {
    // Assert: O is not undefined.
    // Assert: IsPropertyKey(P) is true.
    // Assert: args is a List.
    const method = o[p]; // tslint:disable-line:ban-types
    if (method === undefined) {
      return undefined;
    }
    return Function.prototype.apply.call(method, o, args);
  }
  exports_73("invokeOrNoop", invokeOrNoop);
  function cloneArrayBuffer(
    srcBuffer,
    srcByteOffset,
    srcLength,
    _cloneConstructor
  ) {
    // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
    return srcBuffer.slice(srcByteOffset, srcByteOffset + srcLength);
  }
  exports_73("cloneArrayBuffer", cloneArrayBuffer);
  function transferArrayBuffer(buffer) {
    // This would in a JS engine context detach the buffer's backing store and return
    // a new ArrayBuffer with the same backing store, invalidating `buffer`,
    // i.e. a move operation in C++ parlance.
    // Sadly ArrayBuffer.transfer is yet to be implemented by a single browser vendor.
    return buffer.slice(0); // copies instead of moves
  }
  exports_73("transferArrayBuffer", transferArrayBuffer);
  function copyDataBlockBytes(toBlock, toIndex, fromBlock, fromIndex, count) {
    new Uint8Array(toBlock, toIndex, count).set(
      new Uint8Array(fromBlock, fromIndex, count)
    );
  }
  exports_73("copyDataBlockBytes", copyDataBlockBytes);
  function supportsSharedArrayBuffer() {
    if (sharedArrayBufferSupported_ === undefined) {
      try {
        new SharedArrayBuffer(16);
        sharedArrayBufferSupported_ = true;
      } catch (e) {
        sharedArrayBufferSupported_ = false;
      }
    }
    return sharedArrayBufferSupported_;
  }
  function cloneValue(value) {
    const valueType = typeof value;
    switch (valueType) {
      case "number":
      case "string":
      case "boolean":
      case "undefined":
      // @ts-ignore
      case "bigint":
        return value;
      case "object": {
        if (objectCloneMemo.has(value)) {
          return objectCloneMemo.get(value);
        }
        if (value === null) {
          return value;
        }
        if (value instanceof Date) {
          return new Date(value.valueOf());
        }
        if (value instanceof RegExp) {
          return new RegExp(value);
        }
        if (supportsSharedArrayBuffer() && value instanceof SharedArrayBuffer) {
          return value;
        }
        if (value instanceof ArrayBuffer) {
          const cloned = cloneArrayBuffer(
            value,
            0,
            value.byteLength,
            ArrayBuffer
          );
          objectCloneMemo.set(value, cloned);
          return cloned;
        }
        if (ArrayBuffer.isView(value)) {
          const clonedBuffer = cloneValue(value.buffer);
          // Use DataViewConstructor type purely for type-checking, can be a DataView or TypedArray.
          // They use the same constructor signature, only DataView has a length in bytes and TypedArrays
          // use a length in terms of elements, so we adjust for that.
          let length;
          if (value instanceof DataView) {
            length = value.byteLength;
          } else {
            length = value.length;
          }
          return new value.constructor(clonedBuffer, value.byteOffset, length);
        }
        if (value instanceof Map) {
          const clonedMap = new Map();
          objectCloneMemo.set(value, clonedMap);
          value.forEach((v, k) => clonedMap.set(k, cloneValue(v)));
          return clonedMap;
        }
        if (value instanceof Set) {
          const clonedSet = new Map();
          objectCloneMemo.set(value, clonedSet);
          value.forEach((v, k) => clonedSet.set(k, cloneValue(v)));
          return clonedSet;
        }
        // generic object
        const clonedObj = {};
        objectCloneMemo.set(value, clonedObj);
        const sourceKeys = Object.getOwnPropertyNames(value);
        for (const key of sourceKeys) {
          clonedObj[key] = cloneValue(value[key]);
        }
        return clonedObj;
      }
      case "symbol":
      case "function":
      default:
        // TODO this should be a DOMException,
        // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L171
        throw new Error("Uncloneable value in stream");
    }
  }
  exports_73("cloneValue", cloneValue);
  function promiseCall(f, v, args) {
    // tslint:disable-line:ban-types
    try {
      const result = Function.prototype.apply.call(f, v, args);
      return Promise.resolve(result);
    } catch (err) {
      return Promise.reject(err);
    }
  }
  exports_73("promiseCall", promiseCall);
  function createAlgorithmFromUnderlyingMethod(obj, methodName, extraArgs) {
    const method = obj[methodName];
    if (method === undefined) {
      return () => Promise.resolve(undefined);
    }
    if (typeof method !== "function") {
      throw new TypeError(`Field "${methodName}" is not a function.`);
    }
    return function (...fnArgs) {
      return promiseCall(method, obj, fnArgs.concat(extraArgs));
    };
  }
  exports_73(
    "createAlgorithmFromUnderlyingMethod",
    createAlgorithmFromUnderlyingMethod
  );
  /*
    Deprecated for now, all usages replaced by readableStreamCreateReadResult
    
    function createIterResultObject<T>(value: T, done: boolean): IteratorResult<T> {
        return { value, done };
    }
    */
  function validateAndNormalizeHighWaterMark(hwm) {
    const highWaterMark = Number(hwm);
    if (isNaN(highWaterMark) || highWaterMark < 0) {
      throw new RangeError(
        "highWaterMark must be a valid, non-negative integer."
      );
    }
    return highWaterMark;
  }
  exports_73(
    "validateAndNormalizeHighWaterMark",
    validateAndNormalizeHighWaterMark
  );
  function makeSizeAlgorithmFromSizeFunction(sizeFn) {
    if (typeof sizeFn !== "function" && typeof sizeFn !== "undefined") {
      throw new TypeError("size function must be undefined or a function");
    }
    return function (chunk) {
      if (typeof sizeFn === "function") {
        return sizeFn(chunk);
      }
      return 1;
    };
  }
  exports_73(
    "makeSizeAlgorithmFromSizeFunction",
    makeSizeAlgorithmFromSizeFunction
  );
  function createControlledPromise() {
    const conProm = {
      state: 0 /* Pending */,
    };
    conProm.promise = new Promise(function (resolve, reject) {
      conProm.resolve = function (v) {
        conProm.state = 1 /* Resolved */;
        resolve(v);
      };
      conProm.reject = function (e) {
        conProm.state = 2 /* Rejected */;
        reject(e);
      };
    });
    return conProm;
  }
  exports_73("createControlledPromise", createControlledPromise);
  return {
    setters: [],
    execute: function () {
      // common stream fields
      exports_73("state_", Symbol("state_"));
      exports_73("storedError_", Symbol("storedError_"));
      // helper memoisation map for object values
      // weak so it doesn't keep memoized versions of old objects indefinitely.
      objectCloneMemo = new WeakMap();
    },
  };
});
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register("$deno$/web/streams/queue.ts", [], function (
  exports_74,
  context_74
) {
  "use strict";
  let CHUNK_SIZE, QueueImpl;
  const __moduleName = context_74 && context_74.id;
  return {
    setters: [],
    execute: function () {
      CHUNK_SIZE = 16384;
      QueueImpl = class QueueImpl {
        constructor() {
          this.chunks_ = [[]];
          this.readChunk_ = this.writeChunk_ = this.chunks_[0];
          this.length_ = 0;
        }
        push(t) {
          this.writeChunk_.push(t);
          this.length_ += 1;
          if (this.writeChunk_.length === CHUNK_SIZE) {
            this.writeChunk_ = [];
            this.chunks_.push(this.writeChunk_);
          }
        }
        front() {
          if (this.length_ === 0) {
            return undefined;
          }
          return this.readChunk_[0];
        }
        shift() {
          if (this.length_ === 0) {
            return undefined;
          }
          const t = this.readChunk_.shift();
          this.length_ -= 1;
          if (
            this.readChunk_.length === 0 &&
            this.readChunk_ !== this.writeChunk_
          ) {
            this.chunks_.shift();
            this.readChunk_ = this.chunks_[0];
          }
          return t;
        }
        get length() {
          return this.length_;
        }
      };
      exports_74("QueueImpl", QueueImpl);
    },
  };
});
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/queue-mixin.ts",
  ["$deno$/web/streams/queue.ts", "$deno$/web/streams/shared-internals.ts"],
  function (exports_75, context_75) {
    "use strict";
    let queue_ts_1, shared_internals_ts_1, queue_, queueTotalSize_;
    const __moduleName = context_75 && context_75.id;
    function dequeueValue(container) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      // Assert: container.[[queue]] is not empty.
      const pair = container[queue_].shift();
      const newTotalSize = container[queueTotalSize_] - pair.size;
      container[queueTotalSize_] = Math.max(0, newTotalSize); // < 0 can occur due to rounding errors.
      return pair.value;
    }
    exports_75("dequeueValue", dequeueValue);
    function enqueueValueWithSize(container, value, size) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      if (!shared_internals_ts_1.isFiniteNonNegativeNumber(size)) {
        throw new RangeError(
          "Chunk size must be a non-negative, finite numbers"
        );
      }
      container[queue_].push({ value, size });
      container[queueTotalSize_] += size;
    }
    exports_75("enqueueValueWithSize", enqueueValueWithSize);
    function peekQueueValue(container) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      // Assert: container.[[queue]] is not empty.
      return container[queue_].front().value;
    }
    exports_75("peekQueueValue", peekQueueValue);
    function resetQueue(container) {
      // Chrome (as of v67) has a steep performance cliff with large arrays
      // and shift(), around about 50k elements. While this is an unusual case
      // we use a simple wrapper around shift and push that is chunked to
      // avoid this pitfall.
      // @see: https://github.com/stardazed/sd-streams/issues/1
      container[queue_] = new queue_ts_1.QueueImpl();
      // The code below can be used as a plain array implementation of the
      // Queue interface.
      // const q = [] as any;
      // q.front = function() { return this[0]; };
      // container[queue_] = q;
      container[queueTotalSize_] = 0;
    }
    exports_75("resetQueue", resetQueue);
    return {
      setters: [
        function (queue_ts_1_1) {
          queue_ts_1 = queue_ts_1_1;
        },
        function (shared_internals_ts_1_1) {
          shared_internals_ts_1 = shared_internals_ts_1_1;
        },
      ],
      execute: function () {
        exports_75("queue_", (queue_ = Symbol("queue_")));
        exports_75(
          "queueTotalSize_",
          (queueTotalSize_ = Symbol("queueTotalSize_"))
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-internals.ts",
  [
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
  ],
  function (exports_76, context_76) {
    "use strict";
    let shared,
      q,
      controlledReadableStream_,
      pullAlgorithm_,
      cancelAlgorithm_,
      strategySizeAlgorithm_,
      strategyHWM_,
      started_,
      closeRequested_,
      pullAgain_,
      pulling_,
      cancelSteps_,
      pullSteps_,
      autoAllocateChunkSize_,
      byobRequest_,
      controlledReadableByteStream_,
      pendingPullIntos_,
      closedPromise_,
      ownerReadableStream_,
      readRequests_,
      readIntoRequests_,
      associatedReadableByteStreamController_,
      view_,
      reader_,
      readableStreamController_;
    const __moduleName = context_76 && context_76.id;
    // ---- Stream
    function initializeReadableStream(stream) {
      stream[shared.state_] = "readable";
      stream[reader_] = undefined;
      stream[shared.storedError_] = undefined;
      stream[readableStreamController_] = undefined; // mark slot as used for brand check
    }
    exports_76("initializeReadableStream", initializeReadableStream);
    function isReadableStream(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return readableStreamController_ in value;
    }
    exports_76("isReadableStream", isReadableStream);
    function isReadableStreamLocked(stream) {
      return stream[reader_] !== undefined;
    }
    exports_76("isReadableStreamLocked", isReadableStreamLocked);
    function readableStreamGetNumReadIntoRequests(stream) {
      // TODO remove the "as unknown" cast
      // This is in to workaround a compiler error
      // error TS2352: Conversion of type 'SDReadableStreamReader<OutputType>' to type 'SDReadableStreamBYOBReader' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first.
      // Type 'SDReadableStreamReader<OutputType>' is missing the following properties from type 'SDReadableStreamBYOBReader': read, [readIntoRequests_]
      const reader = stream[reader_];
      if (reader === undefined) {
        return 0;
      }
      return reader[readIntoRequests_].length;
    }
    exports_76(
      "readableStreamGetNumReadIntoRequests",
      readableStreamGetNumReadIntoRequests
    );
    function readableStreamGetNumReadRequests(stream) {
      const reader = stream[reader_];
      if (reader === undefined) {
        return 0;
      }
      return reader[readRequests_].length;
    }
    exports_76(
      "readableStreamGetNumReadRequests",
      readableStreamGetNumReadRequests
    );
    function readableStreamCreateReadResult(value, done, forAuthorCode) {
      const prototype = forAuthorCode ? Object.prototype : null;
      const result = Object.create(prototype);
      result.value = value;
      result.done = done;
      return result;
    }
    exports_76(
      "readableStreamCreateReadResult",
      readableStreamCreateReadResult
    );
    function readableStreamAddReadIntoRequest(stream, forAuthorCode) {
      // Assert: ! IsReadableStreamBYOBReader(stream.[[reader]]) is true.
      // Assert: stream.[[state]] is "readable" or "closed".
      const reader = stream[reader_];
      const conProm = shared.createControlledPromise();
      conProm.forAuthorCode = forAuthorCode;
      reader[readIntoRequests_].push(conProm);
      return conProm.promise;
    }
    exports_76(
      "readableStreamAddReadIntoRequest",
      readableStreamAddReadIntoRequest
    );
    function readableStreamAddReadRequest(stream, forAuthorCode) {
      // Assert: ! IsReadableStreamDefaultReader(stream.[[reader]]) is true.
      // Assert: stream.[[state]] is "readable".
      const reader = stream[reader_];
      const conProm = shared.createControlledPromise();
      conProm.forAuthorCode = forAuthorCode;
      reader[readRequests_].push(conProm);
      return conProm.promise;
    }
    exports_76("readableStreamAddReadRequest", readableStreamAddReadRequest);
    function readableStreamHasBYOBReader(stream) {
      const reader = stream[reader_];
      return isReadableStreamBYOBReader(reader);
    }
    exports_76("readableStreamHasBYOBReader", readableStreamHasBYOBReader);
    function readableStreamHasDefaultReader(stream) {
      const reader = stream[reader_];
      return isReadableStreamDefaultReader(reader);
    }
    exports_76(
      "readableStreamHasDefaultReader",
      readableStreamHasDefaultReader
    );
    function readableStreamCancel(stream, reason) {
      if (stream[shared.state_] === "closed") {
        return Promise.resolve(undefined);
      }
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      readableStreamClose(stream);
      const sourceCancelPromise = stream[readableStreamController_][
        cancelSteps_
      ](reason);
      return sourceCancelPromise.then((_) => undefined);
    }
    exports_76("readableStreamCancel", readableStreamCancel);
    function readableStreamClose(stream) {
      // Assert: stream.[[state]] is "readable".
      stream[shared.state_] = "closed";
      const reader = stream[reader_];
      if (reader === undefined) {
        return;
      }
      if (isReadableStreamDefaultReader(reader)) {
        for (const readRequest of reader[readRequests_]) {
          readRequest.resolve(
            readableStreamCreateReadResult(
              undefined,
              true,
              readRequest.forAuthorCode
            )
          );
        }
        reader[readRequests_] = [];
      }
      reader[closedPromise_].resolve();
      reader[closedPromise_].promise.catch(() => {});
    }
    exports_76("readableStreamClose", readableStreamClose);
    function readableStreamError(stream, error) {
      if (stream[shared.state_] !== "readable") {
        throw new RangeError("Stream is in an invalid state");
      }
      stream[shared.state_] = "errored";
      stream[shared.storedError_] = error;
      const reader = stream[reader_];
      if (reader === undefined) {
        return;
      }
      if (isReadableStreamDefaultReader(reader)) {
        for (const readRequest of reader[readRequests_]) {
          readRequest.reject(error);
        }
        reader[readRequests_] = [];
      } else {
        // Assert: IsReadableStreamBYOBReader(reader).
        // TODO remove the "as unknown" cast
        const readIntoRequests = reader[readIntoRequests_];
        for (const readIntoRequest of readIntoRequests) {
          readIntoRequest.reject(error);
        }
        // TODO remove the "as unknown" cast
        reader[readIntoRequests_] = [];
      }
      reader[closedPromise_].reject(error);
    }
    exports_76("readableStreamError", readableStreamError);
    // ---- Readers
    function isReadableStreamDefaultReader(reader) {
      if (typeof reader !== "object" || reader === null) {
        return false;
      }
      return readRequests_ in reader;
    }
    exports_76("isReadableStreamDefaultReader", isReadableStreamDefaultReader);
    function isReadableStreamBYOBReader(reader) {
      if (typeof reader !== "object" || reader === null) {
        return false;
      }
      return readIntoRequests_ in reader;
    }
    exports_76("isReadableStreamBYOBReader", isReadableStreamBYOBReader);
    function readableStreamReaderGenericInitialize(reader, stream) {
      reader[ownerReadableStream_] = stream;
      stream[reader_] = reader;
      const streamState = stream[shared.state_];
      reader[closedPromise_] = shared.createControlledPromise();
      if (streamState === "readable") {
        // leave as is
      } else if (streamState === "closed") {
        reader[closedPromise_].resolve(undefined);
      } else {
        reader[closedPromise_].reject(stream[shared.storedError_]);
        reader[closedPromise_].promise.catch(() => {});
      }
    }
    exports_76(
      "readableStreamReaderGenericInitialize",
      readableStreamReaderGenericInitialize
    );
    function readableStreamReaderGenericRelease(reader) {
      // Assert: reader.[[ownerReadableStream]] is not undefined.
      // Assert: reader.[[ownerReadableStream]].[[reader]] is reader.
      const stream = reader[ownerReadableStream_];
      if (stream === undefined) {
        throw new TypeError("Reader is in an inconsistent state");
      }
      if (stream[shared.state_] === "readable") {
        // code moved out
      } else {
        reader[closedPromise_] = shared.createControlledPromise();
      }
      reader[closedPromise_].reject(new TypeError());
      reader[closedPromise_].promise.catch(() => {});
      stream[reader_] = undefined;
      reader[ownerReadableStream_] = undefined;
    }
    exports_76(
      "readableStreamReaderGenericRelease",
      readableStreamReaderGenericRelease
    );
    function readableStreamBYOBReaderRead(reader, view, forAuthorCode = false) {
      const stream = reader[ownerReadableStream_];
      // Assert: stream is not undefined.
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      return readableByteStreamControllerPullInto(
        stream[readableStreamController_],
        view,
        forAuthorCode
      );
    }
    exports_76("readableStreamBYOBReaderRead", readableStreamBYOBReaderRead);
    function readableStreamDefaultReaderRead(reader, forAuthorCode = false) {
      const stream = reader[ownerReadableStream_];
      // Assert: stream is not undefined.
      if (stream[shared.state_] === "closed") {
        return Promise.resolve(
          readableStreamCreateReadResult(undefined, true, forAuthorCode)
        );
      }
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      // Assert: stream.[[state]] is "readable".
      return stream[readableStreamController_][pullSteps_](forAuthorCode);
    }
    exports_76(
      "readableStreamDefaultReaderRead",
      readableStreamDefaultReaderRead
    );
    function readableStreamFulfillReadIntoRequest(stream, chunk, done) {
      // TODO remove the "as unknown" cast
      const reader = stream[reader_];
      const readIntoRequest = reader[readIntoRequests_].shift(); // <-- length check done in caller
      readIntoRequest.resolve(
        readableStreamCreateReadResult(
          chunk,
          done,
          readIntoRequest.forAuthorCode
        )
      );
    }
    exports_76(
      "readableStreamFulfillReadIntoRequest",
      readableStreamFulfillReadIntoRequest
    );
    function readableStreamFulfillReadRequest(stream, chunk, done) {
      const reader = stream[reader_];
      const readRequest = reader[readRequests_].shift(); // <-- length check done in caller
      readRequest.resolve(
        readableStreamCreateReadResult(chunk, done, readRequest.forAuthorCode)
      );
    }
    exports_76(
      "readableStreamFulfillReadRequest",
      readableStreamFulfillReadRequest
    );
    // ---- DefaultController
    function setUpReadableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm
    ) {
      // Assert: stream.[[readableStreamController]] is undefined.
      controller[controlledReadableStream_] = stream;
      q.resetQueue(controller);
      controller[started_] = false;
      controller[closeRequested_] = false;
      controller[pullAgain_] = false;
      controller[pulling_] = false;
      controller[strategySizeAlgorithm_] = sizeAlgorithm;
      controller[strategyHWM_] = highWaterMark;
      controller[pullAlgorithm_] = pullAlgorithm;
      controller[cancelAlgorithm_] = cancelAlgorithm;
      stream[readableStreamController_] = controller;
      const startResult = startAlgorithm();
      Promise.resolve(startResult).then(
        (_) => {
          controller[started_] = true;
          // Assert: controller.[[pulling]] is false.
          // Assert: controller.[[pullAgain]] is false.
          readableStreamDefaultControllerCallPullIfNeeded(controller);
        },
        (error) => {
          readableStreamDefaultControllerError(controller, error);
        }
      );
    }
    exports_76(
      "setUpReadableStreamDefaultController",
      setUpReadableStreamDefaultController
    );
    function isReadableStreamDefaultController(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return controlledReadableStream_ in value;
    }
    exports_76(
      "isReadableStreamDefaultController",
      isReadableStreamDefaultController
    );
    function readableStreamDefaultControllerHasBackpressure(controller) {
      return !readableStreamDefaultControllerShouldCallPull(controller);
    }
    exports_76(
      "readableStreamDefaultControllerHasBackpressure",
      readableStreamDefaultControllerHasBackpressure
    );
    function readableStreamDefaultControllerCanCloseOrEnqueue(controller) {
      const state = controller[controlledReadableStream_][shared.state_];
      return controller[closeRequested_] === false && state === "readable";
    }
    exports_76(
      "readableStreamDefaultControllerCanCloseOrEnqueue",
      readableStreamDefaultControllerCanCloseOrEnqueue
    );
    function readableStreamDefaultControllerGetDesiredSize(controller) {
      const state = controller[controlledReadableStream_][shared.state_];
      if (state === "errored") {
        return null;
      }
      if (state === "closed") {
        return 0;
      }
      return controller[strategyHWM_] - controller[q.queueTotalSize_];
    }
    exports_76(
      "readableStreamDefaultControllerGetDesiredSize",
      readableStreamDefaultControllerGetDesiredSize
    );
    function readableStreamDefaultControllerClose(controller) {
      // Assert: !ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is true.
      controller[closeRequested_] = true;
      const stream = controller[controlledReadableStream_];
      if (controller[q.queue_].length === 0) {
        readableStreamDefaultControllerClearAlgorithms(controller);
        readableStreamClose(stream);
      }
    }
    exports_76(
      "readableStreamDefaultControllerClose",
      readableStreamDefaultControllerClose
    );
    function readableStreamDefaultControllerEnqueue(controller, chunk) {
      const stream = controller[controlledReadableStream_];
      // Assert: !ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is true.
      if (
        isReadableStreamLocked(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        readableStreamFulfillReadRequest(stream, chunk, false);
      } else {
        // Let result be the result of performing controller.[[strategySizeAlgorithm]], passing in chunk,
        // and interpreting the result as an ECMAScript completion value.
        // impl note: assuming that in JS land this just means try/catch with rethrow
        let chunkSize;
        try {
          chunkSize = controller[strategySizeAlgorithm_](chunk);
        } catch (error) {
          readableStreamDefaultControllerError(controller, error);
          throw error;
        }
        try {
          q.enqueueValueWithSize(controller, chunk, chunkSize);
        } catch (error) {
          readableStreamDefaultControllerError(controller, error);
          throw error;
        }
      }
      readableStreamDefaultControllerCallPullIfNeeded(controller);
    }
    exports_76(
      "readableStreamDefaultControllerEnqueue",
      readableStreamDefaultControllerEnqueue
    );
    function readableStreamDefaultControllerError(controller, error) {
      const stream = controller[controlledReadableStream_];
      if (stream[shared.state_] !== "readable") {
        return;
      }
      q.resetQueue(controller);
      readableStreamDefaultControllerClearAlgorithms(controller);
      readableStreamError(stream, error);
    }
    exports_76(
      "readableStreamDefaultControllerError",
      readableStreamDefaultControllerError
    );
    function readableStreamDefaultControllerCallPullIfNeeded(controller) {
      if (!readableStreamDefaultControllerShouldCallPull(controller)) {
        return;
      }
      if (controller[pulling_]) {
        controller[pullAgain_] = true;
        return;
      }
      if (controller[pullAgain_]) {
        throw new RangeError("Stream controller is in an invalid state.");
      }
      controller[pulling_] = true;
      controller[pullAlgorithm_](controller).then(
        (_) => {
          controller[pulling_] = false;
          if (controller[pullAgain_]) {
            controller[pullAgain_] = false;
            readableStreamDefaultControllerCallPullIfNeeded(controller);
          }
        },
        (error) => {
          readableStreamDefaultControllerError(controller, error);
        }
      );
    }
    exports_76(
      "readableStreamDefaultControllerCallPullIfNeeded",
      readableStreamDefaultControllerCallPullIfNeeded
    );
    function readableStreamDefaultControllerShouldCallPull(controller) {
      const stream = controller[controlledReadableStream_];
      if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
        return false;
      }
      if (controller[started_] === false) {
        return false;
      }
      if (
        isReadableStreamLocked(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        return true;
      }
      const desiredSize = readableStreamDefaultControllerGetDesiredSize(
        controller
      );
      if (desiredSize === null) {
        throw new RangeError("Stream is in an invalid state.");
      }
      return desiredSize > 0;
    }
    exports_76(
      "readableStreamDefaultControllerShouldCallPull",
      readableStreamDefaultControllerShouldCallPull
    );
    function readableStreamDefaultControllerClearAlgorithms(controller) {
      controller[pullAlgorithm_] = undefined;
      controller[cancelAlgorithm_] = undefined;
      controller[strategySizeAlgorithm_] = undefined;
    }
    exports_76(
      "readableStreamDefaultControllerClearAlgorithms",
      readableStreamDefaultControllerClearAlgorithms
    );
    // ---- BYOBController
    function setUpReadableByteStreamController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      autoAllocateChunkSize
    ) {
      // Assert: stream.[[readableStreamController]] is undefined.
      if (stream[readableStreamController_] !== undefined) {
        throw new TypeError("Cannot reuse streams");
      }
      if (autoAllocateChunkSize !== undefined) {
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      // Set controller.[[controlledReadableByteStream]] to stream.
      controller[controlledReadableByteStream_] = stream;
      // Set controller.[[pullAgain]] and controller.[[pulling]] to false.
      controller[pullAgain_] = false;
      controller[pulling_] = false;
      readableByteStreamControllerClearPendingPullIntos(controller);
      q.resetQueue(controller);
      controller[closeRequested_] = false;
      controller[started_] = false;
      controller[strategyHWM_] = shared.validateAndNormalizeHighWaterMark(
        highWaterMark
      );
      controller[pullAlgorithm_] = pullAlgorithm;
      controller[cancelAlgorithm_] = cancelAlgorithm;
      controller[autoAllocateChunkSize_] = autoAllocateChunkSize;
      controller[pendingPullIntos_] = [];
      stream[readableStreamController_] = controller;
      // Let startResult be the result of performing startAlgorithm.
      const startResult = startAlgorithm();
      Promise.resolve(startResult).then(
        (_) => {
          controller[started_] = true;
          // Assert: controller.[[pulling]] is false.
          // Assert: controller.[[pullAgain]] is false.
          readableByteStreamControllerCallPullIfNeeded(controller);
        },
        (error) => {
          readableByteStreamControllerError(controller, error);
        }
      );
    }
    exports_76(
      "setUpReadableByteStreamController",
      setUpReadableByteStreamController
    );
    function isReadableStreamBYOBRequest(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return associatedReadableByteStreamController_ in value;
    }
    exports_76("isReadableStreamBYOBRequest", isReadableStreamBYOBRequest);
    function isReadableByteStreamController(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return controlledReadableByteStream_ in value;
    }
    exports_76(
      "isReadableByteStreamController",
      isReadableByteStreamController
    );
    function readableByteStreamControllerCallPullIfNeeded(controller) {
      if (!readableByteStreamControllerShouldCallPull(controller)) {
        return;
      }
      if (controller[pulling_]) {
        controller[pullAgain_] = true;
        return;
      }
      // Assert: controller.[[pullAgain]] is false.
      controller[pulling_] = true;
      controller[pullAlgorithm_](controller).then(
        (_) => {
          controller[pulling_] = false;
          if (controller[pullAgain_]) {
            controller[pullAgain_] = false;
            readableByteStreamControllerCallPullIfNeeded(controller);
          }
        },
        (error) => {
          readableByteStreamControllerError(controller, error);
        }
      );
    }
    exports_76(
      "readableByteStreamControllerCallPullIfNeeded",
      readableByteStreamControllerCallPullIfNeeded
    );
    function readableByteStreamControllerClearAlgorithms(controller) {
      controller[pullAlgorithm_] = undefined;
      controller[cancelAlgorithm_] = undefined;
    }
    exports_76(
      "readableByteStreamControllerClearAlgorithms",
      readableByteStreamControllerClearAlgorithms
    );
    function readableByteStreamControllerClearPendingPullIntos(controller) {
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      controller[pendingPullIntos_] = [];
    }
    exports_76(
      "readableByteStreamControllerClearPendingPullIntos",
      readableByteStreamControllerClearPendingPullIntos
    );
    function readableByteStreamControllerClose(controller) {
      const stream = controller[controlledReadableByteStream_];
      // Assert: controller.[[closeRequested]] is false.
      // Assert: stream.[[state]] is "readable".
      if (controller[q.queueTotalSize_] > 0) {
        controller[closeRequested_] = true;
        return;
      }
      if (controller[pendingPullIntos_].length > 0) {
        const firstPendingPullInto = controller[pendingPullIntos_][0];
        if (firstPendingPullInto.bytesFilled > 0) {
          const error = new TypeError();
          readableByteStreamControllerError(controller, error);
          throw error;
        }
      }
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamClose(stream);
    }
    exports_76(
      "readableByteStreamControllerClose",
      readableByteStreamControllerClose
    );
    function readableByteStreamControllerCommitPullIntoDescriptor(
      stream,
      pullIntoDescriptor
    ) {
      // Assert: stream.[[state]] is not "errored".
      let done = false;
      if (stream[shared.state_] === "closed") {
        // Assert: pullIntoDescriptor.[[bytesFilled]] is 0.
        done = true;
      }
      const filledView = readableByteStreamControllerConvertPullIntoDescriptor(
        pullIntoDescriptor
      );
      if (pullIntoDescriptor.readerType === "default") {
        readableStreamFulfillReadRequest(stream, filledView, done);
      } else {
        // Assert: pullIntoDescriptor.[[readerType]] is "byob".
        readableStreamFulfillReadIntoRequest(stream, filledView, done);
      }
    }
    exports_76(
      "readableByteStreamControllerCommitPullIntoDescriptor",
      readableByteStreamControllerCommitPullIntoDescriptor
    );
    function readableByteStreamControllerConvertPullIntoDescriptor(
      pullIntoDescriptor
    ) {
      const { bytesFilled, elementSize } = pullIntoDescriptor;
      // Assert: bytesFilled <= pullIntoDescriptor.byteLength
      // Assert: bytesFilled mod elementSize is 0
      return new pullIntoDescriptor.ctor(
        pullIntoDescriptor.buffer,
        pullIntoDescriptor.byteOffset,
        bytesFilled / elementSize
      );
    }
    exports_76(
      "readableByteStreamControllerConvertPullIntoDescriptor",
      readableByteStreamControllerConvertPullIntoDescriptor
    );
    function readableByteStreamControllerEnqueue(controller, chunk) {
      const stream = controller[controlledReadableByteStream_];
      // Assert: controller.[[closeRequested]] is false.
      // Assert: stream.[[state]] is "readable".
      const { buffer, byteOffset, byteLength } = chunk;
      const transferredBuffer = shared.transferArrayBuffer(buffer);
      if (readableStreamHasDefaultReader(stream)) {
        if (readableStreamGetNumReadRequests(stream) === 0) {
          readableByteStreamControllerEnqueueChunkToQueue(
            controller,
            transferredBuffer,
            byteOffset,
            byteLength
          );
        } else {
          // Assert: controller.[[queue]] is empty.
          const transferredView = new Uint8Array(
            transferredBuffer,
            byteOffset,
            byteLength
          );
          readableStreamFulfillReadRequest(stream, transferredView, false);
        }
      } else if (readableStreamHasBYOBReader(stream)) {
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          transferredBuffer,
          byteOffset,
          byteLength
        );
        readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
          controller
        );
      } else {
        // Assert: !IsReadableStreamLocked(stream) is false.
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          transferredBuffer,
          byteOffset,
          byteLength
        );
      }
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
    exports_76(
      "readableByteStreamControllerEnqueue",
      readableByteStreamControllerEnqueue
    );
    function readableByteStreamControllerEnqueueChunkToQueue(
      controller,
      buffer,
      byteOffset,
      byteLength
    ) {
      controller[q.queue_].push({ buffer, byteOffset, byteLength });
      controller[q.queueTotalSize_] += byteLength;
    }
    exports_76(
      "readableByteStreamControllerEnqueueChunkToQueue",
      readableByteStreamControllerEnqueueChunkToQueue
    );
    function readableByteStreamControllerError(controller, error) {
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] !== "readable") {
        return;
      }
      readableByteStreamControllerClearPendingPullIntos(controller);
      q.resetQueue(controller);
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamError(stream, error);
    }
    exports_76(
      "readableByteStreamControllerError",
      readableByteStreamControllerError
    );
    function readableByteStreamControllerFillHeadPullIntoDescriptor(
      controller,
      size,
      pullIntoDescriptor
    ) {
      // Assert: either controller.[[pendingPullIntos]] is empty, or the first element of controller.[[pendingPullIntos]] is pullIntoDescriptor.
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      pullIntoDescriptor.bytesFilled += size;
    }
    exports_76(
      "readableByteStreamControllerFillHeadPullIntoDescriptor",
      readableByteStreamControllerFillHeadPullIntoDescriptor
    );
    function readableByteStreamControllerFillPullIntoDescriptorFromQueue(
      controller,
      pullIntoDescriptor
    ) {
      const elementSize = pullIntoDescriptor.elementSize;
      const currentAlignedBytes =
        pullIntoDescriptor.bytesFilled -
        (pullIntoDescriptor.bytesFilled % elementSize);
      const maxBytesToCopy = Math.min(
        controller[q.queueTotalSize_],
        pullIntoDescriptor.byteLength - pullIntoDescriptor.bytesFilled
      );
      const maxBytesFilled = pullIntoDescriptor.bytesFilled + maxBytesToCopy;
      const maxAlignedBytes = maxBytesFilled - (maxBytesFilled % elementSize);
      let totalBytesToCopyRemaining = maxBytesToCopy;
      let ready = false;
      if (maxAlignedBytes > currentAlignedBytes) {
        totalBytesToCopyRemaining =
          maxAlignedBytes - pullIntoDescriptor.bytesFilled;
        ready = true;
      }
      const queue = controller[q.queue_];
      while (totalBytesToCopyRemaining > 0) {
        const headOfQueue = queue.front();
        const bytesToCopy = Math.min(
          totalBytesToCopyRemaining,
          headOfQueue.byteLength
        );
        const destStart =
          pullIntoDescriptor.byteOffset + pullIntoDescriptor.bytesFilled;
        shared.copyDataBlockBytes(
          pullIntoDescriptor.buffer,
          destStart,
          headOfQueue.buffer,
          headOfQueue.byteOffset,
          bytesToCopy
        );
        if (headOfQueue.byteLength === bytesToCopy) {
          queue.shift();
        } else {
          headOfQueue.byteOffset += bytesToCopy;
          headOfQueue.byteLength -= bytesToCopy;
        }
        controller[q.queueTotalSize_] -= bytesToCopy;
        readableByteStreamControllerFillHeadPullIntoDescriptor(
          controller,
          bytesToCopy,
          pullIntoDescriptor
        );
        totalBytesToCopyRemaining -= bytesToCopy;
      }
      if (!ready) {
        // Assert: controller[queueTotalSize_] === 0
        // Assert: pullIntoDescriptor.bytesFilled > 0
        // Assert: pullIntoDescriptor.bytesFilled < pullIntoDescriptor.elementSize
      }
      return ready;
    }
    exports_76(
      "readableByteStreamControllerFillPullIntoDescriptorFromQueue",
      readableByteStreamControllerFillPullIntoDescriptorFromQueue
    );
    function readableByteStreamControllerGetDesiredSize(controller) {
      const stream = controller[controlledReadableByteStream_];
      const state = stream[shared.state_];
      if (state === "errored") {
        return null;
      }
      if (state === "closed") {
        return 0;
      }
      return controller[strategyHWM_] - controller[q.queueTotalSize_];
    }
    exports_76(
      "readableByteStreamControllerGetDesiredSize",
      readableByteStreamControllerGetDesiredSize
    );
    function readableByteStreamControllerHandleQueueDrain(controller) {
      // Assert: controller.[[controlledReadableByteStream]].[[state]] is "readable".
      if (controller[q.queueTotalSize_] === 0 && controller[closeRequested_]) {
        readableByteStreamControllerClearAlgorithms(controller);
        readableStreamClose(controller[controlledReadableByteStream_]);
      } else {
        readableByteStreamControllerCallPullIfNeeded(controller);
      }
    }
    exports_76(
      "readableByteStreamControllerHandleQueueDrain",
      readableByteStreamControllerHandleQueueDrain
    );
    function readableByteStreamControllerInvalidateBYOBRequest(controller) {
      const byobRequest = controller[byobRequest_];
      if (byobRequest === undefined) {
        return;
      }
      byobRequest[associatedReadableByteStreamController_] = undefined;
      byobRequest[view_] = undefined;
      controller[byobRequest_] = undefined;
    }
    exports_76(
      "readableByteStreamControllerInvalidateBYOBRequest",
      readableByteStreamControllerInvalidateBYOBRequest
    );
    function readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
      controller
    ) {
      // Assert: controller.[[closeRequested]] is false.
      const pendingPullIntos = controller[pendingPullIntos_];
      while (pendingPullIntos.length > 0) {
        if (controller[q.queueTotalSize_] === 0) {
          return;
        }
        const pullIntoDescriptor = pendingPullIntos[0];
        if (
          readableByteStreamControllerFillPullIntoDescriptorFromQueue(
            controller,
            pullIntoDescriptor
          )
        ) {
          readableByteStreamControllerShiftPendingPullInto(controller);
          readableByteStreamControllerCommitPullIntoDescriptor(
            controller[controlledReadableByteStream_],
            pullIntoDescriptor
          );
        }
      }
    }
    exports_76(
      "readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue",
      readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue
    );
    function readableByteStreamControllerPullInto(
      controller,
      view,
      forAuthorCode
    ) {
      const stream = controller[controlledReadableByteStream_];
      const elementSize = view.BYTES_PER_ELEMENT || 1; // DataView exposes this in Webkit as 1, is not present in FF or Blink
      const ctor = view.constructor; // the typecast here is just for TS typing, it does not influence buffer creation
      const byteOffset = view.byteOffset;
      const byteLength = view.byteLength;
      const buffer = shared.transferArrayBuffer(view.buffer);
      const pullIntoDescriptor = {
        buffer,
        byteOffset,
        byteLength,
        bytesFilled: 0,
        elementSize,
        ctor,
        readerType: "byob",
      };
      if (controller[pendingPullIntos_].length > 0) {
        controller[pendingPullIntos_].push(pullIntoDescriptor);
        return readableStreamAddReadIntoRequest(stream, forAuthorCode);
      }
      if (stream[shared.state_] === "closed") {
        const emptyView = new ctor(
          pullIntoDescriptor.buffer,
          pullIntoDescriptor.byteOffset,
          0
        );
        return Promise.resolve(
          readableStreamCreateReadResult(emptyView, true, forAuthorCode)
        );
      }
      if (controller[q.queueTotalSize_] > 0) {
        if (
          readableByteStreamControllerFillPullIntoDescriptorFromQueue(
            controller,
            pullIntoDescriptor
          )
        ) {
          const filledView = readableByteStreamControllerConvertPullIntoDescriptor(
            pullIntoDescriptor
          );
          readableByteStreamControllerHandleQueueDrain(controller);
          return Promise.resolve(
            readableStreamCreateReadResult(filledView, false, forAuthorCode)
          );
        }
        if (controller[closeRequested_]) {
          const error = new TypeError();
          readableByteStreamControllerError(controller, error);
          return Promise.reject(error);
        }
      }
      controller[pendingPullIntos_].push(pullIntoDescriptor);
      const promise = readableStreamAddReadIntoRequest(stream, forAuthorCode);
      readableByteStreamControllerCallPullIfNeeded(controller);
      return promise;
    }
    exports_76(
      "readableByteStreamControllerPullInto",
      readableByteStreamControllerPullInto
    );
    function readableByteStreamControllerRespond(controller, bytesWritten) {
      bytesWritten = Number(bytesWritten);
      if (!shared.isFiniteNonNegativeNumber(bytesWritten)) {
        throw new RangeError(
          "bytesWritten must be a finite, non-negative number"
        );
      }
      // Assert: controller.[[pendingPullIntos]] is not empty.
      readableByteStreamControllerRespondInternal(controller, bytesWritten);
    }
    exports_76(
      "readableByteStreamControllerRespond",
      readableByteStreamControllerRespond
    );
    function readableByteStreamControllerRespondInClosedState(
      controller,
      firstDescriptor
    ) {
      firstDescriptor.buffer = shared.transferArrayBuffer(
        firstDescriptor.buffer
      );
      // Assert: firstDescriptor.[[bytesFilled]] is 0.
      const stream = controller[controlledReadableByteStream_];
      if (readableStreamHasBYOBReader(stream)) {
        while (readableStreamGetNumReadIntoRequests(stream) > 0) {
          const pullIntoDescriptor = readableByteStreamControllerShiftPendingPullInto(
            controller
          );
          readableByteStreamControllerCommitPullIntoDescriptor(
            stream,
            pullIntoDescriptor
          );
        }
      }
    }
    exports_76(
      "readableByteStreamControllerRespondInClosedState",
      readableByteStreamControllerRespondInClosedState
    );
    function readableByteStreamControllerRespondInReadableState(
      controller,
      bytesWritten,
      pullIntoDescriptor
    ) {
      if (
        pullIntoDescriptor.bytesFilled + bytesWritten >
        pullIntoDescriptor.byteLength
      ) {
        throw new RangeError();
      }
      readableByteStreamControllerFillHeadPullIntoDescriptor(
        controller,
        bytesWritten,
        pullIntoDescriptor
      );
      if (pullIntoDescriptor.bytesFilled < pullIntoDescriptor.elementSize) {
        return;
      }
      readableByteStreamControllerShiftPendingPullInto(controller);
      const remainderSize =
        pullIntoDescriptor.bytesFilled % pullIntoDescriptor.elementSize;
      if (remainderSize > 0) {
        const end =
          pullIntoDescriptor.byteOffset + pullIntoDescriptor.bytesFilled;
        const remainder = shared.cloneArrayBuffer(
          pullIntoDescriptor.buffer,
          end - remainderSize,
          remainderSize,
          ArrayBuffer
        );
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          remainder,
          0,
          remainder.byteLength
        );
      }
      pullIntoDescriptor.buffer = shared.transferArrayBuffer(
        pullIntoDescriptor.buffer
      );
      pullIntoDescriptor.bytesFilled =
        pullIntoDescriptor.bytesFilled - remainderSize;
      readableByteStreamControllerCommitPullIntoDescriptor(
        controller[controlledReadableByteStream_],
        pullIntoDescriptor
      );
      readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
        controller
      );
    }
    exports_76(
      "readableByteStreamControllerRespondInReadableState",
      readableByteStreamControllerRespondInReadableState
    );
    function readableByteStreamControllerRespondInternal(
      controller,
      bytesWritten
    ) {
      const firstDescriptor = controller[pendingPullIntos_][0];
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] === "closed") {
        if (bytesWritten !== 0) {
          throw new TypeError();
        }
        readableByteStreamControllerRespondInClosedState(
          controller,
          firstDescriptor
        );
      } else {
        // Assert: stream.[[state]] is "readable".
        readableByteStreamControllerRespondInReadableState(
          controller,
          bytesWritten,
          firstDescriptor
        );
      }
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
    exports_76(
      "readableByteStreamControllerRespondInternal",
      readableByteStreamControllerRespondInternal
    );
    function readableByteStreamControllerRespondWithNewView(controller, view) {
      // Assert: controller.[[pendingPullIntos]] is not empty.
      const firstDescriptor = controller[pendingPullIntos_][0];
      if (
        firstDescriptor.byteOffset + firstDescriptor.bytesFilled !==
        view.byteOffset
      ) {
        throw new RangeError();
      }
      if (firstDescriptor.byteLength !== view.byteLength) {
        throw new RangeError();
      }
      firstDescriptor.buffer = view.buffer;
      readableByteStreamControllerRespondInternal(controller, view.byteLength);
    }
    exports_76(
      "readableByteStreamControllerRespondWithNewView",
      readableByteStreamControllerRespondWithNewView
    );
    function readableByteStreamControllerShiftPendingPullInto(controller) {
      const descriptor = controller[pendingPullIntos_].shift();
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      return descriptor;
    }
    exports_76(
      "readableByteStreamControllerShiftPendingPullInto",
      readableByteStreamControllerShiftPendingPullInto
    );
    function readableByteStreamControllerShouldCallPull(controller) {
      // Let stream be controller.[[controlledReadableByteStream]].
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] !== "readable") {
        return false;
      }
      if (controller[closeRequested_]) {
        return false;
      }
      if (!controller[started_]) {
        return false;
      }
      if (
        readableStreamHasDefaultReader(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        return true;
      }
      if (
        readableStreamHasBYOBReader(stream) &&
        readableStreamGetNumReadIntoRequests(stream) > 0
      ) {
        return true;
      }
      const desiredSize = readableByteStreamControllerGetDesiredSize(
        controller
      );
      // Assert: desiredSize is not null.
      return desiredSize > 0;
    }
    exports_76(
      "readableByteStreamControllerShouldCallPull",
      readableByteStreamControllerShouldCallPull
    );
    function setUpReadableStreamBYOBRequest(request, controller, view) {
      if (!isReadableByteStreamController(controller)) {
        throw new TypeError();
      }
      if (!ArrayBuffer.isView(view)) {
        throw new TypeError();
      }
      // Assert: !IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is false.
      request[associatedReadableByteStreamController_] = controller;
      request[view_] = view;
    }
    exports_76(
      "setUpReadableStreamBYOBRequest",
      setUpReadableStreamBYOBRequest
    );
    return {
      setters: [
        function (shared_1) {
          shared = shared_1;
        },
        function (q_1) {
          q = q_1;
        },
      ],
      execute: function () {
        // ReadableStreamDefaultController
        exports_76(
          "controlledReadableStream_",
          (controlledReadableStream_ = Symbol("controlledReadableStream_"))
        );
        exports_76(
          "pullAlgorithm_",
          (pullAlgorithm_ = Symbol("pullAlgorithm_"))
        );
        exports_76(
          "cancelAlgorithm_",
          (cancelAlgorithm_ = Symbol("cancelAlgorithm_"))
        );
        exports_76(
          "strategySizeAlgorithm_",
          (strategySizeAlgorithm_ = Symbol("strategySizeAlgorithm_"))
        );
        exports_76("strategyHWM_", (strategyHWM_ = Symbol("strategyHWM_")));
        exports_76("started_", (started_ = Symbol("started_")));
        exports_76(
          "closeRequested_",
          (closeRequested_ = Symbol("closeRequested_"))
        );
        exports_76("pullAgain_", (pullAgain_ = Symbol("pullAgain_")));
        exports_76("pulling_", (pulling_ = Symbol("pulling_")));
        exports_76("cancelSteps_", (cancelSteps_ = Symbol("cancelSteps_")));
        exports_76("pullSteps_", (pullSteps_ = Symbol("pullSteps_")));
        // ReadableByteStreamController
        exports_76(
          "autoAllocateChunkSize_",
          (autoAllocateChunkSize_ = Symbol("autoAllocateChunkSize_"))
        );
        exports_76("byobRequest_", (byobRequest_ = Symbol("byobRequest_")));
        exports_76(
          "controlledReadableByteStream_",
          (controlledReadableByteStream_ = Symbol(
            "controlledReadableByteStream_"
          ))
        );
        exports_76(
          "pendingPullIntos_",
          (pendingPullIntos_ = Symbol("pendingPullIntos_"))
        );
        // ReadableStreamDefaultReader
        exports_76(
          "closedPromise_",
          (closedPromise_ = Symbol("closedPromise_"))
        );
        exports_76(
          "ownerReadableStream_",
          (ownerReadableStream_ = Symbol("ownerReadableStream_"))
        );
        exports_76("readRequests_", (readRequests_ = Symbol("readRequests_")));
        exports_76(
          "readIntoRequests_",
          (readIntoRequests_ = Symbol("readIntoRequests_"))
        );
        // ReadableStreamBYOBRequest
        exports_76(
          "associatedReadableByteStreamController_",
          (associatedReadableByteStreamController_ = Symbol(
            "associatedReadableByteStreamController_"
          ))
        );
        exports_76("view_", (view_ = Symbol("view_")));
        // ReadableStreamBYOBReader
        // ReadableStream
        exports_76("reader_", (reader_ = Symbol("reader_")));
        exports_76(
          "readableStreamController_",
          (readableStreamController_ = Symbol("readableStreamController_"))
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-default-controller.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
  ],
  function (exports_77, context_77) {
    "use strict";
    let rs, shared, q, ReadableStreamDefaultController;
    const __moduleName = context_77 && context_77.id;
    function setUpReadableStreamDefaultControllerFromUnderlyingSource(
      stream,
      underlyingSource,
      highWaterMark,
      sizeAlgorithm
    ) {
      // Assert: underlyingSource is not undefined.
      const controller = Object.create(
        ReadableStreamDefaultController.prototype
      );
      const startAlgorithm = () => {
        return shared.invokeOrNoop(underlyingSource, "start", [controller]);
      };
      const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingSource,
        "pull",
        [controller]
      );
      const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingSource,
        "cancel",
        []
      );
      rs.setUpReadableStreamDefaultController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        sizeAlgorithm
      );
    }
    exports_77(
      "setUpReadableStreamDefaultControllerFromUnderlyingSource",
      setUpReadableStreamDefaultControllerFromUnderlyingSource
    );
    return {
      setters: [
        function (rs_1) {
          rs = rs_1;
        },
        function (shared_2) {
          shared = shared_2;
        },
        function (q_2) {
          q = q_2;
        },
      ],
      execute: function () {
        ReadableStreamDefaultController = class ReadableStreamDefaultController {
          constructor() {
            throw new TypeError();
          }
          get desiredSize() {
            return rs.readableStreamDefaultControllerGetDesiredSize(this);
          }
          close() {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
              throw new TypeError(
                "Cannot close, the stream is already closing or not readable"
              );
            }
            rs.readableStreamDefaultControllerClose(this);
          }
          enqueue(chunk) {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
              throw new TypeError(
                "Cannot enqueue, the stream is closing or not readable"
              );
            }
            rs.readableStreamDefaultControllerEnqueue(this, chunk);
          }
          error(e) {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            rs.readableStreamDefaultControllerError(this, e);
          }
          [(rs.cancelAlgorithm_,
          rs.closeRequested_,
          rs.controlledReadableStream_,
          rs.pullAgain_,
          rs.pullAlgorithm_,
          rs.pulling_,
          rs.strategyHWM_,
          rs.strategySizeAlgorithm_,
          rs.started_,
          q.queue_,
          q.queueTotalSize_,
          rs.cancelSteps_)](reason) {
            q.resetQueue(this);
            const result = this[rs.cancelAlgorithm_](reason);
            rs.readableStreamDefaultControllerClearAlgorithms(this);
            return result;
          }
          [rs.pullSteps_](forAuthorCode) {
            const stream = this[rs.controlledReadableStream_];
            if (this[q.queue_].length > 0) {
              const chunk = q.dequeueValue(this);
              if (this[rs.closeRequested_] && this[q.queue_].length === 0) {
                rs.readableStreamDefaultControllerClearAlgorithms(this);
                rs.readableStreamClose(stream);
              } else {
                rs.readableStreamDefaultControllerCallPullIfNeeded(this);
              }
              return Promise.resolve(
                rs.readableStreamCreateReadResult(chunk, false, forAuthorCode)
              );
            }
            const pendingPromise = rs.readableStreamAddReadRequest(
              stream,
              forAuthorCode
            );
            rs.readableStreamDefaultControllerCallPullIfNeeded(this);
            return pendingPromise;
          }
        };
        exports_77(
          "ReadableStreamDefaultController",
          ReadableStreamDefaultController
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-default-reader.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_78, context_78) {
    "use strict";
    let rs, ReadableStreamDefaultReader;
    const __moduleName = context_78 && context_78.id;
    return {
      setters: [
        function (rs_2) {
          rs = rs_2;
        },
      ],
      execute: function () {
        ReadableStreamDefaultReader = class ReadableStreamDefaultReader {
          constructor(stream) {
            if (!rs.isReadableStream(stream)) {
              throw new TypeError();
            }
            if (rs.isReadableStreamLocked(stream)) {
              throw new TypeError("The stream is locked.");
            }
            rs.readableStreamReaderGenericInitialize(this, stream);
            this[rs.readRequests_] = [];
          }
          get closed() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            return this[rs.closedPromise_].promise;
          }
          cancel(reason) {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            const stream = this[rs.ownerReadableStream_];
            if (stream === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamCancel(stream, reason);
          }
          read() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamDefaultReaderRead(this, true);
          }
          releaseLock() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              throw new TypeError();
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return;
            }
            if (this[rs.readRequests_].length !== 0) {
              throw new TypeError(
                "Cannot release a stream with pending read requests"
              );
            }
            rs.readableStreamReaderGenericRelease(this);
          }
        };
        exports_78("ReadableStreamDefaultReader", ReadableStreamDefaultReader);
        rs.closedPromise_, rs.ownerReadableStream_, rs.readRequests_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-byob-request.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_79, context_79) {
    "use strict";
    let rs, ReadableStreamBYOBRequest;
    const __moduleName = context_79 && context_79.id;
    return {
      setters: [
        function (rs_3) {
          rs = rs_3;
        },
      ],
      execute: function () {
        ReadableStreamBYOBRequest = class ReadableStreamBYOBRequest {
          constructor() {
            throw new TypeError();
          }
          get view() {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            return this[rs.view_];
          }
          respond(bytesWritten) {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            if (
              this[rs.associatedReadableByteStreamController_] === undefined
            ) {
              throw new TypeError();
            }
            // If! IsDetachedBuffer(this.[[view]].[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerRespond(
              this[rs.associatedReadableByteStreamController_],
              bytesWritten
            );
          }
          respondWithNewView(view) {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            if (
              this[rs.associatedReadableByteStreamController_] === undefined
            ) {
              throw new TypeError();
            }
            if (!ArrayBuffer.isView(view)) {
              throw new TypeError("view parameter must be a TypedArray");
            }
            // If! IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerRespondWithNewView(
              this[rs.associatedReadableByteStreamController_],
              view
            );
          }
        };
        exports_79("ReadableStreamBYOBRequest", ReadableStreamBYOBRequest);
        rs.associatedReadableByteStreamController_, rs.view_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-byte-stream-controller.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/readable-stream-byob-request.ts",
  ],
  function (exports_80, context_80) {
    "use strict";
    let rs,
      q,
      shared,
      readable_stream_byob_request_ts_1,
      ReadableByteStreamController;
    const __moduleName = context_80 && context_80.id;
    function setUpReadableByteStreamControllerFromUnderlyingSource(
      stream,
      underlyingByteSource,
      highWaterMark
    ) {
      // Assert: underlyingByteSource is not undefined.
      const controller = Object.create(ReadableByteStreamController.prototype);
      const startAlgorithm = () => {
        return shared.invokeOrNoop(underlyingByteSource, "start", [controller]);
      };
      const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingByteSource,
        "pull",
        [controller]
      );
      const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingByteSource,
        "cancel",
        []
      );
      let autoAllocateChunkSize = underlyingByteSource.autoAllocateChunkSize;
      if (autoAllocateChunkSize !== undefined) {
        autoAllocateChunkSize = Number(autoAllocateChunkSize);
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      rs.setUpReadableByteStreamController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        autoAllocateChunkSize
      );
    }
    exports_80(
      "setUpReadableByteStreamControllerFromUnderlyingSource",
      setUpReadableByteStreamControllerFromUnderlyingSource
    );
    return {
      setters: [
        function (rs_4) {
          rs = rs_4;
        },
        function (q_3) {
          q = q_3;
        },
        function (shared_3) {
          shared = shared_3;
        },
        function (readable_stream_byob_request_ts_1_1) {
          readable_stream_byob_request_ts_1 = readable_stream_byob_request_ts_1_1;
        },
      ],
      execute: function () {
        ReadableByteStreamController = class ReadableByteStreamController {
          constructor() {
            throw new TypeError();
          }
          get byobRequest() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (
              this[rs.byobRequest_] === undefined &&
              this[rs.pendingPullIntos_].length > 0
            ) {
              const firstDescriptor = this[rs.pendingPullIntos_][0];
              const view = new Uint8Array(
                firstDescriptor.buffer,
                firstDescriptor.byteOffset + firstDescriptor.bytesFilled,
                firstDescriptor.byteLength - firstDescriptor.bytesFilled
              );
              const byobRequest = Object.create(
                readable_stream_byob_request_ts_1.ReadableStreamBYOBRequest
                  .prototype
              );
              rs.setUpReadableStreamBYOBRequest(byobRequest, this, view);
              this[rs.byobRequest_] = byobRequest;
            }
            return this[rs.byobRequest_];
          }
          get desiredSize() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            return rs.readableByteStreamControllerGetDesiredSize(this);
          }
          close() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (this[rs.closeRequested_]) {
              throw new TypeError("Stream is already closing");
            }
            if (
              this[rs.controlledReadableByteStream_][shared.state_] !==
              "readable"
            ) {
              throw new TypeError("Stream is closed or errored");
            }
            rs.readableByteStreamControllerClose(this);
          }
          enqueue(chunk) {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (this[rs.closeRequested_]) {
              throw new TypeError("Stream is already closing");
            }
            if (
              this[rs.controlledReadableByteStream_][shared.state_] !==
              "readable"
            ) {
              throw new TypeError("Stream is closed or errored");
            }
            if (!ArrayBuffer.isView(chunk)) {
              throw new TypeError("chunk must be a valid ArrayBufferView");
            }
            // If ! IsDetachedBuffer(chunk.[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerEnqueue(this, chunk);
          }
          error(error) {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            rs.readableByteStreamControllerError(this, error);
          }
          [(rs.autoAllocateChunkSize_,
          rs.byobRequest_,
          rs.cancelAlgorithm_,
          rs.closeRequested_,
          rs.controlledReadableByteStream_,
          rs.pullAgain_,
          rs.pullAlgorithm_,
          rs.pulling_,
          rs.pendingPullIntos_,
          rs.started_,
          rs.strategyHWM_,
          q.queue_,
          q.queueTotalSize_,
          rs.cancelSteps_)](reason) {
            if (this[rs.pendingPullIntos_].length > 0) {
              const firstDescriptor = this[rs.pendingPullIntos_][0];
              firstDescriptor.bytesFilled = 0;
            }
            q.resetQueue(this);
            const result = this[rs.cancelAlgorithm_](reason);
            rs.readableByteStreamControllerClearAlgorithms(this);
            return result;
          }
          [rs.pullSteps_](forAuthorCode) {
            const stream = this[rs.controlledReadableByteStream_];
            // Assert: ! ReadableStreamHasDefaultReader(stream) is true.
            if (this[q.queueTotalSize_] > 0) {
              // Assert: ! ReadableStreamGetNumReadRequests(stream) is 0.
              const entry = this[q.queue_].shift();
              this[q.queueTotalSize_] -= entry.byteLength;
              rs.readableByteStreamControllerHandleQueueDrain(this);
              const view = new Uint8Array(
                entry.buffer,
                entry.byteOffset,
                entry.byteLength
              );
              return Promise.resolve(
                rs.readableStreamCreateReadResult(view, false, forAuthorCode)
              );
            }
            const autoAllocateChunkSize = this[rs.autoAllocateChunkSize_];
            if (autoAllocateChunkSize !== undefined) {
              let buffer;
              try {
                buffer = new ArrayBuffer(autoAllocateChunkSize);
              } catch (error) {
                return Promise.reject(error);
              }
              const pullIntoDescriptor = {
                buffer,
                byteOffset: 0,
                byteLength: autoAllocateChunkSize,
                bytesFilled: 0,
                elementSize: 1,
                ctor: Uint8Array,
                readerType: "default",
              };
              this[rs.pendingPullIntos_].push(pullIntoDescriptor);
            }
            const promise = rs.readableStreamAddReadRequest(
              stream,
              forAuthorCode
            );
            rs.readableByteStreamControllerCallPullIfNeeded(this);
            return promise;
          }
        };
        exports_80(
          "ReadableByteStreamController",
          ReadableByteStreamController
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-byob-reader.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_81, context_81) {
    "use strict";
    let rs, SDReadableStreamBYOBReader;
    const __moduleName = context_81 && context_81.id;
    return {
      setters: [
        function (rs_5) {
          rs = rs_5;
        },
      ],
      execute: function () {
        SDReadableStreamBYOBReader = class SDReadableStreamBYOBReader {
          constructor(stream) {
            if (!rs.isReadableStream(stream)) {
              throw new TypeError();
            }
            if (
              !rs.isReadableByteStreamController(
                stream[rs.readableStreamController_]
              )
            ) {
              throw new TypeError();
            }
            if (rs.isReadableStreamLocked(stream)) {
              throw new TypeError("The stream is locked.");
            }
            rs.readableStreamReaderGenericInitialize(this, stream);
            this[rs.readIntoRequests_] = [];
          }
          get closed() {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            return this[rs.closedPromise_].promise;
          }
          cancel(reason) {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            const stream = this[rs.ownerReadableStream_];
            if (stream === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamCancel(stream, reason);
          }
          read(view) {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            if (!ArrayBuffer.isView(view)) {
              return Promise.reject(
                new TypeError("view argument must be a valid ArrayBufferView")
              );
            }
            // If ! IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is true, return a promise rejected with a TypeError exception.
            if (view.byteLength === 0) {
              return Promise.reject(
                new TypeError("supplied buffer view must be > 0 bytes")
              );
            }
            return rs.readableStreamBYOBReaderRead(this, view, true);
          }
          releaseLock() {
            if (!rs.isReadableStreamBYOBReader(this)) {
              throw new TypeError();
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              throw new TypeError("Reader is not associated with a stream");
            }
            if (this[rs.readIntoRequests_].length > 0) {
              throw new TypeError();
            }
            rs.readableStreamReaderGenericRelease(this);
          }
        };
        exports_81("SDReadableStreamBYOBReader", SDReadableStreamBYOBReader);
        rs.closedPromise_, rs.ownerReadableStream_, rs.readIntoRequests_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/readable-stream-default-controller.ts",
    "$deno$/web/streams/readable-stream-default-reader.ts",
    "$deno$/web/streams/readable-byte-stream-controller.ts",
    "$deno$/web/streams/readable-stream-byob-reader.ts",
  ],
  function (exports_82, context_82) {
    "use strict";
    let rs,
      shared,
      readable_stream_default_controller_ts_1,
      readable_stream_default_reader_ts_1,
      readable_byte_stream_controller_ts_1,
      readable_stream_byob_reader_ts_1,
      SDReadableStream;
    const __moduleName = context_82 && context_82.id;
    function createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm
    ) {
      if (highWaterMark === undefined) {
        highWaterMark = 1;
      }
      if (sizeAlgorithm === undefined) {
        sizeAlgorithm = () => 1;
      }
      // Assert: ! IsNonNegativeNumber(highWaterMark) is true.
      const stream = Object.create(SDReadableStream.prototype);
      rs.initializeReadableStream(stream);
      const controller = Object.create(
        readable_stream_default_controller_ts_1.ReadableStreamDefaultController
          .prototype
      );
      rs.setUpReadableStreamDefaultController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        sizeAlgorithm
      );
      return stream;
    }
    exports_82("createReadableStream", createReadableStream);
    function createReadableByteStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      autoAllocateChunkSize
    ) {
      if (highWaterMark === undefined) {
        highWaterMark = 0;
      }
      // Assert: ! IsNonNegativeNumber(highWaterMark) is true.
      if (autoAllocateChunkSize !== undefined) {
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      const stream = Object.create(SDReadableStream.prototype);
      rs.initializeReadableStream(stream);
      const controller = Object.create(
        readable_byte_stream_controller_ts_1.ReadableByteStreamController
          .prototype
      );
      rs.setUpReadableByteStreamController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        autoAllocateChunkSize
      );
      return stream;
    }
    exports_82("createReadableByteStream", createReadableByteStream);
    function readableStreamTee(stream, cloneForBranch2) {
      if (!rs.isReadableStream(stream)) {
        throw new TypeError();
      }
      const reader = new readable_stream_default_reader_ts_1.ReadableStreamDefaultReader(
        stream
      );
      let closedOrErrored = false;
      let canceled1 = false;
      let canceled2 = false;
      let reason1;
      let reason2;
      const branch1 = {};
      const branch2 = {};
      let cancelResolve;
      const cancelPromise = new Promise((resolve) => (cancelResolve = resolve));
      const pullAlgorithm = () => {
        return rs
          .readableStreamDefaultReaderRead(reader)
          .then(({ value, done }) => {
            if (done && !closedOrErrored) {
              if (!canceled1) {
                rs.readableStreamDefaultControllerClose(
                  branch1[rs.readableStreamController_]
                );
              }
              if (!canceled2) {
                rs.readableStreamDefaultControllerClose(
                  branch2[rs.readableStreamController_]
                );
              }
              closedOrErrored = true;
            }
            if (closedOrErrored) {
              return;
            }
            const value1 = value;
            let value2 = value;
            if (!canceled1) {
              rs.readableStreamDefaultControllerEnqueue(
                branch1[rs.readableStreamController_],
                value1
              );
            }
            if (!canceled2) {
              if (cloneForBranch2) {
                value2 = shared.cloneValue(value2);
              }
              rs.readableStreamDefaultControllerEnqueue(
                branch2[rs.readableStreamController_],
                value2
              );
            }
          });
      };
      const cancel1Algorithm = (reason) => {
        canceled1 = true;
        reason1 = reason;
        if (canceled2) {
          const cancelResult = rs.readableStreamCancel(stream, [
            reason1,
            reason2,
          ]);
          cancelResolve(cancelResult);
        }
        return cancelPromise;
      };
      const cancel2Algorithm = (reason) => {
        canceled2 = true;
        reason2 = reason;
        if (canceled1) {
          const cancelResult = rs.readableStreamCancel(stream, [
            reason1,
            reason2,
          ]);
          cancelResolve(cancelResult);
        }
        return cancelPromise;
      };
      const startAlgorithm = () => undefined;
      branch1 = createReadableStream(
        startAlgorithm,
        pullAlgorithm,
        cancel1Algorithm
      );
      branch2 = createReadableStream(
        startAlgorithm,
        pullAlgorithm,
        cancel2Algorithm
      );
      reader[rs.closedPromise_].promise.catch((error) => {
        if (!closedOrErrored) {
          rs.readableStreamDefaultControllerError(
            branch1[rs.readableStreamController_],
            error
          );
          rs.readableStreamDefaultControllerError(
            branch2[rs.readableStreamController_],
            error
          );
          closedOrErrored = true;
        }
      });
      return [branch1, branch2];
    }
    exports_82("readableStreamTee", readableStreamTee);
    return {
      setters: [
        function (rs_6) {
          rs = rs_6;
        },
        function (shared_4) {
          shared = shared_4;
        },
        function (readable_stream_default_controller_ts_1_1) {
          readable_stream_default_controller_ts_1 = readable_stream_default_controller_ts_1_1;
        },
        function (readable_stream_default_reader_ts_1_1) {
          readable_stream_default_reader_ts_1 = readable_stream_default_reader_ts_1_1;
        },
        function (readable_byte_stream_controller_ts_1_1) {
          readable_byte_stream_controller_ts_1 = readable_byte_stream_controller_ts_1_1;
        },
        function (readable_stream_byob_reader_ts_1_1) {
          readable_stream_byob_reader_ts_1 = readable_stream_byob_reader_ts_1_1;
        },
      ],
      execute: function () {
        SDReadableStream = class SDReadableStream {
          constructor(underlyingSource = {}, strategy = {}) {
            rs.initializeReadableStream(this);
            const sizeFunc = strategy.size;
            const stratHWM = strategy.highWaterMark;
            const sourceType = underlyingSource.type;
            if (sourceType === undefined) {
              const sizeAlgorithm = shared.makeSizeAlgorithmFromSizeFunction(
                sizeFunc
              );
              const highWaterMark = shared.validateAndNormalizeHighWaterMark(
                stratHWM === undefined ? 1 : stratHWM
              );
              readable_stream_default_controller_ts_1.setUpReadableStreamDefaultControllerFromUnderlyingSource(
                this,
                underlyingSource,
                highWaterMark,
                sizeAlgorithm
              );
            } else if (String(sourceType) === "bytes") {
              if (sizeFunc !== undefined) {
                throw new RangeError(
                  "bytes streams cannot have a strategy with a `size` field"
                );
              }
              const highWaterMark = shared.validateAndNormalizeHighWaterMark(
                stratHWM === undefined ? 0 : stratHWM
              );
              readable_byte_stream_controller_ts_1.setUpReadableByteStreamControllerFromUnderlyingSource(
                this,
                underlyingSource,
                highWaterMark
              );
            } else {
              throw new RangeError(
                "The underlying source's `type` field must be undefined or 'bytes'"
              );
            }
          }
          get locked() {
            return rs.isReadableStreamLocked(this);
          }
          getReader(options) {
            if (!rs.isReadableStream(this)) {
              throw new TypeError();
            }
            if (options === undefined) {
              options = {};
            }
            const { mode } = options;
            if (mode === undefined) {
              return new readable_stream_default_reader_ts_1.ReadableStreamDefaultReader(
                this
              );
            } else if (String(mode) === "byob") {
              return new readable_stream_byob_reader_ts_1.SDReadableStreamBYOBReader(
                this
              );
            }
            throw RangeError("mode option must be undefined or `byob`");
          }
          cancel(reason) {
            if (!rs.isReadableStream(this)) {
              return Promise.reject(new TypeError());
            }
            if (rs.isReadableStreamLocked(this)) {
              return Promise.reject(
                new TypeError("Cannot cancel a locked stream")
              );
            }
            return rs.readableStreamCancel(this, reason);
          }
          tee() {
            return readableStreamTee(this, false);
          }
        };
        exports_82("SDReadableStream", SDReadableStream);
        shared.state_,
          shared.storedError_,
          rs.reader_,
          rs.readableStreamController_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/mod.ts",
  ["$deno$/web/streams/readable-stream.ts"],
  function (exports_83, context_83) {
    "use strict";
    const __moduleName = context_83 && context_83.id;
    return {
      setters: [
        function (readable_stream_ts_1_1) {
          exports_83({
            ReadableStream: readable_stream_ts_1_1["SDReadableStream"],
          });
        },
      ],
      execute: function () {},
    };
  }
);
