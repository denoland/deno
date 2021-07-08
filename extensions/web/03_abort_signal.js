// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

// @ts-check
/// <reference path="../../core/internal.d.ts" />

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { setIsTrusted, defineEventHandler } = window.__bootstrap.event;
  const {
    Boolean,
    Set,
    SetPrototypeAdd,
    SetPrototypeClear,
    SetPrototypeDelete,
    Symbol,
    SymbolToStringTag,
    TypeError,
  } = window.__bootstrap.primordials;

  const add = Symbol("add");
  const signalAbort = Symbol("signalAbort");
  const remove = Symbol("remove");

  const illegalConstructorKey = Symbol("illegalConstructorKey");

  class AbortSignal extends EventTarget {
    #aborted = false;
    #abortAlgorithms = new Set();

    static abort() {
      const signal = new AbortSignal(illegalConstructorKey);
      signal[signalAbort]();
      return signal;
    }

    [add](algorithm) {
      SetPrototypeAdd(this.#abortAlgorithms, algorithm);
    }

    [signalAbort]() {
      if (this.#aborted) {
        return;
      }
      this.#aborted = true;
      for (const algorithm of this.#abortAlgorithms) {
        algorithm();
      }
      SetPrototypeClear(this.#abortAlgorithms);
      const event = new Event("abort");
      setIsTrusted(event, true);
      this.dispatchEvent(event);
    }

    [remove](algorithm) {
      SetPrototypeDelete(this.#abortAlgorithms, algorithm);
    }

    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
      this[webidl.brand] = webidl.brand;
    }

    get aborted() {
      return Boolean(this.#aborted);
    }

    get [SymbolToStringTag]() {
      return "AbortSignal";
    }
  }
  defineEventHandler(AbortSignal.prototype, "abort");

  webidl.configurePrototype(AbortSignal);

  class AbortController {
    #signal = new AbortSignal(illegalConstructorKey);

    get signal() {
      return this.#signal;
    }

    abort() {
      this.#signal[signalAbort]();
    }

    get [SymbolToStringTag]() {
      return "AbortController";
    }
  }

  webidl.configurePrototype(AbortController);

  webidl.converters["AbortSignal"] = webidl.createInterfaceConverter(
    "AbortSignal",
    AbortSignal,
  );

  function newSignal() {
    return new AbortSignal(illegalConstructorKey);
  }

  function follow(followingSignal, parentSignal) {
    if (parentSignal.aborted) {
      followingSignal[signalAbort]();
    } else {
      parentSignal[add](() => followingSignal[signalAbort]());
    }
  }

  window.AbortSignal = AbortSignal;
  window.AbortController = AbortController;
  window.__bootstrap.abortSignal = {
    add,
    signalAbort,
    remove,
    follow,
    newSignal,
  };
})(this);
