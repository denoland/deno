// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6
// deno-lint-ignore-file no-process-global

import { primordials } from "ext:core/mod.js";
import { ERR_UNHANDLED_ERROR } from "ext:deno_node/internal/errors.ts";
const {
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  FunctionPrototypeCall,
  FunctionPrototypeApply,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
} = primordials;
import { EventEmitter } from "node:events";

function emitError(e) {
  this.emit("error", e);
}

let stack = [];
export let _stack = stack;
export let active = null;

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
      try {
        return FunctionPrototypeApply(fn, null, ArrayPrototypeSlice(arguments));
      } catch (e) {
        FunctionPrototypeCall(emitError, self, e);
      }
    };
  }

  intercept(fn) {
    // deno-lint-ignore no-this-alias
    const self = this;
    return function (e) {
      if (e) {
        FunctionPrototypeCall(emitError, self, e);
      } else {
        try {
          return FunctionPrototypeApply(
            fn,
            null,
            ArrayPrototypeSlice(arguments, 1),
          );
        } catch (e) {
          FunctionPrototypeCall(emitError, self, e);
        }
      }
    };
  }

  run(fn) {
    try {
      return fn();
    } catch (e) {
      FunctionPrototypeCall(emitError, this, e);
    }
    return this;
  }

  dispose() {
    this.removeAllListeners();
    return this;
  }

  enter() {
    return this;
  }

  exit() {
    return this;
  }
}

function updateExceptionCapture() {
  // TODO(kt3k): implement this
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

    // Just call original `emit` if current EE instance has `error`
    // handler, there's no active domain or this is process
    if (
      shouldEmitError || domain === null || domain === undefined ||
      this === process
    ) {
      return ReflectApply(eventEmit, this, args);
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
