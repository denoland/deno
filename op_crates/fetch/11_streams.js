// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="./11_streams_types.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

((window) => {
  const customInspect = Symbol.for("Deno.customInspect");

  class AssertionError extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AssertionError";
    }
  }

  /**
   * @param {unknown} cond 
   * @param {string=} msg 
   * @returns {asserts cond}
   */
  function assert(cond, msg = "Assertion failed.") {
    if (!cond) {
      throw new AssertionError(msg);
    }
  }

  /** @template T */
  class Deferred {
    /** @type {Promise<T>} */
    #promise;
    /** @type {(reject?: any) => void} */
    #reject;
    /** @type {(value: T | PromiseLike<T>) => void} */
    #resolve;
    /** @type {"pending" | "fulfilled"} */
    #state = "pending";

    constructor() {
      this.#promise = new Promise((resolve, reject) => {
        this.#resolve = resolve;
        this.#reject = reject;
      });
    }

    /** @returns {Promise<T>} */
    get promise() {
      return this.#promise;
    }

    /** @returns {"pending" | "fulfilled"} */
    get state() {
      return this.#state;
    }

    /** @param {any=} reason */
    reject(reason) {
      if (this.#state !== "pending") {
        throw new TypeError("Deferred is already fulfilled.");
      }
      this.#state = "fulfilled";
      this.#reject(reason);
    }

    /** @param {T | PromiseLike<T>} value */
    resolve(value) {
      if (this.#state !== "pending") {
        throw new TypeError("Deferred is already fulfilled.");
      }
      this.#state = "fulfilled";
      this.#resolve(value);
    }
  }

  /**
   * @param {(...args: any[]) => any} fn 
   * @param {boolean} enforcePromise 
   * @returns {(...args: any[]) => any}
   */
  function reflectApply(fn, enforcePromise) {
    if (typeof fn !== "function") {
      throw new TypeError("The property must be a function.");
    }
    return function (...args) {
      if (enforcePromise) {
        try {
          return resolvePromiseWith(Reflect.apply(fn, this, args));
        } catch (err) {
          return Promise.reject(err);
        }
      }
      return Reflect.apply(fn, this, args);
    };
  }

  /**
   * @template I
   * @template O
   * @param {Transformer<I, O>} transformer 
   * @returns {Transformer<I, O>}
   */
  function convertTransformer(transformer) {
    const transformerDict = Object.create(null);
    if (transformer === null) {
      return transformerDict;
    }
    if ("flush" in transformer) {
      transformerDict.flush = reflectApply(transformer.flush, true);
    }
    if ("readableType" in transformer) {
      transformerDict.readableType = transformer.readableType;
    }
    if ("start" in transformer) {
      transformerDict.start = reflectApply(transformer.start, false);
    }
    if ("transform" in transformer) {
      transformerDict.transform = reflectApply(transformer.transform, true);
    }
    if ("writableType" in transformer) {
      transformerDict.writableType = transformer.writableType;
    }
    return transformerDict;
  }

  /**
   * @template W
   * @param {UnderlyingSink<W>} underlyingSink 
   * @returns {UnderlyingSink<W>}
   */
  function convertUnderlyingSink(underlyingSink) {
    const underlyingSinkDict = Object.create(null);
    if (underlyingSink === null) {
      return underlyingSinkDict;
    }
    if ("abort" in underlyingSink) {
      underlyingSinkDict.abort = reflectApply(underlyingSink.abort, true);
    }
    if ("close" in underlyingSink) {
      underlyingSinkDict.close = reflectApply(underlyingSink.close, true);
    }
    if ("start" in underlyingSink) {
      underlyingSinkDict.start = reflectApply(underlyingSink.start, false);
    }
    if (underlyingSink.type) {
      underlyingSinkDict.type = underlyingSink.type;
    }
    if ("write" in underlyingSink) {
      underlyingSinkDict.write = reflectApply(underlyingSink.write, true);
    }
    return underlyingSinkDict;
  }

  /**
   * @template R
   * @param {UnderlyingSource<R>} underlyingSource 
   * @returns {UnderlyingSource<R>}
   */
  function convertUnderlyingSource(underlyingSource) {
    const underlyingSourceDict = Object.create(null);
    if (underlyingSource === null) {
      throw new TypeError("Underlying source cannot be null");
    }
    if (underlyingSource === undefined) {
      return underlyingSourceDict;
    }
    if ("cancel" in underlyingSource) {
      underlyingSourceDict.cancel = reflectApply(underlyingSource.cancel, true);
    }
    if ("pull" in underlyingSource) {
      underlyingSourceDict.pull = reflectApply(underlyingSource.pull, true);
    }
    if ("start" in underlyingSource) {
      underlyingSourceDict.start = reflectApply(underlyingSource.start, false);
    }
    if (underlyingSource.type !== undefined) {
      if (underlyingSourceDict.type === null) {
        throw new TypeError("type cannot be null");
      }
      const type = String(underlyingSource.type);
      if (type !== "bytes") {
        throw new TypeError("invalid underlying source type");
      }
      underlyingSourceDict.type = type;
    }
    return underlyingSourceDict;
  }

  const originalPromise = Promise;
  const originalPromiseThen = Promise.prototype.then;

  /**
   * @template T
   * @template TResult1
   * @template TResult2
   * @param {Promise<T>} promise 
   * @param {(value: T) => TResult1 | PromiseLike<TResult1>} onFulfilled 
   * @param {(reason: any) => TResult2 | PromiseLike<TResult2>=} onRejected 
   * @returns {Promise<TResult1 | TResult2>}
   */
  function performPromiseThen(promise, onFulfilled, onRejected) {
    return originalPromiseThen.call(promise, onFulfilled, onRejected);
  }

  /**
   * @template T
   * @param {T | PromiseLike<T>} value 
   * @returns {Promise<T>}
   */
  function resolvePromiseWith(value) {
    return new originalPromise((resolve) => resolve(value));
  }

  /** @param {any} e */
  function rethrowAssertionErrorRejection(e) {
    if (e && e instanceof AssertionError) {
      queueMicrotask(() => {
        console.error(`Internal Error: ${e.stack}`);
      });
    }
  }

  /** @param {Promise<any>} promise */
  function setPromiseIsHandledToTrue(promise) {
    performPromiseThen(promise, undefined, rethrowAssertionErrorRejection);
  }

  /**
   * @template T
   * @template TResult1
   * @template TResult2
   * @param {Promise<T>} promise 
   * @param {(value: T) => TResult1 | PromiseLike<TResult1>} fulfillmentHandler 
   * @param {(reason: any) => TResult2 | PromiseLike<TResult2>=} rejectionHandler 
   * @returns {Promise<TResult1 | TResult2>}
   */
  function transformPromiseWith(promise, fulfillmentHandler, rejectionHandler) {
    return performPromiseThen(promise, fulfillmentHandler, rejectionHandler);
  }

  /**
   * @template T
   * @template TResult
   * @param {Promise<T>} promise 
   * @param {(value: T) => TResult | PromiseLike<TResult>} onFulfilled 
   * @returns {void}
   */
  function uponFulfillment(promise, onFulfilled) {
    uponPromise(promise, onFulfilled);
  }

  /**
   * @template T
   * @template TResult
   * @param {Promise<T>} promise 
   * @param {(value: T) => TResult | PromiseLike<TResult>} onRejected 
   * @returns {void}
   */
  function uponRejection(promise, onRejected) {
    uponPromise(promise, undefined, onRejected);
  }

  /**
   * @template T
   * @template TResult1
   * @template TResult2
   * @param {Promise<T>} promise 
   * @param {(value: T) => TResult1 | PromiseLike<TResult1>} onFulfilled 
   * @param {(reason: any) => TResult2 | PromiseLike<TResult2>=} onRejected 
   * @returns {void}
   */
  function uponPromise(promise, onFulfilled, onRejected) {
    performPromiseThen(
      performPromiseThen(promise, onFulfilled, onRejected),
      undefined,
      rethrowAssertionErrorRejection,
    );
  }

  const isFakeDetached = Symbol("<<detached>>");

  /**
   * @param {ArrayBufferLike} O
   * @returns {boolean}
   */
  function isDetachedBuffer(O) {
    return isFakeDetached in O;
  }

  /**
   * @param {ArrayBufferLike} O 
   * @returns {ArrayBufferLike}
   */
  function transferArrayBuffer(O) {
    assert(!isDetachedBuffer(O));
    const transferredIshVersion = O.slice(0);
    Object.defineProperty(O, "byteLength", {
      get() {
        return 0;
      },
    });
    O[isFakeDetached] = true;
    return transferredIshVersion;
  }

  const _abortAlgorithm = Symbol("[[abortAlgorithm]]");
  const _abortSteps = Symbol("[[AbortSteps]]");
  const _autoAllocateChunkSize = Symbol("[[autoAllocateChunkSize]]");
  const _backpressure = Symbol("[[backpressure]]");
  const _backpressureChangePromise = Symbol("[[backpressureChangePromise]]");
  const _byobRequest = Symbol("[[byobRequest]]");
  const _cancelAlgorithm = Symbol("[[cancelAlgorithm]]");
  const _cancelSteps = Symbol("[[CancelSteps]]");
  const _close = Symbol("close sentinel");
  const _closeAlgorithm = Symbol("[[closeAlgorithm]]");
  const _closedPromise = Symbol("[[closedPromise]]");
  const _closeRequest = Symbol("[[closeRequest]]");
  const _closeRequested = Symbol("[[closeRequested]]");
  const _controller = Symbol("[[controller]]");
  const _detached = Symbol("[[Detached]]");
  const _disturbed = Symbol("[[disturbed]]");
  const _errorSteps = Symbol("[[ErrorSteps]]");
  const _flushAlgorithm = Symbol("[[flushAlgorithm]]");
  const _globalObject = Symbol("[[globalObject]]");
  const _inFlightCloseRequest = Symbol("[[inFlightCloseRequest]]");
  const _inFlightWriteRequest = Symbol("[[inFlightWriteRequest]]");
  const _pendingAbortRequest = Symbol("[pendingAbortRequest]");
  const _preventCancel = Symbol("[[preventCancel]]");
  const _pullAgain = Symbol("[[pullAgain]]");
  const _pullAlgorithm = Symbol("[[pullAlgorithm]]");
  const _pulling = Symbol("[[pulling]]");
  const _pullSteps = Symbol("[[PullSteps]]");
  const _queue = Symbol("[[queue]]");
  const _queueTotalSize = Symbol("[[queueTotalSize]]");
  const _readable = Symbol("[[readable]]");
  const _reader = Symbol("[[reader]]");
  const _readRequests = Symbol("[[readRequests]]");
  const _readyPromise = Symbol("[[readyPromise]]");
  const _started = Symbol("[[started]]");
  const _state = Symbol("[[state]]");
  const _storedError = Symbol("[[storedError]]");
  const _strategyHWM = Symbol("[[strategyHWM]]");
  const _strategySizeAlgorithm = Symbol("[[strategySizeAlgorithm]]");
  const _stream = Symbol("[[stream]]");
  const _transformAlgorithm = Symbol("[[transformAlgorithm]]");
  const _writable = Symbol("[[writable]]");
  const _writeAlgorithm = Symbol("[[writeAlgorithm]]");
  const _writer = Symbol("[[writer]]");
  const _writeRequests = Symbol("[[writeRequests]]");

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @returns {ReadableStreamDefaultReader<R>}
   */
  function acquireReadableStreamDefaultReader(stream) {
    return new ReadableStreamDefaultReader(stream);
  }

  /**
   * @template W
   * @param {WritableStream<W>} stream 
   * @returns {WritableStreamDefaultWriter<W>}
   */
  function acquireWritableStreamDefaultWriter(stream) {
    return new WritableStreamDefaultWriter(stream);
  }

  /**
   * @template R
   * @param {() => void} startAlgorithm 
   * @param {() => Promise<void>} pullAlgorithm 
   * @param {(reason: any) => Promise<void>} cancelAlgorithm 
   * @param {number=} highWaterMark 
   * @param {((chunk: R) => number)=} sizeAlgorithm 
   * @returns {ReadableStream<R>}
   */
  function createReadableStream(
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark = 1,
    sizeAlgorithm = () => 1,
  ) {
    assert(isNonNegativeNumber(highWaterMark));
    /** @type {ReadableStream} */
    const stream = Object.create(ReadableStream.prototype);
    initializeReadableStream(stream);
    const controller = Object.create(ReadableStreamDefaultController.prototype);
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

  /**
   * @template W
   * @param {(controller: WritableStreamDefaultController<W>) => Promise<void>} startAlgorithm 
   * @param {(chunk: W) => Promise<void>} writeAlgorithm 
   * @param {() => Promise<void>} closeAlgorithm 
   * @param {(reason: any) => Promise<void>} abortAlgorithm 
   * @param {number} highWaterMark 
   * @param {(chunk: W) => number} sizeAlgorithm 
   * @returns {WritableStream<W>}
   */
  function createWritableStream(
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(isNonNegativeNumber(highWaterMark));
    const stream = Object.create(WritableStream.prototype);
    initializeWritableStream(stream);
    const controller = Object.create(WritableStreamDefaultController.prototype);
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

  /**
   * @template T
   * @param {{ [_queue]: Array<ValueWithSize<T>>, [_queueTotalSize]: number }} container
   * @returns {T}
   */
  function dequeueValue(container) {
    assert(_queue in container && _queueTotalSize in container);
    assert(container[_queue].length);
    const valueWithSize = container[_queue].shift();
    container[_queueTotalSize] -= valueWithSize.size;
    if (container[_queueTotalSize] < 0) {
      container[_queueTotalSize] = 0;
    }
    return valueWithSize.value;
  }

  /**
   * @template T
   * @param {{ [_queue]: Array<ValueWithSize<T | _close>>, [_queueTotalSize]: number }} container
   * @param {T} value
   * @param {number} size
   * @returns {void}
   */
  function enqueueValueWithSize(container, value, size) {
    assert(_queue in container && _queueTotalSize in container);
    if (isNonNegativeNumber(size) === false) {
      throw RangeError("chunk size isn't a positive number");
    }
    if (size === Infinity) {
      throw RangeError("chunk size is invalid");
    }
    container[_queue].push({ value, size });
    container[_queueTotalSize] += size;
  }

  /**
   * @param {QueuingStrategy} strategy 
   * @param {number} defaultHWM 
   */
  function extractHighWaterMark(strategy, defaultHWM) {
    if (!("highWaterMark" in strategy)) {
      return defaultHWM;
    }
    const highWaterMark = Number(strategy.highWaterMark);
    if (Number.isNaN(highWaterMark) || highWaterMark < 0) {
      throw RangeError(
        `Expected highWaterMark to be a positive number or Infinity, got "${highWaterMark}".`,
      );
    }
    return highWaterMark;
  }

  /**
   * @template T
   * @param {QueuingStrategy<T>} strategy 
   * @return {(chunk: T) => number}
   */
  function extractSizeAlgorithm(strategy) {
    const { size } = strategy;

    if (!size) {
      return () => 1;
    }
    return (chunk) => size(chunk);
  }

  /**
   * @param {ReadableStream} stream 
   * @returns {void}
   */
  function initializeReadableStream(stream) {
    stream[_state] = "readable";
    stream[_reader] = stream[_storedError] = undefined;
    stream[_disturbed] = false;
  }

  /**
   * @template I
   * @template O
   * @param {TransformStream<I, O>} stream 
   * @param {Deferred<void>} startPromise 
   * @param {number} writableHighWaterMark 
   * @param {(chunk: I) => number} writableSizeAlgorithm 
   * @param {number} readableHighWaterMark 
   * @param {(chunk: O) => number} readableSizeAlgorithm 
   */
  function initializeTransformStream(
    stream,
    startPromise,
    writableHighWaterMark,
    writableSizeAlgorithm,
    readableHighWaterMark,
    readableSizeAlgorithm,
  ) {
    function startAlgorithm() {
      return startPromise.promise;
    }

    function writeAlgorithm(chunk) {
      return transformStreamDefaultSinkWriteAlgorithm(stream, chunk);
    }

    function abortAlgorithm(reason) {
      return transformStreamDefaultSinkAbortAlgorithm(stream, reason);
    }

    function closeAlgorithm() {
      return transformStreamDefaultSinkCloseAlgorithm(stream);
    }

    stream[_writable] = createWritableStream(
      startAlgorithm,
      writeAlgorithm,
      closeAlgorithm,
      abortAlgorithm,
      writableHighWaterMark,
      writableSizeAlgorithm,
    );

    function pullAlgorithm() {
      return transformStreamDefaultSourcePullAlgorithm(stream);
    }

    function cancelAlgorithm(reason) {
      transformStreamErrorWritableAndUnblockWrite(stream, reason);
      return resolvePromiseWith(undefined);
    }

    stream[_readable] = createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      readableHighWaterMark,
      readableSizeAlgorithm,
    );

    stream[_backpressure] = stream[_backpressureChangePromise] = undefined;
    transformStreamSetBackpressure(stream, true);
    stream[_controller] = undefined;
  }

  /** @param {WritableStream} stream */
  function initializeWritableStream(stream) {
    stream[_state] = "writable";
    stream[_storedError] = stream[_writer] = stream[_controller] =
      stream[_inFlightWriteRequest] = stream[_closeRequest] =
        stream[_inFlightCloseRequest] = stream[_pendingAbortRequest] =
          undefined;
    stream[_writeRequests] = [];
    stream[_backpressure] = false;
  }

  /**
   * @param {unknown} v 
   * @returns {v is number}
   */
  function isNonNegativeNumber(v) {
    if (typeof v !== "number") {
      return false;
    }
    if (Number.isNaN(v)) {
      return false;
    }
    if (v < 0) {
      return false;
    }
    return true;
  }

  /**
   * @param {unknown} value 
   * @returns {value is ReadableStream}
   */
  function isReadableStream(value) {
    return !(typeof value !== "object" || value === null ||
      !(_controller in value));
  }

  /**
   * @param {ReadableStream} stream
   * @returns {boolean}
   */
  function isReadableStreamLocked(stream) {
    if (stream[_reader] === undefined) {
      return false;
    }
    return true;
  }

  /**
   * @param {unknown} value 
   * @returns {value is ReadableStreamDefaultReader}
   */
  function isReadableStreamDefaultReader(value) {
    return !(typeof value !== "object" || value === null ||
      !(_readRequests in value));
  }

  /**
   * @param {ReadableStream} stream 
   * @returns {boolean}
   */
  function isReadableStreamDisturbed(stream) {
    assert(isReadableStream(stream));
    return stream[_disturbed];
  }

  /**
   * @param {unknown} value 
   * @returns {value is WritableStream}
   */
  function isWritableStream(value) {
    return !(typeof value !== "object" || value === null ||
      !(_controller in value));
  }

  /**
   * @param {WritableStream} stream 
   * @returns {boolean}
   */
  function isWritableStreamLocked(stream) {
    if (stream[_writer] === undefined) {
      return false;
    }
    return true;
  }

  /**
   * @template T
   * @param {{ [_queue]: Array<ValueWithSize<T | _close>>, [_queueTotalSize]: number }} container
   * @returns {T | _close}
   */
  function peekQueueValue(container) {
    assert(_queue in container && _queueTotalSize in container);
    assert(container[_queue].length);
    const valueWithSize = container[_queue][0];
    return valueWithSize.value;
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {void}
   */
  function readableByteStreamControllerCallPullIfNeeded(controller) {
    const shouldPull = readableByteStreamControllerShouldCallPull(controller);
    if (!shouldPull) {
      return;
    }
    if (controller[_pulling]) {
      controller[_pullAgain] = true;
      return;
    }
    assert(controller[_pullAgain] === false);
    controller[_pulling] = true;
    /** @type {Promise<void>} */
    const pullPromise = controller[_pullAlgorithm](controller);
    setPromiseIsHandledToTrue(
      pullPromise.then(
        () => {
          controller[_pulling] = false;
          if (controller[_pullAgain]) {
            controller[_pullAgain] = false;
            readableByteStreamControllerCallPullIfNeeded(controller);
          }
        },
        (e) => {
          readableByteStreamControllerError(controller, e);
        },
      ),
    );
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {void}
   */
  function readableByteStreamControllerClearAlgorithms(controller) {
    controller[_pullAlgorithm] = undefined;
    controller[_cancelAlgorithm] = undefined;
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @param {any} e 
   */
  function readableByteStreamControllerError(controller, e) {
    /** @type {ReadableStream<ArrayBuffer>} */
    const stream = controller[_stream];
    if (stream[_state] !== "readable") {
      return;
    }
    // 3. Perform ! ReadableByteStreamControllerClearPendingPullIntos(controller).
    resetQueue(controller);
    readableByteStreamControllerClearAlgorithms(controller);
    readableStreamError(stream, e);
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {void}
   */
  function readableByteStreamControllerClose(controller) {
    /** @type {ReadableStream<ArrayBuffer>} */
    const stream = controller[_stream];
    if (controller[_closeRequested] || stream[_state] !== "readable") {
      return;
    }
    if (controller[_queueTotalSize] > 0) {
      controller[_closeRequested] = true;
      return;
    }
    // 3.13.6.4 If controller.[[pendingPullIntos]] is not empty, (BYOB Support)
    readableByteStreamControllerClearAlgorithms(controller);
    readableStreamClose(stream);
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @param {ArrayBufferView} chunk 
   */
  function readableByteStreamControllerEnqueue(controller, chunk) {
    /** @type {ReadableStream<ArrayBuffer>} */
    const stream = controller[_stream];
    if (
      controller[_closeRequested] ||
      controller[_stream][_state] !== "readable"
    ) {
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
        assert(controller[_queue].length === 0);
        const transferredView = new Uint8Array(
          transferredBuffer,
          byteOffset,
          byteLength,
        );
        readableStreamFulfillReadRequest(stream, transferredView, false);
      }
      // 8 Otherwise, if ! ReadableStreamHasBYOBReader(stream) is true,
    } else {
      assert(isReadableStreamLocked(stream) === false);
      readableByteStreamControllerEnqueueChunkToQueue(
        controller,
        transferredBuffer,
        byteOffset,
        byteLength,
      );
    }
    readableByteStreamControllerCallPullIfNeeded(controller);
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @param {ArrayBufferLike} buffer 
   * @param {number} byteOffset 
   * @param {number} byteLength 
   * @returns {void}
   */
  function readableByteStreamControllerEnqueueChunkToQueue(
    controller,
    buffer,
    byteOffset,
    byteLength,
  ) {
    controller[_queue].push({ buffer, byteOffset, byteLength });
    controller[_queueTotalSize] += byteLength;
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {number | null}
   */
  function readableByteStreamControllerGetDesiredSize(controller) {
    const state = controller[_stream][_state];
    if (state === "errored") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return controller[_strategyHWM] - controller[_queueTotalSize];
  }

  /**
   * @param {{ [_queue]: any[], [_queueTotalSize]: number }} container 
   * @returns {void}
   */
  function resetQueue(container) {
    container[_queue] = [];
    container[_queueTotalSize] = 0;
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {void}
   */
  function readableByteStreamControllerHandleQueueDrain(controller) {
    assert(controller[_stream][_state] === "readable");
    if (
      controller[_queueTotalSize] === 0 && controller[_closeRequested]
    ) {
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamClose(controller[_stream]);
    } else {
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
  }

  /**
   * @param {ReadableByteStreamController} controller 
   * @returns {boolean}
   */
  function readableByteStreamControllerShouldCallPull(controller) {
    /** @type {ReadableStream<ArrayBuffer>} */
    const stream = controller[_stream];
    if (
      stream[_state] !== "readable" ||
      controller[_closeRequested] ||
      !controller[_started]
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

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {ReadRequest<R>} readRequest
   * @returns {void}
   */
  function readableStreamAddReadRequest(stream, readRequest) {
    assert(isReadableStreamDefaultReader(stream[_reader]));
    assert(stream[_state] === "readable");
    stream[_reader][_readRequests].push(readRequest);
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {any=} reason 
   * @returns {Promise<void>}
   */
  function readableStreamCancel(stream, reason) {
    stream[_disturbed] = true;
    if (stream[_state] === "closed") {
      return Promise.resolve(undefined);
    }
    if (stream[_state] === "errored") {
      return Promise.reject(stream[_storedError]);
    }
    readableStreamClose(stream);
    /** @type {Promise<void>} */
    const sourceCancelPromise = stream[_controller][_cancelSteps](reason);
    return sourceCancelPromise.then(() => undefined);
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @returns {void}
   */
  function readableStreamClose(stream) {
    assert(stream[_state] === "readable");
    stream[_state] = "closed";
    /** @type {ReadableStreamDefaultReader<R> | undefined} */
    const reader = stream[_reader];
    if (!reader) {
      return;
    }
    if (isReadableStreamDefaultReader(reader)) {
      /** @type {Array<ReadRequest<R>>} */
      const readRequests = reader[_readRequests];
      for (const readRequest of readRequests) {
        readRequest.closeSteps();
      }
      reader[_readRequests] = [];
    }
    // this is non-spec, but when using an async iterator to read, and there is
    // a pending read request, the `closeSteps()` will resolve the close promise
    // causing a promise to be already resolved here.
    if (reader[_closedPromise].state === "pending") {
      reader[_closedPromise].resolve(undefined);
    }
  }

  /** @param {ReadableStreamDefaultController<any>} controller */
  function readableStreamDefaultControllerCallPullIfNeeded(controller) {
    const shouldPull = readableStreamDefaultcontrollerShouldCallPull(
      controller,
    );
    if (shouldPull === false) {
      return;
    }
    if (controller[_pulling] === true) {
      controller[_pullAgain] = true;
      return;
    }
    assert(controller[_pullAgain] === false);
    controller[_pulling] = true;
    const pullPromise = controller[_pullAlgorithm](controller);
    uponFulfillment(pullPromise, () => {
      controller[_pulling] = false;
      if (controller[_pullAgain] === true) {
        controller[_pullAgain] = false;
        readableStreamDefaultControllerCallPullIfNeeded(controller);
      }
    });
    uponRejection(pullPromise, (e) => {
      readableStreamDefaultControllerError(controller, e);
    });
  }

  /**
   * @param {ReadableStreamDefaultController<any>} controller 
   * @returns {boolean}
   */
  function readableStreamDefaultControllerCanCloseOrEnqueue(controller) {
    const state = controller[_stream][_state];
    if (controller[_closeRequested] === false && state === "readable") {
      return true;
    } else {
      return false;
    }
  }

  /** @param {ReadableStreamDefaultController<any>} controller */
  function readableStreamDefaultControllerClearAlgorithms(controller) {
    controller[_pullAlgorithm] = undefined;
    controller[_cancelAlgorithm] = undefined;
    controller[_strategySizeAlgorithm] = undefined;
  }

  /** @param {ReadableStreamDefaultController<any>} controller */
  function readableStreamDefaultControllerClose(controller) {
    if (
      readableStreamDefaultControllerCanCloseOrEnqueue(controller) === false
    ) {
      return;
    }
    const stream = controller[_stream];
    controller[_closeRequested] = true;
    if (controller[_queue].length === 0) {
      readableStreamDefaultControllerClearAlgorithms(controller);
      readableStreamClose(stream);
    }
  }

  /**
   * @template R
   * @param {ReadableStreamDefaultController<R>} controller 
   * @param {R} chunk 
   * @returns {void}
   */
  function readableStreamDefaultControllerEnqueue(controller, chunk) {
    if (
      readableStreamDefaultControllerCanCloseOrEnqueue(controller) === false
    ) {
      return;
    }
    const stream = controller[_stream];
    if (
      isReadableStreamLocked(stream) === true &&
      readableStreamGetNumReadRequests(stream) > 0
    ) {
      readableStreamFulfillReadRequest(stream, chunk, false);
    } else {
      let chunkSize;
      try {
        chunkSize = controller[_strategySizeAlgorithm](chunk);
      } catch (e) {
        readableStreamDefaultControllerError(controller, e);
        throw e;
      }

      try {
        enqueueValueWithSize(controller, chunk, chunkSize);
      } catch (e) {
        readableStreamDefaultControllerError(controller, e);
        throw e;
      }
    }
    readableStreamDefaultControllerCallPullIfNeeded(controller);
  }

  /**
   * @param {ReadableStreamDefaultController<any>} controller 
   * @param {any} e 
   */
  function readableStreamDefaultControllerError(controller, e) {
    const stream = controller[_stream];
    if (stream[_state] !== "readable") {
      return;
    }
    resetQueue(controller);
    readableStreamDefaultControllerClearAlgorithms(controller);
    readableStreamError(stream, e);
  }

  /**
   * @param {ReadableStreamDefaultController<any>} controller 
   * @returns {number | null}
   */
  function readableStreamDefaultControllerGetDesiredSize(controller) {
    const state = controller[_stream][_state];
    if (state === "errored") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return controller[_strategyHWM] - controller[_queueTotalSize];
  }

  /** @param {ReadableStreamDefaultController} controller */
  function readableStreamDefaultcontrollerHasBackpressure(controller) {
    if (readableStreamDefaultcontrollerShouldCallPull(controller) === true) {
      return false;
    } else {
      return true;
    }
  }

  /**
   * @param {ReadableStreamDefaultController<any>} controller 
   * @returns {boolean}
   */
  function readableStreamDefaultcontrollerShouldCallPull(controller) {
    const stream = controller[_stream];
    if (
      readableStreamDefaultControllerCanCloseOrEnqueue(controller) === false
    ) {
      return false;
    }
    if (controller[_started] === false) {
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
    if (desiredSize > 0) {
      return true;
    }
    return false;
  }

  /**
   * @template R
   * @param {ReadableStreamDefaultReader<R>} reader
   * @param {ReadRequest<R>} readRequest
   * @returns {void}
   */
  function readableStreamDefaultReaderRead(reader, readRequest) {
    const stream = reader[_stream];
    assert(stream);
    stream[_disturbed] = true;
    if (stream[_state] === "closed") {
      readRequest.closeSteps();
    } else if (stream[_state] === "errored") {
      readRequest.errorSteps(stream[_storedError]);
    } else {
      assert(stream[_state] === "readable");
      stream[_controller][_pullSteps](readRequest);
    }
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {any} e 
   */
  function readableStreamError(stream, e) {
    assert(stream[_state] === "readable");
    stream[_state] = "errored";
    stream[_storedError] = e;
    /** @type {ReadableStreamDefaultReader<R> | undefined} */
    const reader = stream[_reader];
    if (reader === undefined) {
      return;
    }
    if (isReadableStreamDefaultReader(reader)) {
      /** @type {Array<ReadRequest<R>>} */
      const readRequests = reader[_readRequests];
      for (const readRequest of readRequests) {
        readRequest.errorSteps(e);
      }
      reader[_readRequests] = [];
    }
    // 3.5.6.8 Otherwise, support BYOB Reader
    /** @type {Deferred<void>} */
    const closedPromise = reader[_closedPromise];
    console.log("closedPromise rejected");
    closedPromise.reject(e);
    setPromiseIsHandledToTrue(closedPromise.promise);
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {R} chunk 
   * @param {boolean} done 
   */
  function readableStreamFulfillReadRequest(stream, chunk, done) {
    assert(readableStreamHasDefaultReader(stream) === true);
    /** @type {ReadableStreamDefaultReader<R>} */
    const reader = stream[_reader];
    assert(reader[_readRequests].length);
    /** @type {ReadRequest<R>} */
    const readRequest = reader[_readRequests].shift();
    if (done) {
      readRequest.closeSteps();
    } else {
      readRequest.chunkSteps(chunk);
    }
  }

  /**
   * @param {ReadableStream} stream 
   * @return {number}
   */
  function readableStreamGetNumReadRequests(stream) {
    assert(readableStreamHasDefaultReader(stream) === true);
    return stream[_reader][_readRequests].length;
  }

  /**
   * @param {ReadableStream} stream
   * @returns {boolean}
   */
  function readableStreamHasDefaultReader(stream) {
    const reader = stream[_reader];
    if (reader === undefined) {
      return false;
    }
    if (isReadableStreamDefaultReader(reader)) {
      return true;
    }
    return false;
  }

  /**
   * @template T
   * @param {ReadableStream<T>} source 
   * @param {WritableStream<T>} dest 
   * @param {boolean} preventClose 
   * @param {boolean} preventAbort 
   * @param {boolean} preventCancel 
   * @param {AbortSignal=} signal 
   * @returns {Promise<void>}
   */
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
      typeof preventClose === "boolean" && typeof preventAbort === "boolean" &&
        typeof preventCancel === "boolean",
    );
    assert(signal === undefined || signal instanceof AbortSignal);
    assert(!isReadableStreamLocked(source));
    assert(!isWritableStreamLocked(dest));
    const reader = acquireReadableStreamDefaultReader(source);
    const writer = acquireWritableStreamDefaultWriter(dest);
    source[_disturbed] = true;
    let shuttingDown = false;
    let currentWrite = Promise.resolve(undefined);
    /** @type {Deferred<void>} */
    const promise = new Deferred();
    /** @type {() => void} */
    let abortAlgorithm;
    if (signal) {
      abortAlgorithm = () => {
        const error = new DOMException("Aborted", "AbortError");
        /** @type {Array<() => Promise<void>>} */
        const actions = [];
        if (preventAbort === false) {
          actions.push(() => {
            if (dest[_state] === "writable") {
              return writableStreamAbort(dest, error);
            } else {
              return Promise.resolve(undefined);
            }
          });
        }
        if (preventCancel === false) {
          actions.push(() => {
            if (source[_state] === "readable") {
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

    function pipeLoop() {
      return new Promise((resolveLoop, rejectLoop) => {
        /** @param {boolean} done */
        function next(done) {
          if (done) {
            resolveLoop();
          } else {
            uponPromise(pipeStep(), next, rejectLoop);
          }
        }
        next(false);
      });
    }

    /** @returns {Promise<boolean>} */
    function pipeStep() {
      if (shuttingDown === true) {
        return Promise.resolve(true);
      }

      return transformPromiseWith(writer[_readyPromise].promise, () => {
        return new Promise((resolveRead, rejectRead) => {
          readableStreamDefaultReaderRead(
            reader,
            {
              chunkSteps(chunk) {
                currentWrite = transformPromiseWith(
                  writableStreamDefaultWriterWrite(writer, chunk),
                  undefined,
                  () => {},
                );
                resolveRead(false);
              },
              closeSteps() {
                resolveRead(true);
              },
              errorSteps: rejectRead,
            },
          );
        });
      });
    }

    isOrBecomesErrored(
      source,
      reader[_closedPromise].promise,
      (storedError) => {
        if (preventAbort === false) {
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

    isOrBecomesErrored(dest, writer[_closedPromise].promise, (storedError) => {
      if (preventCancel === false) {
        shutdownWithAction(
          () => readableStreamCancel(source, storedError),
          true,
          storedError,
        );
      } else {
        shutdown(true, storedError);
      }
    });

    isOrBecomesClosed(source, reader[_closedPromise].promise, () => {
      if (preventClose === false) {
        shutdownWithAction(() =>
          writableStreamDefaultWriterCloseWithErrorPropagation(writer)
        );
      } else {
        shutdown();
      }
    });

    if (
      writableStreamCloseQueuedOrInFlight(dest) === true ||
      dest[_state] === "closed"
    ) {
      const destClosed = new TypeError(
        "The destination writable stream closed before all the data could be piped to it.",
      );
      if (preventCancel === false) {
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

    /** @returns {Promise<void>} */
    function waitForWritesToFinish() {
      const oldCurrentWrite = currentWrite;
      return transformPromiseWith(
        currentWrite,
        () =>
          oldCurrentWrite !== currentWrite
            ? waitForWritesToFinish()
            : undefined,
      );
    }

    /**
     * @param {ReadableStream | WritableStream} stream 
     * @param {Promise<any>} promise 
     * @param {(e: any) => void} action 
     */
    function isOrBecomesErrored(stream, promise, action) {
      if (stream[_state] === "errored") {
        action(stream[_storedError]);
      } else {
        uponRejection(promise, action);
      }
    }

    /**
     * @param {ReadableStream} stream 
     * @param {Promise<any>} promise 
     * @param {() => void} action 
     */
    function isOrBecomesClosed(stream, promise, action) {
      if (stream[_state] === "closed") {
        action();
      } else {
        uponFulfillment(promise, action);
      }
    }

    /**
     * @param {() => Promise<void[] | void>} action 
     * @param {boolean=} originalIsError 
     * @param {any=} originalError 
     */
    function shutdownWithAction(action, originalIsError, originalError) {
      function doTheRest() {
        uponPromise(
          action(),
          () => finalize(originalIsError, originalError),
          (newError) => finalize(true, newError),
        );
      }

      if (shuttingDown === true) {
        return;
      }
      shuttingDown = true;

      if (
        dest[_state] === "writable" &&
        writableStreamCloseQueuedOrInFlight(dest) === false
      ) {
        uponFulfillment(waitForWritesToFinish(), doTheRest);
      } else {
        doTheRest();
      }
    }

    /**
     * @param {boolean=} isError 
     * @param {any=} error 
     */
    function shutdown(isError, error) {
      if (shuttingDown) {
        return;
      }
      shuttingDown = true;
      if (
        dest[_state] === "writable" &&
        writableStreamCloseQueuedOrInFlight(dest) === false
      ) {
        uponFulfillment(
          waitForWritesToFinish(),
          () => finalize(isError, error),
        );
      } else {
        finalize(isError, error);
      }
    }

    /**
     * @param {boolean=} isError 
     * @param {any=} error 
     */
    function finalize(isError, error) {
      writableStreamDefaultWriterRelease(writer);
      readableStreamReaderGenericRelease(reader);

      if (signal !== undefined) {
        signal.removeEventListener("abort", abortAlgorithm);
      }
      if (isError) {
        promise.reject(error);
      } else {
        promise.resolve(undefined);
      }
    }
  }

  /**
   * @param {ReadableStreamGenericReader<any>} reader 
   * @param {any} reason 
   * @returns {Promise<void>}
   */
  function readableStreamReaderGenericCancel(reader, reason) {
    const stream = reader[_stream];
    assert(stream !== undefined);
    return readableStreamCancel(stream, reason);
  }

  /**
   * @template R
   * @param {ReadableStreamDefaultReader<R>} reader 
   * @param {ReadableStream<R>} stream 
   */
  function readableStreamReaderGenericInitialize(reader, stream) {
    reader[_stream] = stream;
    stream[_reader] = reader;
    if (stream[_state] === "readable") {
      reader[_closedPromise] = new Deferred();
    } else if (stream[_state] === "closed") {
      reader[_closedPromise] = new Deferred();
      reader[_closedPromise].resolve(undefined);
    } else {
      assert(stream[_state] === "errored");
      reader[_closedPromise] = new Deferred();
      reader[_closedPromise].reject(stream[_storedError]);
      setPromiseIsHandledToTrue(reader[_closedPromise].promise);
    }
  }

  /**
   * @template R
   * @param {ReadableStreamGenericReader<R>} reader 
   */
  function readableStreamReaderGenericRelease(reader) {
    assert(reader[_stream] !== undefined);
    assert(reader[_stream][_reader] === reader);
    if (reader[_stream][_state] === "readable") {
      reader[_closedPromise].reject(
        new TypeError(
          "Reader was released and can no longer be used to monitor the stream's closedness.",
        ),
      );
    } else {
      reader[_closedPromise] = new Deferred();
      reader[_closedPromise].reject(
        new TypeError(
          "Reader was released and can no longer be used to monitor the stream's closedness.",
        ),
      );
    }
    setPromiseIsHandledToTrue(reader[_closedPromise].promise);
    reader[_stream][_reader] = undefined;
    reader[_stream] = undefined;
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {boolean} cloneForBranch2 
   * @returns {[ReadableStream<R>, ReadableStream<R>]}
   */
  function readableStreamTee(stream, cloneForBranch2) {
    assert(isReadableStream(stream));
    assert(typeof cloneForBranch2 === "boolean");
    const reader = acquireReadableStreamDefaultReader(stream);
    let reading = false;
    let canceled1 = false;
    let canceled2 = false;
    /** @type {any} */
    let reason1;
    /** @type {any} */
    let reason2;
    /** @type {ReadableStream<R>} */
    // deno-lint-ignore prefer-const
    let branch1;
    /** @type {ReadableStream<R>} */
    // deno-lint-ignore prefer-const
    let branch2;

    /** @type {Deferred<void>} */
    const cancelPromise = new Deferred();

    function pullAlgorithm() {
      if (reading === true) {
        return Promise.resolve(undefined);
      }
      reading = true;
      /** @type {ReadRequest<R>} */
      const readRequest = {
        chunkSteps(value) {
          queueMicrotask(() => {
            reading = false;
            const value1 = value;
            const value2 = value;

            if (canceled1 === false) {
              readableStreamDefaultControllerEnqueue(
                /** @type {ReadableStreamDefaultController<any>} */ (branch1[
                  _controller
                ]),
                value1,
              );
            }
            if (canceled2 === false) {
              readableStreamDefaultControllerEnqueue(
                /** @type {ReadableStreamDefaultController<any>} */ (branch2[
                  _controller
                ]),
                value2,
              );
            }
          });
        },
        closeSteps() {
          reading = false;
          if (canceled1 === false) {
            readableStreamDefaultControllerClose(
              /** @type {ReadableStreamDefaultController<any>} */ (branch1[
                _controller
              ]),
            );
          }
          if (canceled2 === false) {
            readableStreamDefaultControllerClose(
              /** @type {ReadableStreamDefaultController<any>} */ (branch2[
                _controller
              ]),
            );
          }
          cancelPromise.resolve(undefined);
        },
        errorSteps() {
          reading = false;
        },
      };
      readableStreamDefaultReaderRead(reader, readRequest);
      return Promise.resolve(undefined);
    }

    /**
     * @param {any} reason
     * @returns {Promise<void>}
     */
    function cancel1Algorithm(reason) {
      canceled1 = true;
      reason1 = reason;
      if (canceled2 === true) {
        const compositeReason = [reason1, reason2];
        const cancelResult = readableStreamCancel(stream, compositeReason);
        cancelPromise.resolve(cancelResult);
      }
      return cancelPromise.promise;
    }

    /**
     * @param {any} reason
     * @returns {Promise<void>}
     */
    function cancel2Algorithm(reason) {
      canceled2 = true;
      reason2 = reason;
      if (canceled1 === true) {
        const compositeReason = [reason1, reason2];
        const cancelResult = readableStreamCancel(stream, compositeReason);
        cancelPromise.resolve(cancelResult);
      }
      return cancelPromise.promise;
    }

    function startAlgorithm() {}

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

    uponRejection(reader[_closedPromise].promise, (r) => {
      readableStreamDefaultControllerError(
        /** @type {ReadableStreamDefaultController<any>} */ (branch1[
          _controller
        ]),
        r,
      );
      readableStreamDefaultControllerError(
        /** @type {ReadableStreamDefaultController<any>} */ (branch2[
          _controller
        ]),
        r,
      );
      cancelPromise.resolve(undefined);
    });

    return [branch1, branch2];
  }

  /**
   * @param {ReadableStream<ArrayBuffer>} stream 
   * @param {ReadableByteStreamController} controller 
   * @param {() => void} startAlgorithm 
   * @param {() => Promise<void>} pullAlgorithm 
   * @param {(reason: any) => Promise<void>} cancelAlgorithm 
   * @param {number} highWaterMark 
   * @param {number | undefined} autoAllocateChunkSize 
   */
  function setUpReadableByteStreamController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    autoAllocateChunkSize,
  ) {
    assert(stream[_controller] === undefined);
    if (autoAllocateChunkSize !== undefined) {
      assert(Number.isInteger(autoAllocateChunkSize));
      assert(autoAllocateChunkSize >= 0);
    }
    controller[_stream] = stream;
    controller[_pullAgain] = controller[_pulling] = false;
    controller[_byobRequest] = undefined;
    resetQueue(controller);
    controller[_closeRequested] = controller[_started] = false;
    controller[_strategyHWM] = highWaterMark;
    controller[_pullAlgorithm] = pullAlgorithm;
    controller[_cancelAlgorithm] = cancelAlgorithm;
    controller[_autoAllocateChunkSize] = autoAllocateChunkSize;
    // 12. Set controller.[[pendingPullIntos]] to a new empty list.
    stream[_controller] = controller;
    const startResult = startAlgorithm();
    const startPromise = Promise.resolve(startResult);
    setPromiseIsHandledToTrue(
      startPromise.then(
        () => {
          controller[_started] = true;
          assert(controller[_pulling] === false);
          assert(controller[_pullAgain] === false);
          readableByteStreamControllerCallPullIfNeeded(controller);
        },
        (r) => {
          readableByteStreamControllerError(controller, r);
        },
      ),
    );
  }

  /**
   * @param {ReadableStream<ArrayBuffer>} stream 
   * @param {UnderlyingSource<ArrayBuffer>} underlyingSource 
   * @param {UnderlyingSource<ArrayBuffer>} underlyingSourceDict 
   * @param {number} highWaterMark 
   */
  function setUpReadableByteStreamControllerFromUnderlyingSource(
    stream,
    underlyingSource,
    underlyingSourceDict,
    highWaterMark,
  ) {
    const controller = new ReadableByteStreamController();
    /** @type {() => void} */
    let startAlgorithm = () => undefined;
    /** @type {() => Promise<void>} */
    let pullAlgorithm = () => Promise.resolve(undefined);
    /** @type {(reason: any) => Promise<void>} */
    let cancelAlgorithm = (_reason) => Promise.resolve(undefined);
    if ("start" in underlyingSourceDict) {
      startAlgorithm = () =>
        underlyingSourceDict.start.call(underlyingSource, controller);
    }
    if ("pull" in underlyingSourceDict) {
      pullAlgorithm = () =>
        underlyingSourceDict.pull.call(underlyingSource, controller);
    }
    if ("cancel" in underlyingSourceDict) {
      cancelAlgorithm = (reason) =>
        underlyingSourceDict.cancel.call(underlyingSource, reason);
    }
    // 3.13.27.6 Let autoAllocateChunkSize be ? GetV(underlyingByteSource, "autoAllocateChunkSize").
    /** @type {undefined} */
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

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {ReadableStreamDefaultController<R>} controller 
   * @param {(controller: ReadableStreamDefaultController<R>) => void | Promise<void>} startAlgorithm 
   * @param {(controller: ReadableStreamDefaultController<R>) => Promise<void>} pullAlgorithm 
   * @param {(reason: any) => Promise<void>} cancelAlgorithm 
   * @param {number} highWaterMark 
   * @param {(chunk: R) => number} sizeAlgorithm 
   */
  function setUpReadableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    sizeAlgorithm,
  ) {
    assert(stream[_controller] === undefined);
    controller[_stream] = stream;
    resetQueue(controller);
    controller[_started] = controller[_closeRequested] =
      controller[_pullAgain] = controller[_pulling] = false;
    controller[_strategySizeAlgorithm] = sizeAlgorithm;
    controller[_strategyHWM] = highWaterMark;
    controller[_pullAlgorithm] = pullAlgorithm;
    controller[_cancelAlgorithm] = cancelAlgorithm;
    stream[_controller] = controller;
    const startResult = startAlgorithm(controller);
    const startPromise = resolvePromiseWith(startResult);
    uponPromise(startPromise, () => {
      controller[_started] = true;
      assert(controller[_pulling] === false);
      assert(controller[_pullAgain] === false);
      readableStreamDefaultControllerCallPullIfNeeded(controller);
    }, (r) => {
      readableStreamDefaultControllerError(controller, r);
    });
  }

  /**
   * @template R
   * @param {ReadableStream<R>} stream 
   * @param {UnderlyingSource<R>} underlyingSource 
   * @param {UnderlyingSource<R>} underlyingSourceDict 
   * @param {number} highWaterMark 
   * @param {(chunk: R) => number} sizeAlgorithm 
   */
  function setUpReadableStreamDefaultControllerFromUnderlyingSource(
    stream,
    underlyingSource,
    underlyingSourceDict,
    highWaterMark,
    sizeAlgorithm,
  ) {
    const controller = new ReadableStreamDefaultController();
    /** @type {(controller: ReadableStreamDefaultController<R>) => Promise<void>} */
    let startAlgorithm = () => undefined;
    /** @type {(controller: ReadableStreamDefaultController<R>) => Promise<void>} */
    let pullAlgorithm = () => Promise.resolve(undefined);
    /** @type {(reason?: any) => Promise<void>} */
    let cancelAlgorithm = () => Promise.resolve(undefined);
    if ("start" in underlyingSourceDict) {
      startAlgorithm = () =>
        underlyingSourceDict.start.call(underlyingSource, controller);
    }
    if ("pull" in underlyingSourceDict) {
      pullAlgorithm = () =>
        underlyingSourceDict.pull.call(underlyingSource, controller);
    }
    if ("cancel" in underlyingSourceDict) {
      cancelAlgorithm = (reason) =>
        underlyingSourceDict.cancel.call(underlyingSource, reason);
    }
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

  /**
   * @template R
   * @param {ReadableStreamDefaultReader<R>} reader 
   * @param {ReadableStream<R>} stream 
   */
  function setUpReadableStreamDefaultReader(reader, stream) {
    if (isReadableStreamLocked(stream)) {
      throw new TypeError("ReadableStream is locked.");
    }
    readableStreamReaderGenericInitialize(reader, stream);
    reader[_readRequests] = [];
  }

  /**
   * @template O
   * @param {TransformStream<any, O>} stream 
   * @param {TransformStreamDefaultController<O>} controller 
   * @param {(chunk: O, controller: TransformStreamDefaultController<O>) => Promise<void>} transformAlgorithm 
   * @param {(controller: TransformStreamDefaultController<O>) => Promise<void>} flushAlgorithm 
   */
  function setUpTransformStreamDefaultController(
    stream,
    controller,
    transformAlgorithm,
    flushAlgorithm,
  ) {
    assert(stream instanceof TransformStream);
    assert(stream[_controller] === undefined);
    controller[_stream] = stream;
    stream[_controller] = controller;
    controller[_transformAlgorithm] = transformAlgorithm;
    controller[_flushAlgorithm] = flushAlgorithm;
  }

  /**
   * @template I
   * @template O
   * @param {TransformStream<I, O>} stream 
   * @param {Transformer<I, O>} transformer 
   * @param {Transformer<I, O>} transformerDict 
   */
  function setUpTransformStreamDefaultControllerFromTransformer(
    stream,
    transformer,
    transformerDict,
  ) {
    /** @type {TransformStreamDefaultController<O>} */
    const controller = new TransformStreamDefaultController();
    /** @type {(chunk: O, controller: TransformStreamDefaultController<O>) => Promise<void>} */
    let transformAlgorithm = (chunk) => {
      try {
        transformStreamDefaultControllerEnqueue(controller, chunk);
      } catch (e) {
        return Promise.reject(e);
      }
      return Promise.resolve(undefined);
    };
    /** @type {(controller: TransformStreamDefaultController<O>) => Promise<void>} */
    let flushAlgorithm = () => Promise.resolve(undefined);
    if ("transform" in transformerDict) {
      transformAlgorithm = (chunk, controller) =>
        transformerDict.transform.call(transformer, chunk, controller);
    }
    if ("flush" in transformerDict) {
      flushAlgorithm = (controller) =>
        transformerDict.flush.call(transformer, controller);
    }
    setUpTransformStreamDefaultController(
      stream,
      controller,
      transformAlgorithm,
      flushAlgorithm,
    );
  }

  /**
   * @template W
   * @param {WritableStream<W>} stream 
   * @param {WritableStreamDefaultController<W>} controller 
   * @param {(controller: WritableStreamDefaultController<W>) => Promise<void>} startAlgorithm 
   * @param {(chunk: W, controller: WritableStreamDefaultController<W>) => Promise<void>} writeAlgorithm 
   * @param {() => Promise<void>} closeAlgorithm 
   * @param {(reason?: any) => Promise<void>} abortAlgorithm 
   * @param {number} highWaterMark 
   * @param {(chunk: W) => number} sizeAlgorithm 
   */
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
    assert(stream[_controller] === undefined);
    controller[_stream] = stream;
    stream[_controller] = controller;
    resetQueue(controller);
    controller[_started] = false;
    controller[_strategySizeAlgorithm] = sizeAlgorithm;
    controller[_strategyHWM] = highWaterMark;
    controller[_writeAlgorithm] = writeAlgorithm;
    controller[_closeAlgorithm] = closeAlgorithm;
    controller[_abortAlgorithm] = abortAlgorithm;
    const backpressure = writableStreamDefaultControllerGetBackpressure(
      controller,
    );
    writableStreamUpdateBackpressure(stream, backpressure);
    const startResult = startAlgorithm(controller);
    const startPromise = Promise.resolve(startResult);
    uponPromise(startPromise, () => {
      assert(stream[_state] === "writable" || stream[_state] === "erroring");
      controller[_started] = true;
      writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
    }, (r) => {
      assert(stream[_state] === "writable" || stream[_state] === "erroring");
      controller[_started] = true;
      writableStreamDealWithRejection(stream, r);
    });
  }

  /**
   * @template W
   * @param {WritableStream<W>} stream 
   * @param {UnderlyingSink<W>} underlyingSink 
   * @param {UnderlyingSink<W>} underlyingSinkDict 
   * @param {number} highWaterMark 
   * @param {(chunk: W) => number} sizeAlgorithm 
   */
  function setUpWritableStreamDefaultControllerFromUnderlyingSink(
    stream,
    underlyingSink,
    underlyingSinkDict,
    highWaterMark,
    sizeAlgorithm,
  ) {
    const controller = new WritableStreamDefaultController();
    let startAlgorithm = () => undefined;
    /** @type {(chunk: W) => Promise<void>} */
    let writeAlgorithm = () => Promise.resolve(undefined);
    let closeAlgorithm = () => Promise.resolve(undefined);
    /** @type {(reason?: any) => Promise<void>} */
    let abortAlgorithm = () => Promise.resolve(undefined);
    if ("start" in underlyingSinkDict) {
      startAlgorithm = () =>
        underlyingSinkDict.start.call(underlyingSink, controller);
    }
    if ("write" in underlyingSinkDict) {
      writeAlgorithm = (chunk) =>
        underlyingSinkDict.write.call(underlyingSink, chunk, controller);
    }
    if ("close" in underlyingSinkDict) {
      closeAlgorithm = () => underlyingSinkDict.close.call(underlyingSink);
    }
    if ("abort" in underlyingSinkDict) {
      abortAlgorithm = (reason) =>
        underlyingSinkDict.abort.call(underlyingSink, reason);
    }
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

  /**
   * @template W
   * @param {WritableStreamDefaultWriter<W>} writer 
   * @param {WritableStream<W>} stream 
   */
  function setUpWritableStreamDefaultWriter(writer, stream) {
    if (isWritableStreamLocked(stream) === true) {
      throw new TypeError("The stream is already locked.");
    }
    writer[_stream] = stream;
    stream[_writer] = writer;
    const state = stream[_state];
    if (state === "writable") {
      if (
        writableStreamCloseQueuedOrInFlight(stream) === false &&
        stream[_backpressure] === true
      ) {
        writer[_readyPromise] = new Deferred();
      } else {
        writer[_readyPromise] = new Deferred();
        writer[_readyPromise].resolve(undefined);
      }
      writer[_closedPromise] = new Deferred();
    } else if (state === "erroring") {
      writer[_readyPromise] = new Deferred();
      writer[_readyPromise].reject(stream[_storedError]);
      setPromiseIsHandledToTrue(writer[_readyPromise].promise);
      writer[_closedPromise] = new Deferred();
    } else if (state === "closed") {
      writer[_readyPromise] = new Deferred();
      writer[_readyPromise].resolve(undefined);
      writer[_closedPromise] = new Deferred();
      writer[_closedPromise].resolve(undefined);
    } else {
      assert(state === "errored");
      const storedError = stream[_storedError];
      writer[_readyPromise] = new Deferred();
      writer[_readyPromise].reject(storedError);
      setPromiseIsHandledToTrue(writer[_readyPromise].promise);
      writer[_closedPromise] = new Deferred();
      writer[_closedPromise].reject(storedError);
      setPromiseIsHandledToTrue(writer[_closedPromise].promise);
    }
  }

  /** @param {TransformStreamDefaultController} controller */
  function transformStreamDefaultControllerClearAlgorithms(controller) {
    controller[_transformAlgorithm] = undefined;
    controller[_flushAlgorithm] = undefined;
  }

  /**
   * @template O
   * @param {TransformStreamDefaultController<O>} controller 
   * @param {O} chunk 
   */
  function transformStreamDefaultControllerEnqueue(controller, chunk) {
    const stream = controller[_stream];
    const readableController = stream[_readable][_controller];
    if (
      readableStreamDefaultControllerCanCloseOrEnqueue(
        /** @type {ReadableStreamDefaultController<O>} */ (readableController),
      ) === false
    ) {
      throw new TypeError("Readable stream is unavailable.");
    }
    try {
      readableStreamDefaultControllerEnqueue(
        /** @type {ReadableStreamDefaultController<O>} */ (readableController),
        chunk,
      );
    } catch (e) {
      transformStreamErrorWritableAndUnblockWrite(stream, e);
      throw stream[_readable][_storedError];
    }
    const backpressure = readableStreamDefaultcontrollerHasBackpressure(
      /** @type {ReadableStreamDefaultController<O>} */ (readableController),
    );
    if (backpressure !== stream[_backpressure]) {
      assert(backpressure === true);
      transformStreamSetBackpressure(stream, true);
    }
  }

  /**
   * @param {TransformStreamDefaultController} controller 
   * @param {any=} e 
   */
  function transformStreamDefaultControllerError(controller, e) {
    transformStreamError(controller[_stream], e);
  }

  /**
   * @template O
   * @param {TransformStreamDefaultController<O>} controller 
   * @param {any} chunk 
   * @returns {Promise<void>}
   */
  function transformStreamDefaultControllerPerformTransform(controller, chunk) {
    const transformPromise = controller[_transformAlgorithm](chunk, controller);
    return transformPromiseWith(transformPromise, undefined, (r) => {
      transformStreamError(controller[_stream], r);
      throw r;
    });
  }

  /** @param {TransformStreamDefaultController} controller */
  function transformStreamDefaultControllerTerminate(controller) {
    const stream = controller[_stream];
    const readableController = stream[_readable][_controller];
    readableStreamDefaultControllerClose(
      /** @type {ReadableStreamDefaultController} */ (readableController),
    );
    const error = new TypeError("The stream has been terminated.");
    transformStreamErrorWritableAndUnblockWrite(stream, error);
  }

  /**
   * @param {TransformStream} stream 
   * @param {any=} reason 
   * @returns {Promise<void>}
   */
  function transformStreamDefaultSinkAbortAlgorithm(stream, reason) {
    transformStreamError(stream, reason);
    return Promise.resolve(undefined);
  }

  /**
   * @template I
   * @template O
   * @param {TransformStream<I, O>} stream 
   * @returns {Promise<void>}
   */
  function transformStreamDefaultSinkCloseAlgorithm(stream) {
    const readable = stream[_readable];
    const controller = stream[_controller];
    const flushPromise = controller[_flushAlgorithm](controller);
    transformStreamDefaultControllerClearAlgorithms(controller);
    return transformPromiseWith(flushPromise, () => {
      if (readable[_state] === "errored") {
        throw readable[_storedError];
      }
      readableStreamDefaultControllerClose(
        /** @type {ReadableStreamDefaultController} */ (readable[_controller]),
      );
    }, (r) => {
      transformStreamError(stream, r);
      throw readable[_storedError];
    });
  }

  /**
   * @template I
   * @template O
   * @param {TransformStream<I, O>} stream 
   * @param {I} chunk 
   * @returns {Promise<void>}
   */
  function transformStreamDefaultSinkWriteAlgorithm(stream, chunk) {
    assert(stream[_writable][_state] === "writable");
    const controller = stream[_controller];
    if (stream[_backpressure] === true) {
      const backpressureChangePromise = stream[_backpressureChangePromise];
      assert(backpressureChangePromise !== undefined);
      return transformPromiseWith(backpressureChangePromise.promise, () => {
        const writable = stream[_writable];
        const state = writable[_state];
        if (state === "erroring") {
          throw writable[_storedError];
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

  /**
   * @param {TransformStream} stream
   * @returns {Promise<void>}
   */
  function transformStreamDefaultSourcePullAlgorithm(stream) {
    assert(stream[_backpressure] === true);
    assert(stream[_backpressureChangePromise] !== undefined);
    transformStreamSetBackpressure(stream, false);
    return stream[_backpressureChangePromise].promise;
  }

  /**
   * @param {TransformStream} stream 
   * @param {any=} e 
   */
  function transformStreamError(stream, e) {
    readableStreamDefaultControllerError(
      /** @type {ReadableStreamDefaultController} */ (stream[_readable][
        _controller
      ]),
      e,
    );
    transformStreamErrorWritableAndUnblockWrite(stream, e);
  }

  /**
   * @param {TransformStream} stream 
   * @param {any=} e 
   */
  function transformStreamErrorWritableAndUnblockWrite(stream, e) {
    transformStreamDefaultControllerClearAlgorithms(stream[_controller]);
    writableStreamDefaultControllerErrorIfNeeded(
      stream[_writable][_controller],
      e,
    );
    if (stream[_backpressure] === true) {
      transformStreamSetBackpressure(stream, false);
    }
  }

  /**
   * @param {TransformStream} stream 
   * @param {boolean} backpressure 
   */
  function transformStreamSetBackpressure(stream, backpressure) {
    assert(stream[_backpressure] !== backpressure);
    if (stream[_backpressureChangePromise] !== undefined) {
      stream[_backpressureChangePromise].resolve(undefined);
    }
    stream[_backpressureChangePromise] = new Deferred();
    stream[_backpressure] = backpressure;
  }

  /**
   * @param {WritableStream} stream 
   * @param {any=} reason 
   * @returns {Promise<void>}
   */
  function writableStreamAbort(stream, reason) {
    const state = stream[_state];
    if (state === "closed" || state === "errored") {
      return Promise.resolve(undefined);
    }
    if (stream[_pendingAbortRequest] !== undefined) {
      return stream[_pendingAbortRequest].deferred.promise;
    }
    assert(state === "writable" || state === "erroring");
    let wasAlreadyErroring = false;
    if (state === "erroring") {
      wasAlreadyErroring = true;
      reason = undefined;
    }
    /** Deferred<void> */
    const deferred = new Deferred();
    stream[_pendingAbortRequest] = {
      deferred,
      reason,
      wasAlreadyErroring,
    };
    if (wasAlreadyErroring === false) {
      writableStreamStartErroring(stream, reason);
    }
    return deferred.promise;
  }

  /**
   * @param {WritableStream} stream 
   * @returns {Promise<void>}
   */
  function writableStreamAddWriteRequest(stream) {
    assert(isWritableStreamLocked(stream) === true);
    assert(stream[_state] === "writable");
    /** @type {Deferred<void>} */
    const deferred = new Deferred();
    stream[_writeRequests].push(deferred);
    return deferred.promise;
  }

  /**
   * @param {WritableStream} stream 
   * @returns {Promise<void>}
   */
  function writableStreamClose(stream) {
    const state = stream[_state];
    if (state === "closed" || state === "errored") {
      return Promise.reject(
        new TypeError("Writable stream is closed or errored."),
      );
    }
    assert(state === "writable" || state === "erroring");
    assert(writableStreamCloseQueuedOrInFlight(stream) === false);
    /** @type {Deferred<void>} */
    const deferred = new Deferred();
    stream[_closeRequest] = deferred;
    const writer = stream[_writer];
    if (
      writer !== undefined && stream[_backpressure] === true &&
      state === "writable"
    ) {
      writer[_readyPromise].resolve(undefined);
    }
    writableStreamDefaultControllerClose(stream[_controller]);
    return deferred.promise;
  }

  /**
   * @param {WritableStream} stream 
   * @returns {boolean}
   */
  function writableStreamCloseQueuedOrInFlight(stream) {
    if (
      stream[_closeRequest] === undefined &&
      stream[_inFlightCloseRequest] === undefined
    ) {
      return false;
    }
    return true;
  }

  /**
   * @param {WritableStream} stream 
   * @param {any=} error 
   */
  function writableStreamDealWithRejection(stream, error) {
    const state = stream[_state];
    if (state === "writable") {
      writableStreamStartErroring(stream, error);
      return;
    }
    assert(state === "erroring");
    writableStreamFinishErroring(stream);
  }

  /** 
   * @template W
   * @param {WritableStreamDefaultController<W>} controller
   */
  function writableStreamDefaultControllerAdvanceQueueIfNeeded(controller) {
    const stream = controller[_stream];
    if (controller[_started] === false) {
      return;
    }
    if (stream[_inFlightWriteRequest] !== undefined) {
      return;
    }
    const state = stream[_state];
    assert(state !== "closed" && state !== "errored");
    if (state === "erroring") {
      writableStreamFinishErroring(stream);
      return;
    }
    if (controller[_queue].length === 0) {
      return;
    }
    const value = peekQueueValue(controller);
    if (value === _close) {
      writableStreamDefaultControllerProcessClose(controller);
    } else {
      writableStreamDefaultControllerProcessWrite(controller, value);
    }
  }

  function writableStreamDefaultControllerClearAlgorithms(controller) {
    controller[_writeAlgorithm] = undefined;
    controller[_closeAlgorithm] = undefined;
    controller[_abortAlgorithm] = undefined;
    controller[_strategySizeAlgorithm] = undefined;
  }

  /** @param {WritableStreamDefaultController} controller */
  function writableStreamDefaultControllerClose(controller) {
    enqueueValueWithSize(controller, _close, 0);
    writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
  }

  /**
   * @param {WritableStreamDefaultController} controller 
   * @param {any} error 
   */
  function writableStreamDefaultControllerError(controller, error) {
    const stream = controller[_stream];
    assert(stream[_state] === "writable");
    writableStreamDefaultControllerClearAlgorithms(controller);
    writableStreamStartErroring(stream, error);
  }

  /**
   * @param {WritableStreamDefaultController} controller 
   * @param {any} error 
   */
  function writableStreamDefaultControllerErrorIfNeeded(controller, error) {
    if (controller[_stream][_state] === "writable") {
      writableStreamDefaultControllerError(controller, error);
    }
  }

  /**
   * @param {WritableStreamDefaultController} controller
   * @returns {boolean}
   */
  function writableStreamDefaultControllerGetBackpressure(controller) {
    const desiredSize = writableStreamDefaultControllerGetDesiredSize(
      controller,
    );
    return desiredSize <= 0;
  }

  /**
   * @template W
   * @param {WritableStreamDefaultController<W>} controller 
   * @param {W} chunk 
   * @returns {number}
   */
  function writableStreamDefaultControllerGetChunkSize(controller, chunk) {
    let value;
    try {
      value = controller[_strategySizeAlgorithm](chunk);
    } catch (e) {
      writableStreamDefaultControllerErrorIfNeeded(controller, e);
      return 1;
    }
    return value;
  }

  /**
   * @param {WritableStreamDefaultController} controller 
   * @returns {number}
   */
  function writableStreamDefaultControllerGetDesiredSize(controller) {
    return controller[_strategyHWM] - controller[_queueTotalSize];
  }

  /** @param {WritableStreamDefaultController} controller */
  function writableStreamDefaultControllerProcessClose(controller) {
    const stream = controller[_stream];
    writableStreamMarkCloseRequestInFlight(stream);
    dequeueValue(controller);
    assert(controller[_queue].length === 0);
    const sinkClosePromise = controller[_closeAlgorithm]();
    writableStreamDefaultControllerClearAlgorithms(controller);
    uponPromise(sinkClosePromise, () => {
      writableStreamFinishInFlightClose(stream);
    }, (reason) => {
      writableStreamFinishInFlightCloseWithError(stream, reason);
    });
  }

  /**
   * @template W
   * @param {WritableStreamDefaultController<W>} controller 
   * @param {W} chunk 
   */
  function writableStreamDefaultControllerProcessWrite(controller, chunk) {
    const stream = controller[_stream];
    writableStreamMarkFirstWriteRequestInFlight(stream);
    const sinkWritePromise = controller[_writeAlgorithm](chunk, controller);
    uponPromise(sinkWritePromise, () => {
      writableStreamFinishInFlightWrite(stream);
      const state = stream[_state];
      assert(state === "writable" || state === "erroring");
      dequeueValue(controller);
      if (
        writableStreamCloseQueuedOrInFlight(stream) === false &&
        state === "writable"
      ) {
        const backpressure = writableStreamDefaultControllerGetBackpressure(
          controller,
        );
        writableStreamUpdateBackpressure(stream, backpressure);
      }
      writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
    }, (reason) => {
      if (stream[_state] === "writable") {
        writableStreamDefaultControllerClearAlgorithms(controller);
      }
      writableStreamFinishInFlightWriteWithError(stream, reason);
    });
  }

  /**
   * @template W
   * @param {WritableStreamDefaultController<W>} controller 
   * @param {W} chunk 
   * @param {number} chunkSize 
   */
  function writableStreamDefaultControllerWrite(controller, chunk, chunkSize) {
    try {
      enqueueValueWithSize(controller, chunk, chunkSize);
    } catch (e) {
      writableStreamDefaultControllerErrorIfNeeded(controller, e);
      return;
    }
    const stream = controller[_stream];
    if (
      writableStreamCloseQueuedOrInFlight(stream) === false &&
      stream[_state] === "writable"
    ) {
      const backpressure = writableStreamDefaultControllerGetBackpressure(
        controller,
      );
      writableStreamUpdateBackpressure(stream, backpressure);
    }
    writableStreamDefaultControllerAdvanceQueueIfNeeded(controller);
  }

  /**
   * @param {WritableStreamDefaultWriter} writer 
   * @param {any=} reason 
   * @returns {Promise<void>}
   */
  function writableStreamDefaultWriterAbort(writer, reason) {
    const stream = writer[_stream];
    assert(stream !== undefined);
    return writableStreamAbort(stream, reason);
  }

  /**
   * @param {WritableStreamDefaultWriter} writer 
   * @returns {Promise<void>}
   */
  function writableStreamDefaultWriterClose(writer) {
    const stream = writer[_stream];
    assert(stream !== undefined);
    return writableStreamClose(stream);
  }

  /**
   * @param {WritableStreamDefaultWriter} writer 
   * @returns {Promise<void>}
   */
  function writableStreamDefaultWriterCloseWithErrorPropagation(writer) {
    const stream = writer[_stream];
    assert(stream !== undefined);
    const state = stream[_state];
    if (
      writableStreamCloseQueuedOrInFlight(stream) === true || state === "closed"
    ) {
      return Promise.resolve(undefined);
    }
    if (state === "errored") {
      return Promise.reject(stream[_storedError]);
    }
    assert(state === "writable" || state === "erroring");
    return writableStreamDefaultWriterClose(writer);
  }

  /**
   * @param {WritableStreamDefaultWriter} writer 
   * @param {any=} error 
   */
  function writableStreamDefaultWriterEnsureClosedPromiseRejected(
    writer,
    error,
  ) {
    if (writer[_closedPromise].state === "pending") {
      writer[_closedPromise].reject(error);
    } else {
      writer[_closedPromise] = new Deferred();
      writer[_closedPromise].reject(error);
    }
    setPromiseIsHandledToTrue(writer[_closedPromise].promise);
  }

  /** 
   * @param {WritableStreamDefaultWriter} writer
   * @param {any=} error
   */
  function writableStreamDefaultWriterEnsureReadyPromiseRejected(
    writer,
    error,
  ) {
    if (writer[_readyPromise].state === "pending") {
      writer[_readyPromise].reject(error);
    } else {
      writer[_readyPromise] = new Deferred();
      writer[_readyPromise].reject(error);
    }
    setPromiseIsHandledToTrue(writer[_readyPromise].promise);
  }

  /**
   * @param {WritableStreamDefaultWriter} writer 
   * @returns {number | null}
   */
  function writableStreamDefaultWriterGetDesiredSize(writer) {
    const stream = writer[_stream];
    const state = stream[_state];
    if (state === "errored" || state === "erroring") {
      return null;
    }
    if (state === "closed") {
      return 0;
    }
    return writableStreamDefaultControllerGetDesiredSize(stream[_controller]);
  }

  /** @param {WritableStreamDefaultWriter} writer */
  function writableStreamDefaultWriterRelease(writer) {
    const stream = writer[_stream];
    assert(stream !== undefined);
    assert(stream[_writer] === writer);
    const releasedError = new TypeError(
      "The writer has already been released.",
    );
    writableStreamDefaultWriterEnsureReadyPromiseRejected(
      writer,
      releasedError,
    );
    writableStreamDefaultWriterEnsureClosedPromiseRejected(
      writer,
      releasedError,
    );
    stream[_writer] = undefined;
    writer[_stream] = undefined;
  }

  /**
   * @template W
   * @param {WritableStreamDefaultWriter<W>} writer 
   * @param {W} chunk 
   * @returns {Promise<void>}
   */
  function writableStreamDefaultWriterWrite(writer, chunk) {
    const stream = writer[_stream];
    assert(stream !== undefined);
    const controller = stream[_controller];
    const chunkSize = writableStreamDefaultControllerGetChunkSize(
      controller,
      chunk,
    );
    if (stream !== writer[_stream]) {
      return Promise.reject(new TypeError("Writer's stream is unexpected."));
    }
    const state = stream[_state];
    if (state === "errored") {
      return Promise.reject(stream[_storedError]);
    }
    if (
      writableStreamCloseQueuedOrInFlight(stream) === true || state === "closed"
    ) {
      return Promise.reject(
        new TypeError("The stream is closing or is closed."),
      );
    }
    if (state === "erroring") {
      return Promise.reject(stream[_storedError]);
    }
    assert(state === "writable");
    const promise = writableStreamAddWriteRequest(stream);
    writableStreamDefaultControllerWrite(controller, chunk, chunkSize);
    return promise;
  }

  /** @param {WritableStream} stream */
  function writableStreamFinishErroring(stream) {
    assert(stream[_state] === "erroring");
    assert(writableStreamHasOperationMarkedInFlight(stream) === false);
    stream[_state] = "errored";
    stream[_controller][_errorSteps]();
    const storedError = stream[_storedError];
    for (const writeRequest of stream[_writeRequests]) {
      writeRequest.reject(storedError);
    }
    stream[_writeRequests] = [];
    if (stream[_pendingAbortRequest] === undefined) {
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      return;
    }
    const abortRequest = stream[_pendingAbortRequest];
    stream[_pendingAbortRequest] = undefined;
    if (abortRequest.wasAlreadyErroring === true) {
      abortRequest.deferred.reject(storedError);
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
      return;
    }
    const promise = stream[_controller][_abortSteps](abortRequest.reason);
    uponPromise(promise, () => {
      abortRequest.deferred.resolve(undefined);
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
    }, (reason) => {
      abortRequest.deferred.reject(reason);
      writableStreamRejectCloseAndClosedPromiseIfNeeded(stream);
    });
  }

  /** @param {WritableStream} stream */
  function writableStreamFinishInFlightClose(stream) {
    assert(stream[_inFlightCloseRequest] !== undefined);
    stream[_inFlightCloseRequest].resolve(undefined);
    stream[_inFlightCloseRequest] = undefined;
    const state = stream[_state];
    assert(state === "writable" || state === "erroring");
    if (state === "erroring") {
      stream[_storedError] = undefined;
      if (stream[_pendingAbortRequest] !== undefined) {
        stream[_pendingAbortRequest].deferred.resolve(undefined);
        stream[_pendingAbortRequest] = undefined;
      }
    }
    stream[_state] = "closed";
    const writer = stream[_writer];
    if (writer !== undefined) {
      writer[_closedPromise].resolve(undefined);
    }
    assert(stream[_pendingAbortRequest] === undefined);
    assert(stream[_storedError] === undefined);
  }

  /**
   * @param {WritableStream} stream 
   * @param {any=} error 
   */
  function writableStreamFinishInFlightCloseWithError(stream, error) {
    assert(stream[_inFlightCloseRequest] !== undefined);
    stream[_inFlightCloseRequest].reject(error);
    stream[_inFlightCloseRequest] = undefined;
    assert(stream[_state] === "writable" || stream[_state] === "erroring");
    if (stream[_pendingAbortRequest] !== undefined) {
      stream[_pendingAbortRequest].deferred.reject(error);
      stream[_pendingAbortRequest] = undefined;
    }
    writableStreamDealWithRejection(stream, error);
  }

  /** @param {WritableStream} stream */
  function writableStreamFinishInFlightWrite(stream) {
    assert(stream[_inFlightWriteRequest] !== undefined);
    stream[_inFlightWriteRequest].resolve(undefined);
    stream[_inFlightWriteRequest] = undefined;
  }

  /**
   * @param {WritableStream} stream 
   * @param {any=} error 
   */
  function writableStreamFinishInFlightWriteWithError(stream, error) {
    assert(stream[_inFlightWriteRequest] !== undefined);
    stream[_inFlightWriteRequest].reject(error);
    stream[_inFlightWriteRequest] = undefined;
    assert(stream[_state] === "writable" || stream[_state] === "erroring");
    writableStreamDealWithRejection(stream, error);
  }

  /**
   * @param {WritableStream} stream
   * @returns {boolean}
   */
  function writableStreamHasOperationMarkedInFlight(stream) {
    if (
      stream[_inFlightWriteRequest] === undefined &&
      stream[_controller][_inFlightCloseRequest] === undefined
    ) {
      return false;
    }
    return true;
  }

  /** @param {WritableStream} stream */
  function writableStreamMarkCloseRequestInFlight(stream) {
    assert(stream[_inFlightCloseRequest] === undefined);
    assert(stream[_closeRequest] !== undefined);
    stream[_inFlightCloseRequest] = stream[_closeRequest];
    stream[_closeRequest] = undefined;
  }

  /**
   * @template W
   * @param {WritableStream<W>} stream
   * */
  function writableStreamMarkFirstWriteRequestInFlight(stream) {
    assert(stream[_inFlightWriteRequest] === undefined);
    assert(stream[_writeRequests].length);
    const writeRequest = stream[_writeRequests].shift();
    stream[_inFlightWriteRequest] = writeRequest;
  }

  /** @param {WritableStream} stream */
  function writableStreamRejectCloseAndClosedPromiseIfNeeded(stream) {
    assert(stream[_state] === "errored");
    if (stream[_closeRequest] !== undefined) {
      assert(stream[_inFlightCloseRequest] === undefined);
      stream[_closeRequest].reject(stream[_storedError]);
      stream[_closeRequest] = undefined;
    }
    const writer = stream[_writer];
    if (writer !== undefined) {
      writer[_closedPromise].reject(stream[_storedError]);
      setPromiseIsHandledToTrue(writer[_closedPromise].promise);
    }
  }

  /**
   * @param {WritableStream} stream 
   * @param {any=} reason 
   */
  function writableStreamStartErroring(stream, reason) {
    assert(stream[_storedError] === undefined);
    assert(stream[_state] === "writable");
    const controller = stream[_controller];
    assert(controller);
    stream[_state] = "erroring";
    stream[_storedError] = reason;
    const writer = stream[_writer];
    if (writer) {
      writableStreamDefaultWriterEnsureReadyPromiseRejected(writer, reason);
    }
    if (
      writableStreamHasOperationMarkedInFlight(stream) === false &&
      controller[_started] === true
    ) {
      writableStreamFinishErroring(stream);
    }
  }

  /**
   * @param {WritableStream} stream 
   * @param {boolean} backpressure 
   */
  function writableStreamUpdateBackpressure(stream, backpressure) {
    assert(stream[_state] === "writable");
    assert(writableStreamCloseQueuedOrInFlight(stream) === false);
    const writer = stream[_writer];
    if (writer !== undefined && backpressure !== stream[_backpressure]) {
      if (backpressure === true) {
        writer[_readyPromise] = new Deferred();
      } else {
        assert(backpressure === false);
        writer[_readyPromise].resolve(undefined);
      }
    }
    stream[_backpressure] = backpressure;
  }

  /**
   * @template T
   * @param {T} value 
   * @param {boolean} done 
   * @returns {IteratorResult<T>}
   */
  function createIteratorResult(value, done) {
    const result = Object.create(null);
    Object.defineProperties(result, {
      value: { value, writable: true, enumerable: true, configurable: true },
      done: {
        value: done,
        writable: true,
        enumerable: true,
        configurable: true,
      },
    });
    return result;
  }

  /** @type {AsyncIterator<unknown, unknown>} */
  const asyncIteratorPrototype = Object.getPrototypeOf(
    Object.getPrototypeOf(async function* () {}).prototype,
  );

  /** @type {AsyncIterator<unknown>} */
  const readableStreamAsyncIteratorPrototype = Object.setPrototypeOf({
    /** @returns {Promise<IteratorResult<unknown>>} */
    next() {
      /** @type {ReadableStreamDefaultReader} */
      const reader = this[_reader];
      if (reader[_stream] === undefined) {
        return Promise.reject(
          new TypeError(
            "Cannot get the next iteration result once the reader has been released.",
          ),
        );
      }
      /** @type {Deferred<IteratorResult<any>>} */
      const promise = new Deferred();
      /** @type {ReadRequest} */
      const readRequest = {
        chunkSteps(chunk) {
          promise.resolve(createIteratorResult(chunk, false));
        },
        closeSteps() {
          readableStreamReaderGenericRelease(reader);
          promise.resolve(createIteratorResult(undefined, true));
        },
        errorSteps(e) {
          readableStreamReaderGenericRelease(reader);
          promise.reject(e);
        },
      };
      readableStreamDefaultReaderRead(reader, readRequest);
      return promise.promise;
    },
    /**
     * @param {unknown} arg 
     * @returns {Promise<IteratorResult<unknown>>}
     */
    async return(arg) {
      /** @type {ReadableStreamDefaultReader} */
      const reader = this[_reader];
      if (reader[_stream] === undefined) {
        return createIteratorResult(undefined, true);
      }
      assert(reader[_readRequests].length === 0);
      if (this[_preventCancel] === false) {
        const result = readableStreamReaderGenericCancel(reader, arg);
        readableStreamReaderGenericRelease(reader);
        await result;
        return createIteratorResult(arg, true);
      }
      readableStreamReaderGenericRelease(reader);
      return createIteratorResult(undefined, true);
    },
  }, asyncIteratorPrototype);

  class ByteLengthQueuingStrategy {
    /** @type {number} */
    highWaterMark;

    /** @param {{ highWaterMark: number }} init */
    constructor(init) {
      if (
        typeof init !== "object" || init === null || !("highWaterMark" in init)
      ) {
        throw new TypeError(
          "init must be an object that contains a property named highWaterMark",
        );
      }
      const { highWaterMark } = init;
      this[_globalObject] = window;
      this.highWaterMark = Number(highWaterMark);
    }

    /** @returns {(chunk: ArrayBufferView) => number} */
    get size() {
      initializeByteLengthSizeFunction(this[_globalObject]);
      return byteSizeFunctionWeakMap.get(this[_globalObject]);
    }
  }

  /** @type {WeakMap<typeof globalThis, (chunk: ArrayBufferView) => number>} */
  const byteSizeFunctionWeakMap = new WeakMap();

  function initializeByteLengthSizeFunction(globalObject) {
    if (byteSizeFunctionWeakMap.has(globalObject)) {
      return;
    }
    byteSizeFunctionWeakMap.set(globalObject, function size(chunk) {
      return chunk.byteLength;
    });
  }

  class CountQueuingStrategy {
    /** @type {number} */
    highWaterMark;

    /** @param {{ highWaterMark: number }} init */
    constructor(init) {
      if (
        typeof init !== "object" || init === null || !("highWaterMark" in init)
      ) {
        throw new TypeError(
          "init must be an object that contains a property named highWaterMark",
        );
      }
      const { highWaterMark } = init;
      this[_globalObject] = window;
      this.highWaterMark = Number(highWaterMark);
    }

    /** @returns {(chunk: any) => 1} */
    get size() {
      initializeCountSizeFunction(this[_globalObject]);
      return countSizeFunctionWeakMap.get(this[_globalObject]);
    }
  }

  /** @type {WeakMap<typeof globalThis, () => 1>} */
  const countSizeFunctionWeakMap = new WeakMap();

  /** @param {typeof globalThis} globalObject */
  function initializeCountSizeFunction(globalObject) {
    if (countSizeFunctionWeakMap.has(globalObject)) {
      return;
    }
    countSizeFunctionWeakMap.set(globalObject, function size() {
      return 1;
    });
  }

  /** @template R */
  class ReadableStream {
    /** @type {ReadableStreamDefaultController | ReadableByteStreamController} */
    [_controller];
    /** @type {boolean} */
    [_detached];
    /** @type {boolean} */
    [_disturbed];
    /** @type {ReadableStreamDefaultReader | undefined} */
    [_reader];
    /** @type {"readable" | "closed" | "errored"} */
    [_state];
    /** @type {any} */
    [_storedError];

    /**
     * @param {UnderlyingSource<R>=} underlyingSource 
     * @param {QueuingStrategy<R>=} strategy 
     */
    constructor(underlyingSource, strategy = {}) {
      const underlyingSourceDict = convertUnderlyingSource(underlyingSource);
      initializeReadableStream(this);
      if (underlyingSourceDict.type === "bytes") {
        if (strategy.size !== undefined) {
          throw new RangeError(
            `When underlying source is "bytes", strategy.size must be undefined.`,
          );
        }
        const highWaterMark = extractHighWaterMark(strategy, 0);
        setUpReadableByteStreamControllerFromUnderlyingSource(
          // @ts-ignore cannot easily assert this is ReadableStream<ArrayBuffer>
          this,
          underlyingSource,
          underlyingSourceDict,
          highWaterMark,
        );
      } else {
        assert(!("type" in underlyingSourceDict));
        const sizeAlgorithm = extractSizeAlgorithm(strategy);
        const highWaterMark = extractHighWaterMark(strategy, 1);
        setUpReadableStreamDefaultControllerFromUnderlyingSource(
          this,
          underlyingSource,
          underlyingSourceDict,
          highWaterMark,
          sizeAlgorithm,
        );
      }
    }

    /** @returns {boolean} */
    get locked() {
      return isReadableStreamLocked(this);
    }

    /**
     * @param {any=} reason 
     * @returns {Promise<void>}
     */
    cancel(reason) {
      if (isReadableStreamLocked(this)) {
        Promise.reject(new TypeError("Cannot cancel a locked ReadableStream."));
      }
      return readableStreamCancel(this, reason);
    }

    /**
     * @param {ReadableStreamGetReaderOptions=} options 
     * @returns {ReadableStreamDefaultReader<R>}
     */
    getReader(options = {}) {
      if (typeof options !== "object") {
        throw new TypeError("options must be an object");
      }
      if (options === null) {
        options = {};
      }
      /** @type {any} */
      let { mode } = options;
      if (mode === undefined) {
        return acquireReadableStreamDefaultReader(this);
      }
      mode = String(mode);
      if (mode !== "byob") {
        throw new TypeError("Invalid mode.");
      }
      // 3. Return ? AcquireReadableStreamBYOBReader(this).
      throw new RangeError(`Unsupported mode "${String(mode)}"`);
    }

    /**
     * @template T
     * @param {{ readable: ReadableStream<T>, writable: WritableStream<R> }} transform 
     * @param {PipeOptions=} options 
     * @returns {ReadableStream<T>}
     */
    pipeThrough(
      { readable, writable },
      { preventClose, preventAbort, preventCancel, signal } = {},
    ) {
      if (isReadableStreamLocked(this)) {
        throw new TypeError("ReadableStream is already locked.");
      }
      if (isWritableStreamLocked(writable)) {
        throw new TypeError("Target WritableStream is already locked.");
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

    /**
     * @param {WritableStream<R>} destination 
     * @param {PipeOptions=} options
     * @returns {Promise<void>}
     */
    pipeTo(
      destination,
      {
        preventClose = false,
        preventAbort = false,
        preventCancel = false,
        signal,
      } = {},
    ) {
      if (isReadableStreamLocked(this)) {
        return Promise.reject(
          new TypeError("ReadableStream is already locked."),
        );
      }
      if (isWritableStreamLocked(destination)) {
        return Promise.reject(
          new TypeError("destination WritableStream is already locked."),
        );
      }
      return readableStreamPipeTo(
        this,
        destination,
        preventClose,
        preventAbort,
        preventCancel,
        signal,
      );
    }

    /** @returns {[ReadableStream<R>, ReadableStream<R>]} */
    tee() {
      return readableStreamTee(this, false);
    }

    /**
     * @param {ReadableStreamIteratorOptions=} options 
     * @returns {AsyncIterableIterator<R>}
     */
    [Symbol.asyncIterator]({ preventCancel } = {}) {
      /** @type {AsyncIterableIterator<R>} */
      const iterator = Object.create(readableStreamAsyncIteratorPrototype);
      const reader = acquireReadableStreamDefaultReader(this);
      iterator[_reader] = reader;
      iterator[_preventCancel] = preventCancel;
      return iterator;
    }

    [customInspect]() {
      return `${this.constructor.name} ${
        Deno.inspect({ locked: this.locked })
      }`;
    }
  }

  /** @template R */
  class ReadableStreamGenericReader {
    /** @type {Deferred<void>} */
    [_closedPromise];
    /** @type {ReadableStream<R> | undefined} */
    [_stream];

    get closed() {
      return this[_closedPromise].promise;
    }

    /**
     * @param {any} reason
     * @returns {Promise<void>}
     */
    cancel(reason) {
      if (this[_stream] === undefined) {
        return Promise.reject(
          new TypeError("Reader has no associated stream."),
        );
      }
      return readableStreamReaderGenericCancel(this, reason);
    }
  }

  /** @template R */
  class ReadableStreamDefaultReader extends ReadableStreamGenericReader {
    /** @type {ReadRequest[]} */
    [_readRequests];

    /** @param {ReadableStream<R>} stream */
    constructor(stream) {
      if (!(stream instanceof ReadableStream)) {
        throw new TypeError("stream is not a ReadableStream");
      }
      super();
      setUpReadableStreamDefaultReader(this, stream);
    }

    /** @returns {Promise<ReadableStreamReadResult<R>>} */
    read() {
      if (this[_stream] === undefined) {
        return Promise.reject(
          new TypeError("Reader has no associated stream."),
        );
      }
      /** @type {Deferred<ReadableStreamReadResult<R>>} */
      const promise = new Deferred();
      /** @type {ReadRequest<R>} */
      const readRequest = {
        chunkSteps(chunk) {
          promise.resolve({ value: chunk, done: false });
        },
        closeSteps() {
          promise.resolve({ value: undefined, done: true });
        },
        errorSteps(e) {
          promise.reject(e);
        },
      };
      readableStreamDefaultReaderRead(this, readRequest);
      return promise.promise;
    }

    /** @returns {void} */
    releaseLock() {
      if (this[_stream] === undefined) {
        return;
      }
      if (this[_readRequests].length) {
        throw new TypeError(
          "There are pending read requests, so the reader cannot be release.",
        );
      }
      readableStreamReaderGenericRelease(this);
    }

    [customInspect]() {
      return `${this.constructor.name} { closed: ${String(this.closed)} }`;
    }
  }

  class ReadableByteStreamController {
    /** @type {number | undefined} */
    [_autoAllocateChunkSize];
    /** @type {null} */
    [_byobRequest];
    /** @type {(reason: any) => Promise<void>} */
    [_cancelAlgorithm];
    /** @type {boolean} */
    [_closeRequested];
    /** @type {boolean} */
    [_pullAgain];
    /** @type {(controller: this) => Promise<void>} */
    [_pullAlgorithm];
    /** @type {boolean} */
    [_pulling];
    /** @type {ReadableByteStreamQueueEntry[]} */
    [_queue];
    /** @type {number} */
    [_queueTotalSize];
    /** @type {boolean} */
    [_started];
    /** @type {number} */
    [_strategyHWM];
    /** @type {ReadableStream<ArrayBuffer>} */
    [_stream];

    get byobRequest() {
      return undefined;
    }

    /** @returns {number | null} */
    get desiredSize() {
      return readableByteStreamControllerGetDesiredSize(this);
    }

    /** @returns {void} */
    close() {
      if (this[_closeRequested] === true) {
        throw new TypeError("Closed already requested.");
      }
      if (this[_stream][_state] !== "readable") {
        throw new TypeError(
          "ReadableByteStreamController's stream is not in a readable state.",
        );
      }
      readableByteStreamControllerClose(this);
    }

    /**
     * @param {ArrayBufferView} chunk 
     * @returns {void}
     */
    enqueue(chunk) {
      if (chunk.byteLength === 0) {
        throw new TypeError("chunk must have a non-zero byteLength.");
      }
      if (chunk.buffer.byteLength === 0) {
        throw new TypeError("chunk's buffer must have a non-zero byteLength.");
      }
      if (this[_closeRequested] === true) {
        throw new TypeError(
          "Cannot enqueue chunk after a close has been requested.",
        );
      }
      if (this[_stream][_state] !== "readable") {
        throw new TypeError(
          "Cannot enqueue chunk when underlying stream is not readable.",
        );
      }
      return readableByteStreamControllerEnqueue(this, chunk);
    }

    /**
     * @param {any=} e 
     * @returns {void}
     */
    error(e) {
      readableByteStreamControllerError(this, e);
    }

    /**
     * @param {any} reason 
     * @returns {Promise<void>}
     */
    [_cancelSteps](reason) {
      // 4.7.4. CancelStep 1. If this.[[pendingPullIntos]] is not empty,
      resetQueue(this);
      const result = this[_cancelAlgorithm](reason);
      readableByteStreamControllerClearAlgorithms(this);
      return result;
    }

    /**
     * @param {ReadRequest<ArrayBuffer>} readRequest 
     * @returns {void}
     */
    [_pullSteps](readRequest) {
      /** @type {ReadableStream<ArrayBuffer>} */
      const stream = this[_stream];
      assert(readableStreamHasDefaultReader(stream));
      if (this[_queueTotalSize] > 0) {
        assert(readableStreamGetNumReadRequests(stream) === 0);
        const entry = this[_queue].shift();
        this[_queueTotalSize] -= entry.byteLength;
        readableByteStreamControllerHandleQueueDrain(this);
        const view = new Uint8Array(
          entry.buffer,
          entry.byteOffset,
          entry.byteLength,
        );
        readRequest.chunkSteps(view);
        return;
      }
      // 4. Let autoAllocateChunkSize be this.[[autoAllocateChunkSize]].
      // 5. If autoAllocateChunkSize is not undefined,
      readableStreamAddReadRequest(stream, readRequest);
      readableByteStreamControllerCallPullIfNeeded(this);
    }
  }

  /** @template R */
  class ReadableStreamDefaultController {
    /** @type {(reason: any) => Promise<void>} */
    [_cancelAlgorithm];
    /** @type {boolean} */
    [_closeRequested];
    /** @type {boolean} */
    [_pullAgain];
    /** @type {(controller: this) => Promise<void>} */
    [_pullAlgorithm];
    /** @type {boolean} */
    [_pulling];
    /** @type {Array<ValueWithSize<R>>} */
    [_queue];
    /** @type {number} */
    [_queueTotalSize];
    /** @type {boolean} */
    [_started];
    /** @type {number} */
    [_strategyHWM];
    /** @type {(chunk: R) => number} */
    [_strategySizeAlgorithm];
    /** @type {ReadableStream<R>} */
    [_stream];

    /** @returns {number | null} */
    get desiredSize() {
      return readableStreamDefaultControllerGetDesiredSize(this);
    }

    /** @returns {void} */
    close() {
      if (readableStreamDefaultControllerCanCloseOrEnqueue(this) === false) {
        throw new TypeError("The stream controller cannot close or enqueue.");
      }
      readableStreamDefaultControllerClose(this);
    }

    /**
     * @param {R} chunk 
     * @returns {void}
     */
    enqueue(chunk) {
      if (readableStreamDefaultControllerCanCloseOrEnqueue(this) === false) {
        throw new TypeError("The stream controller cannot close or enqueue.");
      }
      readableStreamDefaultControllerEnqueue(this, chunk);
    }

    /**
     * @param {any=} e 
     * @returns {void}
     */
    error(e) {
      readableStreamDefaultControllerError(this, e);
    }

    /**
     * @param {any} reason 
     * @returns {Promise<void>}
     */
    [_cancelSteps](reason) {
      resetQueue(this);
      const result = this[_cancelAlgorithm](reason);
      readableStreamDefaultControllerClearAlgorithms(this);
      return result;
    }

    /**
     * @param {ReadRequest<R>} readRequest 
     * @returns {void}
     */
    [_pullSteps](readRequest) {
      const stream = this[_stream];
      if (this[_queue].length) {
        const chunk = dequeueValue(this);
        if (this[_closeRequested] && this[_queue].length === 0) {
          readableStreamDefaultControllerClearAlgorithms(this);
          readableStreamClose(stream);
        } else {
          readableStreamDefaultControllerCallPullIfNeeded(this);
        }
        readRequest.chunkSteps(chunk);
      } else {
        readableStreamAddReadRequest(stream, readRequest);
        readableStreamDefaultControllerCallPullIfNeeded(this);
      }
    }
  }

  /**
   * @template I
   * @template O
   */
  class TransformStream {
    /** @type {boolean} */
    [_backpressure];
    /** @type {Deferred<void>} */
    [_backpressureChangePromise];
    /** @type {TransformStreamDefaultController<O>} */
    [_controller];
    /** @type {boolean} */
    [_detached];
    /** @type {ReadableStream<O>} */
    [_readable];
    /** @type {WritableStream<I>} */
    [_writable];

    /**
     * 
     * @param {Transformer<I, O>} transformer 
     * @param {QueuingStrategy<I>} writableStrategy 
     * @param {QueuingStrategy<O>} readableStrategy 
     */
    constructor(
      transformer = null,
      writableStrategy = {},
      readableStrategy = {},
    ) {
      const transformerDict = convertTransformer(transformer);
      if (transformerDict.readableType) {
        throw new RangeError("readableType transformers not supported.");
      }
      if (transformerDict.writableType) {
        throw new RangeError("writableType transformers not supported.");
      }
      const readableHighWaterMark = extractHighWaterMark(readableStrategy, 0);
      const readableSizeAlgorithm = extractSizeAlgorithm(readableStrategy);
      const writableHighWaterMark = extractHighWaterMark(writableStrategy, 1);
      const writableSizeAlgorithm = extractSizeAlgorithm(writableStrategy);
      /** @type {Deferred<void>} */
      const startPromise = new Deferred();
      initializeTransformStream(
        this,
        startPromise,
        writableHighWaterMark,
        writableSizeAlgorithm,
        readableHighWaterMark,
        readableSizeAlgorithm,
      );
      setUpTransformStreamDefaultControllerFromTransformer(
        this,
        transformer,
        transformerDict,
      );
      if ("start" in transformerDict) {
        startPromise.resolve(
          transformerDict.start.call(transformer, this[_controller]),
        );
      } else {
        startPromise.resolve(undefined);
      }
    }

    /** @returns {ReadableStream<O>} */
    get readable() {
      return this[_readable];
    }

    /** @returns {WritableStream<I>} */
    get writable() {
      return this[_writable];
    }

    [customInspect]() {
      return `${this.constructor.name} ${
        Deno.inspect(
          { readable: this.readable, writable: this.writable },
          { depth: 1 },
        )
      }`;
    }
  }

  /** @template O */
  class TransformStreamDefaultController {
    /** @type {(controller: this) => Promise<void>} */
    [_flushAlgorithm];
    /** @type {TransformStream<O>} */
    [_stream];
    /** @type {(chunk: O, controller: this) => Promise<void>} */
    [_transformAlgorithm];

    /** @returns {number | null} */
    get desiredSize() {
      const readableController = this[_stream][_readable][_controller];
      return readableStreamDefaultControllerGetDesiredSize(
        /** @type {ReadableStreamDefaultController<O>} */ (readableController),
      );
    }

    /**
     * @param {O} chunk 
     * @returns {void}
     */
    enqueue(chunk) {
      transformStreamDefaultControllerEnqueue(this, chunk);
    }

    /**
     * @param {any=} reason 
     * @returns {void}
     */
    error(reason) {
      transformStreamDefaultControllerError(this, reason);
    }

    /** @returns {void} */
    terminate() {
      transformStreamDefaultControllerTerminate(this);
    }
  }

  /** @template W */
  class WritableStream {
    /** @type {boolean} */
    [_backpressure];
    /** @type {Deferred<void> | undefined} */
    [_closeRequest];
    /** @type {WritableStreamDefaultController<W>} */
    [_controller];
    /** @type {boolean} */
    [_detached];
    /** @type {Deferred<void> | undefined} */
    [_inFlightWriteRequest];
    /** @type {Deferred<void> | undefined} */
    [_inFlightCloseRequest];
    /** @type {PendingAbortRequest | undefined} */
    [_pendingAbortRequest];
    /** @type {"writable" | "closed" | "erroring" | "errored"} */
    [_state];
    /** @type {any} */
    [_storedError];
    /** @type {WritableStreamDefaultWriter<W>} */
    [_writer];
    /** @type {Deferred<void>[]} */
    [_writeRequests];

    /**
     * @param {UnderlyingSink<W>=} underlyingSink 
     * @param {QueuingStrategy<W>=} strategy 
     */
    constructor(underlyingSink = null, strategy = {}) {
      const underlyingSinkDict = convertUnderlyingSink(underlyingSink);
      if (underlyingSinkDict.type != null) {
        throw new RangeError(
          'WritableStream does not support "type" in the underlying sink.',
        );
      }
      initializeWritableStream(this);
      const sizeAlgorithm = extractSizeAlgorithm(strategy);
      const highWaterMark = extractHighWaterMark(strategy, 1);
      setUpWritableStreamDefaultControllerFromUnderlyingSink(
        this,
        underlyingSink,
        underlyingSinkDict,
        highWaterMark,
        sizeAlgorithm,
      );
    }

    /** @returns {boolean} */
    get locked() {
      return isWritableStreamLocked(this);
    }

    /**
     * @param {any=} reason 
     * @returns {Promise<void>}
     */
    abort(reason) {
      if (isWritableStreamLocked(this)) {
        return Promise.reject(
          new TypeError(
            "The writable stream is locked, therefore cannot be aborted.",
          ),
        );
      }
      return writableStreamAbort(this, reason);
    }

    /** @returns {Promise<void>} */
    close() {
      if (isWritableStreamLocked(this)) {
        return Promise.reject(
          new TypeError(
            "The writable stream is locked, therefore cannot be closed.",
          ),
        );
      }
      if (writableStreamCloseQueuedOrInFlight(this) === true) {
        return Promise.reject(
          new TypeError("The writable stream is already closing."),
        );
      }
      return writableStreamClose(this);
    }

    /** @returns {WritableStreamDefaultWriter<W>} */
    getWriter() {
      return acquireWritableStreamDefaultWriter(this);
    }

    [customInspect]() {
      return `${this.constructor.name} ${
        Deno.inspect({ locked: this.locked })
      }`;
    }
  }

  /** @template W */
  class WritableStreamDefaultWriter {
    /** @type {Deferred<void>} */
    [_closedPromise];

    /** @type {Deferred<void>} */
    [_readyPromise];

    /** @type {WritableStream<W>} */
    [_stream];

    constructor(stream) {
      setUpWritableStreamDefaultWriter(this, stream);
    }

    /** @returns {Promise<void>} */
    get closed() {
      return this[_closedPromise].promise;
    }

    /** @returns {number} */
    get desiredSize() {
      if (this[_stream] === undefined) {
        throw new TypeError(
          "A writable stream is not associated with the writer.",
        );
      }
      return writableStreamDefaultWriterGetDesiredSize(this);
    }

    /** @returns {Promise<void>} */
    get ready() {
      return this[_readyPromise].promise;
    }

    /**
     * @param {any} reason 
     * @returns {Promise<void>}
     */
    abort(reason) {
      if (this[_stream] === undefined) {
        return Promise.reject(
          new TypeError("A writable stream is not associated with the writer."),
        );
      }
      return writableStreamDefaultWriterAbort(this, reason);
    }

    /** @returns {Promise<void>} */
    close() {
      const stream = this[_stream];
      if (stream === undefined) {
        return Promise.reject(
          new TypeError("A writable stream is not associated with the writer."),
        );
      }
      if (writableStreamCloseQueuedOrInFlight(stream) === true) {
        return Promise.reject(
          new TypeError("The associated stream is already closing."),
        );
      }
      return writableStreamDefaultWriterClose(this);
    }

    /** @returns {void} */
    releaseLock() {
      const stream = this[_stream];
      if (stream === undefined) {
        return;
      }
      assert(stream[_writer] !== undefined);
      writableStreamDefaultWriterRelease(this);
    }

    /**
     * @param {W} chunk 
     * @returns {Promise<void>}
     */
    write(chunk) {
      if (this[_stream] === undefined) {
        return Promise.reject(
          new TypeError("A writable stream is not associate with the writer."),
        );
      }
      return writableStreamDefaultWriterWrite(this, chunk);
    }
  }

  /** @template W */
  class WritableStreamDefaultController {
    /** @type {(reason?: any) => Promise<void>} */
    [_abortAlgorithm];
    /** @type {() => Promise<void>} */
    [_closeAlgorithm];
    /** @type {ValueWithSize<W | _close>[]} */
    [_queue];
    /** @type {number} */
    [_queueTotalSize];
    /** @type {boolean} */
    [_started];
    /** @type {number} */
    [_strategyHWM];
    /** @type {(chunk: W) => number} */
    [_strategySizeAlgorithm];
    /** @type {WritableStream<W>} */
    [_stream];
    /** @type {(chunk: W, controller: this) => Promise<void>} */
    [_writeAlgorithm];

    /**
     * @param {any=} e
     * @returns {void}
     */
    error(e) {
      const state = this[_stream][_state];
      if (state !== "writable") {
        return;
      }
      writableStreamDefaultControllerError(this, e);
    }

    /**
     * @param {any=} reason 
     * @returns {Promise<void>}
     */
    [_abortSteps](reason) {
      const result = this[_abortAlgorithm](reason);
      writableStreamDefaultControllerClearAlgorithms(this);
      return result;
    }

    [_errorSteps]() {
      resetQueue(this);
    }
  }

  window.__bootstrap.streams = {
    // Non-Public
    isReadableStreamDisturbed,
    // Exposed in global runtime scope
    ByteLengthQueuingStrategy,
    CountQueuingStrategy,
    ReadableStream,
    ReadableStreamDefaultReader,
    TransformStream,
    WritableStream,
    WritableStreamDefaultWriter,
  };
})(this);
