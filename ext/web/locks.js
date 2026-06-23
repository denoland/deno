// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_lock_manager_await_lock,
  op_lock_manager_await_steal,
  op_lock_manager_cancel,
  op_lock_manager_query,
  op_lock_manager_release,
  op_lock_manager_request,
} from "ext:core/ops";

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { DOMException } = core.loadExtScript("ext:deno_web/01_dom_exception.js");

const {
  PromisePrototypeThen,
  SafePromiseRace,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  Symbol,
  TypeError,
} = primordials;

// Lock names are DOMStrings that may contain lone surrogates, but the op
// boundary converts strings to UTF-8, which replaces lone surrogates with
// U+FFFD. That would make otherwise-distinct names (e.g. "\uD800" and
// "\uFFFD") collide as keys in the lock manager. To round-trip names
// losslessly, lone
// surrogates and the escape marker itself are escaped into the private-use area
// before crossing into Rust, and decoded again when read back via query().
const LOCK_NAME_ESCAPE = 0xE000;
const LOCK_NAME_ESCAPE_CHAR = StringFromCharCode(LOCK_NAME_ESCAPE);

function encodeLockName(name) {
  let result = null;
  for (let i = 0; i < name.length; i++) {
    const c = StringPrototypeCharCodeAt(name, i);
    let replacement = null;
    if (c === LOCK_NAME_ESCAPE) {
      replacement = StringFromCharCode(LOCK_NAME_ESCAPE, LOCK_NAME_ESCAPE);
    } else if (
      c >= 0xD800 && c <= 0xDBFF &&
      StringPrototypeCharCodeAt(name, i + 1) >= 0xDC00 &&
      StringPrototypeCharCodeAt(name, i + 1) <= 0xDFFF
    ) {
      // Valid surrogate pair: survives UTF-8 conversion, so keep it as-is.
      if (result !== null) {
        result += StringFromCharCode(c, StringPrototypeCharCodeAt(name, i + 1));
      }
      i++;
      continue;
    } else if (c >= 0xD800 && c <= 0xDFFF) {
      // Lone surrogate: map into the private-use area [0xE001, 0xE800].
      replacement = StringFromCharCode(LOCK_NAME_ESCAPE, c - 0xD800 + 0xE001);
    }
    if (replacement !== null) {
      if (result === null) result = StringPrototypeSlice(name, 0, i);
      result += replacement;
    } else if (result !== null) {
      result += StringFromCharCode(c);
    }
  }
  return result === null ? name : result;
}

function decodeLockName(name) {
  if (StringPrototypeIndexOf(name, LOCK_NAME_ESCAPE_CHAR) === -1) return name;
  let result = "";
  for (let i = 0; i < name.length; i++) {
    const c = StringPrototypeCharCodeAt(name, i);
    if (c === LOCK_NAME_ESCAPE) {
      const next = StringPrototypeCharCodeAt(name, i + 1);
      result += next === LOCK_NAME_ESCAPE
        ? LOCK_NAME_ESCAPE_CHAR
        : StringFromCharCode(next - 0xE001 + 0xD800);
      i++;
    } else {
      result += StringFromCharCode(c);
    }
  }
  return result;
}

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
      encodeLockName(name),
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
      // Lock requests are processed asynchronously, so a signal aborted
      // synchronously right after request() returns must still abort the
      // request before the callback runs. Yield a microtask, then re-check.
      if (options.signal) {
        await null;
        if (options.signal.aborted) {
          op_lock_manager_release(heldRid);
          throw options.signal.reason ??
            new DOMException(
              "The lock request was aborted",
              "AbortError",
            );
        }
      }
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
      // Run the callback while concurrently watching for the lock being
      // stolen. Awaiting the steal op also keeps the event loop alive for as
      // long as the lock is held.
      const callbackPromise = (async () => await callback(lock))();
      const stolen = await SafePromiseRace([
        op_lock_manager_await_steal(heldRid),
        PromisePrototypeThen(callbackPromise, () => false),
      ]);
      if (stolen) {
        throw new DOMException(
          "The lock was broken",
          "AbortError",
        );
      }
      return await callbackPromise;
    } finally {
      op_lock_manager_release(heldRid);
    }
  }

  query() {
    webidl.assertBranded(this, LockManagerPrototype);
    const { held, pending } = op_lock_manager_query();
    for (let i = 0; i < held.length; i++) {
      held[i].name = decodeLockName(held[i].name);
    }
    for (let i = 0; i < pending.length; i++) {
      pending[i].name = decodeLockName(pending[i].name);
    }
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
