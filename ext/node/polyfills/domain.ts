// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6
// deno-lint-ignore-file no-process-global

(function () {
const { core, primordials } = __bootstrap;
const { ERR_UNHANDLED_ERROR } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { AsyncHook } = core.loadExtScript(
  "ext:deno_node/internal/async_hooks.ts",
);
const {
  ArrayPrototypeEvery,
  ArrayPrototypeIndexOf,
  ArrayPrototypeLastIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  FunctionPrototypeCall,
  FunctionPrototypeApply,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  SafeMap,
} = primordials;
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");

function emitError(e) {
  this.emit("error", e);
}

let stack = [];
let _stack = stack;
let active = null;

// Map asyncId -> domain for tracking async operations
const pairing = new SafeMap();

// Async hook to track domain associations across async operations
const asyncHook = new AsyncHook({
  init(asyncId, _type, _triggerAsyncId, resource) {
    if (process.domain !== null && process.domain !== undefined) {
      // Record which domain this async operation belongs to
      pairing.set(asyncId, process.domain);
      // Attach domain to resource
      if (typeof resource === "object" && resource !== null) {
        ObjectDefineProperty(resource, "domain", {
          __proto__: null,
          configurable: true,
          enumerable: false,
          value: process.domain,
          writable: true,
        });
      }
    }
  },
  before(asyncId) {
    const domain = pairing.get(asyncId);
    if (domain !== undefined) {
      domain.enter();
    }
  },
  after(asyncId) {
    const domain = pairing.get(asyncId);
    if (domain !== undefined) {
      domain.exit();
    }
  },
  destroy(asyncId) {
    pairing.delete(asyncId);
  },
});

function create() {
  return new Domain();
}

function createDomain() {
  return new Domain();
}

class Domain extends EventEmitter {
  members = [];

  constructor() {
    super();
    patchEventEmitter();
    asyncHook.enable();

    this.on("removeListener", updateExceptionCapture);
    this.on("newListener", updateExceptionCapture);
  }

  add(ee) {
    if (ee.domain === this) {
      return;
    }

    if (ee.domain) {
      ee.domain.remove(ee);
    }

    if (this.domain && (ObjectPrototypeIsPrototypeOf(Domain.prototype, ee))) {
      for (let d = this.domain; d; d = d.domain) {
        if (ee === d) return;
      }
    }

    ObjectDefineProperty(ee, "domain", {
      __proto__: null,
      configurable: true,
      enumerable: false,
      value: this,
      writable: true,
    });
    ArrayPrototypePush(this.members, ee);
  }

  remove(ee) {
    ee.domain = null;
    const index = ArrayPrototypeIndexOf(this.members, ee);
    if (index !== -1) {
      ArrayPrototypeSplice(this.members, index, 1);
    }
  }

  bind(fn) {
    // deno-lint-ignore no-this-alias
    const self = this;
    return function () {
      self.enter();
      try {
        const ret = FunctionPrototypeApply(
          fn,
          this,
          ArrayPrototypeSlice(arguments),
        );
        self.exit();
        return ret;
      } catch (e) {
        self.exit();
        if (typeof e === "object" && e !== null) {
          e.domainBound = fn;
          e.domainThrown = false;
          ObjectDefineProperty(e, "domain", {
            __proto__: null,
            configurable: true,
            enumerable: false,
            value: self,
            writable: true,
          });
        }
        FunctionPrototypeCall(emitError, self, e);
      }
    };
  }

  intercept(fn) {
    // deno-lint-ignore no-this-alias
    const self = this;
    return function (e) {
      if (e) {
        if (typeof e === "object" && e !== null) {
          e.domainBound = fn;
          e.domainThrown = false;
          ObjectDefineProperty(e, "domain", {
            __proto__: null,
            configurable: true,
            enumerable: false,
            value: self,
            writable: true,
          });
        }
        FunctionPrototypeCall(emitError, self, e);
      } else {
        self.enter();
        try {
          const ret = FunctionPrototypeApply(
            fn,
            this,
            ArrayPrototypeSlice(arguments, 1),
          );
          self.exit();
          return ret;
        } catch (e) {
          self.exit();
          if (typeof e === "object" && e !== null) {
            e.domainBound = fn;
            e.domainThrown = false;
            ObjectDefineProperty(e, "domain", {
              __proto__: null,
              configurable: true,
              enumerable: false,
              value: self,
              writable: true,
            });
          }
          FunctionPrototypeCall(emitError, self, e);
        }
      }
    };
  }

  run(fn, ...args) {
    this.enter();
    try {
      const ret = FunctionPrototypeApply(fn, this, args);
      this.exit();
      return ret;
    } catch (e) {
      this.exit();
      if (typeof e === "object" && e !== null) {
        e.domainThrown = true;
        ObjectDefineProperty(e, "domain", {
          __proto__: null,
          configurable: true,
          enumerable: false,
          value: this,
          writable: true,
        });
      }
      FunctionPrototypeCall(emitError, this, e);
    }
  }

  dispose() {
    this._disposed = true;
    this.removeAllListeners();
    return this;
  }

  enter() {
    active = process.domain = this;
    ArrayPrototypePush(stack, this);
    updateExceptionCapture();
    return this;
  }

  exit() {
    // Use lastIndexOf (most recent occurrence) and remove everything from that
    // position onwards. This matches Node.js behavior: exiting a domain also
    // exits all domains that were entered after its most recent entry.
    const index = ArrayPrototypeLastIndexOf(stack, this);
    if (index !== -1) {
      ArrayPrototypeSplice(stack, index);
    }
    active = stack.length === 0 ? null : stack[stack.length - 1];
    process.domain = active;
    updateExceptionCapture();
    return this;
  }
}

let exceptionCaptureActive = false;

function updateExceptionCapture() {
  const shouldCapture = !ArrayPrototypeEvery(
    stack,
    (domain) => domain.listenerCount("error") === 0,
  );

  if (shouldCapture && !exceptionCaptureActive) {
    exceptionCaptureActive = true;
    process.setUncaughtExceptionCaptureCallback(
      domainUncaughtExceptionHandler,
    );
  } else if (!shouldCapture && exceptionCaptureActive) {
    exceptionCaptureActive = false;
    process.setUncaughtExceptionCaptureCallback(null);
  }
}

process.on("newListener", (name, listener) => {
  if (
    name === "uncaughtException" &&
    listener !== domainUncaughtExceptionClear
  ) {
    // The first uncaughtException listener must clear the domain stack before
    // user code runs.
    process.removeListener(name, domainUncaughtExceptionClear);
    process.prependListener(name, domainUncaughtExceptionClear);
  }
});

process.on("removeListener", (name, listener) => {
  if (
    name === "uncaughtException" &&
    listener !== domainUncaughtExceptionClear
  ) {
    const listeners = process.listeners("uncaughtException");
    if (
      listeners.length === 1 &&
      listeners[0] === domainUncaughtExceptionClear
    ) {
      process.removeListener(name, domainUncaughtExceptionClear);
    }
  }
});

function domainUncaughtExceptionClear() {
  stack.length = 0;
  active = process.domain = null;
  updateExceptionCapture();
}

function domainUncaughtExceptionHandler(er) {
  let caught = false;
  const curDomain = process.domain;
  if (!curDomain || curDomain._disposed) {
    // No active domain or domain has been disposed, re-throw
    throw er;
  }

  if (typeof er === "object" && er !== null) {
    ObjectDefineProperty(er, "domain", {
      __proto__: null,
      configurable: true,
      enumerable: false,
      value: curDomain,
      writable: true,
    });
    er.domainThrown = true;
  }

  // Run the error handler outside of its domain, but within its parent.
  // A domain may have been entered more than once, so remove all adjacent
  // entries for the currently active domain.
  while (active === curDomain) {
    curDomain.exit();
  }

  if (stack.length === 0) {
    // Without a domain error listener, leave the error for the process-level
    // uncaughtException handler instead of emitting an error event that throws.
    if (curDomain.listenerCount("error") > 0) {
      process.setUncaughtExceptionCaptureCallback(null);
      try {
        caught = curDomain.emit("error", er);
      } finally {
        updateExceptionCapture();
      }
    }
  } else {
    try {
      caught = curDomain.emit("error", er);
    } catch (handlerError) {
      // Let the parent domain handle errors thrown by a child domain's handler.
      // If there is no parent, pass the error on to the process-level handler.
      updateExceptionCapture();
      if (stack.length > 0) {
        active = process.domain = stack[stack.length - 1];
        caught = domainUncaughtExceptionHandler(handlerError);
      } else {
        throw handlerError;
      }
    }
  }

  // An uncaught exception ends the current turn. No entered domains should
  // remain active when unrelated work begins on a later turn.
  domainUncaughtExceptionClear();

  return caught;
}

let patched = false;
/** Patches EventEmitter method to make it domain-aware.
 * This happens at top-level of domain module in Node. That works because
 * Node uses cjs for internal modules. We do this patching at constructor
 * of Domain class to best approximate that behavior. */
function patchEventEmitter() {
  if (patched) return;
  patched = true;

  EventEmitter.usingDomains = true;

  const eventInit = EventEmitter.init;
  EventEmitter.init = function (opts) {
    ObjectDefineProperty(this, "domain", {
      __proto__: null,
      configurable: true,
      enumerable: false,
      value: null,
      writable: true,
    });
    if (active && !ObjectPrototypeIsPrototypeOf(Domain.prototype, this)) {
      this.domain = active;
    }

    return FunctionPrototypeCall(eventInit, this, opts);
  };

  const eventEmit = EventEmitter.prototype.emit;
  EventEmitter.prototype.emit = function emit(...args) {
    const domain = this.domain;

    const type = args[0];
    const shouldEmitError = type === "error" &&
      this.listenerCount(type) > 0;

    // No domain on this emitter or this is process - just call original emit
    if (domain === null || domain === undefined || this === process) {
      return ReflectApply(eventEmit, this, args);
    }

    // If the emitter has an error handler and a domain, wrap with
    // domain.enter()/exit() to preserve domain context in the handler.
    // Only exit on success - on error, the domainUncaughtExceptionHandler
    // handles cleanup (same pattern as timer async hooks).
    if (shouldEmitError) {
      domain.enter();
      const ret = ReflectApply(eventEmit, this, args);
      domain.exit();
      return ret;
    }

    if (type === "error") {
      const er = args.length > 1 && args[1]
        ? args[1]
        : new ERR_UNHANDLED_ERROR();

      if (typeof er === "object") {
        er.domainEmitter = this;
        ObjectDefineProperty(er, "domain", {
          __proto__: null,
          configurable: true,
          enumerable: false,
          value: domain,
          writable: true,
        });
        er.domainThrown = false;
      }

      // Remove the current domain (and its duplicates) from the domains stack and
      // set the active domain to its parent (if any) so that the domain's error
      // handler doesn't run in its own context. This prevents any event emitter
      // created or any exception thrown in that error handler from recursively
      // executing that error handler.
      const origDomainsStack = ArrayPrototypeSlice(stack);
      const origActiveDomain = process.domain;

      // Travel the domains stack from top to bottom to find the first domain
      // instance that is not a duplicate of the current active domain.
      let idx = stack.length - 1;
      while (idx > -1 && process.domain === stack[idx]) {
        --idx;
      }

      // Change the stack to not contain the current active domain, and only the
      // domains above it on the stack.
      if (idx < 0) {
        stack.length = 0;
      } else {
        ArrayPrototypeSplice(stack, idx + 1);
      }

      // Change the current active domain
      if (stack.length > 0) {
        active = process.domain = stack[stack.length - 1];
      } else {
        active = process.domain = null;
      }

      updateExceptionCapture();

      domain.emit("error", er);

      // Now that the domain's error handler has completed, restore the domains
      // stack and the active domain to their original values.
      _stack = stack = origDomainsStack;
      active = process.domain = origActiveDomain;
      updateExceptionCapture();

      return false;
    }

    domain.enter();
    const ret = ReflectApply(eventEmit, this, args);
    domain.exit();

    return ret;
  };
}

return {
  default: {
    _stack,
    create,
    active,
    createDomain,
    Domain,
  },
  _stack,
  create,
  active,
  createDomain,
  Domain,
};
})();
