// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
import {
  AsyncVariable,
  getAsyncContext,
  setAsyncContext,
} from "ext:runtime/01_async_context.js";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { newAsyncId } from "ext:deno_node/internal/async_hooks.ts";

const {
  ObjectDefineProperties,
  ReflectApply,
  FunctionPrototypeBind,
  ArrayPrototypeUnshift,
  ObjectFreeze,
} = primordials;

export class AsyncResource {
  type: string;
  #snapshot: unknown;
  #asyncId: number;

  constructor(type: string) {
    this.type = type;
    this.#snapshot = getAsyncContext();
    this.#asyncId = newAsyncId();
  }

  asyncId() {
    return this.#asyncId;
  }

  runInAsyncScope(
    fn: (...args: unknown[]) => unknown,
    thisArg: unknown,
    ...args: unknown[]
  ) {
    const previousContext = getAsyncContext();
    try {
      setAsyncContext(this.#snapshot);
      return ReflectApply(fn, thisArg, args);
    } finally {
      setAsyncContext(previousContext);
    }
  }

  emitDestroy() {}

  bind(fn: (...args: unknown[]) => unknown, thisArg) {
    validateFunction(fn, "fn");
    let bound;
    if (thisArg === undefined) {
      // deno-lint-ignore no-this-alias
      const resource = this;
      bound = function (...args) {
        ArrayPrototypeUnshift(args, fn, this);
        return ReflectApply(resource.runInAsyncScope, resource, args);
      };
    } else {
      bound = FunctionPrototypeBind(this.runInAsyncScope, this, fn, thisArg);
    }
    ObjectDefineProperties(bound, {
      "length": {
        __proto__: null,
        configurable: true,
        enumerable: false,
        value: fn.length,
        writable: false,
      },
    });
    return bound;
  }

  static bind(
    fn: (...args: unknown[]) => unknown,
    type?: string,
    thisArg?: AsyncResource,
  ) {
    type = type || fn.name || "bound-anonymous-fn";
    return (new AsyncResource(type)).bind(fn, thisArg);
  }
}

export class AsyncLocalStorage {
  #variable = new AsyncVariable();
  enabled = false;

  // deno-lint-ignore no-explicit-any
  run(store: any, callback: any, ...args: any[]): any {
    this.enabled = true;
    const previous = this.#variable.enter(store);
    try {
      return ReflectApply(callback, null, args);
    } finally {
      setAsyncContext(previous);
    }
  }

  // deno-lint-ignore no-explicit-any
  exit(callback: (...args: unknown[]) => any, ...args: any[]): any {
    if (!this.enabled) {
      return ReflectApply(callback, null, args);
    }
    this.enabled = false;
    try {
      return ReflectApply(callback, null, args);
    } finally {
      this.enabled = true;
    }
  }

  // deno-lint-ignore no-explicit-any
  getStore(): any {
    if (!this.enabled) {
      return undefined;
    }
    return this.#variable.get();
  }

  enterWith(store: unknown) {
    this.enabled = true;
    this.#variable.enter(store);
  }

  disable() {
    this.enabled = false;
  }

  static bind(fn: (...args: unknown[]) => unknown) {
    return AsyncResource.bind(fn);
  }

  static snapshot() {
    return AsyncLocalStorage.bind((
      cb: (...args: unknown[]) => unknown,
      ...args: unknown[]
    ) => ReflectApply(cb, null, args));
  }
}

export function executionAsyncId() {
  return 0;
}

export function triggerAsyncId() {
  return 0;
}

export function executionAsyncResource() {
  return {};
}

export const asyncWrapProviders = ObjectFreeze({ __proto__: null });

class AsyncHook {
  enable() {
  }

  disable() {
  }
}

export function createHook() {
  return new AsyncHook();
}

export default {
  AsyncLocalStorage,
  createHook,
  executionAsyncId,
  triggerAsyncId,
  executionAsyncResource,
  asyncWrapProviders,
  AsyncResource,
};
