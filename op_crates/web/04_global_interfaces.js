// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";
((window) => {
  const { EventTarget } = window;

  const illegalConstructorKey = Symbol("illegalConstructorKey");

  class Window extends EventTarget {
    constructor(key = null) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
    }

    get [Symbol.toStringTag]() {
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

    get [Symbol.toStringTag]() {
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

    get [Symbol.toStringTag]() {
      return "DedicatedWorkerGlobalScope";
    }
  }

  window.__bootstrap = (window.__bootstrap || {});
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
