// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

// @ts-check
/// <reference path="../../core/internal.d.ts" />

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { setIsTrusted, defineEventHandler } = window.__bootstrap.event;
  const {
    Set,
    SetPrototypeAdd,
    SetPrototypeDelete,
    Symbol,
    TypeError,
  } = window.__bootstrap.primordials;

  const add = Symbol("[[add]]");
  const signalAbort = Symbol("[[signalAbort]]");
  const remove = Symbol("[[remove]]");
  const abortReason = Symbol("[[abortReason]]");
  const abortAlgos = Symbol("[[abortAlgos]]");
  const signal = Symbol("[[signal]]");

  const illegalConstructorKey = Symbol("illegalConstructorKey");

  class AbortSignal extends EventTarget {
    static abort(reason = undefined) {
      if (reason !== undefined) {
        reason = webidl.converters.any(reason);
      }
      const signal = new AbortSignal(illegalConstructorKey);
      signal[signalAbort](reason);
      return signal;
    }

    [add](algorithm) {
      if (this.aborted) {
        return;
      }
      if (this[abortAlgos] === null) {
        this[abortAlgos] = new Set();
      }
      SetPrototypeAdd(this[abortAlgos], algorithm);
    }

    [signalAbort](
      reason = new DOMException("The signal has been aborted", "AbortError"),
    ) {
      if (this.aborted) {
        return;
      }
      this[abortReason] = reason;
      if (this[abortAlgos] !== null) {
        for (const algorithm of this[abortAlgos]) {
          algorithm();
        }
        this[abortAlgos] = null;
      }
      const event = new Event("abort");
      setIsTrusted(event, true);
      this.dispatchEvent(event);
    }

    [remove](algorithm) {
      this[abortAlgos] && SetPrototypeDelete(this[abortAlgos], algorithm);
    }

    constructor(key = null) {
      if (key != illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }
      super();
      this[abortReason] = undefined;
      this[abortAlgos] = null;
      this[webidl.brand] = webidl.brand;
    }

    get aborted() {
      webidl.assertBranded(this, AbortSignal);
      return this[abortReason] !== undefined;
    }

    get reason() {
      webidl.assertBranded(this, AbortSignal);
      return this[abortReason];
    }

    throwIfAborted() {
      webidl.assertBranded(this, AbortSignal);
      if (this[abortReason] !== undefined) {
        throw this[abortReason];
      }
    }
  }
  defineEventHandler(AbortSignal.prototype, "abort");

  webidl.configurePrototype(AbortSignal);

  class AbortController {
    [signal] = new AbortSignal(illegalConstructorKey);

    constructor() {
      this[webidl.brand] = webidl.brand;
    }

    get signal() {
      webidl.assertBranded(this, AbortController);
      return this[signal];
    }

    abort(reason) {
      webidl.assertBranded(this, AbortController);
      this[signal][signalAbort](reason);
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
    if (followingSignal.aborted) {
      return;
    }
    if (parentSignal.aborted) {
      followingSignal[signalAbort](parentSignal.reason);
    } else {
      parentSignal[add](() =>
        followingSignal[signalAbort](parentSignal.reason)
      );
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
