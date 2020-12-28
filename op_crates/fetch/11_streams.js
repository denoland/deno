// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// This code closely follows the WHATWG Stream Specification
// See: https://streams.spec.whatwg.org/
//
// There are some parts that are not fully implemented, and there are some
// comments which point to steps of the specification that are not implemented.

((window) => {
  const customInspect = Symbol.for("Deno.customInspect");

  function cloneArrayBuffer(
    srcBuffer,
    srcByteOffset,
    srcLength,
    _cloneConstructor,
  ) {
    // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
    return srcBuffer.slice(
      srcByteOffset,
      srcByteOffset + srcLength,
    );
  }

  const objectCloneMemo = new WeakMap();

  /** Clone a value in a similar way to structured cloning.  It is similar to a
 * StructureDeserialize(StructuredSerialize(...)). */
  function cloneValue(value) {
    switch (typeof value) {
      case "number":
      case "string":
      case "boolean":
      case "undefined":
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
        if (value instanceof SharedArrayBuffer) {
          return value;
        }
        if (value instanceof ArrayBuffer) {
          const cloned = cloneArrayBuffer(
            value,
            0,
            value.byteLength,
            ArrayBuffer,
          );
          objectCloneMemo.set(value, cloned);
          return cloned;
        }
        if (ArrayBuffer.isView(value)) {
          const clonedBuffer = cloneValue(value.buffer);
          // Use DataViewConstructor type purely for type-checking, can be a
          // DataView or TypedArray.  They use the same constructor signature,
          // only DataView has a length in bytes and TypedArrays use a length in
          // terms of elements, so we adjust for that.
          let length;
          if (value instanceof DataView) {
            length = value.byteLength;
          } else {
            length = value.length;
          }
          return new (value.constructor)(
            clonedBuffer,
            value.byteOffset,
            length,
          );
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
        // fallthrough
      default:
        throw new DOMException("Uncloneable value in stream", "DataCloneError");
    }
  }

  function setFunctionName(fn, value) {
    Object.defineProperty(fn, "name", { value, configurable: true });
  }

  class AssertionError extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
    }
  }

  const sym = {
    abortAlgorithm: Symbol("abortAlgorithm"),
    abortSteps: Symbol("abortSteps"),
    asyncIteratorReader: Symbol("asyncIteratorReader"),
    autoAllocateChunkSize: Symbol("autoAllocateChunkSize"),
    backpressure: Symbol("backpressure"),
    backpressureChangePromise: Symbol("backpressureChangePromise"),
    byobRequest: Symbol("byobRequest"),
    cancelAlgorithm: Symbol("cancelAlgorithm"),
    cancelSteps: Symbol("cancelSteps"),
    closeAlgorithm: Symbol("closeAlgorithm"),
    closedPromise: Symbol("closedPromise"),
    closeRequest: Symbol("closeRequest"),
    closeRequested: Symbol("closeRequested"),
    controlledReadableByteStream: Symbol(
      "controlledReadableByteStream",
    ),
    controlledReadableStream: Symbol("controlledReadableStream"),
    controlledTransformStream: Symbol("controlledTransformStream"),
    controlledWritableStream: Symbol("controlledWritableStream"),
    disturbed: Symbol("disturbed"),
    errorSteps: Symbol("errorSteps"),
    flushAlgorithm: Symbol("flushAlgorithm"),
    forAuthorCode: Symbol("forAuthorCode"),
    inFlightWriteRequest: Symbol("inFlightWriteRequest"),
    inFlightCloseRequest: Symbol("inFlightCloseRequest"),
    isFakeDetached: Symbol("isFakeDetached"),
    ownerReadableStream: Symbol("ownerReadableStream"),
    ownerWritableStream: Symbol("ownerWritableStream"),
    pendingAbortRequest: Symbol("pendingAbortRequest"),
    preventCancel: Symbol("preventCancel"),
    pullAgain: Symbol("pullAgain"),
    pullAlgorithm: Symbol("pullAlgorithm"),
    pulling: Symbol("pulling"),
    pullSteps: Symbol("pullSteps"),
    queue: Symbol("queue"),
    queueTotalSize: Symbol("queueTotalSize"),
    readable: Symbol("readable"),
    readableStreamController: Symbol("readableStreamController"),
    reader: Symbol("reader"),
    readRequests: Symbol("readRequests"),
    readyPromise: Symbol("readyPromise"),
    started: Symbol("started"),
    state: Symbol("state"),
    storedError: Symbol("storedError"),
    strategyHWM: Symbol("strategyHWM"),
    strategySizeAlgorithm: Symbol("strategySizeAlgorithm"),
    transformAlgorithm: Symbol("transformAlgorithm"),
    transformStreamController: Symbol("transformStreamController"),
    writableStreamController: Symbol("writableStreamController"),
    writeAlgorithm: Symbol("writeAlgorithm"),
    writable: Symbol("writable"),
    writer: Symbol("writer"),
    writeRequests: Symbol("writeRequests"),
  };
  class ReadableByteStreamController {
    constructor() {
      throw new TypeError(
        "ReadableByteStreamController's constructor cannot be called.",
      );
    }

    get byobRequest() {
      return undefined;
    }

    get desiredSize() {
      if (!isReadableByteStreamController(this)) {
        throw new TypeError("Invalid ReadableByteStreamController.");
      }
      return readableByteStreamControllerGetDesiredSize(this);
    }

    close() {
      if (!isReadableByteStreamController(this)) {
        throw new TypeError("Invalid ReadableByteStreamController.");
      }
      if (this[sym.closeRequested]) {
        throw new TypeError("Closed already requested.");
      }
      if (this[sym.controlledReadableByteStream][sym.state] !== "readable") {
        throw new TypeError(
          "ReadableByteStreamController's stream is not in a readable state.",
        );
      }
      readableByteStreamControllerClose(this);
    }

    enqueue(chunk) {
      if (!isReadableByteStreamController(this)) {
        throw new TypeError("Invalid ReadableByteStreamController.");
      }
      if (this[sym.closeRequested]) {
        throw new TypeError("Closed already requested.");
      }
      if (this[sym.controlledReadableByteStream][sym.state] !== "readable") {
        throw new TypeError(
          "ReadableByteStreamController's stream is not in a readable state.",
        );
      }
      if (!ArrayBuffer.isView(chunk)) {
        throw new TypeError(
          "You can only enqueue array buffer views when using a ReadableByteStreamController",
        );
      }
      if (isDetachedBuffer(chunk.buffer)) {
        throw new TypeError(
          "Cannot enqueue a view onto a detached ArrayBuffer",
        );
      }
      readableByteStreamControllerEnqueue(this, chunk);
    }

    error(error) {
      if (!isReadableByteStreamController(this)) {
        throw new TypeError("Invalid ReadableByteStreamController.");
      }
      readableByteStreamControllerError(this, error);
    }

    [sym.cancelSteps](reason) {
      // 3.11.5.1.1 If this.[[pendingPullIntos]] is not empty,
      resetQueue(this);
      const result = this[sym.cancelAlgorithm](reason);
      readableByteStreamControllerClearAlgorithms(this);
      return result;
    }

    [sym.pullSteps]() {
      const stream = this[sym.controlledReadableByteStream];
      assert(readableStreamHasDefaultReader(stream));
      if (this[sym.queueTotalSize] > 0) {
        assert(readableStreamGetNumReadRequests(stream) === 0);
        const entry = this[sym.queue].shift();
        assert(entry);
        this[sym.queueTotalSize] -= entry.size;
        readableByteStreamControllerHandleQueueDrain(this);
        const view = new Uint8Array(entry.value, entry.offset, entry.size);
        return Promise.resolve(
          readableStreamCreateReadResult(
            view,
            false,
            stream[sym.reader][sym.forAuthorCode],
          ),
        );
      }
      // 3.11.5.2.5 If autoAllocateChunkSize is not undefined,
      const promise = readableStreamAddReadRequest(stream);
      readableByteStreamControllerCallPullIfNeeded(this);
      return promise;
    }

    [customInspect]() {
      return `${this.constructor.name} { byobRequest: ${
        String(this.byobRequest)
      }, desiredSize: ${String(this.desiredSize)} }`;
    }
  }

  class ReadableStreamDefaultController {
    constructor() {
      throw new TypeError(
        "ReadableStreamDefaultController's constructor cannot be called.",
      );
    }

    get desiredSize() {
      if (!isReadableStreamDefaultController(this)) {
        throw new TypeError("Invalid ReadableStreamDefaultController.");
      }
      return readableStreamDefaultControllerGetDesiredSize(this);
    }

    close() {
      if (!isReadableStreamDefaultController(this)) {
        throw new TypeError("Invalid ReadableStreamDefaultController.");
      }
      if (!readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
        throw new TypeError(
          "ReadableStreamDefaultController cannot close or enqueue.",
        );
      }
      readableStreamDefaultControllerClose(this);
    }

    enqueue(chunk) {
      if (!isReadableStreamDefaultController(this)) {
        throw new TypeError("Invalid ReadableStreamDefaultController.");
      }
      if (!readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
        throw new TypeError("ReadableSteamController cannot enqueue.");
      }
      return readableStreamDefaultControllerEnqueue(this, chunk);
    }

    error(error) {
      if (!isReadableStreamDefaultController(this)) {
        throw new TypeError("Invalid ReadableStreamDefaultController.");
      }
      readableStreamDefaultControllerError(this, error);
    }

    [sym.cancelSteps](reason) {
      resetQueue(this);
      const result = this[sym.cancelAlgorithm](reason);
      readableStreamDefaultControllerClearAlgorithms(this);
      return result;
    }

    [sym.pullSteps]() {
      const stream = this[sym.controlledReadableStream];
      if (this[sym.queue].length) {
        const chunk = dequeueValue(this);
        if (this[sym.closeRequested] && this[sym.queue].length === 0) {
          readableStreamDefaultControllerClearAlgorithms(this);
          readableStreamClose(stream);
        } else {
          readableStreamDefaultControllerCallPullIfNeeded(this);
        }
        return Promise.resolve(
          readableStreamCreateReadResult(
            chunk,
            false,
            stream[sym.reader][sym.forAuthorCode],
          ),
        );
      }
      const pendingPromise = readableStreamAddReadRequest(stream);
      readableStreamDefaultControllerCallPullIfNeeded(this);
      return pendingPromise;
    }

    [customInspect]() {
      return `${this.constructor.name} { desiredSize: ${
        String(this.desiredSize)
      } }`;
    }
  }

  class ReadableStreamDefaultReader {
    constructor(stream) {
      if (!isReadableStream(stream)) {
        throw new TypeError("stream is not a ReadableStream.");
      }
      if (isReadableStreamLocked(stream)) {
        throw new TypeError("stream is locked.");
      }
      readableStreamReaderGenericInitialize(this, stream);
      this[sym.readRequests] = [];
    }

    get closed() {
      if (!isReadableStreamDefaultReader(this)) {
        return Promise.reject(
          new TypeError("Invalid ReadableStreamDefaultReader."),
        );
      }
      return (
        this[sym.closedPromise].promise ??
          Promise.reject(new TypeError("Invalid reader."))
      );
    }

    cancel(reason) {
      if (!isReadableStreamDefaultReader(this)) {
        return Promise.reject(
          new TypeError("Invalid ReadableStreamDefaultReader."),
        );
      }
      if (!this[sym.ownerReadableStream]) {
        return Promise.reject(new TypeError("Invalid reader."));
      }
      return readableStreamReaderGenericCancel(this, reason);
    }

    read() {
      if (!isReadableStreamDefaultReader(this)) {
        return Promise.reject(
          new TypeError("Invalid ReadableStreamDefaultReader."),
        );
      }
      if (!this[sym.ownerReadableStream]) {
        return Promise.reject(new TypeError("Invalid reader."));
      }
      return readableStreamDefaultReaderRead(this);
    }

    releaseLock() {
      if (!isReadableStreamDefaultReader(this)) {
        throw new TypeError("Invalid ReadableStreamDefaultReader.");
      }
      if (this[sym.ownerReadableStream] === undefined) {
        return;
      }
      if (this[sym.readRequests].length) {
        throw new TypeError("Cannot release lock with pending read requests.");
      }
      readableStreamReaderGenericRelease(this);
    }

    [customInspect]() {
      return `${this.constructor.name} { closed: Promise }`;
    }
  }

  const AsyncIteratorPrototype = Object
    .getPrototypeOf(Object.getPrototypeOf(async function* () {}).prototype);

  const ReadableStreamAsyncIteratorPrototype = Object.setPrototypeOf({
    next() {
      if (!isReadableStreamAsyncIterator(this)) {
        return Promise.reject(
          new TypeError("invalid ReadableStreamAsyncIterator."),
        );
      }
      const reader = this[sym.asyncIteratorReader];
      if (!reader[sym.ownerReadableStream]) {
        return Promise.reject(
          new TypeError("reader owner ReadableStream is undefined."),
        );
      }
      return readableStreamDefaultReaderRead(reader).then((result) => {
        assert(typeof result === "object");
        const { done } = result;
        assert(typeof done === "boolean");
        if (done) {
          readableStreamReaderGenericRelease(reader);
        }
        const { value } = result;
        return readableStreamCreateReadResult(value, done, true);
      });
    },
    return(
      value,
    ) {
      if (!isReadableStreamAsyncIterator(this)) {
        return Promise.reject(
          new TypeError("invalid ReadableStreamAsyncIterator."),
        );
      }
      const reader = this[sym.asyncIteratorReader];
      if (!reader[sym.ownerReadableStream]) {
        return Promise.reject(
          new TypeError("reader owner ReadableStream is undefined."),
        );
      }
      if (reader[sym.readRequests].length) {
        return Promise.reject(
          new TypeError("reader has outstanding read requests."),
        );
      }
      if (!this[sym.preventCancel]) {
        const result = readableStreamReaderGenericCancel(reader, value);
        readableStreamReaderGenericRelease(reader);
        return result.then(() =>
          readableStreamCreateReadResult(value, true, true)
        );
      }
      readableStreamReaderGenericRelease(reader);
      return Promise.resolve(
        readableStreamCreateReadResult(value, true, true),
      );
    },
  }, AsyncIteratorPrototype);

  class ReadableStream {
    constructor(
      underlyingSource = {},
      strategy = {},
    ) {
      initializeReadableStream(this);
      const { size } = strategy;
      let { highWaterMark } = strategy;
      const { type } = underlyingSource;

      if (underlyingSource.type == "bytes") {
        if (size !== undefined) {
          throw new RangeError(
            `When underlying source is "bytes", strategy.size must be undefined.`,
          );
        }
        highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark ?? 0);
        setUpReadableByteStreamControllerFromUnderlyingSource(
          this,
          underlyingSource,
          highWaterMark,
        );
      } else if (type === undefined) {
        const sizeAlgorithm = makeSizeAlgorithmFromSizeFunction(size);
        highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark ?? 1);
        setUpReadableStreamDefaultControllerFromUnderlyingSource(
          this,
          underlyingSource,
          highWaterMark,
          sizeAlgorithm,
        );
      } else {
        throw new RangeError(
          `Valid values for underlyingSource are "bytes" or undefined.  Received: "${type}".`,
        );
      }
    }

    get locked() {
      if (!isReadableStream(this)) {
        throw new TypeError("Invalid ReadableStream.");
      }
      return isReadableStreamLocked(this);
    }

    cancel(reason) {
      if (!isReadableStream(this)) {
        return Promise.reject(new TypeError("Invalid ReadableStream."));
      }
      if (isReadableStreamLocked(this)) {
        return Promise.reject(
          new TypeError("Cannot cancel a locked ReadableStream."),
        );
      }
      return readableStreamCancel(this, reason);
    }

    getIterator({
      preventCancel,
    } = {}) {
      if (!isReadableStream(this)) {
        throw new TypeError("Invalid ReadableStream.");
      }
      const reader = acquireReadableStreamDefaultReader(this);
      const iterator = Object.create(ReadableStreamAsyncIteratorPrototype);
      iterator[sym.asyncIteratorReader] = reader;
      iterator[sym.preventCancel] = Boolean(preventCancel);
      return iterator;
    }

    getReader({ mode } = {}) {
      if (!isReadableStream(this)) {
        throw new TypeError("Invalid ReadableStream.");
      }
      if (mode === undefined) {
        return acquireReadableStreamDefaultReader(this, true);
      }
      mode = String(mode);
      // 3.2.5.4.4 If mode is "byob", return ? AcquireReadableStreamBYOBReader(this, true).
      throw new RangeError(`Unsupported mode "${mode}"`);
    }

    pipeThrough(
      {
        writable,
        readable,
      },
      { preventClose, preventAbort, preventCancel, signal } = {},
    ) {
      if (!isReadableStream(this)) {
        throw new TypeError("Invalid ReadableStream.");
      }
      if (!isWritableStream(writable)) {
        throw new TypeError("writable is not a valid WritableStream.");
      }
      if (!isReadableStream(readable)) {
        throw new TypeError("readable is not a valid ReadableStream.");
      }
      preventClose = Boolean(preventClose);
      preventAbort = Boolean(preventAbort);
      preventCancel = Boolean(preventCancel);
      if (signal && !(signal instanceof AbortSignal)) {
        throw new TypeError("Invalid signal.");
      }
      if (isReadableStreamLocked(this)) {
        throw new TypeError("ReadableStream is locked.");
      }
      if (isWritableStreamLocked(writable)) {
        throw new TypeError("writable is locked.");
      }
      const promise = readableStreamPipeTo(
        this,
        writable,
        preventClose,
        preventAbort,
        preventCancel,
        signal,
      );
      setPromiseIsHandledToTrue(promise);
      return readable;
    }

    pipeTo(
      dest,
      { preventClose, preventAbort, preventCancel, signal } = {},
    ) {
      if (!isReadableStream(this)) {
        return Promise.reject(new TypeError("Invalid ReadableStream."));
      }
      if (!isWritableStream(dest)) {
        return Promise.reject(
          new TypeError("dest is not a valid WritableStream."),
        );
      }
      preventClose = Boolean(preventClose);
      preventAbort = Boolean(preventAbort);
      preventCancel = Boolean(preventCancel);
      if (signal && !(signal instanceof AbortSignal)) {
        return Promise.reject(new TypeError("Invalid signal."));
      }
      if (isReadableStreamLocked(this)) {
        return Promise.reject(new TypeError("ReadableStream is locked."));
      }
      if (isWritableStreamLocked(dest)) {
        return Promise.reject(new TypeError("dest is locked."));
      }
      return readableStreamPipeTo(
        this,
        dest,
        preventClose,
        preventAbort,
        preventCancel,
        signal,
      );
    }

    tee() {
      if (!isReadableStream(this)) {
        throw new TypeError("Invalid ReadableStream.");
      }
      return readableStreamTee(this, false);
    }

    [customInspect]() {
      return `${this.constructor.name} { locked: ${String(this.locked)} }`;
    }

    [Symbol.asyncIterator](
      options = {},
    ) {
      return this.getIterator(options);
    }
  }

  class TransformStream {
    constructor(
      transformer = {},
      writableStrategy = {},
      readableStrategy = {},
    ) {
      const writableSizeFunction = writableStrategy.size;
      let writableHighWaterMark = writableStrategy.highWaterMark;
      const readableSizeFunction = readableStrategy.size;
      let readableHighWaterMark = readableStrategy.highWaterMark;
      const writableType = transformer.writableType;
      if (writableType !== undefined) {
        throw new RangeError(
          `Expected transformer writableType to be undefined, received "${
            String(writableType)
          }"`,
        );
      }
      const writableSizeAlgorithm = makeSizeAlgorithmFromSizeFunction(
        writableSizeFunction,
      );
      if (writableHighWaterMark === undefined) {
        writableHighWaterMark = 1;
      }
      writableHighWaterMark = validateAndNormalizeHighWaterMark(
        writableHighWaterMark,
      );
      const readableType = transformer.readableType;
      if (readableType !== undefined) {
        throw new RangeError(
          `Expected transformer readableType to be undefined, received "${
            String(readableType)
          }"`,
        );
      }
      const readableSizeAlgorithm = makeSizeAlgorithmFromSizeFunction(
        readableSizeFunction,
      );
      if (readableHighWaterMark === undefined) {
        readableHighWaterMark = 1;
      }
      readableHighWaterMark = validateAndNormalizeHighWaterMark(
        readableHighWaterMark,
      );
      const startPromise = getDeferred();
      initializeTransformStream(
        this,
        startPromise.promise,
        writableHighWaterMark,
        writableSizeAlgorithm,
        readableHighWaterMark,
        readableSizeAlgorithm,
      );
      // the brand check expects this, and the brand check occurs in the following
      // but the property hasn't been defined.
      Object.defineProperty(this, sym.transformStreamController, {
        value: undefined,
        writable: true,
        configurable: true,
      });
      setUpTransformStreamDefaultControllerFromTransformer(this, transformer);
      const startResult = invokeOrNoop(
        transformer,
        "start",
        this[sym.transformStreamController],
      );
      startPromise.resolve(startResult);
    }

    get readable() {
      if (!isTransformStream(this)) {
        throw new TypeError("Invalid TransformStream.");
      }
      return this[sym.readable];
    }

    get writable() {
      if (!isTransformStream(this)) {
        throw new TypeError("Invalid TransformStream.");
      }
      return this[sym.writable];
    }

    [customInspect]() {
      return this.constructor.name;
    }
  }

  class TransformStreamDefaultController {
    constructor() {
      throw new TypeError(
        "TransformStreamDefaultController's constructor cannot be called.",
      );
    }

    get desiredSize() {
      if (!isTransformStreamDefaultController(this)) {
        throw new TypeError("Invalid TransformStreamDefaultController.");
      }
      const readableController = this[sym.controlledTransformStream][
        sym.readable
      ][sym.readableStreamController];
      return readableStreamDefaultControllerGetDesiredSize(
        readableController,
      );
    }

    enqueue(chunk) {
      if (!isTransformStreamDefaultController(this)) {
        throw new TypeError("Invalid TransformStreamDefaultController.");
      }
      transformStreamDefaultControllerEnqueue(this, chunk);
    }

    error(reason) {
      if (!isTransformStreamDefaultController(this)) {
        throw new TypeError("Invalid TransformStreamDefaultController.");
      }
      transformStreamDefaultControllerError(this, reason);
    }

    terminate() {
      if (!isTransformStreamDefaultController(this)) {
        throw new TypeError("Invalid TransformStreamDefaultController.");
      }
      transformStreamDefaultControllerTerminate(this);
    }

    [customInspect]() {
      return `${this.constructor.name} { desiredSize: ${
        String(this.desiredSize)
      } }`;
    }
  }

  class WritableStreamDefaultController {
    constructor() {
      throw new TypeError(
        "WritableStreamDefaultController's constructor cannot be called.",
      );
    }

    error(e) {
      if (!isWritableStreamDefaultController(this)) {
        throw new TypeError("Invalid WritableStreamDefaultController.");
      }
      const state = this[sym.controlledWritableStream][sym.state];
      if (state !== "writable") {
        return;
      }
      writableStreamDefaultControllerError(this, e);
    }

    [sym.abortSteps](reason) {
      const result = this[sym.abortAlgorithm](reason);
      writableStreamDefaultControllerClearAlgorithms(this);
      return result;
    }

    [sym.errorSteps]() {
      resetQueue(this);
    }

    [customInspect]() {
      return `${this.constructor.name} { }`;
    }
  }

  class WritableStreamDefaultWriter {
    constructor(stream) {
      if (!isWritableStream(stream)) {
        throw new TypeError("Invalid stream.");
      }
      if (isWritableStreamLocked(stream)) {
        throw new TypeError("Cannot create a writer for a locked stream.");
      }
      this[sym.ownerWritableStream] = stream;
      stream[sym.writer] = this;
      const state = stream[sym.state];
      if (state === "writable") {
        if (
          !writableStreamCloseQueuedOrInFlight(stream) &&
          stream[sym.backpressure]
        ) {
          this[sym.readyPromise] = getDeferred();
        } else {
          this[sym.readyPromise] = { promise: Promise.resolve() };
        }
        this[sym.closedPromise] = getDeferred();
      } else if (state === "erroring") {
        this[sym.readyPromise] = {
          promise: Promise.reject(stream[sym.storedError]),
        };
        setPromiseIsHandledToTrue(this[sym.readyPromise].promise);
        this[sym.closedPromise] = getDeferred();
      } else if (state === "closed") {
        this[sym.readyPromise] = { promise: Promise.resolve() };
        this[sym.closedPromise] = { promise: Promise.resolve() };
      } else {
        assert(state === "errored");
        const storedError = stream[sym.storedError];
        this[sym.readyPromise] = { promise: Promise.reject(storedError) };
        setPromiseIsHandledToTrue(this[sym.readyPromise].promise);
        this[sym.closedPromise] = { promise: Promise.reject(storedError) };
        setPromiseIsHandledToTrue(this[sym.closedPromise].promise);
      }
    }

    get closed() {
      if (!isWritableStreamDefaultWriter(this)) {
        return Promise.reject(
          new TypeError("Invalid WritableStreamDefaultWriter."),
        );
      }
      return this[sym.closedPromise].promise;
    }

    get desiredSize() {
      if (!isWritableStreamDefaultWriter(this)) {
        throw new TypeError("Invalid WritableStreamDefaultWriter.");
      }
      if (!this[sym.ownerWritableStream]) {
        throw new TypeError("WritableStreamDefaultWriter has no owner.");
      }
      return writableStreamDefaultWriterGetDesiredSize(this);
    }

    get ready() {
      if (!isWritableStreamDefaultWriter(this)) {
        return Promise.reject(
          new TypeError("Invalid WritableStreamDefaultWriter."),
        );
      }
      return this[sym.readyPromise].promise;
    }

    abort(reason) {
      if (!isWritableStreamDefaultWriter(this)) {
        return Promise.reject(
          new TypeError("Invalid WritableStreamDefaultWriter."),
        );
      }
      if (!this[sym.ownerWritableStream]) {
        Promise.reject(
          new TypeError("WritableStreamDefaultWriter has no owner."),
        );
      }
      return writableStreamDefaultWriterAbort(this, reason);
    }

    close() {
      if (!isWritableStreamDefaultWriter(this)) {
        return Promise.reject(
          new TypeError("Invalid WritableStreamDefaultWriter."),
        );
      }
      const stream = this[sym.ownerWritableStream];
      if (!stream) {
        Promise.reject(
          new TypeError("WritableStreamDefaultWriter has no owner."),
        );
      }
      if (writableStreamCloseQueuedOrInFlight(stream)) {
        Promise.reject(
          new TypeError("Stream is in an invalid state to be closed."),
        );
      }
      return writableStreamDefaultWriterClose(this);
    }

    releaseLock() {
      if (!isWritableStreamDefaultWriter(this)) {
        throw new TypeError("Invalid WritableStreamDefaultWriter.");
      }
      const stream = this[sym.ownerWritableStream];
      if (!stream) {
        return;
      }
      assert(stream[sym.writer]);
      writableStreamDefaultWriterRelease(this);
    }

    write(chunk) {
      if (!isWritableStreamDefaultWriter(this)) {
        return Promise.reject(
          new TypeError("Invalid WritableStreamDefaultWriter."),
        );
      }
      if (!this[sym.ownerWritableStream]) {
        Promise.reject(
          new TypeError("WritableStreamDefaultWriter has no owner."),
        );
      }
      return writableStreamDefaultWriterWrite(this, chunk);
    }

    [customInspect]() {
      return `${this.constructor.name} { closed: Promise, desiredSize: ${
        String(this.desiredSize)
      }, ready: Promise }`;
    }
  }

  class WritableStream {
    constructor(
      underlyingSink = {},
      strategy = {},
    ) {
      initializeWritableStream(this);
      const size = strategy.size;
      let highWaterMark = strategy.highWaterMark ?? 1;
      const { type } = underlyingSink;
      if (type !== undefined) {
        throw new RangeError(`Sink type of "${String(type)}" not supported.`);
      }
      const sizeAlgorithm = makeSizeAlgorithmFromSizeFunction(size);
      highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
      setUpWritableStreamDefaultControllerFromUnderlyingSink(
        this,
        underlyingSink,
        highWaterMark,
        sizeAlgorithm,
      );
    }

    get locked() {
      if (!isWritableStream(this)) {
        throw new TypeError("Invalid WritableStream.");
      }
      return isWritableStreamLocked(this);
    }

    abort(reason) {
      if (!isWritableStream(this)) {
        return Promise.reject(new TypeError("Invalid WritableStream."));
      }
      if (isWritableStreamLocked(this)) {
        return Promise.reject(
          new TypeError("Cannot abort a locked WritableStream."),
        );
      }
      return writableStreamAbort(this, reason);
    }

    close() {
      if (!isWritableStream(this)) {
        return Promise.reject(new TypeError("Invalid WritableStream."));
      }
      if (isWritableStreamLocked(this)) {
        return Promise.reject(
          new TypeError("Cannot abort a locked WritableStream."),
        );
      }
      if (writableStreamCloseQueuedOrInFlight(this)) {
        return Promise.reject(
          new TypeError("Cannot close an already closing WritableStream."),
        );
      }
      return writableStreamClose(this);
    }

    getWriter() {
      if (!isWritableStream(this)) {
        throw new TypeError("Invalid WritableStream.");
      }
      return acquireWritableStreamDefaultWriter(this);
    }

    [customInspect]() {
      return `${this.constructor.name} { locked: ${String(this.locked)} }`;
    }
  }

  function acquireReadableStreamDefaultReader(
    stream,
    forAuthorCode = false,
  ) {
    const reader = new ReadableStreamDefaultReader(stream);
    reader[sym.forAuthorCode] = forAuthorCode;
    return reader;
  }

  function acquireWritableStreamDefaultWriter(
    stream,
  ) {
    return new WritableStreamDefaultWriter(stream);
  }

  function call(
    fn,
    v,
    args,
  ) {
    return Function.prototype.apply.call(fn, v, args);
  }

  function createAlgorithmFromUnderlyingMethod(
    underlyingObject,
    methodName,
    algoArgCount,
    ...extraArgs
  ) {
    const method = underlyingObject[methodName];
    if (method) {
      if (!isCallable(method)) {
        throw new TypeError("method is not callable");
      }
      if (algoArgCount === 0) {
        // deno-lint-ignore require-await
        return async () => call(method, underlyingObject, extraArgs);
      } else {
        // deno-lint-ignore require-await
        return async (arg) => {
          const fullArgs = [arg, ...extraArgs];
          return call(method, underlyingObject, fullArgs);
        };
      }
    }
    // deno-lint-ignore require-await
    return async () => undefined;
  }

  function createReadableStream(
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark = 1,
    sizeAlgorithm = () => 1,
  ) {
    highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
    const stream = Object.create(
      ReadableStream.prototype,
    );
    initializeReadableStream(stream);
    const controller = Object.create(
      ReadableStreamDefaultController.prototype,
    );
    setUpReadableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm,
    );
    return stream;
  }

  function createWritableStream(
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark = 1,
    sizeAlgorithm = () => 1,
  ) {
    highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
    const stream = Object.create(WritableStream.prototype);
    initializeWritableStream(stream);
    const controller = Object.create(
      WritableStreamDefaultController.prototype,
    );
    setUpWritableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      writeAlgorithm,
      closeAlgorithm,
      abortAlgorithm,
      highWaterMark,
      sizeAlgorithm,
    );
    return stream;
  }

  function dequeueValue(container) {
    assert(sym.queue in container && sym.queueTotalSize in container);
    assert(container[sym.queue].length);
    const pair = container[sym.queue].shift();
    container[sym.queueTotalSize] -= pair.size;
    if (container[sym.queueTotalSize] <= 0) {
      container[sym.queueTotalSize] = 0;
    }
    return pair.value;
  }

  function enqueueValueWithSize(
    container,
    value,
    size,
  ) {
    assert(sym.queue in container && sym.queueTotalSize in container);
    size = Number(size);
    if (!isFiniteNonNegativeNumber(size)) {
      throw new RangeError("size must be a finite non-negative number.");
    }
    container[sym.queue].push({ value, size });
    container[sym.queueTotalSize] += size;
  }

  /** Non-spec mechanism to "unwrap" a promise and store it to be resolved
   * later. */
  function getDeferred() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve: resolve, reject: reject };
  }

  function initializeReadableStream(
    stream,
  ) {
    stream[sym.state] = "readable";
    stream[sym.reader] = stream[sym.storedError] = undefined;
    stream[sym.disturbed] = false;
  }

  function initializeTransformStream(
    stream,
    startPromise,
    writableHighWaterMark,
    writableSizeAlgorithm,
    readableHighWaterMark,
    readableSizeAlgorithm,
  ) {
    const startAlgorithm = () => startPromise;
    const writeAlgorithm = (chunk) =>
      transformStreamDefaultSinkWriteAlgorithm(stream, chunk);
    const abortAlgorithm = (reason) =>
      transformStreamDefaultSinkAbortAlgorithm(stream, reason);
    const closeAlgorithm = () =>
      transformStreamDefaultSinkCloseAlgorithm(stream);
    stream[sym.writable] = createWritableStream(
      startAlgorithm,
      writeAlgorithm,
      closeAlgorithm,
      abortAlgorithm,
      writableHighWaterMark,
      writableSizeAlgorithm,
    );
    const pullAlgorithm = () =>
      transformStreamDefaultSourcePullAlgorithm(stream);
    const cancelAlgorithm = (reason) => {
      transformStreamErrorWritableAndUnblockWrite(stream, reason);
      return Promise.resolve(undefined);
    };
    stream[sym.readable] = createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      readableHighWaterMark,
      readableSizeAlgorithm,
    );
    stream[sym.backpressure] = stream[sym.backpressureChangePromise] =
      undefined;
    transformStreamSetBackpressure(stream, true);
    Object.defineProperty(stream, sym.transformStreamController, {
      value: undefined,
      configurable: true,
    });
  }

  function initializeWritableStream(
    stream,
  ) {
    stream[sym.state] = "writable";
    stream[sym.storedError] = stream[sym.writer] = stream[
      sym.writableStreamController
    ] = stream[sym.inFlightWriteRequest] = stream[sym.closeRequest] = stream[
      sym.inFlightCloseRequest
    ] = stream[sym.pendingAbortRequest] = undefined;
    stream[sym.writeRequests] = [];
    stream[sym.backpressure] = false;
  }

  function invokeOrNoop(
    o,
    p,
    ...args
  ) {
    assert(o);
    const method = o[p];
    if (!method) {
      return undefined;
    }
    return call(method, o, args);
  }

  function isCallable(value) {
    return typeof value === "function";
  }

  function isDetachedBuffer(value) {
    return sym.isFakeDetached in value;
  }

  function isFiniteNonNegativeNumber(v) {
    return Number.isFinite(v) && (v) >= 0;
  }

  function isReadableByteStreamController(
    x,
  ) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.controlledReadableByteStream in x)
    );
  }

  function isReadableStream(x) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.readableStreamController in x)
    );
  }

  function isReadableStreamAsyncIterator(
    x,
  ) {
    if (typeof x !== "object" || x === null) {
      return false;
    }
    return sym.asyncIteratorReader in x;
  }

  function isReadableStreamDefaultController(
    x,
  ) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.controlledReadableStream in x)
    );
  }

  function isReadableStreamDefaultReader(
    x,
  ) {
    return !(typeof x !== "object" || x === null || !(sym.readRequests in x));
  }

  function isReadableStreamLocked(stream) {
    assert(isReadableStream(stream));
    return !!stream[sym.reader];
  }

  function isReadableStreamDisturbed(stream) {
    assert(isReadableStream(stream));
    return !!stream[sym.disturbed];
  }

  function isTransformStream(x) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.transformStreamController in x)
    );
  }

  function isTransformStreamDefaultController(
    x,
  ) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.controlledTransformStream in x)
    );
  }

  function isWritableStream(x) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.writableStreamController in x)
    );
  }

  function isWritableStreamDefaultController(
    x,
  ) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.controlledWritableStream in x)
    );
  }

  function isWritableStreamDefaultWriter(
    x,
  ) {
    return !(
      typeof x !== "object" ||
      x === null ||
      !(sym.ownerWritableStream in x)
    );
  }

  function isWritableStreamLocked(stream) {
    assert(isWritableStream(stream));
    return stream[sym.writer] !== undefined;
  }

  function makeSizeAlgorithmFromSizeFunction(
    size,
  ) {
    if (size === undefined) {
      return () => 1;
    }
    if (typeof size !== "function") {
      throw new TypeError("size must be callable.");
    }
    return (chunk) => {
      return size.call(undefined, chunk);
    };
  }

  function peekQueueValue(container) {
    assert(sym.queue in container && sym.queueTotalSize in container);
    assert(container[sym.queue].length);
    const [pair] = container[sym.queue];
    return pair.value;
  }

  function readableByteStreamControllerShouldCallPull(
    controller,
  ) {
    const stream = controller[sym.controlledReadableByteStream];
    if (
      stream[sym.state] !== "readable" ||
      controller[sym.closeRequested] ||
      !controller[sym.started]
    ) {
      return false;
    }
    if (
      readableStreamHasDefaultReader(stream) &&
      readableStreamGetNumReadRequests(stream) > 0
    ) {
      return true;
    }
    // 3.13.25.6 If ! ReadableStreamHasBYOBReader(stream) is true and !
    //            ReadableStreamGetNumReadIntoRequests(stream) > 0, return true.
    const desiredSize = readableByteStreamControllerGetDesiredSize(controller);
    assert(desiredSize !== null);
    return desiredSize > 0;
  }

  function readableByteStreamControllerCallPullIfNeeded(
    controller,
  ) {
    const shouldPull = readableByteStreamControllerShouldCallPull(controller);
    if (!shouldPull) {
      return;
    }
    if (controller[sym.pulling]) {
      controller[sym.pullAgain] = true;
      return;
    }
    assert(controller[sym.pullAgain] === false);
    controller[sym.pulling] = true;
    const pullPromise = controller[sym.pullAlgorithm]();
    setPromiseIsHandledToTrue(
      pullPromise.then(
        () => {
          controller[sym.pulling] = false;
          if (controller[sym.pullAgain]) {
            controller[sym.pullAgain] = false;
            readableByteStreamControllerCallPullIfNeeded(controller);
          }
        },
        (e) => {
          readableByteStreamControllerError(controller, e);
        },
      ),
    );
  }

  function readableByteStreamControllerClearAlgorithms(
    controller,
  ) {
    controller[sym.pullAlgorithm] = undefined;
    controller[sym.cancelAlgorithm] = undefined;
  }

  function readableByteStreamControllerClose(
    controller,
  ) {
    const stream = controller[sym.controlledReadableByteStream];
    if (controller[sym.closeRequested] || stream[sym.state] !== "readable") {
      return;
    }
    if (controller[sym.queueTotalSize] > 0) {
      controller[sym.closeRequested] = true;
      return;
    }
    // 3.13.6.4 If controller.[[pendingPullIntos]] is not empty, (BYOB Support)
    readableByteStreamControllerClearAlgorithms(controller);
    readableStreamClose(stream);
  }

  function readableByteStreamControllerEnqueue(
    controller,
    chunk,
  ) {
    const stream = controller[sym.controlledReadableByteStream];
    if (controller[sym.closeRequested] || stream[sym.state] !== "readable") {
      return;
    }
    const { buffer, byteOffset, byteLength } = chunk;
    const transferredBuffer = transferArrayBuffer(buffer);
    if (readableStreamHasDefaultReader(stream)) {
      if (readableStreamGetNumReadRequests(stream) === 0) {
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          transferredBuffer,
          byteOffset,
          byteLength,
        );
      } else {
        assert(controller[sym.queue].length === 0);
        const transferredView = new Uint8Array(
          transferredBuffer,
          byteOffset,
          byteLength,
        );
        readableStreamFulfillReadRequest(stream, transferredView, false);
      }
      // 3.13.9.8 Otherwise, if ! ReadableStreamHasBYOBReader(stream) is true
    } else {
      assert(!isReadableStreamLocked(stream));
      readableByteStreamControllerEnqueueChunkToQueue(
        controller,
        transferredBuffer,
        byteOffset,
        byteLength,
      );
    }
    readableByteStreamControllerCallPullIfNeeded(controller);
  }

  function readableByteStreamControllerEnqueueChunkToQueue(
    controller,
    buffer,
    byteOffset,
    byteLength,
  ) {
    controller[sym.queue].push({
      value: buffer,
      offset: byteOffset,
      size: byteLength,
    });
    controller[sym.queueTotalSize] += byteLength;
  }

  function readableByteStreamControllerError(
    controller,
    e,
  ) {
    const stream = controller[sym.controlledReadableByteStream];
    if (stream[sym.state] !== "readable") {
      return;
    }
    // 3.13.11.3 Perform ! ReadableByteStreamControllerClearPendingPullIntos(controller).
    resetQueue(controller);
    readableByteStreamControllerClearAlgorithms(controller);
    readableStreamError(stream, e);
  }

  function readableByteStreamControllerGetDesiredSize(
    controller,
  ) {
    const stream = controller[sym.controlledReadableByteStream];
    const state = stream[sym.state];
    if (state === "errored") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
  }

  function readableByteStreamControllerHandleQueueDrain(
    controller,
  ) {
    assert(
      controller[sym.controlledReadableByteStream][sym.state] === "readable",
    );
    if (
      controller[sym.queueTotalSize] === 0 && controller[sym.closeRequested]
    ) {
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamClose(controller[sym.controlledReadableByteStream]);
    } else {
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
  }

  function readableStreamAddReadRequest(
    stream,
  ) {
    assert(isReadableStreamDefaultReader(stream[sym.reader]));
    assert(stream[sym.state] === "readable");
    const promise = getDeferred();
    stream[sym.reader][sym.readRequests].push(promise);
    return promise.promise;
  }

  function readableStreamCancel(
    stream,
    reason,
  ) {
    stream[sym.disturbed] = true;
    if (stream[sym.state] === "closed") {
      return Promise.resolve();
    }
    if (stream[sym.state] === "errored") {
      return Promise.reject(stream[sym.storedError]);
    }
    readableStreamClose(stream);
    return stream[sym.readableStreamController][sym.cancelSteps](reason).then(
      () => undefined,
    );
  }

  function readableStreamClose(stream) {
    assert(stream[sym.state] === "readable");
    stream[sym.state] = "closed";
    const reader = stream[sym.reader];
    if (!reader) {
      return;
    }
    if (isReadableStreamDefaultReader(reader)) {
      for (const readRequest of reader[sym.readRequests]) {
        assert(readRequest.resolve);
        readRequest.resolve(
          readableStreamCreateReadResult(
            undefined,
            true,
            reader[sym.forAuthorCode],
          ),
        );
      }
      reader[sym.readRequests] = [];
    }
    const resolve = reader[sym.closedPromise].resolve;
    assert(resolve);
    resolve();
  }

  function readableStreamCreateReadResult(
    value,
    done,
    forAuthorCode,
  ) {
    const prototype = forAuthorCode ? Object.prototype : null;
    assert(typeof done === "boolean");
    const obj = Object.create(prototype);
    Object.defineProperties(obj, {
      value: { value, writable: true, enumerable: true, configurable: true },
      done: {
        value: done,
        writable: true,
        enumerable: true,
        configurable: true,
      },
    });
    return obj;
  }

  function readableStreamDefaultControllerCallPullIfNeeded(
    controller,
  ) {
    const shouldPull = readableStreamDefaultControllerShouldCallPull(
      controller,
    );
    if (!shouldPull) {
      return;
    }
    if (controller[sym.pulling]) {
      controller[sym.pullAgain] = true;
      return;
    }
    assert(controller[sym.pullAgain] === false);
    controller[sym.pulling] = true;
    const pullPromise = controller[sym.pullAlgorithm]();
    pullPromise.then(
      () => {
        controller[sym.pulling] = false;
        if (controller[sym.pullAgain]) {
          controller[sym.pullAgain] = false;
          readableStreamDefaultControllerCallPullIfNeeded(controller);
        }
      },
      (e) => {
        readableStreamDefaultControllerError(controller, e);
      },
    );
  }

  function readableStreamDefaultControllerCanCloseOrEnqueue(
    controller,
  ) {
    const state = controller[sym.controlledReadableStream][sym.state];
    return !controller[sym.closeRequested] && state === "readable";
  }

  function readableStreamDefaultControllerClearAlgorithms(
    controller,
  ) {
    controller[sym.pullAlgorithm] = undefined;
    controller[sym.cancelAlgorithm] = undefined;
    controller[sym.strategySizeAlgorithm] = undefined;
  }

  function readableStreamDefaultControllerClose(
    controller,
  ) {
    if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
      return;
    }
    const stream = controller[sym.controlledReadableStream];
    controller[sym.closeRequested] = true;
    if (controller[sym.queue].length === 0) {
      readableStreamDefaultControllerClearAlgorithms(controller);
      readableStreamClose(stream);
    }
  }

  function readableStreamDefaultControllerEnqueue(
    controller,
    chunk,
  ) {
    if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
      return;
    }
    const stream = controller[sym.controlledReadableStream];
    if (
      isReadableStreamLocked(stream) &&
      readableStreamGetNumReadRequests(stream) > 0
    ) {
      readableStreamFulfillReadRequest(stream, chunk, false);
    } else {
      try {
        const chunkSize = controller[sym.strategySizeAlgorithm](chunk);
        enqueueValueWithSize(controller, chunk, chunkSize);
      } catch (err) {
        readableStreamDefaultControllerError(controller, err);
        throw err;
      }
    }
    readableStreamDefaultControllerCallPullIfNeeded(controller);
  }

  function readableStreamDefaultControllerGetDesiredSize(
    controller,
  ) {
    const stream = controller[sym.controlledReadableStream];
    const state = stream[sym.state];
    if (state === "errored") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
  }

  function readableStreamDefaultControllerError(
    controller,
    e,
  ) {
    const stream = controller[sym.controlledReadableStream];
    if (stream[sym.state] !== "readable") {
      return;
    }
    resetQueue(controller);
    readableStreamDefaultControllerClearAlgorithms(controller);
    readableStreamError(stream, e);
  }

  function readableStreamDefaultControllerHasBackpressure(
    controller,
  ) {
    return readableStreamDefaultControllerShouldCallPull(controller);
  }

  function readableStreamDefaultControllerShouldCallPull(
    controller,
  ) {
    const stream = controller[sym.controlledReadableStream];
    if (
      !readableStreamDefaultControllerCanCloseOrEnqueue(controller) ||
      controller[sym.started] === false
    ) {
      return false;
    }
    if (
      isReadableStreamLocked(stream) &&
      readableStreamGetNumReadRequests(stream) > 0
    ) {
      return true;
    }
    const desiredSize = readableStreamDefaultControllerGetDesiredSize(
      controller,
    );
    assert(desiredSize !== null);
    return desiredSize > 0;
  }

  function readableStreamDefaultReaderRead(
    reader,
  ) {
    const stream = reader[sym.ownerReadableStream];
    assert(stream);
    stream[sym.disturbed] = true;
    if (stream[sym.state] === "closed") {
      return Promise.resolve(
        readableStreamCreateReadResult(
          undefined,
          true,
          reader[sym.forAuthorCode],
        ),
      );
    }
    if (stream[sym.state] === "errored") {
      return Promise.reject(stream[sym.storedError]);
    }
    assert(stream[sym.state] === "readable");
    return (stream[
      sym.readableStreamController
    ])[sym.pullSteps]();
  }

  function readableStreamError(stream, e) {
    assert(isReadableStream(stream));
    assert(stream[sym.state] === "readable");
    stream[sym.state] = "errored";
    stream[sym.storedError] = e;
    const reader = stream[sym.reader];
    if (reader === undefined) {
      return;
    }
    if (isReadableStreamDefaultReader(reader)) {
      for (const readRequest of reader[sym.readRequests]) {
        assert(readRequest.reject);
        readRequest.reject(e);
        readRequest.reject = undefined;
        readRequest.resolve = undefined;
      }
      reader[sym.readRequests] = [];
    }
    // 3.5.6.8 Otherwise, support BYOB Reader
    reader[sym.closedPromise].reject(e);
    reader[sym.closedPromise].reject = undefined;
    reader[sym.closedPromise].resolve = undefined;
    setPromiseIsHandledToTrue(reader[sym.closedPromise].promise);
  }

  function readableStreamFulfillReadRequest(
    stream,
    chunk,
    done,
  ) {
    const reader = stream[sym.reader];
    const readRequest = reader[sym.readRequests].shift();
    assert(readRequest.resolve);
    readRequest.resolve(
      readableStreamCreateReadResult(chunk, done, reader[sym.forAuthorCode]),
    );
  }

  function readableStreamGetNumReadRequests(
    stream,
  ) {
    return stream[sym.reader]?.[sym.readRequests].length ?? 0;
  }

  function readableStreamHasDefaultReader(
    stream,
  ) {
    const reader = stream[sym.reader];
    return !(reader === undefined || !isReadableStreamDefaultReader(reader));
  }

  function readableStreamPipeTo(
    source,
    dest,
    preventClose,
    preventAbort,
    preventCancel,
    signal,
  ) {
    assert(isReadableStream(source));
    assert(isWritableStream(dest));
    assert(
      typeof preventClose === "boolean" &&
        typeof preventAbort === "boolean" &&
        typeof preventCancel === "boolean",
    );
    assert(signal === undefined || signal instanceof AbortSignal);
    assert(!isReadableStreamLocked(source));
    assert(!isWritableStreamLocked(dest));
    const reader = acquireReadableStreamDefaultReader(source);
    const writer = acquireWritableStreamDefaultWriter(dest);
    source[sym.disturbed] = true;
    let shuttingDown = false;
    const promise = getDeferred();
    let abortAlgorithm;
    if (signal) {
      abortAlgorithm = () => {
        const error = new DOMException("Abort signal received.", "AbortSignal");
        const actions = [];
        if (!preventAbort) {
          actions.push(() => {
            if (dest[sym.state] === "writable") {
              return writableStreamAbort(dest, error);
            } else {
              return Promise.resolve(undefined);
            }
          });
        }
        if (!preventCancel) {
          actions.push(() => {
            if (source[sym.state] === "readable") {
              return readableStreamCancel(source, error);
            } else {
              return Promise.resolve(undefined);
            }
          });
        }
        shutdownWithAction(
          () => Promise.all(actions.map((action) => action())),
          true,
          error,
        );
      };
      if (signal.aborted) {
        abortAlgorithm();
        return promise.promise;
      }
      signal.addEventListener("abort", abortAlgorithm);
    }

    let currentWrite = Promise.resolve();

    // At this point, the spec becomes non-specific and vague.  Most of the rest
    // of this code is based on the reference implementation that is part of the
    // specification.  This is why the functions are only scoped to this function
    // to ensure they don't leak into the spec compliant parts.

    function isOrBecomesClosed(
      stream,
      promise,
      action,
    ) {
      if (stream[sym.state] === "closed") {
        action();
      } else {
        setPromiseIsHandledToTrue(promise.then(action));
      }
    }

    function isOrBecomesErrored(
      stream,
      promise,
      action,
    ) {
      if (stream[sym.state] === "errored") {
        action(stream[sym.storedError]);
      } else {
        setPromiseIsHandledToTrue(promise.catch((error) => action(error)));
      }
    }

    function finalize(isError, error) {
      writableStreamDefaultWriterRelease(writer);
      readableStreamReaderGenericRelease(reader);

      if (signal) {
        signal.removeEventListener("abort", abortAlgorithm);
      }
      if (isError) {
        promise.reject(error);
      } else {
        promise.resolve();
      }
    }

    function waitForWritesToFinish() {
      const oldCurrentWrite = currentWrite;
      return currentWrite.then(() =>
        oldCurrentWrite !== currentWrite ? waitForWritesToFinish() : undefined
      );
    }

    function shutdownWithAction(
      action,
      originalIsError,
      originalError,
    ) {
      function doTheRest() {
        setPromiseIsHandledToTrue(
          action().then(
            () => finalize(originalIsError, originalError),
            (newError) => finalize(true, newError),
          ),
        );
      }

      if (shuttingDown) {
        return;
      }
      shuttingDown = true;

      if (
        dest[sym.state] === "writable" &&
        writableStreamCloseQueuedOrInFlight(dest) === false
      ) {
        setPromiseIsHandledToTrue(waitForWritesToFinish().then(doTheRest));
      } else {
        doTheRest();
      }
    }

    function shutdown(isError, error) {
      if (shuttingDown) {
        return;
      }
      shuttingDown = true;

      if (
        dest[sym.state] === "writable" &&
        !writableStreamCloseQueuedOrInFlight(dest)
      ) {
        setPromiseIsHandledToTrue(
          waitForWritesToFinish().then(() => finalize(isError, error)),
        );
      }
      finalize(isError, error);
    }

    function pipeStep() {
      if (shuttingDown) {
        return Promise.resolve(true);
      }
      return writer[sym.readyPromise].promise.then(() => {
        return readableStreamDefaultReaderRead(reader).then(
          ({ value, done }) => {
            if (done === true) {
              return true;
            }
            currentWrite = writableStreamDefaultWriterWrite(
              writer,
              value,
            ).then(undefined, () => {});
            return false;
          },
        );
      });
    }

    function pipeLoop() {
      return new Promise((resolveLoop, rejectLoop) => {
        function next(done) {
          if (done) {
            resolveLoop(undefined);
          } else {
            setPromiseIsHandledToTrue(pipeStep().then(next, rejectLoop));
          }
        }
        next(false);
      });
    }

    isOrBecomesErrored(
      source,
      reader[sym.closedPromise].promise,
      (storedError) => {
        if (!preventAbort) {
          shutdownWithAction(
            () => writableStreamAbort(dest, storedError),
            true,
            storedError,
          );
        } else {
          shutdown(true, storedError);
        }
      },
    );

    isOrBecomesErrored(
      dest,
      writer[sym.closedPromise].promise,
      (storedError) => {
        if (!preventCancel) {
          shutdownWithAction(
            () => readableStreamCancel(source, storedError),
            true,
            storedError,
          );
        } else {
          shutdown(true, storedError);
        }
      },
    );

    isOrBecomesClosed(source, reader[sym.closedPromise].promise, () => {
      if (!preventClose) {
        shutdownWithAction(() =>
          writableStreamDefaultWriterCloseWithErrorPropagation(writer)
        );
      }
    });

    if (
      writableStreamCloseQueuedOrInFlight(dest) ||
      dest[sym.state] === "closed"
    ) {
      const destClosed = new TypeError(
        "The destination writable stream closed before all data could be piped to it.",
      );
      if (!preventCancel) {
        shutdownWithAction(
          () => readableStreamCancel(source, destClosed),
          true,
          destClosed,
        );
      } else {
        shutdown(true, destClosed);
      }
    }

    setPromiseIsHandledToTrue(pipeLoop());
    return promise.promise;
  }

  function readableStreamReaderGenericCancel(
    reader,
    reason,
  ) {
    const stream = reader[sym.ownerReadableStream];
    assert(stream);
    return readableStreamCancel(stream, reason);
  }

  function readableStreamReaderGenericInitialize(
    reader,
    stream,
  ) {
    reader[sym.forAuthorCode] = true;
    reader[sym.ownerReadableStream] = stream;
    stream[sym.reader] = reader;
    if (stream[sym.state] === "readable") {
      reader[sym.closedPromise] = getDeferred();
    } else if (stream[sym.state] === "closed") {
      reader[sym.closedPromise] = { promise: Promise.resolve() };
    } else {
      assert(stream[sym.state] === "errored");
      reader[sym.closedPromise] = {
        promise: Promise.reject(stream[sym.storedError]),
      };
      setPromiseIsHandledToTrue(reader[sym.closedPromise].promise);
    }
  }

  function readableStreamReaderGenericRelease(
    reader,
  ) {
    assert(reader[sym.ownerReadableStream]);
    assert(reader[sym.ownerReadableStream][sym.reader] === reader);
    const closedPromise = reader[sym.closedPromise];
    if (reader[sym.ownerReadableStream][sym.state] === "readable") {
      assert(closedPromise.reject);
      closedPromise.reject(new TypeError("ReadableStream state is readable."));
    } else {
      closedPromise.promise = Promise.reject(
        new TypeError("Reading is closed."),
      );
      delete closedPromise.reject;
      delete closedPromise.resolve;
    }
    setPromiseIsHandledToTrue(closedPromise.promise);
    reader[sym.ownerReadableStream][sym.reader] = undefined;
    reader[sym.ownerReadableStream] = undefined;
  }

  function readableStreamTee(
    stream,
    cloneForBranch2,
  ) {
    assert(isReadableStream(stream));
    assert(typeof cloneForBranch2 === "boolean");
    const reader = acquireReadableStreamDefaultReader(stream);
    let reading = false;
    let canceled1 = false;
    let canceled2 = false;
    let reason1 = undefined;
    let reason2 = undefined;
    // deno-lint-ignore prefer-const
    let branch1;
    // deno-lint-ignore prefer-const
    let branch2;
    const cancelPromise = getDeferred();
    const pullAlgorithm = () => {
      if (reading) {
        return Promise.resolve();
      }
      reading = true;
      const readPromise = readableStreamDefaultReaderRead(reader).then(
        (result) => {
          reading = false;
          assert(typeof result === "object");
          const { done } = result;
          assert(typeof done === "boolean");
          if (done) {
            if (!canceled1) {
              readableStreamDefaultControllerClose(
                branch1[
                  sym.readableStreamController
                ],
              );
            }
            if (!canceled2) {
              readableStreamDefaultControllerClose(
                branch2[
                  sym.readableStreamController
                ],
              );
            }
            return;
          }
          const { value } = result;
          const value1 = value;
          let value2 = value;
          if (!canceled2 && cloneForBranch2) {
            value2 = cloneValue(value2);
          }
          if (!canceled1) {
            readableStreamDefaultControllerEnqueue(
              branch1[
                sym.readableStreamController
              ],
              value1,
            );
          }
          if (!canceled2) {
            readableStreamDefaultControllerEnqueue(
              branch2[
                sym.readableStreamController
              ],
              value2,
            );
          }
        },
      );
      setPromiseIsHandledToTrue(readPromise);
      return Promise.resolve();
    };
    const cancel1Algorithm = (reason) => {
      canceled1 = true;
      reason1 = reason;
      if (canceled2) {
        const compositeReason = [reason1, reason2];
        const cancelResult = readableStreamCancel(stream, compositeReason);
        cancelPromise.resolve(cancelResult);
      }
      return cancelPromise.promise;
    };
    const cancel2Algorithm = (reason) => {
      canceled2 = true;
      reason2 = reason;
      if (canceled1) {
        const compositeReason = [reason1, reason2];
        const cancelResult = readableStreamCancel(stream, compositeReason);
        cancelPromise.resolve(cancelResult);
      }
      return cancelPromise.promise;
    };
    const startAlgorithm = () => undefined;
    branch1 = createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancel1Algorithm,
    );
    branch2 = createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancel2Algorithm,
    );
    setPromiseIsHandledToTrue(
      reader[sym.closedPromise].promise.catch((r) => {
        readableStreamDefaultControllerError(
          branch1[
            sym.readableStreamController
          ],
          r,
        );
        readableStreamDefaultControllerError(
          branch2[
            sym.readableStreamController
          ],
          r,
        );
      }),
    );
    return [branch1, branch2];
  }

  function resetQueue(container) {
    assert(sym.queue in container && sym.queueTotalSize in container);
    container[sym.queue] = [];
    container[sym.queueTotalSize] = 0;
  }

  /** An internal function which mimics the behavior of setting the promise to
   * handled in JavaScript.  In this situation, an assertion failure, which
   * shouldn't happen will get thrown, instead of swallowed. */
  function setPromiseIsHandledToTrue(promise) {
    promise.then(undefined, (e) => {
      if (e && e instanceof AssertionError) {
        queueMicrotask(() => {
          throw e;
        });
      }
    });
  }

  function setUpReadableByteStreamController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    autoAllocateChunkSize,
  ) {
    assert(stream[sym.readableStreamController] === undefined);
    if (autoAllocateChunkSize !== undefined) {
      assert(Number.isInteger(autoAllocateChunkSize));
      assert(autoAllocateChunkSize >= 0);
    }
    controller[sym.controlledReadableByteStream] = stream;
    controller[sym.pulling] = controller[sym.pullAgain] = false;
    controller[sym.byobRequest] = undefined;
    controller[sym.queue] = [];
    controller[sym.queueTotalSize] = 0;
    controller[sym.closeRequested] = controller[sym.started] = false;
    controller[sym.strategyHWM] = validateAndNormalizeHighWaterMark(
      highWaterMark,
    );
    controller[sym.pullAlgorithm] = pullAlgorithm;
    controller[sym.cancelAlgorithm] = cancelAlgorithm;
    controller[sym.autoAllocateChunkSize] = autoAllocateChunkSize;
    // 3.13.26.12 Set controller.[[pendingPullIntos]] to a new empty List.
    stream[sym.readableStreamController] = controller;
    const startResult = startAlgorithm();
    const startPromise = Promise.resolve(startResult);
    setPromiseIsHandledToTrue(
      startPromise.then(
        () => {
          controller[sym.started] = true;
          assert(!controller[sym.pulling]);
          assert(!controller[sym.pullAgain]);
          readableByteStreamControllerCallPullIfNeeded(controller);
        },
        (r) => {
          readableByteStreamControllerError(controller, r);
        },
      ),
    );
  }

  function setUpReadableByteStreamControllerFromUnderlyingSource(
    stream,
    underlyingByteSource,
    highWaterMark,
  ) {
    assert(underlyingByteSource);
    const controller = Object.create(
      ReadableByteStreamController.prototype,
    );
    const startAlgorithm = () => {
      return invokeOrNoop(underlyingByteSource, "start", controller);
    };
    const pullAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingByteSource,
      "pull",
      0,
      controller,
    );
    setFunctionName(pullAlgorithm, "[[pullAlgorithm]]");
    const cancelAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingByteSource,
      "cancel",
      1,
    );
    setFunctionName(cancelAlgorithm, "[[cancelAlgorithm]]");
    // 3.13.27.6 Let autoAllocateChunkSize be ? GetV(underlyingByteSource, "autoAllocateChunkSize").
    const autoAllocateChunkSize = undefined;
    setUpReadableByteStreamController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      autoAllocateChunkSize,
    );
  }

  function setUpReadableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(stream[sym.readableStreamController] === undefined);
    controller[sym.controlledReadableStream] = stream;
    controller[sym.queue] = [];
    controller[sym.queueTotalSize] = 0;
    controller[sym.started] = controller[sym.closeRequested] = controller[
      sym.pullAgain
    ] = controller[sym.pulling] = false;
    controller[sym.strategySizeAlgorithm] = sizeAlgorithm;
    controller[sym.strategyHWM] = highWaterMark;
    controller[sym.pullAlgorithm] = pullAlgorithm;
    controller[sym.cancelAlgorithm] = cancelAlgorithm;
    stream[sym.readableStreamController] = controller;
    const startResult = startAlgorithm();
    const startPromise = Promise.resolve(startResult);
    setPromiseIsHandledToTrue(
      startPromise.then(
        () => {
          controller[sym.started] = true;
          assert(controller[sym.pulling] === false);
          assert(controller[sym.pullAgain] === false);
          readableStreamDefaultControllerCallPullIfNeeded(controller);
        },
        (r) => {
          readableStreamDefaultControllerError(controller, r);
        },
      ),
    );
  }

  function setUpReadableStreamDefaultControllerFromUnderlyingSource(
    stream,
    underlyingSource,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(underlyingSource);
    const controller = Object.create(
      ReadableStreamDefaultController.prototype,
    );
    const startAlgorithm = () =>
      invokeOrNoop(underlyingSource, "start", controller);
    const pullAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingSource,
      "pull",
      0,
      controller,
    );
    setFunctionName(pullAlgorithm, "[[pullAlgorithm]]");
    const cancelAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingSource,
      "cancel",
      1,
    );
    setFunctionName(cancelAlgorithm, "[[cancelAlgorithm]]");
    setUpReadableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm,
    );
  }

  function setUpTransformStreamDefaultController(
    stream,
    controller,
    transformAlgorithm,
    flushAlgorithm,
  ) {
    assert(isTransformStream(stream));
    assert(stream[sym.transformStreamController] === undefined);
    controller[sym.controlledTransformStream] = stream;
    stream[sym.transformStreamController] = controller;
    controller[sym.transformAlgorithm] = transformAlgorithm;
    controller[sym.flushAlgorithm] = flushAlgorithm;
  }

  function setUpTransformStreamDefaultControllerFromTransformer(
    stream,
    transformer,
  ) {
    assert(transformer);
    const controller = Object.create(
      TransformStreamDefaultController.prototype,
    );
    let transformAlgorithm = (chunk) => {
      try {
        transformStreamDefaultControllerEnqueue(
          controller,
          // it defaults to no tranformation, so I is assumed to be O
          chunk,
        );
      } catch (e) {
        return Promise.reject(e);
      }
      return Promise.resolve();
    };
    const transformMethod = transformer.transform;
    if (transformMethod) {
      if (typeof transformMethod !== "function") {
        throw new TypeError("tranformer.transform must be callable.");
      }
      // deno-lint-ignore require-await
      transformAlgorithm = async (chunk) =>
        call(transformMethod, transformer, [chunk, controller]);
    }
    const flushAlgorithm = createAlgorithmFromUnderlyingMethod(
      transformer,
      "flush",
      0,
      controller,
    );
    setUpTransformStreamDefaultController(
      stream,
      controller,
      transformAlgorithm,
      flushAlgorithm,
    );
  }

  function setUpWritableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(isWritableStream(stream));
    assert(stream[sym.writableStreamController] === undefined);
    controller[sym.controlledWritableStream] = stream;
    stream[sym.writableStreamController] = controller;
    controller[sym.queue] = [];
    controller[sym.queueTotalSize] = 0;
    controller[sym.started] = false;
    controller[sym.strategySizeAlgorithm] = sizeAlgorithm;
    controller[sym.strategyHWM] = highWaterMark;
    controller[sym.writeAlgorithm] = writeAlgorithm;
    controller[sym.closeAlgorithm] = closeAlgorithm;
    controller[sym.abortAlgorithm] = abortAlgorithm;
    const backpressure = writableStreamDefaultControllerGetBackpressure(
      controller,
    );
    writableStreamUpdateBackpressure(stream, backpressure);
    const startResult = startAlgorithm();
    const startPromise = Promise.resolve(startResult);
    setPromiseIsHandledToTrue(
      startPromise.then(
        () => {
          assert(
            stream[sym.state] === "writable" ||
              stream[sym.state] === "erroring",
          );
          controller[sym.started] = true;
          writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
        },
        (r) => {
          assert(
            stream[sym.state] === "writable" ||
              stream[sym.state] === "erroring",
          );
          controller[sym.started] = true;
          writableStreamDealWithRejection(stream, r);
        },
      ),
    );
  }

  function setUpWritableStreamDefaultControllerFromUnderlyingSink(
    stream,
    underlyingSink,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(underlyingSink);
    const controller = Object.create(
      WritableStreamDefaultController.prototype,
    );
    const startAlgorithm = () => {
      return invokeOrNoop(underlyingSink, "start", controller);
    };
    const writeAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingSink,
      "write",
      1,
      controller,
    );
    setFunctionName(writeAlgorithm, "[[writeAlgorithm]]");
    const closeAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingSink,
      "close",
      0,
    );
    setFunctionName(closeAlgorithm, "[[closeAlgorithm]]");
    const abortAlgorithm = createAlgorithmFromUnderlyingMethod(
      underlyingSink,
      "abort",
      1,
    );
    setFunctionName(abortAlgorithm, "[[abortAlgorithm]]");
    setUpWritableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      writeAlgorithm,
      closeAlgorithm,
      abortAlgorithm,
      highWaterMark,
      sizeAlgorithm,
    );
  }

  function transformStreamDefaultControllerClearAlgorithms(
    controller,
  ) {
    controller[sym.transformAlgorithm] = undefined;
    controller[sym.flushAlgorithm] = undefined;
  }

  function transformStreamDefaultControllerEnqueue(
    controller,
    chunk,
  ) {
    const stream = controller[sym.controlledTransformStream];
    const readableController = stream[sym.readable][
      sym.readableStreamController
    ];
    if (!readableStreamDefaultControllerCanCloseOrEnqueue(readableController)) {
      throw new TypeError(
        "TransformStream's readable controller cannot be closed or enqueued.",
      );
    }
    try {
      readableStreamDefaultControllerEnqueue(readableController, chunk);
    } catch (e) {
      transformStreamErrorWritableAndUnblockWrite(stream, e);
      throw stream[sym.readable][sym.storedError];
    }
    const backpressure = readableStreamDefaultControllerHasBackpressure(
      readableController,
    );
    if (backpressure) {
      transformStreamSetBackpressure(stream, true);
    }
  }

  function transformStreamDefaultControllerError(
    controller,
    e,
  ) {
    transformStreamError(controller[sym.controlledTransformStream], e);
  }

  function transformStreamDefaultControllerPerformTransform(
    controller,
    chunk,
  ) {
    const transformPromise = controller[sym.transformAlgorithm](chunk);
    return transformPromise.then(undefined, (r) => {
      transformStreamError(controller[sym.controlledTransformStream], r);
      throw r;
    });
  }

  function transformStreamDefaultSinkAbortAlgorithm(
    stream,
    reason,
  ) {
    transformStreamError(stream, reason);
    return Promise.resolve(undefined);
  }

  function transformStreamDefaultSinkCloseAlgorithm(
    stream,
  ) {
    const readable = stream[sym.readable];
    const controller = stream[sym.transformStreamController];
    const flushPromise = controller[sym.flushAlgorithm]();
    transformStreamDefaultControllerClearAlgorithms(controller);
    return flushPromise.then(
      () => {
        if (readable[sym.state] === "errored") {
          throw readable[sym.storedError];
        }
        const readableController = readable[
          sym.readableStreamController
        ];
        if (
          readableStreamDefaultControllerCanCloseOrEnqueue(readableController)
        ) {
          readableStreamDefaultControllerClose(readableController);
        }
      },
      (r) => {
        transformStreamError(stream, r);
        throw readable[sym.storedError];
      },
    );
  }

  function transformStreamDefaultSinkWriteAlgorithm(
    stream,
    chunk,
  ) {
    assert(stream[sym.writable][sym.state] === "writable");
    const controller = stream[sym.transformStreamController];
    if (stream[sym.backpressure]) {
      const backpressureChangePromise = stream[sym.backpressureChangePromise];
      assert(backpressureChangePromise);
      return backpressureChangePromise.promise.then(() => {
        const writable = stream[sym.writable];
        const state = writable[sym.state];
        if (state === "erroring") {
          throw writable[sym.storedError];
        }
        assert(state === "writable");
        return transformStreamDefaultControllerPerformTransform(
          controller,
          chunk,
        );
      });
    }
    return transformStreamDefaultControllerPerformTransform(controller, chunk);
  }

  function transformStreamDefaultSourcePullAlgorithm(
    stream,
  ) {
    assert(stream[sym.backpressure] === true);
    assert(stream[sym.backpressureChangePromise] !== undefined);
    transformStreamSetBackpressure(stream, false);
    return stream[sym.backpressureChangePromise].promise;
  }

  function transformStreamError(
    stream,
    e,
  ) {
    readableStreamDefaultControllerError(
      stream[sym.readable][
        sym.readableStreamController
      ],
      e,
    );
    transformStreamErrorWritableAndUnblockWrite(stream, e);
  }

  function transformStreamDefaultControllerTerminate(
    controller,
  ) {
    const stream = controller[sym.controlledTransformStream];
    const readableController = stream[sym.readable][
      sym.readableStreamController
    ];
    readableStreamDefaultControllerClose(readableController);
    const error = new TypeError("TransformStream is closed.");
    transformStreamErrorWritableAndUnblockWrite(stream, error);
  }

  function transformStreamErrorWritableAndUnblockWrite(
    stream,
    e,
  ) {
    transformStreamDefaultControllerClearAlgorithms(
      stream[sym.transformStreamController],
    );
    writableStreamDefaultControllerErrorIfNeeded(
      stream[sym.writable][sym.writableStreamController],
      e,
    );
    if (stream[sym.backpressure]) {
      transformStreamSetBackpressure(stream, false);
    }
  }

  function transformStreamSetBackpressure(
    stream,
    backpressure,
  ) {
    assert(stream[sym.backpressure] !== backpressure);
    if (stream[sym.backpressureChangePromise] !== undefined) {
      stream[sym.backpressureChangePromise].resolve(undefined);
    }
    stream[sym.backpressureChangePromise] = getDeferred();
    stream[sym.backpressure] = backpressure;
  }

  function transferArrayBuffer(buffer) {
    assert(!isDetachedBuffer(buffer));
    const transferredIshVersion = buffer.slice(0);

    Object.defineProperty(buffer, "byteLength", {
      get() {
        return 0;
      },
    });
    buffer[sym.isFakeDetached] = true;

    return transferredIshVersion;
  }

  function validateAndNormalizeHighWaterMark(
    highWaterMark,
  ) {
    highWaterMark = Number(highWaterMark);
    if (Number.isNaN(highWaterMark) || highWaterMark < 0) {
      throw new RangeError(
        `highWaterMark must be a positive number or Infinity.  Received: ${highWaterMark}.`,
      );
    }
    return highWaterMark;
  }

  function writableStreamAbort(
    stream,
    reason,
  ) {
    const state = stream[sym.state];
    if (state === "closed" || state === "errored") {
      return Promise.resolve(undefined);
    }
    if (stream[sym.pendingAbortRequest]) {
      return stream[sym.pendingAbortRequest].promise.promise;
    }
    assert(state === "writable" || state === "erroring");
    let wasAlreadyErroring = false;
    if (state === "erroring") {
      wasAlreadyErroring = true;
      reason = undefined;
    }
    const promise = getDeferred();
    stream[sym.pendingAbortRequest] = { promise, reason, wasAlreadyErroring };

    if (wasAlreadyErroring === false) {
      writableStreamStartErroring(stream, reason);
    }
    return promise.promise;
  }

  function writableStreamAddWriteRequest(
    stream,
  ) {
    assert(isWritableStream(stream));
    assert(stream[sym.state] === "writable");
    const promise = getDeferred();
    stream[sym.writeRequests].push(promise);
    return promise.promise;
  }

  function writableStreamClose(
    stream,
  ) {
    const state = stream[sym.state];
    if (state === "closed" || state === "errored") {
      return Promise.reject(
        new TypeError(
          "Cannot close an already closed or errored WritableStream.",
        ),
      );
    }
    assert(!writableStreamCloseQueuedOrInFlight(stream));
    const promise = getDeferred();
    stream[sym.closeRequest] = promise;
    const writer = stream[sym.writer];
    if (writer && stream[sym.backpressure] && state === "writable") {
      writer[sym.readyPromise].resolve();
      writer[sym.readyPromise].resolve = undefined;
      writer[sym.readyPromise].reject = undefined;
    }
    writableStreamDefaultControllerClose(stream[sym.writableStreamController]);
    return promise.promise;
  }

  function writableStreamCloseQueuedOrInFlight(
    stream,
  ) {
    return !(
      stream[sym.closeRequest] === undefined &&
      stream[sym.inFlightCloseRequest] === undefined
    );
  }

  function writableStreamDealWithRejection(
    stream,
    error,
  ) {
    const state = stream[sym.state];
    if (state === "writable") {
      writableStreamStartErroring(stream, error);
      return;
    }
    assert(state === "erroring");
    writableStreamFinishErroring(stream);
  }

  function writableStreamDefaultControllerAdvanceQueueIfNeeded(
    controller,
  ) {
    const stream = controller[sym.controlledWritableStream];
    if (!controller[sym.started]) {
      return;
    }
    if (stream[sym.inFlightWriteRequest]) {
      return;
    }
    const state = stream[sym.state];
    assert(state !== "closed" && state !== "errored");
    if (state === "erroring") {
      writableStreamFinishErroring(stream);
      return;
    }
    if (!controller[sym.queue].length) {
      return;
    }
    const writeRecord = peekQueueValue(controller);
    if (writeRecord === "close") {
      writableStreamDefaultControllerProcessClose(controller);
    } else {
      writableStreamDefaultControllerProcessWrite(
        controller,
        writeRecord.chunk,
      );
    }
  }

  function writableStreamDefaultControllerClearAlgorithms(
    controller,
  ) {
    controller[sym.writeAlgorithm] = undefined;
    controller[sym.closeAlgorithm] = undefined;
    controller[sym.abortAlgorithm] = undefined;
    controller[sym.strategySizeAlgorithm] = undefined;
  }

  function writableStreamDefaultControllerClose(
    controller,
  ) {
    enqueueValueWithSize(controller, "close", 0);
    writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
  }

  function writableStreamDefaultControllerError(
    controller,
    error,
  ) {
    const stream = controller[sym.controlledWritableStream];
    assert(stream[sym.state] === "writable");
    writableStreamDefaultControllerClearAlgorithms(controller);
    writableStreamStartErroring(stream, error);
  }

  function writableStreamDefaultControllerErrorIfNeeded(
    controller,
    error,
  ) {
    if (controller[sym.controlledWritableStream][sym.state] === "writable") {
      writableStreamDefaultControllerError(controller, error);
    }
  }

  function writableStreamDefaultControllerGetBackpressure(
    controller,
  ) {
    const desiredSize = writableStreamDefaultControllerGetDesiredSize(
      controller,
    );
    return desiredSize <= 0;
  }

  function writableStreamDefaultControllerGetChunkSize(
    controller,
    chunk,
  ) {
    let returnValue;
    try {
      returnValue = controller[sym.strategySizeAlgorithm](chunk);
    } catch (e) {
      writableStreamDefaultControllerErrorIfNeeded(controller, e);
      return 1;
    }
    return returnValue;
  }

  function writableStreamDefaultControllerGetDesiredSize(
    controller,
  ) {
    return controller[sym.strategyHWM] - controller[sym.queueTotalSize];
  }

  function writableStreamDefaultControllerProcessClose(
    controller,
  ) {
    const stream = controller[sym.controlledWritableStream];
    writableStreamMarkCloseRequestInFlight(stream);
    dequeueValue(controller);
    assert(controller[sym.queue].length === 0);
    const sinkClosePromise = controller[sym.closeAlgorithm]();
    writableStreamDefaultControllerClearAlgorithms(controller);
    setPromiseIsHandledToTrue(
      sinkClosePromise.then(
        () => {
          writableStreamFinishInFlightClose(stream);
        },
        (reason) => {
          writableStreamFinishInFlightCloseWithError(stream, reason);
        },
      ),
    );
  }

  function writableStreamDefaultControllerProcessWrite(
    controller,
    chunk,
  ) {
    const stream = controller[sym.controlledWritableStream];
    writableStreamMarkFirstWriteRequestInFlight(stream);
    const sinkWritePromise = controller[sym.writeAlgorithm](chunk);
    setPromiseIsHandledToTrue(
      sinkWritePromise.then(
        () => {
          writableStreamFinishInFlightWrite(stream);
          const state = stream[sym.state];
          assert(state === "writable" || state === "erroring");
          dequeueValue(controller);
          if (
            !writableStreamCloseQueuedOrInFlight(stream) &&
            state === "writable"
          ) {
            const backpressure = writableStreamDefaultControllerGetBackpressure(
              controller,
            );
            writableStreamUpdateBackpressure(stream, backpressure);
          }
          writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
        },
        (reason) => {
          if (stream[sym.state] === "writable") {
            writableStreamDefaultControllerClearAlgorithms(controller);
          }
          writableStreamFinishInFlightWriteWithError(stream, reason);
        },
      ),
    );
  }

  function writableStreamDefaultControllerWrite(
    controller,
    chunk,
    chunkSize,
  ) {
    const writeRecord = { chunk };
    try {
      enqueueValueWithSize(controller, writeRecord, chunkSize);
    } catch (e) {
      writableStreamDefaultControllerErrorIfNeeded(controller, e);
      return;
    }
    const stream = controller[sym.controlledWritableStream];
    if (
      !writableStreamCloseQueuedOrInFlight(stream) &&
      stream[sym.state] === "writable"
    ) {
      const backpressure = writableStreamDefaultControllerGetBackpressure(
        controller,
      );
      writableStreamUpdateBackpressure(stream, backpressure);
    }
    writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
  }

  function writableStreamDefaultWriterAbort(
    writer,
    reason,
  ) {
    const stream = writer[sym.ownerWritableStream];
    assert(stream);
    return writableStreamAbort(stream, reason);
  }

  function writableStreamDefaultWriterClose(
    writer,
  ) {
    const stream = writer[sym.ownerWritableStream];
    assert(stream);
    return writableStreamClose(stream);
  }

  function writableStreamDefaultWriterCloseWithErrorPropagation(
    writer,
  ) {
    const stream = writer[sym.ownerWritableStream];
    assert(stream);
    const state = stream[sym.state];
    if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
      return Promise.resolve();
    }
    if (state === "errored") {
      return Promise.reject(stream[sym.storedError]);
    }
    assert(state === "writable" || state === "erroring");
    return writableStreamDefaultWriterClose(writer);
  }

  function writableStreamDefaultWriterEnsureClosePromiseRejected(
    writer,
    error,
  ) {
    if (writer[sym.closedPromise].reject) {
      writer[sym.closedPromise].reject(error);
    } else {
      writer[sym.closedPromise] = {
        promise: Promise.reject(error),
      };
    }
    setPromiseIsHandledToTrue(writer[sym.closedPromise].promise);
  }

  function writableStreamDefaultWriterEnsureReadyPromiseRejected(
    writer,
    error,
  ) {
    if (writer[sym.readyPromise].reject) {
      writer[sym.readyPromise].reject(error);
      writer[sym.readyPromise].reject = undefined;
      writer[sym.readyPromise].resolve = undefined;
    } else {
      writer[sym.readyPromise] = {
        promise: Promise.reject(error),
      };
    }
    setPromiseIsHandledToTrue(writer[sym.readyPromise].promise);
  }

  function writableStreamDefaultWriterWrite(
    writer,
    chunk,
  ) {
    const stream = writer[sym.ownerWritableStream];
    assert(stream);
    const controller = stream[sym.writableStreamController];
    assert(controller);
    const chunkSize = writableStreamDefaultControllerGetChunkSize(
      controller,
      chunk,
    );
    if (stream !== writer[sym.ownerWritableStream]) {
      return Promise.reject("Writer has incorrect WritableStream.");
    }
    const state = stream[sym.state];
    if (state === "errored") {
      return Promise.reject(stream[sym.storedError]);
    }
    if (writableStreamCloseQueuedOrInFlight(stream) || state === "closed") {
      return Promise.reject(new TypeError("The stream is closed or closing."));
    }
    if (state === "erroring") {
      return Promise.reject(stream[sym.storedError]);
    }
    assert(state === "writable");
    const promise = writableStreamAddWriteRequest(stream);
    writableStreamDefaultControllerWrite(controller, chunk, chunkSize);
    return promise;
  }

  function writableStreamDefaultWriterGetDesiredSize(
    writer,
  ) {
    const stream = writer[sym.ownerWritableStream];
    const state = stream[sym.state];
    if (state === "errored" || state === "erroring") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return writableStreamDefaultControllerGetDesiredSize(
      stream[sym.writableStreamController],
    );
  }

  function writableStreamDefaultWriterRelease(
    writer,
  ) {
    const stream = writer[sym.ownerWritableStream];
    assert(stream);
    assert(stream[sym.writer] === writer);
    const releasedError = new TypeError(
      "Writer was released and can no longer be used to monitor the stream's closedness.",
    );
    writableStreamDefaultWriterEnsureReadyPromiseRejected(
      writer,
      releasedError,
    );
    writableStreamDefaultWriterEnsureClosePromiseRejected(
      writer,
      releasedError,
    );
    stream[sym.writer] = undefined;
    writer[sym.ownerWritableStream] = undefined;
  }

  function writableStreamFinishErroring(stream) {
    assert(stream[sym.state] === "erroring");
    assert(!writableStreamHasOperationMarkedInFlight(stream));
    stream[sym.state] = "errored";
    stream[sym.writableStreamController][sym.errorSteps]();
    const storedError = stream[sym.storedError];
    for (const writeRequest of stream[sym.writeRequests]) {
      assert(writeRequest.reject);
      writeRequest.reject(storedError);
    }
    stream[sym.writeRequests] = [];
    if (!stream[sym.pendingAbortRequest]) {
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      return;
    }
    const abortRequest = stream[sym.pendingAbortRequest];
    assert(abortRequest);
    stream[sym.pendingAbortRequest] = undefined;
    if (abortRequest.wasAlreadyErroring) {
      assert(abortRequest.promise.reject);
      abortRequest.promise.reject(storedError);
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      return;
    }
    const promise = stream[sym.writableStreamController][sym.abortSteps](
      abortRequest.reason,
    );
    setPromiseIsHandledToTrue(
      promise.then(
        () => {
          assert(abortRequest.promise.resolve);
          abortRequest.promise.resolve();
          writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
        },
        (reason) => {
          assert(abortRequest.promise.reject);
          abortRequest.promise.reject(reason);
          writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
        },
      ),
    );
  }

  function writableStreamFinishInFlightClose(
    stream,
  ) {
    assert(stream[sym.inFlightCloseRequest]);
    stream[sym.inFlightCloseRequest]?.resolve();
    stream[sym.inFlightCloseRequest] = undefined;
    const state = stream[sym.state];
    assert(state === "writable" || state === "erroring");
    if (state === "erroring") {
      stream[sym.storedError] = undefined;
      if (stream[sym.pendingAbortRequest]) {
        stream[sym.pendingAbortRequest].promise.resolve();
        stream[sym.pendingAbortRequest] = undefined;
      }
    }
    stream[sym.state] = "closed";
    const writer = stream[sym.writer];
    if (writer) {
      writer[sym.closedPromise].resolve();
    }
    assert(stream[sym.pendingAbortRequest] === undefined);
    assert(stream[sym.storedError] === undefined);
  }

  function writableStreamFinishInFlightCloseWithError(
    stream,
    error,
  ) {
    assert(stream[sym.inFlightCloseRequest]);
    stream[sym.inFlightCloseRequest]?.reject(error);
    stream[sym.inFlightCloseRequest] = undefined;
    assert(
      stream[sym.state] === "writable" || stream[sym.state] === "erroring",
    );
    if (stream[sym.pendingAbortRequest]) {
      stream[sym.pendingAbortRequest]?.promise.reject(error);
      stream[sym.pendingAbortRequest] = undefined;
    }
    writableStreamDealWithRejection(stream, error);
  }

  function writableStreamFinishInFlightWrite(
    stream,
  ) {
    assert(stream[sym.inFlightWriteRequest]);
    stream[sym.inFlightWriteRequest].resolve();
    stream[sym.inFlightWriteRequest] = undefined;
  }

  function writableStreamFinishInFlightWriteWithError(
    stream,
    error,
  ) {
    assert(stream[sym.inFlightWriteRequest]);
    stream[sym.inFlightWriteRequest].reject(error);
    stream[sym.inFlightWriteRequest] = undefined;
    assert(
      stream[sym.state] === "writable" || stream[sym.state] === "erroring",
    );
    writableStreamDealWithRejection(stream, error);
  }

  function writableStreamHasOperationMarkedInFlight(
    stream,
  ) {
    return !(
      stream[sym.inFlightWriteRequest] === undefined &&
      stream[sym.inFlightCloseRequest] === undefined
    );
  }

  function writableStreamMarkCloseRequestInFlight(
    stream,
  ) {
    assert(stream[sym.inFlightCloseRequest] === undefined);
    assert(stream[sym.closeRequest] !== undefined);
    stream[sym.inFlightCloseRequest] = stream[sym.closeRequest];
    stream[sym.closeRequest] = undefined;
  }

  function writableStreamMarkFirstWriteRequestInFlight(
    stream,
  ) {
    assert(stream[sym.inFlightWriteRequest] === undefined);
    assert(stream[sym.writeRequests].length);
    const writeRequest = stream[sym.writeRequests].shift();
    stream[sym.inFlightWriteRequest] = writeRequest;
  }

  function writableStreamRejectCloseAndClosedPromiseIfNeeded(
    stream,
  ) {
    assert(stream[sym.state] === "errored");
    if (stream[sym.closeRequest]) {
      assert(stream[sym.inFlightCloseRequest] === undefined);
      stream[sym.closeRequest].reject(stream[sym.storedError]);
      stream[sym.closeRequest] = undefined;
    }
    const writer = stream[sym.writer];
    if (writer) {
      writer[sym.closedPromise].reject(stream[sym.storedError]);
      setPromiseIsHandledToTrue(writer[sym.closedPromise].promise);
    }
  }

  function writableStreamStartErroring(
    stream,
    reason,
  ) {
    assert(stream[sym.storedError] === undefined);
    assert(stream[sym.state] === "writable");
    const controller = stream[sym.writableStreamController];
    assert(controller);
    stream[sym.state] = "erroring";
    stream[sym.storedError] = reason;
    const writer = stream[sym.writer];
    if (writer) {
      writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, reason);
    }
    if (
      !writableStreamHasOperationMarkedInFlight(stream) &&
      controller[sym.started]
    ) {
      writableStreamFinishErroring(stream);
    }
  }

  function writableStreamUpdateBackpressure(
    stream,
    backpressure,
  ) {
    assert(stream[sym.state] === "writable");
    assert(!writableStreamCloseQueuedOrInFlight(stream));
    const writer = stream[sym.writer];
    if (writer && backpressure !== stream[sym.backpressure]) {
      if (backpressure) {
        writer[sym.readyPromise] = getDeferred();
      } else {
        assert(backpressure === false);
        writer[sym.readyPromise].resolve();
        writer[sym.readyPromise].resolve = undefined;
        writer[sym.readyPromise].reject = undefined;
      }
    }
    stream[sym.backpressure] = backpressure;
  }

  class CountQueuingStrategy {
    constructor({ highWaterMark }) {
      this.highWaterMark = highWaterMark;
    }

    size() {
      return 1;
    }

    [customInspect]() {
      return `${this.constructor.name} { highWaterMark: ${
        String(this.highWaterMark)
      }, size: f }`;
    }
  }

  Object.defineProperty(CountQueuingStrategy.prototype, "size", {
    enumerable: true,
  });

  class ByteLengthQueuingStrategy {
    constructor({ highWaterMark }) {
      this.highWaterMark = highWaterMark;
    }

    size(chunk) {
      return chunk.byteLength;
    }

    [customInspect]() {
      return `${this.constructor.name} { highWaterMark: ${
        String(this.highWaterMark)
      }, size: f }`;
    }
  }

  Object.defineProperty(ByteLengthQueuingStrategy.prototype, "size", {
    enumerable: true,
  });

  window.__bootstrap.streams = {
    ReadableStream,
    TransformStream,
    WritableStream,
    isReadableStreamDisturbed,
    CountQueuingStrategy,
    ByteLengthQueuingStrategy,
  };
})(this);
