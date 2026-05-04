// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

// `debugImpls` and `testEnabled` are initialized with safe defaults so that
// calls to `debuglog()` before `initializeDebugEnv()` do not crash. This can
// happen when internal stream code triggers debug logging during bootstrap
// before the Node process is fully initialized (e.g. when stdin is unavailable
// in compiled binaries run as Windows services or detached processes).
// deno-fmt-ignore-file
(function () {
  let debugImpls: Record<string, (...args: unknown[]) => void> = Object.create(
    null,
  );
  let testEnabled: (str: string) => boolean = () => false;

  // `debugEnv` is initial value of process.env.NODE_DEBUG
  function initializeDebugEnv(debugEnv: string) {
    debugImpls = Object.create(null);
    if (debugEnv) {
      // This is run before any user code, it's OK not to use primordials.
      debugEnv = debugEnv.replace(/[|\\{}()[\]^$+?.]/g, "\\$&")
        .replaceAll("*", ".*")
        .replaceAll(",", "$|^");
      const debugEnvRegex = new RegExp(`^${debugEnv}$`, "i");
      testEnabled = (str) => debugEnvRegex.exec(str) !== null;
    } else {
      testEnabled = () => false;
    }
  }

  // Emits warning when user sets
  // NODE_DEBUG=http or NODE_DEBUG=http2.
  function emitWarningIfNeeded(set: string) {
    if ("HTTP" === set || "HTTP2" === set) {
      // deno-lint-ignore no-console
      console.warn(
        "Setting the NODE_DEBUG environment variable " +
          "to '" + set.toLowerCase() + "' can expose sensitive " +
          "data (such as passwords, tokens and authentication headers) " +
          "in the resulting log.",
      );
    }
  }

  const noop = () => {};

  function debuglogImpl(
    enabled: boolean,
    set: string,
  ): (...args: unknown[]) => void {
    if (debugImpls[set] === undefined) {
      if (enabled) {
        emitWarningIfNeeded(set);
        debugImpls[set] = function debug(msg, ...args: unknown[]) {
          // deno-lint-ignore no-console
          console.error("%s %s: " + msg, set, String(Deno.pid), ...args);
        };
      } else {
        debugImpls[set] = noop;
      }
    }

    return debugImpls[set];
  }

  // debuglogImpl depends on process.pid and process.env.NODE_DEBUG,
  // so it needs to be called lazily in top scopes of internal modules
  // that may be loaded before these run time states are allowed to
  // be accessed.
  function debuglog(
    set: string,
    cb?: (debug: (...args: unknown[]) => void) => void,
  ) {
    function init() {
      set = set.toUpperCase();
      enabled = testEnabled(set);
    }

    let debug = (...args: unknown[]): void => {
      init();
      // Only invokes debuglogImpl() when the debug function is
      // called for the first time.
      debug = debuglogImpl(enabled, set);

      if (typeof cb === "function") {
        cb(debug);
      }

      return debug(...args);
    };

    let enabled: boolean;
    let test = () => {
      init();
      test = () => enabled;
      return enabled;
    };

    const logger = (...args: unknown[]) => debug(...args);

    Object.defineProperty(logger, "enabled", {
      get() {
        return test();
      },
      configurable: true,
      enumerable: true,
    });

    return logger;
  }

  // One second in milliseconds.
  const kSecond = 1000;
  const kMinute = 60 * kSecond;
  const kHour = 60 * kMinute;

  function pad(value: number | string): string {
    return `${value}`.padStart(2, "0");
  }

  function formatTime(ms: number): string {
    let hours = 0;
    let minutes = 0;
    let seconds = 0;

    if (ms >= kSecond) {
      if (ms >= kMinute) {
        if (ms >= kHour) {
          hours = Math.floor(ms / kHour);
          ms = ms % kHour;
        }
        minutes = Math.floor(ms / kMinute);
        ms = ms % kMinute;
      }
      seconds = ms / kSecond;
    }

    if (hours !== 0 || minutes !== 0) {
      const fixed = seconds.toFixed(3).split(".");
      const secondsStr = fixed[0];
      const msStr = fixed[1];
      const res = hours !== 0 ? `${hours}:${pad(minutes)}` : minutes;
      return `${res}:${pad(secondsStr)}.${msStr} (${
        hours !== 0 ? "h:m" : ""
      }m:ss.mmm)`;
    }

    if (seconds !== 0) {
      return `${seconds.toFixed(3)}s`;
    }

    return `${Number(ms.toFixed(3))}ms`;
  }

  const __default_export__ = { debuglog, formatTime };

  return {
    initializeDebugEnv,
    debuglog,
    formatTime,
    default: __default_export__,
  };
})()
