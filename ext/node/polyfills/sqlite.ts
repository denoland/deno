// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  DatabaseSync: DatabaseSyncOp,
  op_node_database_backup,
  Session,
  StatementSync,
} = core.ops;
const { isUint8Array } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);
const { URLPrototype } = core.loadExtScript("ext:deno_web/00_url.js");

const {
  FunctionPrototypeCall,
  ObjectDefineProperty,
  ObjectDefineProperties,
  ObjectGetOwnPropertyDescriptor,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  Proxy,
  ReflectConstruct,
  SafeSet,
  SafeWeakMap,
  StringPrototypeIncludes,
  SymbolDispose,
  SymbolFor,
  TypeError,
} = primordials;

// Keep in sync with LIMIT_MAPPING in ext/node_sqlite/database.rs.
const LIMIT_NAMES = [
  "length",
  "sqlLength",
  "column",
  "exprDepth",
  "compoundSelect",
  "vdbeOp",
  "functionArg",
  "attach",
  "likePatternLength",
  "variableNumber",
  "triggerDepth",
];

const LIMIT_NAMES_SET = new SafeSet(LIMIT_NAMES);

const nativeLimitsGetter = ObjectGetOwnPropertyDescriptor(
  DatabaseSyncOp.prototype,
  "limits",
).get;

function createLimitsProxy(nativeLimits) {
  return new Proxy(nativeLimits, {
    get(target, prop, _receiver) {
      if (typeof prop !== "string" || !LIMIT_NAMES_SET.has(prop)) {
        return undefined;
      }
      return target[prop];
    },
    set(target, prop, value, _receiver) {
      if (typeof prop !== "string" || !LIMIT_NAMES_SET.has(prop)) {
        return false;
      }

      target[prop] = value;
      return true;
    },
    has(_target, prop) {
      return typeof prop === "string" &&
        LIMIT_NAMES_SET.has(prop);
    },
    ownKeys(_target) {
      return LIMIT_NAMES;
    },
    getOwnPropertyDescriptor(target, prop) {
      if (typeof prop !== "string" || !LIMIT_NAMES_SET.has(prop)) {
        return undefined;
      }

      return {
        value: target[prop],
        writable: true,
        enumerable: true,
        configurable: true,
      };
    },
  });
}

class ConstructCallRequiredError extends TypeError {
  code;
  constructor() {
    super("Cannot call constructor without `new`");
    this.code = "ERR_CONSTRUCT_CALL_REQUIRED";
  }
}

class InvalidArgTypeError extends TypeError {
  code;
  constructor(message) {
    super(message);
    this.code = "ERR_INVALID_ARG_TYPE";
  }
}

class InvalidURLSchemeError extends TypeError {
  code;
  constructor() {
    super("The URL must be of scheme file:");
    this.code = "ERR_INVALID_URL_SCHEME";
  }
}

const parsePath = (path) => {
  let parsedPath;
  if (typeof path === "string") {
    parsedPath = path;
  } else if (isUint8Array(path)) {
    const decoder = new TextDecoder("utf8");
    parsedPath = decoder.decode(path);
  } else if (ObjectPrototypeIsPrototypeOf(URLPrototype, path)) {
    if (path.protocol !== "file:") {
      throw new InvalidURLSchemeError();
    }
    parsedPath = path.href;
  }

  if (
    typeof parsedPath === "undefined" ||
    StringPrototypeIncludes(parsedPath, "\0")
  ) {
    throw new InvalidArgTypeError(
      'The "path" argument must be a string, Uint8Array, or URL without null bytes.',
    );
  }

  return parsedPath;
};

// Using ES5 class allows custom error to be thrown
// when called without `new`.
function DatabaseSync(
  path,
  options,
) {
  if (new.target === undefined) {
    throw new ConstructCallRequiredError();
  }
  return ReflectConstruct(
    DatabaseSyncOp,
    [parsePath(path), options],
    new.target,
  );
}
ObjectSetPrototypeOf(DatabaseSync.prototype, DatabaseSyncOp.prototype);
ObjectSetPrototypeOf(DatabaseSync, DatabaseSyncOp);

// deno-lint-ignore require-await
async function backup(
  sourceDb,
  path,
  options,
) {
  if (!ObjectPrototypeIsPrototypeOf(DatabaseSync.prototype, sourceDb)) {
    throw new InvalidArgTypeError(
      'The "sourceDb" argument must be an object.',
    );
  }

  // TODO(Tango992): Implement async op
  return op_node_database_backup(
    sourceDb,
    parsePath(path),
    options,
  );
}
ObjectDefineProperty(backup, "length", {
  __proto__: null,
  value: 2,
  enumerable: false,
  configurable: true,
  writable: false,
});

const constants = {
  SQLITE_CHANGESET_OMIT: 0,
  SQLITE_CHANGESET_REPLACE: 1,
  SQLITE_CHANGESET_ABORT: 2,

  SQLITE_CHANGESET_DATA: 1,
  SQLITE_CHANGESET_NOTFOUND: 2,
  SQLITE_CHANGESET_CONFLICT: 3,
  SQLITE_CHANGESET_CONSTRAINT: 4,
  SQLITE_CHANGESET_FOREIGN_KEY: 5,

  SQLITE_OK: 0,
  SQLITE_DENY: 1,
  SQLITE_IGNORE: 2,
  SQLITE_CREATE_INDEX: 1,
  SQLITE_CREATE_TABLE: 2,
  SQLITE_CREATE_TEMP_INDEX: 3,
  SQLITE_CREATE_TEMP_TABLE: 4,
  SQLITE_CREATE_TEMP_TRIGGER: 5,
  SQLITE_CREATE_TEMP_VIEW: 6,
  SQLITE_CREATE_TRIGGER: 7,
  SQLITE_CREATE_VIEW: 8,
  SQLITE_DELETE: 9,
  SQLITE_DROP_INDEX: 10,
  SQLITE_DROP_TABLE: 11,
  SQLITE_DROP_TEMP_INDEX: 12,
  SQLITE_DROP_TEMP_TABLE: 13,
  SQLITE_DROP_TEMP_TRIGGER: 14,
  SQLITE_DROP_TEMP_VIEW: 15,
  SQLITE_DROP_TRIGGER: 16,
  SQLITE_DROP_VIEW: 17,
  SQLITE_INSERT: 18,
  SQLITE_PRAGMA: 19,
  SQLITE_READ: 20,
  SQLITE_SELECT: 21,
  SQLITE_TRANSACTION: 22,
  SQLITE_UPDATE: 23,
  SQLITE_ATTACH: 24,
  SQLITE_DETACH: 25,
  SQLITE_ALTER_TABLE: 26,
  SQLITE_REINDEX: 27,
  SQLITE_ANALYZE: 28,
  SQLITE_CREATE_VTABLE: 29,
  SQLITE_DROP_VTABLE: 30,
  SQLITE_FUNCTION: 31,
  SQLITE_SAVEPOINT: 32,
  SQLITE_COPY: 0,
  SQLITE_RECURSIVE: 33,
};

const sqliteTypeSymbol = SymbolFor("sqlite-type");
const limitsCache = new SafeWeakMap();

ObjectDefineProperties(DatabaseSync.prototype, {
  [sqliteTypeSymbol]: {
    __proto__: null,
    value: "node:sqlite",
    enumerable: false,
    configurable: true,
  },
  [SymbolDispose]: {
    __proto__: null,
    value: function () {
      try {
        this.close();
      } catch {
        // Ignore errors.
      }
    },
    enumerable: true,
    configurable: true,
    writable: true,
  },
  limits: {
    __proto__: null,
    get() {
      let cached = limitsCache.get(this);
      if (cached === undefined) {
        const nativeLimits = FunctionPrototypeCall(nativeLimitsGetter, this);
        cached = createLimitsProxy(nativeLimits);
        limitsCache.set(this, cached);
      }
      return cached;
    },
    enumerable: true,
    configurable: true,
  },
});

ObjectDefineProperties(Session.prototype, {
  [SymbolDispose]: {
    __proto__: null,
    value: function () {
      try {
        this.close();
      } catch {
        // Ignore errors.
      }
    },
    enumerable: true,
    configurable: true,
    writable: true,
  },
});

return {
  backup,
  constants,
  DatabaseSync,
  StatementSync,
};
})();
