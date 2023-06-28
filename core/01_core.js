// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Array,
    ArrayPrototypeFill,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    Error,
    ErrorCaptureStackTrace,
    MapPrototypeDelete,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeSet,
    ObjectAssign,
    ObjectDefineProperty,
    ObjectFreeze,
    ObjectFromEntries,
    ObjectKeys,
    Promise,
    PromiseReject,
    PromiseResolve,
    PromisePrototypeThen,
    Proxy,
    RangeError,
    ReferenceError,
    ReflectHas,
    ReflectApply,
    SafeArrayIterator,
    SafeMap,
    SafePromisePrototypeFinally,
    StringPrototypeSlice,
    StringPrototypeSplit,
    SymbolFor,
    SyntaxError,
    TypeError,
    URIError,
    setQueueMicrotask,
  } = window.__bootstrap.primordials;
  const { ops, asyncOps } = window.Deno.core;

  const build = {
    target: "unknown",
    arch: "unknown",
    os: "unknown",
    vendor: "unknown",
    env: undefined,
  };

  function setBuildInfo(target) {
    const { 0: arch, 1: vendor, 2: os, 3: env } = StringPrototypeSplit(
      target,
      "-",
      4,
    );
    build.target = target;
    build.arch = arch;
    build.vendor = vendor;
    build.os = os;
    build.env = env;
    ObjectFreeze(build);
  }

  const errorMap = {};
  // Builtin v8 / JS errors
  registerErrorClass("Error", Error);
  registerErrorClass("RangeError", RangeError);
  registerErrorClass("ReferenceError", ReferenceError);
  registerErrorClass("SyntaxError", SyntaxError);
  registerErrorClass("TypeError", TypeError);
  registerErrorClass("URIError", URIError);

  let nextPromiseId = 1;
  const promiseMap = new SafeMap();
  const RING_SIZE = 4 * 1024;
  const NO_PROMISE = null; // Alias to null is faster than plain nulls
  const promiseRing = ArrayPrototypeFill(new Array(RING_SIZE), NO_PROMISE);
  // TODO(bartlomieju): it future use `v8::Private` so it's not visible
  // to users. Currently missing bindings.
  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  let opCallTracingEnabled = false;
  const opCallTraces = new SafeMap();

  function enableOpCallTracing() {
    opCallTracingEnabled = true;
  }

  function isOpCallTracingEnabled() {
    return opCallTracingEnabled;
  }

  function movePromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Move old promise from ring to map
    const oldPromise = promiseRing[idx];
    if (oldPromise !== NO_PROMISE) {
      const oldPromiseId = promiseId - RING_SIZE;
      MapPrototypeSet(promiseMap, oldPromiseId, oldPromise);
    }
    return promiseRing[idx] = NO_PROMISE;
  }

  function setPromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Move old promise from ring to map
    const oldPromise = promiseRing[idx];
    if (oldPromise !== NO_PROMISE) {
      const oldPromiseId = promiseId - RING_SIZE;
      MapPrototypeSet(promiseMap, oldPromiseId, oldPromise);
    }
    // Set new promise
    return promiseRing[idx] = newPromise();
  }

  function getPromise(promiseId) {
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      const promise = MapPrototypeGet(promiseMap, promiseId);
      MapPrototypeDelete(promiseMap, promiseId);
      return promise;
    }
    // Otherwise take from ring
    const idx = promiseId % RING_SIZE;
    const promise = promiseRing[idx];
    promiseRing[idx] = NO_PROMISE;
    return promise;
  }

  function newPromise() {
    let resolve, reject;
    const promise = new Promise((resolve_, reject_) => {
      resolve = resolve_;
      reject = reject_;
    });
    promise.resolve = resolve;
    promise.reject = reject;
    return promise;
  }

  function hasPromise(promiseId) {
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      return MapPrototypeHas(promiseMap, promiseId);
    }
    // Otherwise check it in ring
    const idx = promiseId % RING_SIZE;
    return promiseRing[idx] != NO_PROMISE;
  }

  const macrotaskCallbacks = [];
  const nextTickCallbacks = [];

  function setMacrotaskCallback(cb) {
    ArrayPrototypePush(macrotaskCallbacks, cb);
  }

  function setNextTickCallback(cb) {
    ArrayPrototypePush(nextTickCallbacks, cb);
  }

  // This function has variable number of arguments. The last argument describes
  // if there's a "next tick" scheduled by the Node.js compat layer. Arguments
  // before last are alternating integers and any values that describe the
  // responses of async ops.
  function eventLoopTick() {
    // First respond to all pending ops.
    for (let i = 0; i < arguments.length - 1; i += 2) {
      const promiseId = arguments[i];
      const res = arguments[i + 1];
      const promise = getPromise(promiseId);
      promise.resolve(res);
    }
    // Drain nextTick queue if there's a tick scheduled.
    if (arguments[arguments.length - 1]) {
      for (let i = 0; i < nextTickCallbacks.length; i++) {
        nextTickCallbacks[i]();
      }
    } else {
      ops.op_run_microtasks();
    }
    // Finally drain macrotask queue.
    for (let i = 0; i < macrotaskCallbacks.length; i++) {
      const cb = macrotaskCallbacks[i];
      while (true) {
        const res = cb();

        // If callback returned `undefined` then it has no work to do, we don't
        // need to perform microtask checkpoint.
        if (res === undefined) {
          break;
        }

        ops.op_run_microtasks();
        // If callback returned `true` then it has no more work to do, stop
        // calling it then.
        if (res === true) {
          break;
        }
      }
    }
  }

  function registerErrorClass(className, errorClass) {
    registerErrorBuilder(className, (msg) => new errorClass(msg));
  }

  function registerErrorBuilder(className, errorBuilder) {
    if (typeof errorMap[className] !== "undefined") {
      throw new TypeError(`Error class for "${className}" already registered`);
    }
    errorMap[className] = errorBuilder;
  }

  function buildCustomError(className, message, code) {
    let error;
    try {
      error = errorMap[className]?.(message);
    } catch (e) {
      throw new Error(
        `Unable to build custom error for "${className}"\n  ${e.message}`,
      );
    }
    // Strip buildCustomError() calls from stack trace
    if (typeof error == "object") {
      ErrorCaptureStackTrace(error, buildCustomError);
      if (code) {
        error.code = code;
      }
    }
    return error;
  }

  function unwrapOpError(hideFunction) {
    return (res) => {
      // .$err_class_name is a special key that should only exist on errors
      const className = res?.$err_class_name;
      if (!className) {
        return res;
      }

      const errorBuilder = errorMap[className];
      const err = errorBuilder ? errorBuilder(res.message) : new Error(
        `Unregistered error class: "${className}"\n  ${res.message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
      );
      // Set .code if error was a known OS error, see error_codes.rs
      if (res.code) {
        err.code = res.code;
      }
      // Strip unwrapOpResult() and errorBuilder() calls from stack trace
      ErrorCaptureStackTrace(err, hideFunction);
      throw err;
    };
  }

  function unwrapOpResultNewPromise(id, res, hideFunction) {
    // .$err_class_name is a special key that should only exist on errors
    if (res?.$err_class_name) {
      const className = res.$err_class_name;
      const errorBuilder = errorMap[className];
      const err = errorBuilder ? errorBuilder(res.message) : new Error(
        `Unregistered error class: "${className}"\n  ${res.message}\n  Classes of errors returned from ops should be registered via Deno.core.registerErrorClass().`,
      );
      // Set .code if error was a known OS error, see error_codes.rs
      if (res.code) {
        err.code = res.code;
      }
      // Strip unwrapOpResult() and errorBuilder() calls from stack trace
      ErrorCaptureStackTrace(err, hideFunction);
      return PromiseReject(err);
    }
    const promise = PromiseResolve(res);
    promise[promiseIdSymbol] = id;
    return promise;
  }

  /*
Basic codegen.

TODO(mmastrac): automate this (handlebars?)

let s = "";
const vars = "abcdefghijklm";
for (let i = 0; i < 10; i++) {
  let args = "";
  for (let j = 0; j < i; j++) {
    args += `${vars[j]},`;
  }
  s += `
      case ${i}:
        fn = function async_op_${i}(${args}) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, ${args});
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_${i});
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_${i});
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(setPromise(id), unwrapOpError(eventLoopTick));
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;
  `;
}
  */

  // This function is called once per async stub
  function asyncStub(opName, args) {
    setUpAsyncStub(opName);
    return ReflectApply(ops[opName], undefined, args);
  }

  function setUpAsyncStub(opName) {
    const originalOp = asyncOps[opName];
    let fn;
    // The body of this switch statement can be generated using the script above.
    switch (originalOp.length - 1) {
      case 0:
        fn = function async_op_0() {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_0);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_0);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 1:
        fn = function async_op_1(a) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_1);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_1);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 2:
        fn = function async_op_2(a, b) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_2);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_2);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 3:
        fn = function async_op_3(a, b, c) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_3);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_3);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 4:
        fn = function async_op_4(a, b, c, d) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_4);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_4);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 5:
        fn = function async_op_5(a, b, c, d, e) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d, e);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_5);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_5);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 6:
        fn = function async_op_6(a, b, c, d, e, f) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d, e, f);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_6);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_6);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 7:
        fn = function async_op_7(a, b, c, d, e, f, g) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d, e, f, g);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_7);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_7);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 8:
        fn = function async_op_8(a, b, c, d, e, f, g, h) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d, e, f, g, h);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_8);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_8);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      case 9:
        fn = function async_op_9(a, b, c, d, e, f, g, h, i) {
          const id = nextPromiseId++;
          try {
            const maybeResult = originalOp(id, a, b, c, d, e, f, g, h, i);
            if (maybeResult !== undefined) {
              movePromise(id);
              return unwrapOpResultNewPromise(id, maybeResult, async_op_9);
            }
          } catch (err) {
            movePromise(id);
            ErrorCaptureStackTrace(err, async_op_9);
            return PromiseReject(err);
          }
          let promise = PromisePrototypeThen(
            setPromise(id),
            unwrapOpError(eventLoopTick),
          );
          promise = handleOpCallTracing(opName, id, promise);
          promise[promiseIdSymbol] = id;
          return promise;
        };
        break;

      default:
        throw new Error(
          `Too many arguments for async op codegen (length of ${opName} was ${
            originalOp.length - 1
          })`,
        );
    }
    ObjectDefineProperty(fn, "name", {
      value: opName,
      configurable: false,
      writable: false,
    });
    return (ops[opName] = fn);
  }

  function opAsync(name, ...args) {
    const id = nextPromiseId++;
    try {
      const maybeResult = asyncOps[name](id, ...new SafeArrayIterator(args));
      if (maybeResult !== undefined) {
        movePromise(id);
        return unwrapOpResultNewPromise(id, maybeResult, opAsync);
      }
    } catch (err) {
      movePromise(id);
      if (!ReflectHas(asyncOps, name)) {
        return PromiseReject(new TypeError(`${name} is not a registered op`));
      }
      ErrorCaptureStackTrace(err, opAsync);
      return PromiseReject(err);
    }
    let promise = PromisePrototypeThen(
      setPromise(id),
      unwrapOpError(eventLoopTick),
    );
    promise = handleOpCallTracing(name, id, promise);
    promise[promiseIdSymbol] = id;
    return promise;
  }

  function handleOpCallTracing(opName, promiseId, p) {
    if (opCallTracingEnabled) {
      const stack = StringPrototypeSlice(new Error().stack, 6);
      MapPrototypeSet(opCallTraces, promiseId, { opName, stack });
      return SafePromisePrototypeFinally(
        p,
        () => MapPrototypeDelete(opCallTraces, promiseId),
      );
    } else {
      return p;
    }
  }

  function refOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    ops.op_ref_op(promiseId);
  }

  function unrefOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    ops.op_unref_op(promiseId);
  }

  function resources() {
    return ObjectFromEntries(ops.op_resources());
  }

  function metrics() {
    const { 0: aggregate, 1: perOps } = ops.op_metrics();
    aggregate.ops = ObjectFromEntries(ArrayPrototypeMap(
      ops.op_op_names(),
      (opName, opId) => [opName, perOps[opId]],
    ));
    return aggregate;
  }

  let reportExceptionCallback = undefined;

  // Used to report errors thrown from functions passed to `queueMicrotask()`.
  // The callback will be passed the thrown error. For example, you can use this
  // to dispatch an error event to the global scope.
  // In other words, set the implementation for
  // https://html.spec.whatwg.org/multipage/webappapis.html#report-the-exception
  function setReportExceptionCallback(cb) {
    if (typeof cb != "function") {
      throw new TypeError("expected a function");
    }
    reportExceptionCallback = cb;
  }

  function queueMicrotask(cb) {
    if (typeof cb != "function") {
      throw new TypeError("expected a function");
    }
    return ops.op_queue_microtask(() => {
      try {
        cb();
      } catch (error) {
        if (reportExceptionCallback) {
          reportExceptionCallback(error);
        } else {
          throw error;
        }
      }
    });
  }

  // Some "extensions" rely on "BadResource" and "Interrupted" errors in the
  // JS code (eg. "deno_net") so they are provided in "Deno.core" but later
  // reexported on "Deno.errors"
  class BadResource extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BadResource";
    }
  }
  const BadResourcePrototype = BadResource.prototype;

  class Interrupted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Interrupted";
    }
  }
  const InterruptedPrototype = Interrupted.prototype;

  const promiseHooks = [
    [], // init
    [], // before
    [], // after
    [], // resolve
  ];

  function setPromiseHooks(init, before, after, resolve) {
    const hooks = [init, before, after, resolve];
    for (let i = 0; i < hooks.length; i++) {
      const hook = hooks[i];
      // Skip if no callback was provided for this hook type.
      if (hook == null) {
        continue;
      }
      // Verify that the type of `hook` is a function.
      if (typeof hook !== "function") {
        throw new TypeError(`Expected function at position ${i}`);
      }
      // Add the hook to the list.
      ArrayPrototypePush(promiseHooks[i], hook);
    }

    const wrappedHooks = ArrayPrototypeMap(promiseHooks, (hooks) => {
      switch (hooks.length) {
        case 0:
          return undefined;
        case 1:
          return hooks[0];
        case 2:
          return create2xHookWrapper(hooks[0], hooks[1]);
        case 3:
          return create3xHookWrapper(hooks[0], hooks[1], hooks[2]);
        default:
          return createHookListWrapper(hooks);
      }

      // The following functions are used to create wrapper functions that call
      // all the hooks in a list of a certain length. The reason to use a
      // function that creates a wrapper is to minimize the number of objects
      // captured in the closure.
      function create2xHookWrapper(hook1, hook2) {
        return function (promise, parent) {
          hook1(promise, parent);
          hook2(promise, parent);
        };
      }
      function create3xHookWrapper(hook1, hook2, hook3) {
        return function (promise, parent) {
          hook1(promise, parent);
          hook2(promise, parent);
          hook3(promise, parent);
        };
      }
      function createHookListWrapper(hooks) {
        return function (promise, parent) {
          for (let i = 0; i < hooks.length; i++) {
            const hook = hooks[i];
            hook(promise, parent);
          }
        };
      }
    });

    ops.op_set_promise_hooks(
      wrappedHooks[0],
      wrappedHooks[1],
      wrappedHooks[2],
      wrappedHooks[3],
    );
  }

  // Eagerly initialize ops for snapshot purposes
  for (const opName of new SafeArrayIterator(ObjectKeys(asyncOps))) {
    setUpAsyncStub(opName);
  }

  function ensureFastOps() {
    return new Proxy({}, {
      get(_target, opName) {
        if (ops[opName] === undefined) {
          throw new Error(`Unknown or disabled op '${opName}'`);
        }
        if (asyncOps[opName] !== undefined) {
          return setUpAsyncStub(opName);
        } else {
          return ops[opName];
        }
      },
    });
  }

  const {
    op_close: close,
    op_try_close: tryClose,
    op_read: read,
    op_read_all: readAll,
    op_write: write,
    op_write_all: writeAll,
    op_read_sync: readSync,
    op_write_sync: writeSync,
    op_shutdown: shutdown,
  } = ensureFastOps();

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    asyncStub,
    ensureFastOps,
    opAsync,
    resources,
    metrics,
    registerErrorBuilder,
    registerErrorClass,
    buildCustomError,
    eventLoopTick,
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    enableOpCallTracing,
    isOpCallTracingEnabled,
    opCallTraces,
    refOp,
    unrefOp,
    setReportExceptionCallback,
    setPromiseHooks,
    close,
    tryClose,
    read,
    readAll,
    write,
    writeAll,
    readSync,
    writeSync,
    shutdown,
    print: (msg, isErr) => ops.op_print(msg, isErr),
    setMacrotaskCallback,
    setNextTickCallback,
    runMicrotasks: () => ops.op_run_microtasks(),
    hasTickScheduled: () => ops.op_has_tick_scheduled(),
    setHasTickScheduled: (bool) => ops.op_set_has_tick_scheduled(bool),
    evalContext: (
      source,
      specifier,
    ) => ops.op_eval_context(source, specifier),
    createHostObject: () => ops.op_create_host_object(),
    encode: (text) => ops.op_encode(text),
    decode: (buffer) => ops.op_decode(buffer),
    serialize: (
      value,
      options,
      errorCallback,
    ) => ops.op_serialize(value, options, errorCallback),
    deserialize: (buffer, options) => ops.op_deserialize(buffer, options),
    getPromiseDetails: (promise) => ops.op_get_promise_details(promise),
    getProxyDetails: (proxy) => ops.op_get_proxy_details(proxy),
    isProxy: (value) => ops.op_is_proxy(value),
    memoryUsage: () => ops.op_memory_usage(),
    setWasmStreamingCallback: (fn) => ops.op_set_wasm_streaming_callback(fn),
    abortWasmStreaming: (
      rid,
      error,
    ) => ops.op_abort_wasm_streaming(rid, error),
    destructureError: (error) => ops.op_destructure_error(error),
    opNames: () => ops.op_op_names(),
    eventLoopHasMoreWork: () => ops.op_event_loop_has_more_work(),
    setPromiseRejectCallback: (fn) => ops.op_set_promise_reject_callback(fn),
    byteLength: (str) => ops.op_str_byte_length(str),
    build,
    setBuildInfo,
  });

  ObjectAssign(globalThis.__bootstrap, { core });
  const internals = {};
  ObjectAssign(globalThis.__bootstrap, { internals });
  ObjectAssign(globalThis.Deno, { core });

  // Direct bindings on `globalThis`
  ObjectAssign(globalThis, { queueMicrotask });
  setQueueMicrotask(queueMicrotask);
})(globalThis);
