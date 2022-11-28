// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    Set,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  function bindSignal(signo) {
    return ops.op_signal_bind(signo);
  }

  function pollSignal(rid) {
    const promise = core.opAsync("op_signal_poll", rid);
    core.unrefOp(promise[SymbolFor("Deno.core.internalPromiseId")]);
    return promise;
  }

  function unbindSignal(rid) {
    ops.op_signal_unbind(rid);
  }

  // Stores signal listeners and resource data. This has type of
  // `Record<string, { rid: number | undefined, listeners: Set<() => void> }`
  const signalData = {};

  /** Gets the signal handlers and resource data of the given signal */
  function getSignalData(signo) {
    return signalData[signo] ??
      (signalData[signo] = { rid: undefined, listeners: new Set() });
  }

  function checkSignalListenerType(listener) {
    if (typeof listener !== "function") {
      throw new TypeError(
        `Signal listener must be a function. "${typeof listener}" is given.`,
      );
    }
  }

  function addSignalListener(signo, listener) {
    checkSignalListenerType(listener);

    const sigData = getSignalData(signo);
    sigData.listeners.add(listener);

    if (!sigData.rid) {
      // If signal resource doesn't exist, create it.
      // The program starts listening to the signal
      sigData.rid = bindSignal(signo);
      loop(sigData);
    }
  }

  function removeSignalListener(signo, listener) {
    checkSignalListenerType(listener);

    const sigData = getSignalData(signo);
    sigData.listeners.delete(listener);

    if (sigData.listeners.size === 0 && sigData.rid) {
      unbindSignal(sigData.rid);
      sigData.rid = undefined;
    }
  }

  async function loop(sigData) {
    while (sigData.rid) {
      if (await pollSignal(sigData.rid)) {
        return;
      }
      for (const listener of sigData.listeners) {
        listener();
      }
    }
  }

  window.__bootstrap.signals = {
    addSignalListener,
    removeSignalListener,
  };
})(this);
