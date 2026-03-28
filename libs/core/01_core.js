// Copyright 2018-2026 the Deno authors. MIT license.
"use strict";

((window) => {
  const {
    ArrayPrototypeMap,
    ArrayPrototypePush,
    Error,
    ErrorCaptureStackTrace,
    FunctionPrototypeBind,
    ObjectAssign,
    ObjectFreeze,
    ObjectFromEntries,
    ObjectKeys,
    ObjectHasOwn,
    setQueueMicrotask,
    SafeMap,
    SafeWeakMap,
    StringPrototypeSlice,
    Symbol,
    SymbolFor,
    TypedArrayPrototypeGetLength,
    TypedArrayPrototypeJoin,
    TypedArrayPrototypeSlice,
    TypedArrayPrototypeGetSymbolToStringTag,
    TypeError,
  } = window.__bootstrap.primordials;
  const {
    ops,
    hasPromise,
    promiseIdSymbol,
    registerErrorClass,
  } = window.Deno.core;
  const {
    __setLeakTracingEnabled,
    __isLeakTracingEnabled,
    __initializeCoreMethods,
    __resolvePromise,
    FixedQueue,
  } = window.__infra;
  const __timers = window.__timers;
  delete window.__timers;
  const {
    op_abort_wasm_streaming,
    op_current_user_call_site,
    op_compile_function,
    op_decode,
    op_deserialize,
    op_destructure_error,
    op_dispatch_exception,
    op_encode,
    op_encode_binary_string,
    op_eval_context,
    op_structured_clone,
    op_event_loop_has_more_work,
    op_get_extras_binding_object,
    op_get_promise_details,
    op_get_proxy_details,
    op_get_ext_import_meta_proto,
    op_drain_pending_rejections,
    op_lazy_load_esm,
    op_memory_usage,
    op_op_names,
    op_print,
    op_queue_microtask,
    op_ref_op,
    op_resources,
    op_run_microtasks,
    op_serialize,
    op_add_main_module_handler,
    op_set_handled_promise_rejection_handler,
    op_set_promise_hooks,
    op_set_wasm_streaming_callback,
    op_str_byte_length,
    op_unref_op,
    op_cancel_handle,
    op_leak_tracing_enable,
    op_leak_tracing_submit,
    op_leak_tracing_get_all,
    op_leak_tracing_get,
    op_immediate_check,

    op_is_any_array_buffer,
    op_is_arguments_object,
    op_is_array_buffer,
    op_is_array_buffer_view,
    op_is_async_function,
    op_is_big_int_object,
    op_is_boolean_object,
    op_is_boxed_primitive,
    op_is_data_view,
    op_is_date,
    op_is_generator_function,
    op_is_generator_object,
    op_is_map,
    op_is_map_iterator,
    op_is_module_namespace_object,
    op_is_native_error,
    op_is_number_object,
    op_is_promise,
    op_is_proxy,
    op_is_reg_exp,
    op_is_set,
    op_is_set_iterator,
    op_is_shared_array_buffer,
    op_is_string_object,
    op_is_symbol_object,
    op_is_typed_array,
    op_is_weak_map,
    op_is_weak_set,
  } = ops;

  const {
    getContinuationPreservedEmbedderData,
    setContinuationPreservedEmbedderData,
  } = op_get_extras_binding_object();

  // core/infra collaborative code
  delete window.__infra;

  __initializeCoreMethods(
    submitLeakTrace,
  );

  function submitLeakTrace(id) {
    const error = new Error();
    ErrorCaptureStackTrace(error, submitLeakTrace);
    // "Error\n".length == 6
    op_leak_tracing_submit(0, id, StringPrototypeSlice(error.stack, 6));
  }

  let unhandledPromiseRejectionHandler = () => false;

  // ---------------------------------------------------------------------------
  // Immediate queue (ImmediateList linked list + drain loop)
  // ---------------------------------------------------------------------------
  class ImmediateList {
    constructor() {
      this.head = null;
      this.tail = null;
    }
    append(item) {
      if (this.tail !== null) {
        this.tail._idleNext = item;
        item._idlePrev = this.tail;
      } else {
        this.head = item;
      }
      this.tail = item;
    }
    remove(item) {
      if (item._idleNext) {
        item._idleNext._idlePrev = item._idlePrev;
      }
      if (item._idlePrev) {
        item._idlePrev._idleNext = item._idleNext;
      }
      if (item === this.head) {
        this.head = item._idleNext;
      }
      if (item === this.tail) {
        this.tail = item._idlePrev;
      }
      item._idleNext = null;
      item._idlePrev = null;
    }
  }

  const immediateQueue = new ImmediateList();
  const outstandingQueue = new ImmediateList();

  // Shared buffer with Rust - avoids JS-to-Rust op calls for immediate info.
  // Indices: 0 = count, 1 = ref_count, 2 = has_outstanding
  const kImmCount = 0;
  const kImmRefCount = 1;
  const kImmHasOutstanding = 2;
  const kRefed = Symbol("refed");
  let immediateInfo;

  function queueImmediate(immediate) {
    immediateInfo[kImmCount]++;
    immediateQueue.append(immediate);
  }

  function clearImmediate(immediate) {
    if (!immediate || immediate._destroyed) {
      return;
    }
    immediateInfo[kImmCount]--;
    immediate._destroyed = true;
    if (immediate[kRefed]) {
      immediateInfo[kImmRefCount]--;
      if (immediateInfo[kImmRefCount] === 0) {
        op_immediate_check(false);
      }
    }
    immediate[kRefed] = null;
    immediate._onImmediate = null;
    immediateQueue.remove(immediate);
  }

  function runImmediates() {
    const queue = outstandingQueue.head !== null
      ? outstandingQueue
      : immediateQueue;
    let immediate = queue.head;
    if (queue !== outstandingQueue) {
      queue.head = queue.tail = null;
      immediateInfo[kImmHasOutstanding] = 1;
    }

    let prevImmediate;
    let ranAtLeastOneImmediate = false;
    while (immediate !== null) {
      if (ranAtLeastOneImmediate) {
        runNextTicks();
      } else {
        ranAtLeastOneImmediate = true;
      }

      if (immediate._destroyed) {
        outstandingQueue.head = immediate = prevImmediate._idleNext;
        continue;
      }

      immediate._destroyed = true;

      immediateInfo[kImmCount]--;
      if (immediate[kRefed]) {
        immediateInfo[kImmRefCount]--;
        if (immediateInfo[kImmRefCount] === 0) {
          op_immediate_check(false);
        }
      }
      immediate[kRefed] = null;

      prevImmediate = immediate;

      const asyncId = immediate.asyncId;
      emitBefore(asyncId, immediate.triggerAsyncId, immediate);

      try {
        const argv = immediate._argv;
        if (!argv) {
          immediate._onImmediate();
        } else {
          immediate._onImmediate(...argv);
        }
      } finally {
        immediate._onImmediate = null;
        emitDestroy(asyncId);
        outstandingQueue.head = immediate = immediate._idleNext;
      }

      emitAfter(asyncId);
    }

    if (queue === outstandingQueue) {
      outstandingQueue.head = null;
    }

    immediateInfo[kImmHasOutstanding] = 0;
  }

  // ---------------------------------------------------------------------------
  // NextTick queue
  //
  // Closely mirrors Node.js lib/internal/process/task_queues.js.
  // The queue and drain loop live here in core; Node-specific concerns
  // (validation, async hooks init, exit check) stay in ext/node/.
  // ---------------------------------------------------------------------------
  const queue = new FixedQueue();

  // Async hook emit functions. Default to no-ops; ext/node/ replaces them
  // at bootstrap via setAsyncHooksEmit() with the real implementations
  // from async_hooks.ts (emitBefore, emitAfter, emitDestroy).
  let emitBefore = (_asyncId, _triggerAsyncId, _resource) => {};
  let emitAfter = (_asyncId) => {};
  let emitDestroy = (_asyncId) => {};

  function setAsyncHooksEmit(before, after, destroy) {
    emitBefore = before;
    emitAfter = after;
    emitDestroy = destroy;
  }

  // Shared buffer with Rust - avoids JS-to-Rust op calls for tick scheduling.
  // Index 0: hasTickScheduled
  // Index 1: hasRejectionToWarn (set by Rust in promise_reject_callback)
  // Set by Rust during store_js_callbacks via Deno.core.__tickInfo.
  const kHasTickScheduled = 0;
  const kHasRejectionToWarn = 1;
  let tickInfo;

  function hasTickScheduled() {
    return tickInfo[kHasTickScheduled] === 1;
  }

  function hasRejectionToWarn() {
    return tickInfo[kHasRejectionToWarn] === 1;
  }

  function setHasRejectionToWarn(value) {
    tickInfo[kHasRejectionToWarn] = value ? 1 : 0;
  }

  function setHasTickScheduled(value) {
    tickInfo[kHasTickScheduled] = value ? 1 : 0;
  }

  // Enqueue a tick object. The object must have { callback, args } and
  // an async context snapshot. It may contain additional fields for
  // async hooks (asyncId, triggerAsyncId).
  function queueNextTick(tickObject) {
    if (queue.isEmpty()) {
      setHasTickScheduled(true);
    }
    queue.push(tickObject);
  }

  // Drain pending promise rejections from the Rust-side queue and process
  // them through the unhandledPromiseRejectionHandler. Returns true if any
  // rejections were processed (matching Node.js processPromiseRejections).
  function processPromiseRejections() {
    tickInfo[kHasRejectionToWarn] = 0;
    const rejections = op_drain_pending_rejections();
    if (rejections === undefined) {
      return false;
    }
    for (let i = 0; i < rejections.length; i += 3) {
      const prevContext = getAsyncContext();
      setAsyncContext(rejections[i + 2]);
      try {
        const handled = unhandledPromiseRejectionHandler(
          rejections[i],
          rejections[i + 1],
        );
        if (!handled) {
          const err = rejections[i + 1];
          op_dispatch_exception(err, true);
        }
      } finally {
        setAsyncContext(prevContext);
      }
    }
    return true;
  }

  // Matches Node.js processTicksAndRejections() from
  // lib/internal/process/task_queues.js
  function processTicksAndRejections() {
    let tock;
    do {
      // deno-lint-ignore no-cond-assign
      while ((tock = queue.shift()) !== null) {
        const oldContext = getAsyncContext();
        setAsyncContext(tock.snapshot);

        const asyncId = tock.asyncId;
        emitBefore(asyncId, tock.triggerAsyncId, tock);

        try {
          const callback = tock.callback;
          if (tock.args === undefined) {
            callback();
          } else {
            const args = tock.args;
            switch (args.length) {
              case 1:
                callback(args[0]);
                break;
              case 2:
                callback(args[0], args[1]);
                break;
              case 3:
                callback(args[0], args[1], args[2]);
                break;
              case 4:
                callback(args[0], args[1], args[2], args[3]);
                break;
              default:
                callback(...args);
            }
          }
          emitAfter(asyncId);
        } catch (e) {
          // In Node.js, errors propagate from JS to C++ (TryCatch in
          // node_task_queue.cc InternalCallbackScope::Close), which calls
          // TriggerUncaughtException and then re-enters the drain loop.
          // We approximate this by catching here and routing through
          // reportExceptionCallback (which triggers uncaughtException),
          // then continuing the drain loop.
          reportExceptionCallback(e);
        } finally {
          emitDestroy(asyncId);
        }

        setAsyncContext(oldContext);
      }
      op_run_microtasks();
    } while (!queue.isEmpty() || processPromiseRejections());
    setHasTickScheduled(false);
    setHasRejectionToWarn(false);
  }

  // Flush microtasks and drain the nextTick queue if work is pending.
  // Under Explicit microtask policy, microtasks (promise continuations)
  // don't run automatically. We flush them here so that any ticks they
  // schedule are discovered and drained in the same iteration.
  //
  // IMPORTANT: When ticks are already scheduled, we skip the microtask
  // flush and go straight to processTicksAndRejections, which drains
  // ticks BEFORE running microtasks. This preserves the Node.js
  // invariant that nextTick callbacks fire before Promise.then
  // continuations in the same event loop phase.
  //
  // This is the single drain function used by: __eventLoopTick (from
  // Rust), __drainNextTickAndMacrotasks (I/O tight loop), runNextTicks
  // (interleaved between timer/immediate callbacks), and runImmediates.
  function drainTicks() {
    if (!hasTickScheduled() && !hasRejectionToWarn()) {
      op_run_microtasks();
      if (!hasTickScheduled() && !hasRejectionToWarn()) {
        return;
      }
    }
    processTicksAndRejections();
  }

  // Alias for timer/immediate interleaving (matches Node.js name).
  const runNextTicks = drainTicks;

  // Wire runNextTicks into the timer module so processTimers can
  // interleave nextTick drains between timer callbacks.
  __timers.setRunNextTicks(runNextTicks);
  // Wire reportException so timer callback errors are dispatched
  // via the uncaught exception handler rather than propagating.
  // Use a wrapper since reportExceptionCallback is defined later.
  __timers.setReportException((e) => reportExceptionCallback(e));

  // Shared buffer for timer next-expiry, backed by ContextState::timer_expiry.
  // JS writes after processing timers; Rust reads to schedule next wake-up.
  //   positive = next expiry (has refed timers)
  //   negative = next expiry negated (only unrefed timers)
  //   0.0 = no timers remain
  let timerExpiry;

  // Combined event loop tick: process timers + resolve ops.
  // Called from Rust with args: (timerNow, promiseId, isOk, res, ...)
  // timerNow > 0 means timers should be processed; 0 means skip.
  // Remaining args are completed async op results in triplets.
  //
  // NOTE: This does NOT drain ticks. Under Explicit microtask policy,
  // microtasks from op resolution are deferred. Rust calls
  // __drainNextTickAndMacrotasks separately after this returns,
  // with the correct microtask checkpoint ordering to preserve
  // the nextTick-before-then invariant.
  function __eventLoopTick(timerNow) {
    // 1. Process expired timers if the timer deadline fired
    if (timerNow >= 0) {
      timerExpiry[0] = __timers.processTimers(timerNow);
    }
    // 2. Resolve all completed async ops (args after timerNow)
    for (let i = 1; i < arguments.length; i += 3) {
      const promiseId = arguments[i];
      const isOk = arguments[i + 1];
      const res = arguments[i + 2];
      __resolvePromise(promiseId, res, isOk);
    }
  }

  // Drain nextTick/microtask queues only (no timers or ops).
  // Used in the I/O tight loop and when ticks are pending without ops.
  function __drainNextTickAndMacrotasks() {
    drainTicks();
  }

  // Phase 2: Handle unhandled promise rejections.
  // Called from Rust with a flat array: [promise, reason, context, promise, reason, context, ...]
  function __handleRejections() {
    for (let i = 0; i < arguments.length; i += 3) {
      // Restore the async context that was active when the promise was
      // rejected, so that AsyncLocalStorage.getStore() works correctly
      // inside unhandledrejection handlers (matching Node.js behavior).
      const prevContext = getAsyncContext();
      setAsyncContext(arguments[i + 2]);
      try {
        const handled = unhandledPromiseRejectionHandler(
          arguments[i],
          arguments[i + 1],
        );
        if (!handled) {
          const err = arguments[i + 1];
          op_dispatch_exception(err, true);
        }
      } finally {
        setAsyncContext(prevContext);
      }
    }
  }

  // Report an exception (called from Rust for timer callback errors).
  function __reportException(e) {
    reportExceptionCallback(e);
  }

  function runImmediateCallbacks() {
    try {
      runImmediates();
    } catch (e) {
      reportExceptionCallback(e);
    }
  }

  function refOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    op_ref_op(promiseId);
  }

  function unrefOp(promiseId) {
    if (!hasPromise(promiseId)) {
      return;
    }
    op_unref_op(promiseId);
  }

  function refOpPromise(promise) {
    refOp(promise[promiseIdSymbol]);
  }

  function unrefOpPromise(promise) {
    unrefOp(promise[promiseIdSymbol]);
  }

  function resources() {
    return ObjectFromEntries(op_resources());
  }

  let reportExceptionCallback = (error) => {
    op_dispatch_exception(error, false);
  };

  // Used to report errors thrown from functions passed to `queueMicrotask()`.
  // The callback will be passed the thrown error. For example, you can use this
  // to dispatch an error event to the global scope.
  // In other words, set the implementation for
  // https://html.spec.whatwg.org/multipage/webappapis.html#report-the-exception
  function setReportExceptionCallback(cb) {
    if (cb === null || cb === undefined) {
      reportExceptionCallback = (error) => {
        op_dispatch_exception(error, false);
      };
    } else {
      if (typeof cb != "function") {
        throw new TypeError("expected a function");
      }
      reportExceptionCallback = cb;
    }
  }

  function queueMicrotask(cb) {
    if (typeof cb != "function") {
      throw new TypeError("expected a function");
    }
    return op_queue_microtask(() => {
      try {
        cb();
      } catch (error) {
        reportExceptionCallback(error);
      }
    });
  }

  // Some "extensions" rely on "BadResource", "Interrupted", "NotCapable"
  // errors in the JS code (eg. "deno_net") so they are provided in "Deno.core"
  // but later reexported on "Deno.errors"
  class BadResource extends Error {
    constructor(msg, options) {
      super(msg, options);
      this.name = "BadResource";
    }
  }
  const BadResourcePrototype = BadResource.prototype;

  class Interrupted extends Error {
    constructor(msg, options) {
      super(msg, options);
      this.name = "Interrupted";
    }
  }
  const InterruptedPrototype = Interrupted.prototype;

  class NotCapable extends Error {
    constructor(msg, options) {
      super(msg, options);
      this.name = "NotCapable";
    }
  }
  const NotCapablePrototype = NotCapable.prototype;

  registerErrorClass("BadResource", BadResource);
  registerErrorClass("Interrupted", Interrupted);
  registerErrorClass("NotCapable", NotCapable);

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

    op_set_promise_hooks(
      wrappedHooks[0],
      wrappedHooks[1],
      wrappedHooks[2],
      wrappedHooks[3],
    );
  }

  const {
    op_close: close,
    op_try_close: tryClose,
    op_read: read,
    op_read_all: readAll,
    op_write: write,
    op_write_all: writeAll,
    op_write_type_error: writeTypeError,
    op_read_sync: readSync,
    op_write_sync: writeSync,
    op_shutdown: shutdown,
    op_is_terminal: isTerminal,
  } = ops;

  const callSiteRetBuf = new Uint32Array(2);
  const callSiteRetBufU8 = new Uint8Array(callSiteRetBuf.buffer);

  function currentUserCallSite() {
    const fileName = op_current_user_call_site(callSiteRetBufU8);
    const lineNumber = callSiteRetBuf[0];
    const columnNumber = callSiteRetBuf[1];
    return { fileName, lineNumber, columnNumber };
  }

  const hostObjectBrand = SymbolFor("Deno.core.hostObject");
  const transferableResources = {};
  const registerTransferableResource = (name, send, receive) => {
    if (transferableResources[name]) {
      throw new Error(`${name} is already registered`);
    }
    transferableResources[name] = { send, receive };
  };
  const getTransferableResource = (name) => transferableResources[name];
  const cloneableDeserializers = { __proto__: null };
  const registerCloneableResource = (name, deserialize) => {
    if (cloneableDeserializers[name]) {
      throw new Error(`${name} is already registered`);
    }
    cloneableDeserializers[name] = deserialize;
  };
  const getCloneableDeserializers = () => cloneableDeserializers;

  // A helper function that will bind our own console implementation
  // with default implementation of Console from V8. This will cause
  // console messages to be piped to inspector console.
  //
  // We are using `Deno.core.callConsole` binding to preserve proper stack
  // frames in inspector console. This has to be done because V8 considers
  // the last JS stack frame as gospel for the inspector. In our case we
  // specifically want the latest user stack frame to be the one that matters
  // though.
  //
  // Inspired by:
  // https://github.com/nodejs/node/blob/1317252dfe8824fd9cfee125d2aaa94004db2f3b/lib/internal/util/inspector.js#L39-L61
  function wrapConsole(customConsole, consoleFromV8) {
    const callConsole = window.Deno.core.callConsole;

    const keys = ObjectKeys(consoleFromV8);
    for (let i = 0; i < keys.length; ++i) {
      const key = keys[i];
      if (ObjectHasOwn(customConsole, key)) {
        customConsole[key] = FunctionPrototypeBind(
          callConsole,
          customConsole,
          consoleFromV8[key],
          customConsole[key],
        );
      } else {
        // Add additional console APIs from the inspector
        customConsole[key] = consoleFromV8[key];
      }
    }
  }

  // Minimal console implementation, that uses `Deno.core.print` under the hood.
  // It's not fully fledged and is meant to make debugging slightly easier when working with
  // only `deno_core` crate.
  class CoreConsole {
    log = (...args) => {
      op_print(`${consoleStringify(...args)}\n`, false);
    };

    debug = (...args) => {
      op_print(`${consoleStringify(...args)}\n`, false);
    };

    warn = (...args) => {
      op_print(`${consoleStringify(...args)}\n`, false);
    };

    error = (...args) => {
      op_print(`${consoleStringify(...args)}\n`, false);
    };
  }

  // Default impl of contextual logging
  op_get_ext_import_meta_proto().log = function internalLog(level, ...args) {
    console.error(`[${level.toUpperCase()}]`, ...args);
  };

  const consoleStringify = (...args) => args.map(consoleStringifyArg).join(" ");

  const consoleStringifyArg = (arg) => {
    if (
      typeof arg === "string" || typeof arg === "boolean" ||
      typeof arg === "number" || arg === null || arg === undefined
    ) {
      return arg;
    }
    const tag = TypedArrayPrototypeGetSymbolToStringTag(arg);
    if (op_is_typed_array(arg)) {
      return `${tag}(${TypedArrayPrototypeGetLength(arg)}) [${
        TypedArrayPrototypeJoin(TypedArrayPrototypeSlice(arg, 0, 10), ", ")
      }]`;
    }
    if (tag !== undefined) {
      tag + " " + JSON.stringify(arg, undefined, 2);
    } else {
      return JSON.stringify(arg, undefined, 2);
    }
  };

  const v8Console = globalThis.console;
  const coreConsole = new CoreConsole();
  globalThis.console = coreConsole;
  wrapConsole(coreConsole, v8Console);

  function propWritable(value) {
    return {
      value,
      writable: true,
      enumerable: true,
      configurable: true,
    };
  }

  function propNonEnumerable(value) {
    return {
      value,
      writable: true,
      enumerable: false,
      configurable: true,
    };
  }

  function propReadOnly(value) {
    return {
      value,
      enumerable: true,
      writable: false,
      configurable: true,
    };
  }

  function propGetterOnly(getter) {
    return {
      get: getter,
      set() {},
      enumerable: true,
      configurable: true,
    };
  }

  function propWritableLazyLoaded(getter, loadFn) {
    let valueIsSet = false;
    let value;

    return {
      get() {
        const loadedValue = loadFn();
        if (valueIsSet) {
          return value;
        } else {
          return getter(loadedValue);
        }
      },
      set(v) {
        loadFn();
        valueIsSet = true;
        value = v;
      },
      enumerable: true,
      configurable: true,
    };
  }

  function propNonEnumerableLazyLoaded(getter, loadFn) {
    let valueIsSet = false;
    let value;

    return {
      get() {
        const loadedValue = loadFn();
        if (valueIsSet) {
          return value;
        } else {
          return getter(loadedValue);
        }
      },
      set(v) {
        loadFn();
        valueIsSet = true;
        value = v;
      },
      enumerable: false,
      configurable: true,
    };
  }

  function createLazyLoader(specifier) {
    let value;

    return function lazyLoad() {
      if (!value) {
        value = op_lazy_load_esm(specifier);
      }
      return value;
    };
  }

  const getAsyncContext = getContinuationPreservedEmbedderData;
  const setAsyncContext = setContinuationPreservedEmbedderData;

  function scopeAsyncContext(ctx) {
    const old = getAsyncContext();
    setAsyncContext(ctx);
    return {
      __proto__: null,
      [Symbol.dispose]() {
        setAsyncContext(old);
      },
    };
  }

  let asyncVariableCounter = 0;
  class AsyncVariable {
    #id = asyncVariableCounter++;
    #data = new SafeWeakMap();

    enter(value) {
      const previousContextMapping = getAsyncContext();
      const entry = { id: this.#id };
      const asyncContextMapping = {
        __proto__: null,
        ...previousContextMapping,
        [this.#id]: entry,
      };
      this.#data.set(entry, value);
      setAsyncContext(asyncContextMapping);
      return previousContextMapping;
    }

    get() {
      const current = getAsyncContext();
      const entry = current?.[this.#id];
      if (entry) {
        return this.#data.get(entry);
      }
      return undefined;
    }
  }

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    internalRidSymbol: Symbol("Deno.internal.rid"),
    internalFdSymbol: Symbol("Deno.internal.fd"),
    resources,
    __eventLoopTick,
    __setTickInfo(buf) {
      tickInfo = buf;
    },
    __setImmediateInfo(buf) {
      immediateInfo = buf;
    },
    __setTimerExpiry(buf) {
      timerExpiry = buf;
    },
    __drainNextTickAndMacrotasks,
    __handleRejections,
    __reportException,
    __setTimerInfo: __timers.__setTimerInfo,
    immediateRefCount(increase) {
      if (increase) {
        if (immediateInfo[kImmRefCount] === 0) {
          op_immediate_check(true);
        }
        immediateInfo[kImmRefCount]++;
      } else {
        immediateInfo[kImmRefCount]--;
        if (immediateInfo[kImmRefCount] === 0) {
          op_immediate_check(false);
        }
      }
    },
    runImmediateCallbacks,
    BadResource,
    BadResourcePrototype,
    Interrupted,
    InterruptedPrototype,
    NotCapable,
    NotCapablePrototype,
    refOpPromise,
    unrefOpPromise,
    setReportExceptionCallback,
    setPromiseHooks,
    consoleStringify,
    close,
    tryClose,
    read,
    readAll,
    write,
    writeAll,
    writeTypeError,
    readSync,
    writeSync,
    shutdown,
    isTerminal,
    print: (msg, isErr) => op_print(msg, isErr),
    setLeakTracingEnabled: (enabled) => {
      __setLeakTracingEnabled(enabled);
      op_leak_tracing_enable(enabled);
    },
    isLeakTracingEnabled: () => __isLeakTracingEnabled(),
    getAllLeakTraces: () => {
      const traces = op_leak_tracing_get_all();
      return new SafeMap(traces);
    },
    getLeakTraceForPromise: (promise) =>
      op_leak_tracing_get(0, promise[promiseIdSymbol]),
    queueNextTick,
    processTicksAndRejections,
    runNextTicks,
    setAsyncHooksEmit,
    queueImmediate,
    clearImmediate,
    runImmediates,
    immediateQueue,
    kRefed,
    runMicrotasks: () => op_run_microtasks(),
    hasTickScheduled,
    setHasTickScheduled,
    compileFunction: (
      source,
      specifier,
      hostDefinedOptions,
      params,
    ) => {
      const [result, error] = op_compile_function(
        source,
        specifier,
        hostDefinedOptions,
        params,
      );
      if (error) {
        const { 0: thrown, 1: isNativeError, 2: isCompileError } = error;
        return [
          result,
          {
            thrown,
            isNativeError,
            isCompileError,
          },
        ];
      }
      return [result, null];
    },
    evalContext: (
      source,
      specifier,
      hostDefinedOptions,
    ) => {
      const [result, error] = op_eval_context(
        source,
        specifier,
        hostDefinedOptions,
      );
      if (error) {
        const { 0: thrown, 1: isNativeError, 2: isCompileError } = error;
        return [
          result,
          {
            thrown,
            isNativeError,
            isCompileError,
          },
        ];
      }
      return [result, null];
    },
    hostObjectBrand,
    registerTransferableResource,
    getTransferableResource,
    registerCloneableResource,
    getCloneableDeserializers,
    encode: (text) => op_encode(text),
    encodeBinaryString: (buffer) => op_encode_binary_string(buffer),
    decode: (buffer) => op_decode(buffer),
    structuredClone: (value, deserializers) =>
      op_structured_clone(value, deserializers ?? cloneableDeserializers),
    serialize: (
      value,
      options,
      errorCallback,
    ) => {
      return op_serialize(
        value,
        options?.hostObjects,
        options?.transferredArrayBuffers,
        options?.forStorage ?? false,
        errorCallback,
      );
    },
    deserialize: (buffer, options) => {
      return op_deserialize(
        buffer,
        options?.hostObjects,
        options?.transferredArrayBuffers,
        options?.deserializers,
        options?.forStorage ?? false,
      );
    },
    getPromiseDetails: (promise) => op_get_promise_details(promise),
    getProxyDetails: (proxy) => op_get_proxy_details(proxy),
    isAnyArrayBuffer: (value) => op_is_any_array_buffer(value),
    isArgumentsObject: (value) => op_is_arguments_object(value),
    isArrayBuffer: (value) => op_is_array_buffer(value),
    isArrayBufferView: (value) => op_is_array_buffer_view(value),
    isAsyncFunction: (value) => op_is_async_function(value),
    isBigIntObject: (value) => op_is_big_int_object(value),
    isBooleanObject: (value) => op_is_boolean_object(value),
    isBoxedPrimitive: (value) => op_is_boxed_primitive(value),
    isDataView: (value) => op_is_data_view(value),
    isDate: (value) => op_is_date(value),
    isGeneratorFunction: (value) => op_is_generator_function(value),
    isGeneratorObject: (value) => op_is_generator_object(value),
    isMap: (value) => op_is_map(value),
    isMapIterator: (value) => op_is_map_iterator(value),
    isModuleNamespaceObject: (value) => op_is_module_namespace_object(value),
    isNativeError: (value) => op_is_native_error(value),
    isNumberObject: (value) => op_is_number_object(value),
    isPromise: (value) => op_is_promise(value),
    isProxy: (value) => op_is_proxy(value),
    isRegExp: (value) => op_is_reg_exp(value),
    isSet: (value) => op_is_set(value),
    isSetIterator: (value) => op_is_set_iterator(value),
    isSharedArrayBuffer: (value) => op_is_shared_array_buffer(value),
    isStringObject: (value) => op_is_string_object(value),
    isSymbolObject: (value) => op_is_symbol_object(value),
    isTypedArray: (value) => op_is_typed_array(value),
    isWeakMap: (value) => op_is_weak_map(value),
    isWeakSet: (value) => op_is_weak_set(value),
    memoryUsage: () => op_memory_usage(),
    setWasmStreamingCallback: (fn) => op_set_wasm_streaming_callback(fn),
    abortWasmStreaming: (
      rid,
      error,
    ) => op_abort_wasm_streaming(rid, error),
    destructureError: (error) => op_destructure_error(error),
    opNames: () => op_op_names(),
    eventLoopHasMoreWork: () => op_event_loop_has_more_work(),
    byteLength: (str) => op_str_byte_length(str),
    addMainModuleHandler: (handler) => op_add_main_module_handler(handler),
    setHandledPromiseRejectionHandler: (handler) =>
      op_set_handled_promise_rejection_handler(handler),
    setUnhandledPromiseRejectionHandler: (handler) =>
      unhandledPromiseRejectionHandler = handler,
    reportUnhandledException: (e) => op_dispatch_exception(e, false),
    reportUnhandledPromiseRejection: (e) => op_dispatch_exception(e, true),
    createTimer: __timers.createTimer,
    cancelTimer: __timers.cancelTimer,
    refreshTimer: __timers.refreshTimer,
    refTimer: __timers.refTimer,
    unrefTimer: __timers.unrefTimer,
    currentUserCallSite,
    wrapConsole,
    v8Console,
    propReadOnly,
    propWritable,
    propNonEnumerable,
    propGetterOnly,
    propWritableLazyLoaded,
    propNonEnumerableLazyLoaded,
    createLazyLoader,
    createCancelHandle: () => op_cancel_handle(),
    getAsyncContext,
    setAsyncContext,
    scopeAsyncContext,
    AsyncVariable,
  });

  const internals = {};
  ObjectAssign(globalThis.__bootstrap, { core, internals });
  ObjectAssign(globalThis.Deno, { core });
  ObjectFreeze(globalThis.__bootstrap.core);

  // Direct bindings on `globalThis`
  ObjectAssign(globalThis, { queueMicrotask });
  setQueueMicrotask(queueMicrotask);
})(globalThis);
