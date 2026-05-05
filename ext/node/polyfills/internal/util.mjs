// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { normalizeEncoding } = core.loadExtScript(
  "ext:deno_node/internal/normalize_encoding.ts",
);
const {
  ObjectCreate,
  StringPrototypeToUpperCase,
} = core.loadExtScript("ext:deno_node/internal/primordials.mjs");
const { ERR_UNKNOWN_SIGNAL } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { os } = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
);
const { validateFunction } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { isNativeError } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);

// deno-lint-ignore prefer-primordials
const AtomicsWait = Atomics.wait;

const {
  ArrayPrototypePush,
  ErrorPrototype,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectGetPrototypeOf,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetOwnPropertyDescriptors,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  Promise,
  ReflectApply,
  ReflectConstruct,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeHas,
  SafeWeakRef,
  StringPrototypeReplace,
  SymbolFor,
  WeakRefPrototypeDeref,
} = primordials;

const customInspectSymbol = SymbolFor("nodejs.util.inspect.custom");
const kEnumerableProperty = ObjectCreate(null);
kEnumerableProperty.enumerable = true;

const kEmptyObject = ObjectFreeze(ObjectCreate(null));

function once(callback) {
  let called = false;
  return function (...args) {
    if (called) return;
    called = true;
    ReflectApply(callback, this, args);
  };
}

// In addition to being accessible through util.promisify.custom,
// this symbol is registered globally and can be accessed in any environment as
// Symbol.for('nodejs.util.promisify.custom').
const kCustomPromisifiedSymbol = SymbolFor(
  "nodejs.util.promisify.custom",
);
// This is an internal Node symbol used by functions returning multiple
// arguments, e.g. ['bytesRead', 'buffer'] for fs.read().
const kCustomPromisifyArgsSymbol = SymbolFor(
  "nodejs.util.promisify.customArgs",
);

const customPromisifyArgs = kCustomPromisifyArgsSymbol;

/** @param {string} str */
function removeColors(str) {
  return StringPrototypeReplace(str, colorRegExp, "");
}

/**
 * @param {unknown} e
 * @returns {boolean}
 */
function isError(e) {
  // An error could be an instance of Error while not being a native error
  // or could be from a different realm and not be instance of Error but still
  // be a native error.
  return isNativeError(e) || ObjectPrototypeIsPrototypeOf(ErrorPrototype, e);
}

function promisify(
  original,
) {
  validateFunction(original, "original");
  if (original[kCustomPromisifiedSymbol]) {
    const fn = original[kCustomPromisifiedSymbol];

    validateFunction(fn, "util.promisify.custom");

    return ObjectDefineProperty(fn, kCustomPromisifiedSymbol, {
      __proto__: null,
      value: fn,
      enumerable: false,
      writable: false,
      configurable: true,
    });
  }

  // Names to create an object from in case the callback receives multiple
  // arguments, e.g. ['bytesRead', 'buffer'] for fs.read.
  const argumentNames = original[kCustomPromisifyArgsSymbol];
  function fn(...args) {
    return new Promise((resolve, reject) => {
      ArrayPrototypePush(args, (err, ...values) => {
        if (err) {
          return reject(err);
        }
        if (argumentNames !== undefined && values.length > 1) {
          const obj = {};
          for (let i = 0; i < argumentNames.length; i++) {
            obj[argumentNames[i]] = values[i];
          }
          resolve(obj);
        } else {
          resolve(values[0]);
        }
      });
      ReflectApply(original, this, args);
    });
  }

  ObjectSetPrototypeOf(fn, ObjectGetPrototypeOf(original));

  ObjectDefineProperty(fn, kCustomPromisifiedSymbol, {
    __proto__: null,
    value: fn,
    enumerable: false,
    writable: false,
    configurable: true,
  });
  return ObjectDefineProperties(
    fn,
    ObjectGetOwnPropertyDescriptors(original),
  );
}

let signalsToNamesMapping;
function getSignalsToNamesMapping() {
  if (signalsToNamesMapping !== undefined) {
    return signalsToNamesMapping;
  }

  signalsToNamesMapping = ObjectCreate(null);
  for (const key in os.signals) {
    signalsToNamesMapping[os.signals[key]] = key;
  }

  return signalsToNamesMapping;
}

function convertToValidSignal(signal) {
  if (typeof signal === "number" && getSignalsToNamesMapping()[signal]) {
    return signal;
  }

  if (typeof signal === "string") {
    const signalName = os.signals[StringPrototypeToUpperCase(signal)];
    if (signalName) return signalName;
  }

  throw new ERR_UNKNOWN_SIGNAL(signal);
}

const codesWarned = new SafeSet();

const experimentalWarnings = new SafeSet();

function emitExperimentalWarning(feature, messagePrefix, code, ctor) {
  if (SetPrototypeHas(experimentalWarnings, feature)) return;
  SetPrototypeAdd(experimentalWarnings, feature);
  let msg =
    `${feature} is an experimental feature and might change at any time`;
  if (messagePrefix) {
    msg = messagePrefix + msg;
  }
  globalThis.process.emitWarning(msg, "ExperimentalWarning", code, ctor);
}

const pendingCodesWarned = new SafeSet();

// Internal deprecator for pending --pending-deprecation. Emits the warning only
// when --pending-deprecation is set and --no-deprecation is not.
function pendingDeprecate(fn, msg, code) {
  function deprecated(...args) {
    const process = globalThis.process;
    if (
      process.execArgv?.includes("--pending-deprecation") &&
      !process.noDeprecation
    ) {
      if (code !== undefined) {
        if (!SetPrototypeHas(pendingCodesWarned, code)) {
          process.emitWarning(msg, "DeprecationWarning", code, deprecated);
          SetPrototypeAdd(pendingCodesWarned, code);
        }
      } else {
        process.emitWarning(msg, "DeprecationWarning", deprecated);
      }
    }
    return ReflectApply(fn, this, args);
  }

  ObjectDefineProperty(deprecated, "length", {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(fn, "length"),
  });

  return deprecated;
}

function deprecateInstantiation(Constructor, deprecationCode, ...args) {
  if (!SetPrototypeHas(codesWarned, deprecationCode)) {
    SetPrototypeAdd(codesWarned, deprecationCode);
    globalThis.process.emitWarning(
      `Instantiating ${Constructor.name} without the 'new' keyword has been deprecated.`,
      "DeprecationWarning",
      deprecationCode,
    );
  }
  return ReflectConstruct(Constructor, args);
}

class WeakReference {
  #weak = null;
  #strong = null;
  #refCount = 0;
  constructor(object) {
    this.#weak = new SafeWeakRef(object);
  }

  incRef() {
    this.#refCount++;
    if (this.#refCount === 1) {
      const derefed = WeakRefPrototypeDeref(this.#weak);
      if (derefed !== undefined) {
        this.#strong = derefed;
      }
    }
    return this.#refCount;
  }

  decRef() {
    this.#refCount--;
    if (this.#refCount === 0) {
      this.#strong = null;
    }
    return this.#refCount;
  }

  get() {
    return WeakRefPrototypeDeref(this.#weak);
  }
}

promisify.custom = kCustomPromisifiedSymbol;

let _sleepView;

function sleep(msec) {
  if (_sleepView === undefined) {
    // deno-lint-ignore prefer-primordials
    const buffer = new SharedArrayBuffer(4);
    // deno-lint-ignore prefer-primordials
    _sleepView = new Int32Array(buffer);
  }
  AtomicsWait(_sleepView, 0, 0, msec);
}

return {
  convertToValidSignal,
  customInspectSymbol,
  customPromisifyArgs,
  deprecateInstantiation,
  emitExperimentalWarning,
  isError,
  kEmptyObject,
  kEnumerableProperty,
  kCustomPromisifiedSymbol,
  normalizeEncoding,
  once,
  pendingDeprecate,
  promisify,
  removeColors,
  sleep,
  WeakReference,
  default: {
    convertToValidSignal,
    customInspectSymbol,
    customPromisifyArgs,
    deprecateInstantiation,
    emitExperimentalWarning,
    isError,
    kEmptyObject,
    kEnumerableProperty,
    normalizeEncoding,
    once,
    pendingDeprecate,
    promisify,
    removeColors,
    sleep,
  },
};
})();
