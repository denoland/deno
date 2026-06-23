// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_lock_manager_await_lock,
  op_lock_manager_cancel,
  op_lock_manager_query,
  op_lock_manager_release,
  op_lock_manager_request,
} from "ext:core/ops";

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

const { Symbol, StringPrototypeStartsWith, TypeError } = primordials;

const _name = Symbol("[[name]]");
const _mode = Symbol("[[mode]]");

class LockManager {
  [webidl.brand] = webidl.brand;

  constructor() {
    webidl.illegalConstructor();
  }

  async request(name, optionsOrCallback, callback = undefined) {
    webidl.assertBranded(this, LockManagerPrototype);

    const prefix = "Failed to execute 'request'";
    webidl.requiredArguments(arguments.length, 2, prefix);

    let options;
    if (arguments.length === 2) {
      options = {};
      callback = optionsOrCallback;
    } else {
      options = optionsOrCallback;
    }

    if (typeof callback !== "function") {
      throw new TypeError("callback must be a function");
    }

    options = webidl.converters.LockOptions(
      options,
      prefix,
      "Argument 2",
    );

    if (StringPrototypeStartsWith(name, "-")) {
      throw new DOMException(
        "'name' must not start with '-'",
        "NotSupportedError",
      );
    }
    if (options.steal && options.ifAvailable) {
      throw new DOMException(
        "'steal' and 'ifAvailable' are exclusive",
        "NotSupportedError",
      );
    }
    if (options.steal && options.mode !== "exclusive") {
      throw new DOMException(
        "'mode' must be 'exclusive' if 'steal' is specified",
        "NotSupportedError",
      );
    }
    if (options.signal && (options.steal || options.ifAvailable)) {
      throw new DOMException(
        "'signal' cannot be provided with 'steal' or 'ifAvailable'",
        "NotSupportedError",
      );
    }

    if (options.signal) {
      options.signal.throwIfAborted();
    }

    const { status, rid } = op_lock_manager_request(
      name,
      options.mode,
      options.ifAvailable,
      options.steal,
    );

    // status 2 = ifAvailable but lock not grantable
    if (status === 2) {
      return await callback(null);
    }

    let heldRid;
    if (status === 0) {
      // Granted immediately
      heldRid = rid;
    } else {
      // Pending (status === 1) - wait for the lock to be granted
      if (options.signal) {
        const onAbort = () => op_lock_manager_cancel(rid);
        options.signal.addEventListener("abort", onAbort, { once: true });
        try {
          heldRid = await op_lock_manager_await_lock(rid);
        } finally {
          options.signal.removeEventListener("abort", onAbort);
        }
        if (heldRid == null) {
          throw options.signal.reason ??
            new DOMException(
              "The lock request was aborted",
              "AbortError",
            );
        }
      } else {
        heldRid = await op_lock_manager_await_lock(rid);
        if (heldRid == null) {
          throw new DOMException(
            "The lock request was aborted",
            "AbortError",
          );
        }
      }
    }

    try {
      const lock = webidl.createBranded(Lock);
      lock[_name] = name;
      lock[_mode] = options.mode;
      return await callback(lock);
    } finally {
      op_lock_manager_release(heldRid);
    }
  }

  query() {
    webidl.assertBranded(this, LockManagerPrototype);
    const { held, pending } = op_lock_manager_query();
    return { held, pending };
  }
}

webidl.configureInterface(LockManager);
const LockManagerPrototype = LockManager.prototype;

class Lock {
  [_name];
  [_mode];
  [webidl.brand] = webidl.brand;

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

webidl.configureInterface(Lock);
const LockPrototype = Lock.prototype;

webidl.converters.LockMode = webidl.createEnumConverter("LockMode", [
  "shared",
  "exclusive",
]);

webidl.converters.LockOptions = webidl
  .createDictionaryConverter(
    "LockOptions",
    [
      {
        key: "mode",
        converter: webidl.converters["LockMode"],
        defaultValue: "exclusive",
      },
      {
        key: "ifAvailable",
        converter: webidl.converters["boolean"],
        defaultValue: false,
      },
      {
        key: "steal",
        converter: webidl.converters["boolean"],
        defaultValue: false,
      },
      {
        key: "signal",
        converter: webidl.converters["AbortSignal"],
      },
    ],
  );

const locks = webidl.createBranded(LockManager);

export { Lock, LockManager, locks };
