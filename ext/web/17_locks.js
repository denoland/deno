// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  Promise,
  PromiseReject,
  PromiseResolve,
  Symbol,
} = primordials;

import { op_lock_query, op_lock_release, op_lock_request } from "ext:core/ops";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "./01_dom_exception.js";

const _name = Symbol("[[name]]");
const _mode = Symbol("[[mode]]");
const _resourceId = Symbol("[[resourceId]]");

// Generate a unique client ID for this isolate
let clientId = `client-${
  Math.random().toString(36).substr(2, 9)
}-${Date.now()}`;

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

  async request(name, callbackOrOptions, callback) {
    const prefix = "Failed to execute 'request' on 'LockManager'";

    // Wrap validation in try-catch to convert sync errors to promise rejections
    let options;
    webidl.requiredArguments(arguments.length, 1, prefix);

    name = webidl.converters.DOMString(name, prefix, "Argument 1");

    if (callback !== undefined) {
      callback = webidl.converters.Function(callback, prefix, "Argument 3");
      if (callbackOrOptions !== undefined) {
        options = webidl.converters.LockOptions(
          callbackOrOptions,
          prefix,
          "Argument 2",
        );
      } else {
        options = webidl.converters.LockOptions(
          {},
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
      options = webidl.converters.LockOptions(
        {},
        prefix,
        "Argument 2",
      );
    }

    if (name.startsWith("-")) {
      throw new DOMException(
        `Failed to execute 'request' on 'LockManager': Names cannot start with '-'`,
        "NotSupportedError",
      );
    }

    // Validate mutually exclusive options
    if (options.steal && options.ifAvailable) {
      throw new DOMException(
        "A NotSupportedError should be thrown if both 'steal' and 'ifAvailable' are specified.",
        "NotSupportedError",
      );
    }

    if (options.steal && options.signal) {
      throw new DOMException(
        "Request with signal and steal=true should fail",
        "NotSupportedError",
      );
    }

    if (options.ifAvailable && options.signal) {
      throw new DOMException(
        "Request with signal and ifAvailable=true should fail",
        "NotSupportedError",
      );
    }

    if (options.steal && options.mode === "shared") {
      throw new DOMException(
        "Request with mode=shared and steal=true should fail",
        "NotSupportedError",
      );
    }

    // Generate AbortController if signal is provided
    let abortController;
    if (options.signal) {
      if (options.signal.aborted) {
        throw new DOMException(
          "The operation was aborted",
          "AbortError",
        );
      }
      abortController = options.signal;
    }

    let aborted = false;
    let onAbort;

    if (abortController) {
      onAbort = () => {
        aborted = true;
        throw new DOMException(
          "The operation was aborted",
          "AbortError",
        );
      };
      abortController.addEventListener("abort", onAbort);
    }

    try {
      const resourceId = await op_lock_request(
        name,
        options.mode,
        clientId,
        options.ifAvailable,
        options.steal,
      );

      if (aborted) return;

      if (resourceId === null && options.ifAvailable) {
        return null;
      }

      if (resourceId === null) {
        throw new DOMException(
          "Lock request failed",
          "NotSupportedError",
        );
      }

      // Create the lock object
      const lock = webidl.createBranded(Lock);
      lock[_name] = name;
      lock[_mode] = options.mode;
      lock[_resourceId] = resourceId;

      try {
        const result = await callback(lock);
        if (!aborted) {
          return result;
        }
      } catch (error) {
        if (!aborted) {
          throw error;
        }
      } finally {
        // Always release the lock
        if (resourceId !== null && resourceId !== undefined) {
          try {
            op_lock_release(resourceId);
          } catch {
            // Ignore cleanup errors
          }
        }
      }
    } catch (error) {
      if (!aborted) {
        throw error;
      }
    } finally {
      if (onAbort && abortController) {
        abortController.removeEventListener("abort", onAbort);
      }
    }
  }

  query() {
    webidl.assertBranded(this, LockManagerPrototype);
    return PromiseResolve(op_lock_query());
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
