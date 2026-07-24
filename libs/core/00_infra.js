// Copyright 2018-2026 the Deno authors. MIT license.
"use strict";

((window) => {
  const {
    Array,
    ArrayPrototypeFill,
    Error,
    ErrorCaptureStackTrace,
    MapPrototypeDelete,
    MapPrototypeGet,
    MapPrototypeHas,
    MapPrototypeSet,
    ObjectAssign,
    ObjectDefineProperty,
    ObjectFreeze,
    Promise,
    PromiseReject,
    PromiseResolve,
    PromisePrototypeCatch,
    RangeError,
    ReferenceError,
    SafeArrayIterator,
    SafeMap,
    StringPrototypeSplit,
    SymbolFor,
    SyntaxError,
    TypeError,
    URIError,
  } = window.__bootstrap.primordials;

  // Promise ids must be non-negative and fit in an i32 (Rust's `PromiseId`);
  // the increment in the async op stubs wraps back to 0 past 2^31 - 1.
  let nextPromiseId = 0;
  const promiseMap = new SafeMap();
  const RING_SIZE = 4 * 1024;
  const NO_PROMISE = null; // Alias to null is faster than plain nulls
  const promiseRing = ArrayPrototypeFill(new Array(RING_SIZE), NO_PROMISE);
  // TODO(bartlomieju): in the future use `v8::Private` so it's not visible
  // to users. Currently missing bindings.
  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  let isLeakTracingEnabled = false;
  let submitLeakTrace;

  // Exposed for testing promise id wraparound behavior.
  function __setNextPromiseId(promiseId) {
    nextPromiseId = promiseId;
  }

  function __setLeakTracingEnabled(enabled) {
    isLeakTracingEnabled = enabled;
  }

  function __isLeakTracingEnabled() {
    return isLeakTracingEnabled;
  }

  function __initializeCoreMethods(submitLeakTrace_) {
    submitLeakTrace = submitLeakTrace_;
  }

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
  // Maps a registered error class name to its constructor. Unlike `errorMap`
  // (which stores builder closures), this lets native error construction
  // restore the exact prototype. Null prototype and immutable entries keep
  // lookups independent of inherited properties or later map mutations.
  const errorConstructors = { __proto__: null };
  // Builtin v8 / JS errors
  registerErrorClass("Error", Error);
  registerErrorClass("RangeError", RangeError);
  registerErrorClass("ReferenceError", ReferenceError);
  registerErrorClass("SyntaxError", SyntaxError);
  registerErrorClass("TypeError", TypeError);
  registerErrorClass("URIError", URIError);

  function buildCustomError(className, message, additionalProperties) {
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
      if (additionalProperties) {
        const keys = [];
        for (const property of new SafeArrayIterator(additionalProperties)) {
          const key = property[0];
          if (!(key in error)) {
            keys.push(key);
            error[key] = property[1];
          }
        }
        Object.defineProperty(error, SymbolFor("errorAdditionalPropertyKeys"), {
          value: keys,
          writable: false,
          enumerable: false,
          configurable: false,
        });
      }
    }
    return error;
  }

  function registerErrorClass(className, errorClass) {
    registerErrorBuilder(className, (msg) => new errorClass(msg));
    ObjectDefineProperty(errorConstructors, className, {
      value: errorClass,
      writable: false,
      enumerable: true,
      configurable: false,
    });
  }

  function registerErrorBuilder(className, errorBuilder) {
    if (typeof errorMap[className] !== "undefined") {
      throw new TypeError(`Error class for "${className}" already registered`);
    }
    errorMap[className] = errorBuilder;
  }

  function setPromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Move old promise from ring to map
    const oldPromise = promiseRing[idx];
    if (oldPromise !== NO_PROMISE) {
      // Keyed on the entry's own id rather than `promiseId - RING_SIZE` so it
      // stays correct across the id counter wrapping back to 0. The one
      // unavoidable limit: if a single promise stays pending while 2^31 more
      // ids are dispatched its id is reused, and this set silently overwrites
      // (loses) the older map entry. Not a regression: the old code broke far
      // sooner, and no real workload keeps an op pending across 2^31 dispatches.
      MapPrototypeSet(promiseMap, oldPromise[2], oldPromise);
    }

    const promise = new Promise((resolve, reject) => {
      promiseRing[idx] = [resolve, reject, promiseId];
    });
    const wrappedPromise = PromisePrototypeCatch(
      promise,
      function __opRejectHandler(res) {
        // recreate the stacktrace and strip internal event loop frames
        ErrorCaptureStackTrace(res, __opRejectHandler);
        throw res;
      },
    );
    wrappedPromise[promiseIdSymbol] = promiseId;
    return wrappedPromise;
  }

  function __resolvePromise(promiseId, res, isOk) {
    // A promise stays in the ring until its slot is reclaimed by a newer
    // promise, at which point it moves to the map. The entry stores its own
    // promise id, so a slot reused after the id counter wrapped around is
    // never mistaken for an older promise that is now in the map.
    const idx = promiseId % RING_SIZE;
    let promise = promiseRing[idx];
    if (promise !== NO_PROMISE && promise[2] === promiseId) {
      promiseRing[idx] = NO_PROMISE;
    } else {
      promise = MapPrototypeGet(promiseMap, promiseId);
      if (!promise) {
        throw "Missing promise @ " + promiseId;
      }
      MapPrototypeDelete(promiseMap, promiseId);
    }
    if (isOk) {
      promise[0](res);
    } else {
      promise[1](res);
    }
  }

  function hasPromise(promiseId) {
    const idx = promiseId % RING_SIZE;
    // Loose comparison on purpose: `promiseId` can be `undefined` (e.g.
    // `unrefOpPromise()` with a promise from an op that completed eagerly and
    // has no promise id attached), making `idx` NaN and the ring lookup
    // return `undefined` instead of the `NO_PROMISE` (null) sentinel.
    const promise = promiseRing[idx];
    if (promise != NO_PROMISE && promise[2] === promiseId) {
      return true;
    }
    return MapPrototypeHas(promiseMap, promiseId);
  }

  function setUpAsyncStub(opName, originalOp, maybeProto) {
    let fn;

    // The body of this switch statement can be generated using the script above.
    switch (originalOp.length - 1) {
      /* BEGIN TEMPLATE setUpAsyncStub */
      /* DO NOT MODIFY: use rebuild_async_stubs.js to regenerate */
      case 0:
        fn = function async_op_0() {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_0);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 1:
        fn = function async_op_1(a) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_1);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 2:
        fn = function async_op_2(a, b) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_2);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 3:
        fn = function async_op_3(a, b, c) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_3);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 4:
        fn = function async_op_4(a, b, c, d) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_4);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 5:
        fn = function async_op_5(a, b, c, d, e) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d, e);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_5);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 6:
        fn = function async_op_6(a, b, c, d, e, f) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d, e, f);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_6);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 7:
        fn = function async_op_7(a, b, c, d, e, f, g) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d, e, f, g);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_7);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 8:
        fn = function async_op_8(a, b, c, d, e, f, g, h) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d, e, f, g, h);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_8);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      case 9:
        fn = function async_op_9(a, b, c, d, e, f, g, h, i) {
          const id = nextPromiseId;
          try {
            // deno-fmt-ignore
            const maybeResult = originalOp.call(this, id, a, b, c, d, e, f, g, h, i);
            if (maybeResult !== undefined) {
              return PromiseResolve(maybeResult);
            }
          } catch (err) {
            ErrorCaptureStackTrace(err, async_op_9);
            return PromiseReject(err);
          }
          if (isLeakTracingEnabled) {
            submitLeakTrace(id);
          }
          nextPromiseId = (id + 1) & 0x7fffffff;
          return setPromise(id);
        };
        break;
      /* END TEMPLATE */

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

    if (maybeProto) {
      ObjectDefineProperty(fn, "prototype", {
        value: maybeProto.prototype,
        configurable: false,
        writable: false,
      });
      maybeProto.prototype[opName] = fn;
    }

    return fn;
  }

  // ---------------------------------------------------------------------------
  // FixedQueue: a singly-linked list of fixed-size circular buffers.
  // Used by the nextTick queue (and available to other core subsystems).
  //
  // Closely mirrors Node.js lib/internal/fixed_queue.js.
  //
  // Copyright Joyent, Inc. and other Node contributors.
  //
  // Permission is hereby granted, free of charge, to any person obtaining a
  // copy of this software and associated documentation files (the
  // "Software"), to deal in the Software without restriction, including
  // without limitation the rights to use, copy, modify, merge, publish,
  // distribute, sublicense, and/or sell copies of the Software, and to permit
  // persons to whom the Software is furnished to do so, subject to the
  // following conditions:
  //
  // The above copyright notice and this permission notice shall be included
  // in all copies or substantial portions of the Software.
  //
  // THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  // OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  // MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  // NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  // DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  // OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  // USE OR OTHER DEALINGS IN THE SOFTWARE.
  // ---------------------------------------------------------------------------
  const kQueueSize = 2048;
  const kQueueMask = kQueueSize - 1;

  class FixedCircularBuffer {
    constructor() {
      this.bottom = 0;
      this.top = 0;
      this.list = new Array(kQueueSize);
      this.next = null;
    }
    isEmpty() {
      return this.top === this.bottom;
    }
    isFull() {
      return ((this.top + 1) & kQueueMask) === this.bottom;
    }
    push(data) {
      this.list[this.top] = data;
      this.top = (this.top + 1) & kQueueMask;
    }
    shift() {
      const nextItem = this.list[this.bottom];
      if (nextItem === undefined) return null;
      this.list[this.bottom] = undefined;
      this.bottom = (this.bottom + 1) & kQueueMask;
      return nextItem;
    }
  }

  class FixedQueue {
    constructor() {
      this.head = this.tail = new FixedCircularBuffer();
    }
    isEmpty() {
      return this.head.isEmpty();
    }
    push(data) {
      if (this.head.isFull()) {
        this.head = this.head.next = new FixedCircularBuffer();
      }
      this.head.push(data);
    }
    shift() {
      const tail = this.tail;
      const next = tail.shift();
      if (tail.isEmpty() && tail.next !== null) {
        this.tail = tail.next;
      }
      return next;
    }
  }

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    build,
    setBuildInfo,
    registerErrorBuilder,
    buildCustomError,
    registerErrorClass,
    errorConstructors,
    setUpAsyncStub,
    hasPromise,
    promiseIdSymbol,
    __setNextPromiseId,
  });

  const infra = {
    __resolvePromise,
    __setLeakTracingEnabled,
    __isLeakTracingEnabled,
    __initializeCoreMethods,
    FixedQueue,
  };

  ObjectAssign(globalThis, { __infra: infra });
  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });
})(globalThis);
