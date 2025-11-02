// Copyright 2018-2025 the Deno authors. MIT license.

import {
  op_lock_manager_query,
  op_lock_manager_release,
  op_lock_manager_request,
} from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";

const { Symbol, StringPrototypeStartsWith } = primordials;

const _name = Symbol("[[name]]");
const _mode = Symbol("[[mode]]");

export class LockManager {
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

    const rid = await op_lock_manager_request(
      name,
      options.mode,
      options.ifAvailable,
      options.steal,
    );
    if (rid) {
      try {
        const lock = webidl.createBranded(Lock);
        lock[_name] = name;
        lock[_mode] = options.mode;
        const r = await callback(lock);
        return r;
      } finally {
        await op_lock_manager_release(rid);
      }
    } else {
      return callback(null);
    }
  }

  async query() {
    webidl.assertBranded(this, LockManagerPrototype);
    const { held, pending } = await op_lock_manager_query();
    return { held, pending };
  }
}

webidl.configureInterface(LockManager);
const LockManagerPrototype = LockManager.prototype;

export class Lock {
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

export const locks = webidl.createBranded(LockManager);
