// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />

import { primordials } from "ext:core/mod.js";
const {
  Symbol,
  SymbolToStringTag,
  TypeError,
} = primordials;
import { EventTarget } from "./02_event.js";

const illegalConstructorKey = Symbol("illegalConstructorKey");

class Window extends EventTarget {
  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
    super();
  }

  get [SymbolToStringTag]() {
    return "Window";
  }
}

class WorkerGlobalScope extends EventTarget {
  constructor(key = null) {
    if (key != illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
    super();
  }

  get [SymbolToStringTag]() {
    return "WorkerGlobalScope";
  }
}

class DedicatedWorkerGlobalScope extends WorkerGlobalScope {
  constructor(key = null) {
    if (key != illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
    super();
  }

  get [SymbolToStringTag]() {
    return "DedicatedWorkerGlobalScope";
  }
}

const dedicatedWorkerGlobalScopeConstructorDescriptor = {
  configurable: true,
  enumerable: false,
  value: DedicatedWorkerGlobalScope,
  writable: true,
};

const windowConstructorDescriptor = {
  configurable: true,
  enumerable: false,
  value: Window,
  writable: true,
};

const workerGlobalScopeConstructorDescriptor = {
  configurable: true,
  enumerable: false,
  value: WorkerGlobalScope,
  writable: true,
};

export {
  DedicatedWorkerGlobalScope,
  dedicatedWorkerGlobalScopeConstructorDescriptor,
  Window,
  windowConstructorDescriptor,
  WorkerGlobalScope,
  workerGlobalScopeConstructorDescriptor,
};
