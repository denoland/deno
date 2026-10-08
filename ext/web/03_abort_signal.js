// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayIsArray,
  ArrayPrototypePush,
  FunctionPrototypeApply,
  ObjectPrototypeIsPrototypeOf,
  SafeFinalizationRegistry,
  SafeSet,
  SafeSetIterator,
  SafeWeakRef,
  SetPrototypeAdd,
  SetPrototypeDelete,
  SetPrototypeGetSize,
  Symbol,
  SymbolFor,
  TypeError,
  TypeErrorPrototype,
  WeakRefPrototypeDeref,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { assert } = core.loadExtScript("ext:deno_web/00_infra.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const {
  defineEventHandler,
  Event,
  EventTarget,
  listenerCount,
  setIsTrusted,
} = core.loadExtScript("ext:deno_web/02_event.js");

const add = Symbol("[[add]]");
const signalAbort = Symbol("[[signalAbort]]");
const remove = Symbol("[[remove]]");
const runAbortSteps = Symbol("[[runAbortSteps]]");
const abortReason = Symbol("[[abortReason]]");
const abortAlgos = Symbol("[[abortAlgos]]");
const dependent = Symbol("[[dependent]]");
const sourceSignals = Symbol("[[sourceSignals]]");
const dependentSignals = Symbol("[[dependentSignals]]");
const signal = Symbol("[[signal]]");
const timerId = Symbol("[[timerId]]");
const activeDependents = Symbol("[[activeDependents]]");

const illegalConstructorKey = Symbol("illegalConstructorKey");

// `dependentSignals` stores a WeakRef. A GC before the source aborts drops
// the dependent, and the abort never arrives.
function retainActiveDependent(dependentSignal) {
  const sources = dependentSignal[sourceSignals];
  if (sources === null) {
    return;
  }
  for (const weakRef of new SafeSetIterator(sources)) {
    const sourceSignal = WeakRefPrototypeDeref(weakRef);
    if (sourceSignal === undefined) continue;
    sourceSignal[activeDependents] ??= new SafeSet();
    SetPrototypeAdd(sourceSignal[activeDependents], dependentSignal);
  }
}

function releaseActiveDependent(dependentSignal) {
  const sources = dependentSignal[sourceSignals];
  if (sources === null) {
    return;
  }
  for (const weakRef of new SafeSetIterator(sources)) {
    const sourceSignal = WeakRefPrototypeDeref(weakRef);
    if (sourceSignal !== undefined && sourceSignal[activeDependents]) {
      SetPrototypeDelete(sourceSignal[activeDependents], dependentSignal);
    }
  }
}

function hasAbortAlgorithms(signal) {
  const algos = signal[abortAlgos];
  return algos !== null && SetPrototypeGetSize(algos) !== 0;
}

// When a dependent signal created by AbortSignal.any() is GC'd, clean up
// its WeakRef from each source signal's dependentSignals set.
// This prevents unbounded growth of the set when .any() is called in a loop.
const dependentSignalCleanupRegistry = new SafeFinalizationRegistry(
  (prevent) => {
    const sourceSignal = WeakRefPrototypeDeref(prevent.ref);
    if (sourceSignal === undefined) return;
    const deps = sourceSignal[dependentSignals];
    if (deps === null) return;
    SetPrototypeDelete(deps, prevent.weak);
  },
);

class AbortSignal extends EventTarget {
  [abortReason] = undefined;
  [abortAlgos] = null;
  [dependent] = false;
  [sourceSignals] = null;
  [dependentSignals] = null;
  [timerId] = null;
  [webidl.brand] = webidl.brand;

  static any(signals) {
    const prefix = "Failed to execute 'AbortSignal.any'";
    // Node tags these validation errors with `ERR_INVALID_ARG_TYPE`; the
    // WebIDL machinery otherwise throws plain (code-less) TypeErrors.
    try {
      webidl.requiredArguments(arguments.length, 1, prefix);
    } catch (e) {
      e.code = "ERR_INVALID_ARG_TYPE";
      throw e;
    }
    // Validate array elements up front so a non-AbortSignal member reports
    // Node's `signals[i] is not of type AbortSignal.` message. Non-array
    // iterables fall through to the sequence converter below.
    if (ArrayIsArray(signals)) {
      for (let i = 0; i < signals.length; i++) {
        if (!ObjectPrototypeIsPrototypeOf(AbortSignalPrototype, signals[i])) {
          const err = new TypeError(
            `signals[${i}] is not of type AbortSignal.`,
          );
          err.code = "ERR_INVALID_ARG_TYPE";
          throw err;
        }
      }
    }
    try {
      return createDependentAbortSignal(signals, prefix);
    } catch (e) {
      if (
        ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e) &&
        e.code === undefined
      ) {
        e.code = "ERR_INVALID_ARG_TYPE";
      }
      throw e;
    }
  }

  static abort(reason = undefined) {
    if (reason !== undefined) {
      reason = webidl.converters.any(reason);
    }
    const signal = new AbortSignal(illegalConstructorKey);
    signal[signalAbort](reason);
    return signal;
  }

  static timeout(millis) {
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

    const signal = new AbortSignal(illegalConstructorKey);
    signal[timerId] = core.createSystemTimer(
      () => {
        core.cancelTimer(signal[timerId]);
        signal[timerId] = null;
        signal[signalAbort](
          new DOMException(
            "The operation was aborted due to timeout",
            "TimeoutError",
          ),
        );
      },
      millis,
      false, // start unrefed (like Node.js)
    );
    return signal;
  }

  [add](algorithm) {
    if (this.aborted) {
      return;
    }
    this[abortAlgos] ??= new SafeSet();
    SetPrototypeAdd(this[abortAlgos], algorithm);
    // Fetch and streams cancel through this path. Refing the timer would
    // pin the event loop for an internal cancellation.
    retainActiveDependent(this);
  }

  [signalAbort](
    reason = new DOMException("The signal has been aborted", "AbortError"),
  ) {
    if (this.aborted) {
      return;
    }
    this[abortReason] = reason;

    const dependentSignalsToAbort = [];
    if (this[dependentSignals] !== null) {
      for (const weakRef of new SafeSetIterator(this[dependentSignals])) {
        const dependentSignal = WeakRefPrototypeDeref(weakRef);
        if (
          dependentSignal !== undefined &&
          dependentSignal[abortReason] === undefined
        ) {
          dependentSignal[abortReason] = this[abortReason];
          ArrayPrototypePush(dependentSignalsToAbort, dependentSignal);
        }
      }
    }

    this[runAbortSteps]();

    if (dependentSignalsToAbort.length !== 0) {
      for (let i = 0; i < dependentSignalsToAbort.length; ++i) {
        const dependentSignal = dependentSignalsToAbort[i];
        dependentSignal[runAbortSteps]();
      }
    }
  }

  [runAbortSteps]() {
    const algos = this[abortAlgos];
    this[abortAlgos] = null;

    if (algos !== null) {
      for (const algorithm of new SafeSetIterator(algos)) {
        algorithm();
      }
    }

    if (listenerCount(this, "abort") > 0) {
      const event = new Event("abort");
      setIsTrusted(event, true);
      super.dispatchEvent(event);
    }

    // Abort has been delivered, so the source no longer needs this dependent.
    releaseActiveDependent(this);
  }

  [remove](algorithm) {
    this[abortAlgos] && SetPrototypeDelete(this[abortAlgos], algorithm);
    if (!hasAbortAlgorithms(this) && listenerCount(this, "abort") === 0) {
      releaseActiveDependent(this);
    }
  }

  constructor(key = null) {
    if (key !== illegalConstructorKey) {
      throw new TypeError("Illegal constructor");
    }
    super();
  }

  get aborted() {
    webidl.assertBranded(this, AbortSignalPrototype);
    return this[abortReason] !== undefined;
  }

  get reason() {
    webidl.assertBranded(this, AbortSignalPrototype);
    return this[abortReason];
  }

  throwIfAborted() {
    webidl.assertBranded(this, AbortSignalPrototype);
    if (this[abortReason] !== undefined) {
      throw this[abortReason];
    }
  }

  // `addEventListener` and `removeEventListener` have to be overridden in
  // order to have the timer block the event loop while there are listeners.
  // `[add]` and `[remove]` don't ref and unref the timer because they can
  // only be used by Deno internals, which use it to essentially cancel async
  // ops which would block the event loop.
  addEventListener() {
    FunctionPrototypeApply(super.addEventListener, this, arguments);
    if (listenerCount(this, "abort") > 0) {
      if (this[timerId] !== null) {
        core.refTimer(this[timerId]);
      } else if (this[sourceSignals] !== null) {
        retainActiveDependent(this);
        for (const weakRef of new SafeSetIterator(this[sourceSignals])) {
          const sourceSignal = WeakRefPrototypeDeref(weakRef);
          if (sourceSignal === undefined) continue;
          if (sourceSignal[timerId] !== null) {
            // For timer sources, also keep the event loop alive.
            core.refTimer(sourceSignal[timerId]);
          }
        }
      }
    }
  }

  removeEventListener() {
    FunctionPrototypeApply(super.removeEventListener, this, arguments);
    if (listenerCount(this, "abort") === 0) {
      if (this[timerId] !== null) {
        core.unrefTimer(this[timerId]);
      } else if (this[sourceSignals] !== null) {
        // Listeners are gone. An abort algorithm still needs the backref.
        if (!hasAbortAlgorithms(this)) {
          releaseActiveDependent(this);
        }
        for (const weakRef of new SafeSetIterator(this[sourceSignals])) {
          const sourceSignal = WeakRefPrototypeDeref(weakRef);
          if (sourceSignal === undefined) continue;
          if (sourceSignal[timerId] !== null) {
            // Timer source: stop refing the event loop only if no other
            // dependent of this timer still has listeners.
            let allInactive = true;
            if (sourceSignal[dependentSignals] !== null) {
              for (
                const depRef of new SafeSetIterator(
                  sourceSignal[dependentSignals],
                )
              ) {
                const dep = WeakRefPrototypeDeref(depRef);
                if (
                  dep !== undefined && dep !== this &&
                  listenerCount(dep, "abort") > 0
                ) {
                  allInactive = false;
                  break;
                }
              }
            }
            if (allInactive) {
              core.unrefTimer(sourceSignal[timerId]);
            }
          }
        }
      }
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
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
  }
}
defineEventHandler(AbortSignal.prototype, "abort");

webidl.configureInterface(AbortSignal);
const AbortSignalPrototype = AbortSignal.prototype;

class AbortController {
  [signal] = new AbortSignal(illegalConstructorKey);

  constructor() {
    this[webidl.brand] = webidl.brand;
  }

  get signal() {
    webidl.assertBranded(this, AbortControllerPrototype);
    return this[signal];
  }

  abort(reason) {
    webidl.assertBranded(this, AbortControllerPrototype);
    this[signal][signalAbort](reason);
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(AbortControllerPrototype, this),
        keys: [
          "signal",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(AbortController);
const AbortControllerPrototype = AbortController.prototype;

webidl.converters.AbortSignal = webidl.createInterfaceConverter(
  "AbortSignal",
  AbortSignal.prototype,
);
webidl.converters["sequence<AbortSignal>"] = webidl.createSequenceConverter(
  webidl.converters.AbortSignal,
);

function newSignal() {
  return new AbortSignal(illegalConstructorKey);
}

function createDependentAbortSignal(signals, prefix) {
  signals = webidl.converters["sequence<AbortSignal>"](
    signals,
    prefix,
    "Argument 1",
  );

  const resultSignal = new AbortSignal(illegalConstructorKey);
  for (let i = 0; i < signals.length; ++i) {
    const signal = signals[i];
    if (signal[abortReason] !== undefined) {
      resultSignal[abortReason] = signal[abortReason];
      return resultSignal;
    }
  }

  resultSignal[dependent] = true;
  resultSignal[sourceSignals] = new SafeSet();
  for (let i = 0; i < signals.length; ++i) {
    const signal = signals[i];
    if (!signal[dependent]) {
      signal[dependentSignals] ??= new SafeSet();
      const signalRef = new SafeWeakRef(signal);
      const resultRef = new SafeWeakRef(resultSignal);
      SetPrototypeAdd(resultSignal[sourceSignals], signalRef);
      SetPrototypeAdd(signal[dependentSignals], resultRef);
      // When resultSignal is GC'd, remove its WeakRef from signal's dependentSignals
      dependentSignalCleanupRegistry.register(resultSignal, {
        ref: signalRef,
        weak: resultRef,
      });
    } else {
      for (
        const sourceSignalRef of new SafeSetIterator(
          signal[sourceSignals],
        )
      ) {
        const sourceSignal = WeakRefPrototypeDeref(sourceSignalRef);
        if (sourceSignal === undefined) continue;
        assert(sourceSignal[abortReason] === undefined);
        assert(!sourceSignal[dependent]);

        // Check if already tracking this source
        let alreadyTracked = false;
        for (
          const existingRef of new SafeSetIterator(
            resultSignal[sourceSignals],
          )
        ) {
          if (WeakRefPrototypeDeref(existingRef) === sourceSignal) {
            alreadyTracked = true;
            break;
          }
        }
        if (alreadyTracked) continue;

        sourceSignal[dependentSignals] ??= new SafeSet();
        const newSourceRef = new SafeWeakRef(sourceSignal);
        const resultRef = new SafeWeakRef(resultSignal);
        SetPrototypeAdd(resultSignal[sourceSignals], newSourceRef);
        SetPrototypeAdd(sourceSignal[dependentSignals], resultRef);
        dependentSignalCleanupRegistry.register(resultSignal, {
          ref: newSourceRef,
          weak: resultRef,
        });
      }
    }
  }

  return resultSignal;
}

return {
  AbortController,
  AbortSignal,
  AbortSignalPrototype,
  add,
  createDependentAbortSignal,
  newSignal,
  remove,
  signalAbort,
  timerId,
};
})();
