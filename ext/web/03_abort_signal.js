// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeEvery,
  FunctionPrototypeApply,
  ObjectPrototypeIsPrototypeOf,
  ObjectDefineProperties,
  ObjectDefineProperty,
  Symbol,
  SymbolFor,
} = primordials;
import {
  AbortController,
  AbortSignal,
  op_event_add_abort_algorithm,
  op_event_create_abort_signal,
  op_event_create_dependent_abort_signal,
  op_event_get_dependent_signals,
  op_event_get_source_signals,
  op_event_remove_abort_algorithm,
  op_event_signal_abort,
} from "ext:core/ops";

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "./01_console.js";
import { DOMException } from "./01_dom_exception.js";
import {
  defineEventHandler,
  EventTarget,
  getListenerCount,
} from "./02_event.js";
import { clearTimeout, refTimer, unrefTimer } from "./02_timers.js";

const timerId = Symbol("[[timerId]]");

ObjectDefineProperty(AbortSignal, "timeout", {
  __proto__: null,
  value: function timeout(millis) {
    const prefix = "Failed to execute 'AbortSignal.timeout'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    millis = webidl.converters["unsigned long long"](
      millis,
      prefix,
      "Argument 1",
      {
        enforceRange: true,
      },
    );

    const signal = op_event_create_abort_signal();
    signal[timerId] = core.queueSystemTimer(
      undefined,
      false,
      millis,
      () => {
        clearTimeout(signal[timerId]);
        signal[timerId] = null;
        op_event_signal_abort(
          signal,
          new DOMException("Signal timed out.", "TimeoutError"),
        );
      },
    );
    unrefTimer(signal[timerId]);
    return signal;
  },
  configurable: true,
  enumerable: true,
  writable: true,
});

const addEventListener_ = EventTarget.prototype.addEventListener;
const removeEventListener_ = EventTarget.prototype.removeEventListener;

// `addEventListener` and `removeEventListener` have to be overridden in
// order to have the timer block the event loop while there are listeners.
// `[add]` and `[remove]` don't ref and unref the timer because they can
// only be used by Deno internals, which use it to essentially cancel async
// ops which would block the event loop.
ObjectDefineProperties(AbortSignal.prototype, {
  addEventListener: {
    __proto__: null,
    value: function addEventListener() {
      FunctionPrototypeApply(addEventListener_, this, arguments);
      if (getListenerCount(this, "abort") > 0) {
        if (this[timerId] != null) {
          refTimer(this[timerId]);
        } else {
          const sourceSignals = op_event_get_source_signals(this);
          for (let i = 0; i < sourceSignals.length; ++i) {
            const sourceSignal = sourceSignals[i];
            if (sourceSignal[timerId] != null) {
              refTimer(sourceSignal[timerId]);
            }
          }
        }
      }
    },
    configurable: true,
    enumerable: true,
    writable: true,
  },
  removeEventListener: {
    __proto__: null,
    value: function removeEventListener() {
      FunctionPrototypeApply(removeEventListener_, this, arguments);
      if (getListenerCount(this, "abort") === 0) {
        if (this[timerId] !== null) {
          unrefTimer(this[timerId]);
        } else {
          const sourceSignals = op_event_get_source_signals(this);
          for (let i = 0; i < sourceSignals.length; ++i) {
            const sourceSignal = sourceSignals[i];
            if (sourceSignal[timerId] !== null) {
              // Check that all dependent signals of the timer signal do not have listeners
              if (
                ArrayPrototypeEvery(
                  op_event_get_dependent_signals(sourceSignal),
                  (dependentSignal) =>
                    dependentSignal === this ||
                    getListenerCount(dependentSignal, "abort") === 0,
                )
              ) {
                unrefTimer(sourceSignal[timerId]);
              }
            }
          }
        }
      }
    },
    configurable: true,
    enumerable: true,
    writable: true,
  },
  [SymbolFor("Deno.privateCustomInspect")]: {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(AbortSignalPrototype, this),
          keys: [
            "aborted",
            "reason",
            "onabort",
          ],
        }),
        inspectOptions,
      );
    },
  },
});

defineEventHandler(AbortSignal.prototype, "abort");

webidl.configureInterface(AbortSignal);
const AbortSignalPrototype = AbortSignal.prototype;

ObjectDefineProperty(
  AbortController.prototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(
            AbortControllerPrototype,
            this,
          ),
          keys: [
            "signal",
          ],
        }),
        inspectOptions,
      );
    },
  },
);

webidl.configureInterface(AbortController);
const AbortControllerPrototype = AbortController.prototype;

webidl.converters.AbortSignal = webidl.createInterfaceConverter(
  "AbortSignal",
  AbortSignal.prototype,
);
webidl.converters["sequence<AbortSignal>"] = webidl.createSequenceConverter(
  webidl.converters.AbortSignal,
);

/**
 * @returns {AbortSignal}
 */
function newSignal() {
  return op_event_create_abort_signal();
}

/**
 * @param {AbortSignal[]} signals
 * @param {string} prefix
 * @returns {AbortSignal}
 */
function createDependentAbortSignal(signals, prefix) {
  return op_event_create_dependent_abort_signal(signals, prefix);
}

/**
 * @param {AbortSignal} signal
 * @param {() => void} algorithm
 */
function addSignalAlgorithm(signal, algorithm) {
  op_event_add_abort_algorithm(signal, algorithm);
}

/**
 * @param {AbortSignal} signal
 * @param {() => void} algorithm
 */
function removeSignalAlgorithm(signal, algorithm) {
  op_event_remove_abort_algorithm(signal, algorithm);
}

/**
 * @param {AbortSignal} signal
 * @param {any} reason
 */
function signalAbort(signal, reason) {
  op_event_signal_abort(signal, reason);
}

export {
  AbortController,
  AbortSignal,
  AbortSignalPrototype,
  addSignalAlgorithm,
  createDependentAbortSignal,
  newSignal,
  removeSignalAlgorithm,
  signalAbort,
  timerId,
};
