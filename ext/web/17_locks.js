// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  PromiseReject,
  Symbol,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "./01_dom_exception.js";

const _name = Symbol("[[name]]");
const _mode = Symbol("[[mode]]");
const _resourceId = Symbol("[[resourceId]]");

webidl.converters.LockMode = webidl.createEnumConverter("LockMode", [
  "exclusive",
  "shared",
]);

webidl.converters.LockOptions = webidl.createDictionaryConverter(
  "LockOptions",
  [
    {
      key: "mode",
      converter: webidl.converters.LockMode,
      defaultValue: "exclusive",
    },
    {
      key: "ifAvailable",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
    {
      key: "steal",
      converter: webidl.converters.boolean,
      defaultValue: false,
    },
    {
      key: "signal",
      converter: webidl.converters.AbortSignal,
    },
  ],
);

class Lock {
  constructor() {
    webidl.illegalConstructor();
  }

  get name() {
    webidl.assertBranded(this, LockPrototype);
    return this[_name];
  }

  get mode() {
    webidl.assertBranded(this, LockPrototype);
    return this[_mode];
  }
}

const LockPrototype = Lock.prototype;

class LockManager {
  constructor() {
    webidl.illegalConstructor();
  }

  request(name, callbackOrOptions, callback) {
    const prefix = "Failed to execute 'request' on 'LockManager'";
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters.DOMString(name, prefix, "Argument 1");

    let options = {};
    if (callback !== undefined) {
      callback = webidl.converters.Function(callback, prefix, "Argument 3");
      if (callbackOrOptions !== undefined) {
        options = webidl.converters.LockOptions(
          callbackOrOptions,
          prefix,
          "Argument 2",
        );
      }
    } else {
      callback = webidl.converters.Function(
        callbackOrOptions,
        prefix,
        "Argument 2",
      );
    }

    if (name.startsWith("-")) {
      return PromiseReject(
        new DOMException(
          `Failed to execute 'request' on 'LockManager': Names cannot start with '-'`,
          "NotSupportedError",
        ),
      );
    }

    throw new Error("Not implemented");
  }

  query() {
    webidl.assertBranded(this, LockManagerPrototype);
    throw new Error("Not implemented");
  }
}

const LockManagerPrototype = LockManager.prototype;

ObjectDefineProperty(Lock.prototype, Symbol.toStringTag, {
  value: "Lock",
  configurable: true,
});

ObjectDefineProperty(LockManager.prototype, Symbol.toStringTag, {
  value: "LockManager",
  configurable: true,
});

const lockManager = webidl.createBranded(LockManager);

export { Lock, LockManager, lockManager };
