// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { DOMException } = window.__bootstrap.domException;
  const { defineEventHandler } = window.__bootstrap.event;
  const { NumberIsNaN, ArrayIsArray, Date, DatePrototypeGetMilliseconds, Set, SetPrototypeHas, SetPrototypeAdd, MathMin } = window.__bootstrap.primordials;

  webidl.converters.IDBTransactionMode = webidl.createEnumConverter(
    "IDBTransactionMode",
    [
      "readonly",
      "readwrite",
      "versionchange",
    ],
  );

  webidl.converters.IDBTransactionDurability = webidl.createEnumConverter(
    "IDBTransactionDurability",
    [
      "default",
      "strict",
      "relaxed",
    ],
  );

  webidl.converters.IDBTransactionOptions = webidl.createDictionaryConverter(
    "IDBTransactionOptions",
    [
      {
        key: "durability",
        converter: webidl.converters.IDBTransactionDurability,
        defaultValue: "default",
      },
    ],
  );

  webidl.converters.IDBObjectStoreParameters = webidl.createDictionaryConverter(
    "IDBObjectStoreParameters",
    [
      {
        key: "keyPath",
        converter: webidl.converters["sequence<DOMString> or DOMString"], // TODO: nullable
        defaultValue: null,
      },
      {
        key: "autoIncrement",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
    ],
  );

  webidl.converters.IDBCursorDirection = webidl.createEnumConverter(
    "IDBCursorDirection",
    [
      "next",
      "nextunique",
      "prev",
      "prevunique",
    ],
  );

  webidl.converters.IDBIndexParameters = webidl.createDictionaryConverter(
    "IDBIndexParameters",
    [
      {
        key: "unique",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
      {
        key: "multiEntry",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
    ],
  );

  // Ref: https://w3c.github.io/IndexedDB/#convert-a-value-to-a-key
  /**
   * @param input {any}
   * @param seen {Set<any>}
   * @returns {(Key | null)}
   */
  function valueToKey(input, seen = new Set()) {
    if (SetPrototypeHas(seen, input)) {
      return null;
    }
    if (webidl.type(input) === "Number") {
      if (NumberIsNaN(input)) {
        return null;
      } else {
        return {
          type: "number",
          value: input,
        };
      }
    } else if (input instanceof Date) {
      const ms = DatePrototypeGetMilliseconds(input);
      if (NumberIsNaN(ms)) {
        return null;
      } else {
        return {
          type: "date",
          value: input,
        };
      }
    } else if (webidl.type(input) === "String") {
      return {
        type: "string",
        value: input,
      }
    } else if () { // TODO: is a buffer source type
      return {
        type: "binary",
        value: input.slice(),
      }
    } else if (ArrayIsArray(input)) {
      SetPrototypeAdd(seen, input);
      const keys = [];
      for (const entry of input) {
        const key = valueToKey(entry, seen);
        if (key === null) {
          return null;
        }
        keys.push(key);
      }
      return {
        type: "array",
        value: keys,
      };
    } else {
      return null;
    }
  }

  // Ref: https://w3c.github.io/IndexedDB/#compare-two-keys
  function compareTwoKeys(a, b) {
    const { type: ta, value: va } = a;
    const { type: tb, value: vb } = b;

    if (ta !== tb) {
      if (ta === "array") {
        return 1;
      } else if (tb === "array") {
        return -1;
      } else if (ta === "binary") {
        return 1;
      } else if (tb === "binary") {
        return -1;
      } else if (ta === "string") {
        return 1;
      } else if (tb === "string") {
        return -1;
      } else if (ta === "date") {
        assert(tb === "date");
        return 1;
      } else {
        return -1;
      }
    }

    switch (ta) {
      case "number":
      case "date": {
        if (va > vb) {
          return 1;
        } else if (va < vb) {
          return -1;
        } else {
          return 0;
        }
      }
      case "string": {
        if (va > vb) {
          return 1;
        } else if (va < vb) {
          return -1;
        } else {
          return 0;
        }
      }
      case "binary": {
        // TODO
      }
      case "array": {
        const len = MathMin(va.length, vb.length);
        for (let i = 0; i < len; i++) {
          const c = compareTwoKeys(va[i], vb[i]);
          if (c !== 0) {
            return c;
          }
        }
        if (va.length > vb.length) {
          return 1;
        } else if (va.length < vb.length) {
          return -1;
        } else {
          return 0;
        }
      }
    }
  }

  const _result = Symbol("[[result]]");
  const _error = Symbol("[[error]]");
  const _source = Symbol("[[source]]");
  const _transaction = Symbol("[[transaction]]");
  const _processed = Symbol("[[processed]]");
  const _done = Symbol("[[done]]");
  // Ref: https://w3c.github.io/IndexedDB/#idbrequest
  class IDBRequest extends EventTarget {
    constructor() {
      super();
      webidl.illegalConstructor();
    }

    [_processed];
    [_done] = false;

    [_result];
    get result() {
      webidl.assertBranded(this, IDBRequestPrototype);
      if (!this[_done]) {
        throw new DOMException("", "InvalidStateError"); // TODO
      }
      return this[_result]; // TODO: or undefined if the request resulted in an error
    }

    [_error] = null;
    get error() {
      webidl.assertBranded(this, IDBRequestPrototype);
      if (!this[_done]) {
        throw new DOMException("", "InvalidStateError"); // TODO
      }
      return this[_error];
    }

    [_source] = null;
    get source() {
      webidl.assertBranded(this, IDBRequestPrototype);
      return this[_source];
    }

    [_transaction] = null;
    get transaction() {
      webidl.assertBranded(this, IDBRequestPrototype);
      return this[_transaction];
    }

    get readyState() {
      webidl.assertBranded(this, IDBRequestPrototype);
      return this[_done] ? "done" : "pending";
    }
  }
  defineEventHandler(IDBRequest.prototype, "success");
  defineEventHandler(IDBRequest.prototype, "error");

  webidl.configurePrototype(IDBRequest);
  const IDBRequestPrototype = IDBRequest.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbopendbrequest
  class IDBOpenDBRequest extends IDBRequest {
    constructor() {
      super();
      webidl.illegalConstructor();
    }
  }
  defineEventHandler(IDBOpenDBRequest.prototype, "blocked");
  defineEventHandler(IDBOpenDBRequest.prototype, "upgradeneeded");

  webidl.configurePrototype(IDBOpenDBRequest);

  /** @type {Set<IDBDatabase>} */
  const connections = new Set();

  // Ref: https://w3c.github.io/IndexedDB/#idbfactory
  class IDBFactory {
    constructor() {
      webidl.illegalConstructor();
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbfactory-open
    open(name, version = undefined) {
      webidl.assertBranded(this, IDBFactoryPrototype);
      const prefix = "Failed to execute 'open' on 'IDBFactory'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      if (version !== undefined) {
        version = webidl.converters["unsigned long long"](version, {
          prefix,
          context: "Argument 2",
          enforceRange: true,
        });
      }

      if (version === 0) {
        throw new TypeError(); // TODO
      }

      const request = webidl.createBranded(IDBOpenDBRequest);

      try {
        const [newVersion, dbVersion] = core.opSync("op_indexeddb_open", name, version);
        const connection = webidl.createBranded(IDBDatabase);
        connection[_name] = name;
        // TODO: connection[_version] = newVersion;
        if (dbVersion < newVersion) {
          for (const conn of connections.values()) {
            if (!conn[_closePending]) {
              conn.dispatchEvent(new IDBVersionChangeEvent("versionchange", {
                bubbles: false,
                cancelable: false,
                oldVersion: dbVersion,
                newVersion,
              }));
            }
          }
          // TODO: why should connections close?
          for (const conn of connections.values()) {
            if (!conn[_closePending]) {
              request.dispatchEvent(new IDBVersionChangeEvent("blocked", {
                bubbles: false,
                cancelable: false,
                oldVersion: dbVersion,
                newVersion,
              }));
              break;
            }
          }
          // Ref: https://w3c.github.io/IndexedDB/#upgrade-transaction-steps
          // TODO: Wait until all connections in openConnections are closed.
          const transaction; // TODO
          // TODO

        }
        request[_result] = connection;
        request[_done] = true;
        request.dispatchEvent(new Event("success"));
      } catch (e) {
        request[_result] = undefined;
        request[_error] = e;
        request[_done] = true;
        request.dispatchEvent(new Event("error", {
          bubbles: true,
          cancelable: true,
        }));
      }

      return request;
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbfactory-deletedatabase
    deleteDatabase(name) {
      webidl.assertBranded(this, IDBFactoryPrototype);
      const prefix = "Failed to execute 'deleteDatabase' on 'IDBFactory'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });

      const request = webidl.createBranded(IDBOpenDBRequest);

      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbfactory-databases
    databases() {
      webidl.assertBranded(this, IDBFactoryPrototype);

      return Promise.resolve([...connections.values()].map((db) => {
        return {
          name: db.name,
          version: db.version,
        };
      }));
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbfactory-cmp
    cmp(first, second) {
      webidl.assertBranded(this, IDBFactoryPrototype);
      const prefix = "Failed to execute 'cmp' on 'IDBFactory'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      first = webidl.converters.any(first, {
        prefix,
        context: "Argument 1",
      });

      second = webidl.converters.any(second, {
        prefix,
        context: "Argument 2",
      });

      const a = valueToKey(first);
      if (a === null) {
        throw new DOMException("Data provided does not meet requirements", "DataError");
      }
      const b = valueToKey(second);
      if (b === null) {
        throw new DOMException("Data provided does not meet requirements", "DataError");
      }

      return compareTwoKeys(a, b);
    }
  }
  webidl.configurePrototype(IDBFactory);
  const IDBFactoryPrototype = IDBFactory.prototype;

  const _name = Symbol("[[name]]");
  const _version = Symbol("[[version]]");
  const _closePending = Symbol("[[closePending]]");
  const _objectStores = Symbol("[[objectStores]]");
  // Ref: https://w3c.github.io/IndexedDB/#idbdatabase
  // TODO: finalizationRegistry
  class IDBDatabase extends EventTarget {
    /** @type {boolean} */
    [_closePending] = false;
    /** @type {Set<ObjectStore>} */
    [_objectStores] = new Set();

    constructor() {
      super();
      webidl.illegalConstructor();
    }

    [_name];
    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-name
    get name() {
      webidl.assertBranded(this, IDBDatabasePrototype);
      return this[_name];
    }

    [_version];
    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-version
    get version() {
      webidl.assertBranded(this, IDBDatabasePrototype);
      return this[_version];
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-objectstorenames
    get objectStoreNames() {
      webidl.assertBranded(this, IDBDatabasePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-transaction
    transaction(storeNames, mode = "readonly", options = {}) {
      webidl.assertBranded(this, IDBDatabasePrototype);
      const prefix = "Failed to execute 'transaction' on 'IDBDatabase'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      storeNames = webidl.converters["sequence<DOMString> or DOMString"](
        storeNames,
        {
          prefix,
          context: "Argument 1",
        },
      );
      mode = webidl.converters.IDBTransactionMode(mode, {
        prefix,
        context: "Argument 2",
      });
      options = webidl.converters.IDBTransactionOptions(options, {
        prefix,
        context: "Argument 3",
      });

      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-close
    close() {
      webidl.assertBranded(this, IDBDatabasePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-createobjectstore
    createObjectStore(name, options = {}) {
      webidl.assertBranded(this, IDBDatabasePrototype);
      const prefix = "Failed to execute 'createObjectStore' on 'IDBDatabase'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      options = webidl.converters.IDBObjectStoreParameters(options, {
        prefix,
        context: "Argument 2",
      });

      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbdatabase-deleteobjectstore
    deleteObjectStore(name) {
      webidl.assertBranded(this, IDBDatabasePrototype);
      const prefix = "Failed to execute 'deleteObjectStore' on 'IDBDatabase'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });

      // TODO
    }
  }
  defineEventHandler(IDBDatabase.prototype, "abort");
  defineEventHandler(IDBDatabase.prototype, "close");
  defineEventHandler(IDBDatabase.prototype, "error");
  defineEventHandler(IDBDatabase.prototype, "versionchange");

  webidl.configurePrototype(IDBDatabase);
  const IDBDatabasePrototype = IDBDatabase.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbobjectstore
  class IDBObjectStore {
    constructor() {
      webidl.illegalConstructor();
    }

    [_name];
    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-name
    get name() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      return this[_name];
    }

    // Ref: https://w3c.github.io/IndexedDB/#ref-for-dom-idbobjectstore-name%E2%91%A2
    set name(name) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      name = webidl.converters.DOMString(name, {
        prefix: "Failed to set 'name' on 'IDBObjectStore'",
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-keypath
    get keyPath() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-indexnames
    get indexNames() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-transaction
    get transaction() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-autoincrement
    get autoIncrement() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-put
    put(value, key) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'put' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.any(value, {
        prefix,
        context: "Argument 1",
      });
      key = webidl.converters.any(key, {
        prefix,
        context: "Argument 2",
      });

      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-add
    add(value, key) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'add' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.any(value, {
        prefix,
        context: "Argument 1",
      });
      key = webidl.converters.any(key, {
        prefix,
        context: "Argument 2",
      });

      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-delete
    delete(query) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'delete' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-clear
    clear() {
      webidl.assertBranded(this, IDBObjectStorePrototype);
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-get
    get(query) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'get' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-getkey
    getKey(query) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'getKey' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-getall
    getAll(query, count = undefined) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'getAll' on 'IDBObjectStore'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      if (count !== undefined) {
        count = webidl.converters["unsigned long"](count, {
          prefix,
          context: "Argument 2",
          enforceRange: true,
        });
      }
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-getallkeys
    getAllKeys(query, count = undefined) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'getAllKeys' on 'IDBObjectStore'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      if (count !== undefined) {
        count = webidl.converters["unsigned long"](count, {
          prefix,
          context: "Argument 2",
          enforceRange: true,
        });
      }
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-count
    count(query) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'count' on 'IDBObjectStore'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-opencursor
    openCursor(query, direction = "next") {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'openCursor' on 'IDBObjectStore'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      direction = webidl.converters.IDBCursorDirection(direction, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-openkeycursor
    openKeyCursor(query, direction = "next") {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'openKeyCursor' on 'IDBObjectStore'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      direction = webidl.converters.IDBCursorDirection(direction, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-index
    index(name) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'index' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-createindex
    createIndex(name, keypath, options = {}) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'createIndex' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      keypath = webidl.converters["sequence<DOMString> or DOMString"](keypath, {
        prefix,
        context: "Argument 2",
      });
      options = webidl.converters.IDBIndexParameters(options, {
        prefix,
        context: "Argument 3",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbobjectstore-deleteindex
    deleteIndex(name) {
      webidl.assertBranded(this, IDBObjectStorePrototype);
      const prefix = "Failed to execute 'deleteIndex' on 'IDBObjectStore'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }
  }
  webidl.configurePrototype(IDBObjectStore);
  const IDBObjectStorePrototype = IDBObjectStore.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbindex
  class IDBIndex {
    constructor() {
      webidl.illegalConstructor();
    }

    [_name];
    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-name
    get name() {
      webidl.assertBranded(this, IDBIndexPrototype);
      return this[_name];
    }

    // Ref: https://w3c.github.io/IndexedDB/#ref-for-dom-idbindex-name%E2%91%A2
    set name(name) {
      webidl.assertBranded(this, IDBIndexPrototype);
      name = webidl.converters.DOMString(name, {
        prefix: "Failed to set 'name' on 'IDBIndex'",
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-objectstore
    get objectStore() {
      webidl.assertBranded(this, IDBIndexPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-keypath
    get keyPath() {
      webidl.assertBranded(this, IDBIndexPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-multientry
    get multiEntry() {
      webidl.assertBranded(this, IDBIndexPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-unique
    get unique() {
      webidl.assertBranded(this, IDBIndexPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-get
    get(query) {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'get' on 'IDBIndex'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-getkey
    getKey(query) {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'getKey' on 'IDBIndex'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-getall
    getAll(query, count = undefined) {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'getAll' on 'IDBIndex'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      if (count !== undefined) {
        count = webidl.converters["unsigned long"](count, {
          prefix,
          context: "Argument 2",
          enforceRange: true,
        });
      }
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-getallkeys
    getAllKeys(query, count = undefined) {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'getAllKeys' on 'IDBIndex'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      if (count !== undefined) {
        count = webidl.converters["unsigned long"](count, {
          prefix,
          context: "Argument 2",
          enforceRange: true,
        });
      }
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-count
    count(query) {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'count' on 'IDBIndex'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-opencursor
    openCursor(query, direction = "next") {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'openCursor' on 'IDBIndex'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      direction = webidl.converters.IDBCursorDirection(direction, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbindex-openkeycursor
    openKeyCursor(query, direction = "next") {
      webidl.assertBranded(this, IDBIndexPrototype);
      const prefix = "Failed to execute 'openKeyCursor' on 'IDBIndex'";
      query = webidl.converters.any(query, {
        prefix,
        context: "Argument 1",
      });
      direction = webidl.converters.IDBCursorDirection(direction, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }
  }
  webidl.configurePrototype(IDBIndex);
  const IDBIndexPrototype = IDBIndex.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbkeyrange
  class IDBKeyRange {
    constructor() {
      webidl.illegalConstructor();
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-lower
    get lower() {
      webidl.assertBranded(this, IDBKeyRangePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-upper
    get upper() {
      webidl.assertBranded(this, IDBKeyRangePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-loweropen
    get lowerOpen() {
      webidl.assertBranded(this, IDBKeyRangePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-upperopen
    get upperOpen() {
      webidl.assertBranded(this, IDBKeyRangePrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-only
    static only(value) {
      const prefix = "Failed to execute 'only' on 'IDBKeyRange'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.any(value, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-lowerbound
    static lowerBound(lower, open = false) {
      const prefix = "Failed to execute 'lowerBound' on 'IDBKeyRange'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      lower = webidl.converters.any(lower, {
        prefix,
        context: "Argument 1",
      });
      open = webidl.converters.boolean(open, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-upperbound
    static upperBound(upper, open = false) {
      const prefix = "Failed to execute 'upperBound' on 'IDBKeyRange'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      upper = webidl.converters.any(upper, {
        prefix,
        context: "Argument 1",
      });
      open = webidl.converters.boolean(open, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbkeyrange-bound
    static bound(lower, upper, lowerOpen = false, upperOpen = false) {
      const prefix = "Failed to execute 'bound' on 'IDBKeyRange'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      lower = webidl.converters.any(lower, {
        prefix,
        context: "Argument 1",
      });
      upper = webidl.converters.any(upper, {
        prefix,
        context: "Argument 2",
      });
      lowerOpen = webidl.converters.boolean(lowerOpen, {
        prefix,
        context: "Argument 3",
      });
      upperOpen = webidl.converters.boolean(upperOpen, {
        prefix,
        context: "Argument 4",
      });
      // TODO
    }

    includes(key) {
      webidl.assertBranded(this, IDBKeyRangePrototype);
      const prefix = "Failed to execute 'includes' on 'IDBKeyRange'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      key = webidl.converters.any(key, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }
  }
  webidl.configurePrototype(IDBKeyRange);
  const IDBKeyRangePrototype = IDBKeyRange.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbcursor
  class IDBCursor {
    constructor() {
      webidl.illegalConstructor();
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-source
    get source() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-direction
    get direction() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-key
    get key() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-primarykey
    get primaryKey() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-request
    get request() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-advance
    advance(count) {
      webidl.assertBranded(this, IDBCursorPrototype);
      const prefix = "Failed to execute 'advance' on 'IDBCursor'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      count = webidl.converters["unsigned long"](count, {
        prefix,
        context: "Argument 1",
        enforceRange: true,
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-continue
    continue(key) {
      webidl.assertBranded(this, IDBCursorPrototype);
      const prefix = "Failed to execute 'key' on 'IDBCursor'";
      key = webidl.converters.any(key, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-continueprimarykey
    continuePrimaryKey(key, primaryKey) {
      webidl.assertBranded(this, IDBCursorPrototype);
      const prefix = "Failed to execute 'continuePrimaryKey' on 'IDBCursor'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      key = webidl.converters.any(key, {
        prefix,
        context: "Argument 1",
      });
      primaryKey = webidl.converters.any(primaryKey, {
        prefix,
        context: "Argument 2",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-update
    update(value) {
      webidl.assertBranded(this, IDBCursorPrototype);
      const prefix = "Failed to execute 'update' on 'IDBCursor'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.any(value, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbcursor-delete
    delete() {
      webidl.assertBranded(this, IDBCursorPrototype);
      // TODO
    }
  }
  webidl.configurePrototype(IDBCursor);
  const IDBCursorPrototype = IDBCursor.prototype;

  // Ref: https://w3c.github.io/IndexedDB/#idbtransaction
  class IDBTransaction extends EventTarget {
    constructor() {
      super();
      webidl.illegalConstructor();
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-objectstorenames
    get objectStoreNames() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-mode
    get mode() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-durability
    get durability() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-db
    get db() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-error
    get error() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-objectstore
    objectStore(name) {
      webidl.assertBranded(this, IDBTransactionPrototype);
      const prefix = "Failed to execute 'objectStore' on 'IDBTransaction'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.DOMString(name, {
        prefix,
        context: "Argument 1",
      });
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-commit
    commit() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }

    // Ref: https://w3c.github.io/IndexedDB/#dom-idbtransaction-abort
    abort() {
      webidl.assertBranded(this, IDBTransactionPrototype);
      // TODO
    }
  }
  defineEventHandler(IDBTransaction.prototype, "abort");
  defineEventHandler(IDBTransaction.prototype, "complete");
  defineEventHandler(IDBTransaction.prototype, "error");

  webidl.configurePrototype(IDBTransaction);
  const IDBTransactionPrototype = IDBTransaction.prototype;

  window.__bootstrap.indexedDb = {
    indexeddb: webidl.createBranded(IDBFactory),
    IDBRequest,
    IDBOpenDBRequest,
    IDBFactory,
    IDBDatabase,
    IDBObjectStore,
    IDBIndex,
    IDBKeyRange,
    IDBCursor,
    IDBTransaction,
  };
})(this);
