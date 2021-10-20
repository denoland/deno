// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { errors } = window.__bootstrap.errors;
  const {
    Set,
    TypeError,
  } = window.__bootstrap.primordials;

  function bindSignal(signo) {
    return core.opSync("op_signal_bind", signo);
  }

  function pollSignal(rid) {
    return core.opAsync("op_signal_poll", rid);
  }

  function unbindSignal(rid) {
    core.opSync("op_signal_unbind", rid);
  }

  // Stores signal handlers, has type of
  // `Record<string, { rid: number | undefined, handlers: Set<() => void> }`
  const handlers = {};

  /** Gets the signal handlers and resource data of the given signal */
  function getSignalData(signo) {
    return handlers[signo] ??
      (handlers[signo] = { rid: undefined, handlers: new Set() });
  }

  function addSignalListener(signo, handler) {
    if (typeof handler !== "function") {
      throw new TypeError(
        `Signal listener must be a function. "${typeof handler}" is given.`,
      );
    }

    const sigData = getSignalData(signo);
    sigData.handlers.add(handler);

    if (!sigData.rid) {
      // If signal resource doesn't exist, create it.
      // The program starts listening to this signal
      sigData.rid = bindSignal(signo);
      loop(sigData);
    }
  }

  function removeSignalListener(signo, handler) {
    if (typeof handler !== "function") {
      throw new TypeError(
        `Signal listener must be a function. "${typeof handler}" is given.`,
      );
    }

    const sigData = getSignalData(signo);
    sigData.handlers.delete(handler);

    if (sigData.handlers.size === 0 && sigData.rid) {
      unbindSignal(sigData.rid);
      sigData.rid = undefined;
    }
  }

  async function loop(sigData) {
    try {
      while (sigData.rid) {
        if (await pollSignal(sigData.rid)) {
          return;
        }
        sigData.handlers.forEach((handler) => {
          handler?.();
        });
      }
    } catch (e) {
      if (e instanceof errors.BadResource) {
        // listener resource is already released
        return;
      }
      // Unknown error, shouldn't happen
      throw e;
    }
  }

  window.__bootstrap.signals = {
    addSignalListener,
    removeSignalListener,
  };
})(this);
