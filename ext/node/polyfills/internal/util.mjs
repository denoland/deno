// Copyright 2018-2025 the Deno authors. MIT license.

import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { normalizeEncoding } from "ext:deno_node/internal/normalize_encoding.ts";
export { normalizeEncoding };
import {
  ObjectCreate,
  StringPrototypeToUpperCase,
} from "ext:deno_node/internal/primordials.mjs";
import { ERR_UNKNOWN_SIGNAL } from "ext:deno_node/internal/errors.ts";
import { os } from "ext:deno_node/internal_binding/constants.ts";
import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypePush,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectGetPrototypeOf,
  ObjectGetOwnPropertyDescriptors,
  ObjectSetPrototypeOf,
  Promise,
  ReflectApply,
  SafeWeakRef,
  StringPrototypeReplace,
  SymbolFor,
  WeakRefPrototypeDeref,
} = primordials;

export const customInspectSymbol = SymbolFor("nodejs.util.inspect.custom");
export const kEnumerableProperty = ObjectCreate(null);
kEnumerableProperty.enumerable = true;

export const kEmptyObject = ObjectFreeze(ObjectCreate(null));

export function once(callback) {
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
export const kCustomPromisifiedSymbol = SymbolFor(
  "nodejs.util.promisify.custom",
);
// This is an internal Node symbol used by functions returning multiple
// arguments, e.g. ['bytesRead', 'buffer'] for fs.read().
const kCustomPromisifyArgsSymbol = SymbolFor(
  "nodejs.util.promisify.customArgs",
);

export const customPromisifyArgs = kCustomPromisifyArgsSymbol;

/** @param {string} str */
export function removeColors(str) {
  return StringPrototypeReplace(str, colorRegExp, "");
}

export function promisify(
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

export function convertToValidSignal(signal) {
  if (typeof signal === "number" && getSignalsToNamesMapping()[signal]) {
    return signal;
  }

  if (typeof signal === "string") {
    const signalName = os.signals[StringPrototypeToUpperCase(signal)];
    if (signalName) return signalName;
  }

  throw new ERR_UNKNOWN_SIGNAL(signal);
}

export function deprecateInstantiation() {}

export class WeakReference {
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

export default {
  convertToValidSignal,
  customInspectSymbol,
  customPromisifyArgs,
  deprecateInstantiation,
  kEmptyObject,
  kEnumerableProperty,
  normalizeEncoding,
  once,
  promisify,
  removeColors,
};
