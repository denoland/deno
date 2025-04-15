// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { primordials } from "ext:core/mod.js";
import { op_webstorage_iterate_keys, Storage } from "ext:core/ops";
const {
  SymbolFor,
  ObjectFromEntries,
  ObjectEntries,
  ReflectDefineProperty,
  ReflectDeleteProperty,
  FunctionPrototypeBind,
  ReflectHas,
  Proxy,
} = primordials;

function createStorage(persistent) {
  const storage = new Storage(persistent);

  const proxy = new Proxy(storage, {
    deleteProperty(target, key) {
      if (typeof key === "symbol") {
        return ReflectDeleteProperty(target, key);
      }
      target.removeItem(key);
      return true;
    },

    defineProperty(target, key, descriptor) {
      if (typeof key === "symbol") {
        return ReflectDefineProperty(target, key, descriptor);
      }
      target.setItem(key, descriptor.value);
      return true;
    },

    get(target, key) {
      if (typeof key === "symbol") {
        return target[key];
      }
      if (ReflectHas(target, key)) {
        const value = target[key];
        if (typeof value === "function") {
          return FunctionPrototypeBind(value, target);
        }
        return value;
      }
      return target.getItem(key) ?? undefined;
    },

    set(target, key, value) {
      if (typeof key === "symbol") {
        return ReflectDefineProperty(target, key, {
          __proto__: null,
          value,
          configurable: true,
        });
      }
      target.setItem(key, value);
      return true;
    },

    has(target, key) {
      if (ReflectHas(target, key)) {
        return true;
      }
      return typeof key === "string" && typeof target.getItem(key) === "string";
    },

    ownKeys() {
      return op_webstorage_iterate_keys(storage);
    },

    getOwnPropertyDescriptor(target, key) {
      if (ReflectHas(target, key)) {
        return undefined;
      }
      if (typeof key === "symbol") {
        return undefined;
      }
      const value = target.getItem(key);
      if (value === null) {
        return undefined;
      }
      return {
        value,
        enumerable: true,
        configurable: true,
        writable: true,
      };
    },
  });

  storage[SymbolFor("Deno.privateCustomInspect")] = function (
    inspect,
    inspectOptions,
  ) {
    return `Storage ${
      inspect({
        ...ObjectFromEntries(ObjectEntries(proxy)),
        length: this.length,
      }, inspectOptions)
    }`;
  };

  return proxy;
}

let localStorageStorage;
function localStorage() {
  if (!localStorageStorage) {
    localStorageStorage = createStorage(true);
  }
  return localStorageStorage;
}

let sessionStorageStorage;
function sessionStorage() {
  if (!sessionStorageStorage) {
    sessionStorageStorage = createStorage(false);
  }
  return sessionStorageStorage;
}

export { localStorage, sessionStorage, Storage };
