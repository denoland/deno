// Copyright 2018-2025 the Deno authors. MIT license.
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
      const oldPromiseId = promiseId - RING_SIZE;
      MapPrototypeSet(promiseMap, oldPromiseId, oldPromise);
    }

    const promise = new Promise((resolve, reject) => {
      promiseRing[idx] = [resolve, reject];
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
    // Check if out of ring bounds, fallback to map
    const outOfBounds = promiseId < nextPromiseId - RING_SIZE;
    if (outOfBounds) {
      const promise = MapPrototypeGet(promiseMap, promiseId);
      if (!promise) {
        throw "Missing promise in map @ " + promiseId;
      }
      MapPrototypeDelete(promiseMap, promiseId);
      if (isOk) {
        promise[0](res);
      } else {
        promise[1](res);
      }
    } else {
      // Otherwise take from ring
      const idx = promiseId % RING_SIZE;
      const promise = promiseRing[idx];
      if (!promise) {
        throw "Missing promise in ring @ " + promiseId;
      }
      promiseRing[idx] = NO_PROMISE;
      if (isOk) {
        promise[0](res);
      } else {
        promise[1](res);
      }
    }
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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
          nextPromiseId = (id + 1) & 0xffffffff;
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

  // Extra Deno.core.* exports
  const core = ObjectAssign(globalThis.Deno.core, {
    build,
    setBuildInfo,
    registerErrorBuilder,
    buildCustomError,
    registerErrorClass,
    setUpAsyncStub,
    hasPromise,
    promiseIdSymbol,
  });

  const infra = {
    __resolvePromise,
    __setLeakTracingEnabled,
    __isLeakTracingEnabled,
    __initializeCoreMethods,
  };

  ObjectAssign(globalThis, { __infra: infra });
  ObjectAssign(globalThis.__bootstrap, { core });
  ObjectAssign(globalThis.Deno, { core });
})(globalThis);
