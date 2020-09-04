// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const add = Symbol("add");
  const signalAbort = Symbol("signalAbort");
  const remove = Symbol("remove");

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
      this.dispatchEvent(new Event("abort"));
    }

    [remove](algorithm) {
      this.#abortAlgorithms.delete(algorithm);
    }

    constructor() {
      super();
      this.onabort = null;
      this.addEventListener("abort", (evt) => {
        const { onabort } = this;
        if (typeof onabort === "function") {
          onabort.call(this, evt);
        }
      });
    }

    get aborted() {
      return Boolean(this.#aborted);
    }

    get [Symbol.toStringTag]() {
      return "AbortSignal";
    }
  }

  class AbortController {
    #signal = new AbortSignal();

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

  window.AbortSignal = AbortSignal;
  window.AbortController = AbortController;
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.abortSignal = {
    add,
    signalAbort,
    remove,
  };
})(this);
