// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
    [SymbolToStringTag] = "Window";

    constructor(key = null) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
    }
  }

  class WorkerGlobalScope extends EventTarget {
    [SymbolToStringTag] = "WorkerGlobalScope";

    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
    }
  }

  class DedicatedWorkerGlobalScope extends WorkerGlobalScope {
    [SymbolToStringTag] = "DedicatedWorkerGlobalScope";

    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
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
