// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

// @ts-check
/// <reference path="../../core/internal.d.ts" />

((window) => {
  const { EventTarget } = window;
  const {
    Symbol,
    SymbolToStringTag,
    TypeError,
  } = window.__bootstrap.primordials;

  const illegalConstructorKey = Symbol("illegalConstructorKey");

  class Window extends EventTarget {
    constructor(key = null) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
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
        throw new TypeError("Illegal constructor.");
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
        throw new TypeError("Illegal constructor.");
      }
      super();
    }

    get [SymbolToStringTag]() {
      return "DedicatedWorkerGlobalScope";
    }
  }

  window.__bootstrap.globalInterfaces = {
    DedicatedWorkerGlobalScope,
    Window,
    WorkerGlobalScope,
    dedicatedWorkerGlobalScopeConstructorDescriptor: {
      configurable: true,
      enumerable: false,
      value: DedicatedWorkerGlobalScope,
      writable: true,
    },
    windowConstructorDescriptor: {
      configurable: true,
      enumerable: false,
      value: Window,
      writable: true,
    },
    workerGlobalScopeConstructorDescriptor: {
      configurable: true,
      enumerable: false,
      value: WorkerGlobalScope,
      writable: true,
    },
  };
})(this);
