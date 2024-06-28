// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_signal_bind, op_signal_poll, op_signal_unbind } from "ext:core/ops";
const {
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeDelete,
  TypeError,
} = primordials;

function bindSignal(signo) {
  return op_signal_bind(signo);
}

function pollSignal(rid) {
  const promise = op_signal_poll(rid);
  core.unrefOpPromise(promise);
  return promise;
}

function unbindSignal(rid) {
  op_signal_unbind(rid);
}

// Stores signal listeners and resource data. This has type of
// `Record<string, { rid: number | undefined, listeners: Set<() => void> }`
const signalData = { __proto__: null };

/** Gets the signal handlers and resource data of the given signal */
function getSignalData(signo) {
  return signalData[signo] ??
    (signalData[signo] = { rid: undefined, listeners: new SafeSet() });
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
  SetPrototypeAdd(sigData.listeners, listener);

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
  SetPrototypeDelete(sigData.listeners, listener);

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
    for (const listener of new SafeSetIterator(sigData.listeners)) {
      listener();
    }
  }
}

export { addSignalListener, removeSignalListener };
