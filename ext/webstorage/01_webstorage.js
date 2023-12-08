// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import * as webidl from "ext:deno_webidl/00_webidl.js";
const {
  Symbol,
  SymbolFor,
  ObjectFromEntries,
  ObjectEntries,
  ReflectDefineProperty,
  ReflectDeleteProperty,
  ReflectGet,
  ReflectHas,
  Proxy,
} = primordials;

const _persistent = Symbol("[[persistent]]");

class Storage {
  [_persistent];

  constructor() {
    webidl.illegalConstructor();
  }

  get length() {
    webidl.assertBranded(this, StoragePrototype);
    return ops.op_webstorage_length(this[_persistent]);
  }

  key(index) {
    webidl.assertBranded(this, StoragePrototype);
    const prefix = "Failed to execute 'key' on 'Storage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    index = webidl.converters["unsigned long"](index, prefix, "Argument 1");

    return ops.op_webstorage_key(index, this[_persistent]);
  }

  setItem(key, value) {
    webidl.assertBranded(this, StoragePrototype);
    const prefix = "Failed to execute 'setItem' on 'Storage'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    key = webidl.converters.DOMString(key, prefix, "Argument 1");
    value = webidl.converters.DOMString(value, prefix, "Argument 2");

    ops.op_webstorage_set(key, value, this[_persistent]);
  }

  getItem(key) {
    webidl.assertBranded(this, StoragePrototype);
    const prefix = "Failed to execute 'getItem' on 'Storage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    key = webidl.converters.DOMString(key, prefix, "Argument 1");

    return ops.op_webstorage_get(key, this[_persistent]);
  }

  removeItem(key) {
    webidl.assertBranded(this, StoragePrototype);
    const prefix = "Failed to execute 'removeItem' on 'Storage'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    key = webidl.converters.DOMString(key, prefix, "Argument 1");

    ops.op_webstorage_remove(key, this[_persistent]);
  }

  clear() {
    webidl.assertBranded(this, StoragePrototype);
    ops.op_webstorage_clear(this[_persistent]);
  }
}

const StoragePrototype = Storage.prototype;

function createStorage(persistent) {
  const storage = webidl.createBranded(Storage);
  storage[_persistent] = persistent;

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

    get(target, key, receiver) {
      if (typeof key === "symbol") {
        return target[key];
      }
      if (ReflectHas(target, key)) {
        return ReflectGet(target, key, receiver);
      }
      return target.getItem(key) ?? undefined;
    },

    set(target, key, value) {
      if (typeof key === "symbol") {
        return ReflectDefineProperty(target, key, {
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
      return ops.op_webstorage_iterate_keys(persistent);
    },

    getOwnPropertyDescriptor(target, key) {
      if (ReflectHas(target, key)) {
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
    return `${this.constructor.name} ${
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
