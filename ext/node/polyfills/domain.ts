// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6
// deno-lint-ignore-file no-process-global

import { primordials } from "ext:core/mod.js";
import { ERR_UNHANDLED_ERROR } from "ext:deno_node/internal/errors.ts";
import { AsyncHook } from "ext:deno_node/internal/async_hooks.ts";
const {
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
import { EventEmitter } from "node:events";

function emitError(e) {
  this.emit("error", e);
}

let stack = [];
export let _stack = stack;
export let active = null;

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

export function create() {
  return new Domain();
}

export function createDomain() {
  return new Domain();
}

export class Domain extends EventEmitter {
  members = [] as EventEmitter[];

  constructor() {
    super();
    patchEventEmitter();
    asyncHook.enable();
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
  if (stack.length > 0 && !exceptionCaptureActive) {
    exceptionCaptureActive = true;
    process.setUncaughtExceptionCaptureCallback(
      domainUncaughtExceptionHandler,
    );
  } else if (stack.length === 0 && exceptionCaptureActive) {
    exceptionCaptureActive = false;
    process.setUncaughtExceptionCaptureCallback(null);
  }
}

function domainUncaughtExceptionHandler(er) {
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

  // Remove the errored domain from the stack. This cleans up the entry
  // left by emitBefore when the callback threw before emitAfter could run.
  if (stack.length === 1) {
    active = process.domain = null;
    stack.length = 0;
  } else {
    const idx = ArrayPrototypeLastIndexOf(stack, curDomain);
    if (idx !== -1) {
      ArrayPrototypeSplice(stack, idx, 1);
    }
    active = process.domain = stack.length > 0 ? stack[stack.length - 1] : null;
  }

  updateExceptionCapture();

  curDomain.emit("error", er);
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

export default {
  _stack,
  create,
  active,
  createDomain,
  Domain,
};
