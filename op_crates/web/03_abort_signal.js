// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { setIsTrusted } = window.__bootstrap.event;

  const add = Symbol("add");
  const signalAbort = Symbol("signalAbort");
  const remove = Symbol("remove");

  const illegalConstructorKey = Symbol("illegalConstructorKey");

  class AbortSignal extends EventTarget {
    #aborted = false;
    #abortAlgorithms = new Set();

    [add](algorithm) {
      this.#abortAlgorithms.add(algorithm);
    }

    [signalAbort]() {
      if (this.#aborted) {
        return;
      }
      this.#aborted = true;
      for (const algorithm of this.#abortAlgorithms) {
        algorithm();
      }
      this.#abortAlgorithms.clear();
      const event = new Event("abort");
      setIsTrusted(event, true);
      this.dispatchEvent(event);
    }

    [remove](algorithm) {
      this.#abortAlgorithms.delete(algorithm);
    }

    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
    }

    get aborted() {
      return Boolean(this.#aborted);
    }

    get [Symbol.toStringTag]() {
      return "AbortSignal";
    }

    static abort() {
      const as = new AbortSignal(illegalConstructorKey);
      as[signalAbort]();
      return as;
    }
  }
  defineEventHandler(AbortSignal.prototype, "abort");
  class AbortController {
    #signal = new AbortSignal(illegalConstructorKey);

    get signal() {
      return this.#signal;
    }

    abort() {
      this.#signal[signalAbort]();
    }

    get [Symbol.toStringTag]() {
      return "AbortController";
    }
  }

  const handlerSymbol = Symbol("eventHandlers");

  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return wrappedHandler.handler.call(this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  // TODO(benjamingr) reuse this here and websocket where possible
  function defineEventHandler(emitter, name) {
    // HTML specification section 8.1.5.1
    Object.defineProperty(emitter, `on${name}`, {
      get() {
        return this[handlerSymbol]?.get(name)?.handler;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = this[handlerSymbol]?.get(name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
        }
        this[handlerSymbol].set(name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }

  window.AbortSignal = AbortSignal;
  window.AbortController = AbortController;
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.abortSignal = {
    add,
    signalAbort,
    remove,
  };
})(this);
