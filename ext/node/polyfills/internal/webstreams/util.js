// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  ArrayBufferPrototypeGetByteLength,
  ArrayBufferPrototypeSlice,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  FunctionPrototypeCall,
  MathMax,
  NumberIsNaN,
  PromisePrototypeThen,
  ReflectGet,
  SymbolAsyncIterator,
  SymbolIterator,
  Uint8Array,
} = primordials;

const {
  ERR_ARG_NOT_ITERABLE,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_STATE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { validateFunction } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { inspect } = core.loadExtScript("ext:deno_node/util.ts");
const assert = core.loadExtScript("ext:deno_node/internal/assert.mjs").default;

const webStreams = core.loadExtScript("ext:deno_web/06_streams.js");

const kState = webStreams.kNodeWebStreamsState;
const kType = webStreams.kNodeWebStreamsType;

const AsyncIterator = {
  __proto__: Object.getPrototypeOf(
    Object.getPrototypeOf(Object.getPrototypeOf((async function* () {})())),
  ),
  next: undefined,
  return: undefined,
};

function extractHighWaterMark(value, defaultHWM) {
  if (value === undefined) return defaultHWM;
  value = +value;
  if (typeof value !== "number" || NumberIsNaN(value) || value < 0) {
    throw new ERR_INVALID_ARG_VALUE.RangeError(
      "strategy.highWaterMark",
      value,
    );
  }
  return value;
}

function extractSizeAlgorithm(size) {
  if (size === undefined) return () => 1;
  validateFunction(size, "strategy.size");
  return size;
}

function customInspect(depth, options, name, data) {
  if (depth < 0) return this;
  const opts = {
    ...options,
    depth: options.depth == null ? null : options.depth - 1,
  };
  return `${name} ${inspect(data, opts)}`;
}

function ArrayBufferViewGetBuffer(view) {
  return ReflectGet(view.constructor.prototype, "buffer", view);
}

function ArrayBufferViewGetByteLength(view) {
  return ReflectGet(view.constructor.prototype, "byteLength", view);
}

function ArrayBufferViewGetByteOffset(view) {
  return ReflectGet(view.constructor.prototype, "byteOffset", view);
}

function cloneAsUint8Array(view) {
  const buffer = ArrayBufferViewGetBuffer(view);
  const byteOffset = ArrayBufferViewGetByteOffset(view);
  const byteLength = ArrayBufferViewGetByteLength(view);
  return new Uint8Array(
    ArrayBufferPrototypeSlice(buffer, byteOffset, byteOffset + byteLength),
  );
}

function canCopyArrayBuffer(
  toBuffer,
  toIndex,
  fromBuffer,
  fromIndex,
  count,
) {
  return toBuffer !== fromBuffer &&
    toIndex + count <= ArrayBufferPrototypeGetByteLength(toBuffer) &&
    fromIndex + count <= ArrayBufferPrototypeGetByteLength(fromBuffer);
}

function copyArrayBuffer(src, srcOffset, dst, dstOffset, size) {
  new Uint8Array(dst, dstOffset, size).set(
    new Uint8Array(src, srcOffset, size),
  );
}

function isBrandCheck(brand) {
  return (value) =>
    value != null && value[kState] !== undefined && value[kType] === brand;
}

function dequeueValue(controller) {
  assert(controller[kState].queue !== undefined);
  assert(controller[kState].queueTotalSize !== undefined);
  assert(controller[kState].queue.length);
  const { value, size } = ArrayPrototypeShift(controller[kState].queue);
  controller[kState].queueTotalSize = MathMax(
    0,
    controller[kState].queueTotalSize - size,
  );
  return value;
}

function resetQueue(controller) {
  assert(controller[kState].queue !== undefined);
  assert(controller[kState].queueTotalSize !== undefined);
  controller[kState].queue.length = 0;
}

function peekQueueValue(controller) {
  assert(controller[kState].queue !== undefined);
  assert(controller[kState].queueTotalSize !== undefined);
  assert(controller[kState].queue.length);
  return controller[kState].queue[0].value;
}

function enqueueValueWithSize(controller, value, size) {
  assert(controller[kState].queue !== undefined);
  assert(controller[kState].queueTotalSize !== undefined);
  size = +size;
  if (
    typeof size !== "number" ||
    NumberIsNaN(size) ||
    size < 0 ||
    size === Infinity
  ) {
    throw new ERR_INVALID_ARG_VALUE.RangeError("size", size);
  }
  ArrayPrototypePush(controller[kState].queue, { value, size });
  controller[kState].queueTotalSize += size;
}

function isPromisePending(promise) {
  if (promise === undefined || !core.isPromise(promise)) return false;
  return core.getPromiseDetails(promise)[0] === 0;
}

function setPromiseHandled(promise) {
  PromisePrototypeThen(promise, undefined, () => {});
}

function createPromiseCallback(name, callback, thisArg, ...args) {
  return (...extraArgs) => {
    try {
      return Promise.resolve(
        FunctionPrototypeCall(callback, thisArg, ...args, ...extraArgs),
      );
    } catch (err) {
      return Promise.reject(err);
    }
  };
}

function invokePromiseCallback(callback, thisArg, ...args) {
  return Promise.resolve(FunctionPrototypeCall(callback, thisArg, ...args));
}

function nonOpStart() {}
function nonOpPull() {
  return Promise.resolve();
}
function nonOpCancel() {
  return Promise.resolve();
}
function nonOpWrite() {
  return Promise.resolve();
}
function nonOpClose() {
  return Promise.resolve();
}
function nonOpFlush() {
  return Promise.resolve();
}

function lazyTransfer() {
  return core.loadExtScript("ext:deno_node/internal/worker/js_transferable.js");
}

function createAsyncFromSyncIterator(syncIteratorRecord) {
  const asyncIterator = {
    next() {
      return Promise.resolve(iteratorNext(syncIteratorRecord));
    },
    return() {
      const iterator = syncIteratorRecord.iterator;
      if (iterator.return === undefined) {
        return Promise.resolve({ done: true, value: undefined });
      }
      return Promise.resolve(FunctionPrototypeCall(iterator.return, iterator));
    },
    [SymbolAsyncIterator]() {
      return this;
    },
  };
  return {
    iterator: asyncIterator,
    nextMethod: asyncIterator.next,
    done: false,
  };
}

function getIterator(obj, kind = "sync", method) {
  if (method === undefined) {
    if (kind === "async") {
      method = obj[SymbolAsyncIterator];
      if (method == null) {
        const syncMethod = obj[SymbolIterator];
        if (syncMethod === undefined) {
          throw new ERR_ARG_NOT_ITERABLE(obj);
        }
        return createAsyncFromSyncIterator(
          getIterator(obj, "sync", syncMethod),
        );
      }
    } else {
      method = obj[SymbolIterator];
    }
  }
  if (method === undefined) {
    throw new ERR_ARG_NOT_ITERABLE(obj);
  }
  const iterator = FunctionPrototypeCall(method, obj);
  if (typeof iterator !== "object" || iterator === null) {
    throw new ERR_INVALID_STATE("The iterator method must return an object");
  }
  return { iterator, nextMethod: iterator.next, done: false };
}

function iteratorNext(iteratorRecord, value) {
  const result = value === undefined
    ? FunctionPrototypeCall(iteratorRecord.nextMethod, iteratorRecord.iterator)
    : FunctionPrototypeCall(
      iteratorRecord.nextMethod,
      iteratorRecord.iterator,
      value,
    );
  if (typeof result !== "object" || result === null) {
    throw new ERR_INVALID_STATE(
      "The iterator.next() method must return an object",
    );
  }
  return result;
}

return {
  ArrayBufferViewGetBuffer,
  ArrayBufferViewGetByteLength,
  ArrayBufferViewGetByteOffset,
  AsyncIterator,
  canCopyArrayBuffer,
  createPromiseCallback,
  cloneAsUint8Array,
  copyArrayBuffer,
  customInspect,
  dequeueValue,
  enqueueValueWithSize,
  extractHighWaterMark,
  extractSizeAlgorithm,
  lazyTransfer,
  invokePromiseCallback,
  isBrandCheck,
  isPromisePending,
  peekQueueValue,
  resetQueue,
  setPromiseHandled,
  nonOpCancel,
  nonOpFlush,
  nonOpPull,
  nonOpStart,
  nonOpWrite,
  getIterator,
  iteratorNext,
  kType,
  kState,
};
})();
