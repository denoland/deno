// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />

// deno-fmt-ignore-file
(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  Symbol,
  SymbolToStringTag,
  TypeError,
} = primordials;
const { EventTarget } = core.loadExtScript("ext:deno_web/02_event.js");

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

return {
  DedicatedWorkerGlobalScope,
  dedicatedWorkerGlobalScopeConstructorDescriptor,
  Window,
  windowConstructorDescriptor,
  WorkerGlobalScope,
  workerGlobalScopeConstructorDescriptor,
};
})()
