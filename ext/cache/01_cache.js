// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const {
    SafeArrayIterator,
    Symbol,
    SymbolFor,
    ObjectDefineProperty,
    ObjectFromEntries,
    ObjectEntries,
    ReflectGet,
    ReflectHas,
    Proxy,
  } = window.__bootstrap.primordials;

  const _persistent = Symbol("[[persistent]]");

  class CacheStorage {
    #storage;

    constructor() {
      webidl.illegalConstructor();
    }

    get length() {
      webidl.assertBranded(this, StoragePrototype);
      return core.opSync("op_webstorage_length", this[_persistent]);
    }

    key(index) {
      webidl.assertBranded(this, StoragePrototype);
      const prefix = "Failed to execute 'key' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });

      return core.opSync("op_webstorage_key", index, this[_persistent]);
    }

    setItem(key, value) {
      webidl.assertBranded(this, StoragePrototype);
      const prefix = "Failed to execute 'setItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 2",
      });

      core.opSync("op_webstorage_set", key, value, this[_persistent]);
    }

    getItem(key) {
      webidl.assertBranded(this, StoragePrototype);
      const prefix = "Failed to execute 'getItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });

      return core.opSync("op_webstorage_get", key, this[_persistent]);
    }

    removeItem(key) {
      webidl.assertBranded(this, StoragePrototype);
      const prefix = "Failed to execute 'removeItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });

      core.opSync("op_webstorage_remove", key, this[_persistent]);
    }

    clear() {
      webidl.assertBranded(this, StoragePrototype);
      core.opSync("op_webstorage_clear", this[_persistent]);
    }
  }

  window.__bootstrap.webStorage = {
    localStorage() {
      if (!localStorage) {
        localStorage = createStorage(true);
      }
      return localStorage;
    },
    sessionStorage() {
      if (!sessionStorage) {
        sessionStorage = createStorage(false);
      }
      return sessionStorage;
    },
    Storage,
  };
})(this);
