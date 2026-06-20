// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// `debugImpls` and `testEnabled` are initialized with safe defaults so that
// calls to `debuglog()` before `initializeDebugEnv()` do not crash. This can
// happen when internal stream code triggers debug logging during bootstrap
// before the Node process is fully initialized (e.g. when stdin is unavailable
// in compiled binaries run as Windows services or detached processes).
(function () {
const { primordials } = __bootstrap;
const {
  MathFloor,
  Number,
  NumberPrototypeToFixed,
  ObjectCreate,
  ObjectDefineProperty,
  ReflectApply,
  RegExpPrototypeExec,
  SafeArrayIterator,
  SafeRegExp,
  String,
  StringPrototypePadStart,
  StringPrototypeReplace,
  StringPrototypeReplaceAll,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
  StringPrototypeToUpperCase,
} = primordials;

let debugImpls: Record<string, (...args: unknown[]) => void> = ObjectCreate(
  null,
);
let testEnabled: (str: string) => boolean = () => false;

// `debugEnv` is initial value of process.env.NODE_DEBUG
function initializeDebugEnv(debugEnv: string) {
  debugImpls = ObjectCreate(null);
  if (debugEnv) {
    debugEnv = StringPrototypeReplaceAll(
      StringPrototypeReplaceAll(
        StringPrototypeReplace(
          debugEnv,
          new SafeRegExp(/[|\\{}()[\]^$+?.]/g),
          "\\$&",
        ),
        "*",
        ".*",
      ),
      ",",
      "$|^",
    );
    const debugEnvRegex = new SafeRegExp(`^${debugEnv}$`, "i");
    testEnabled = (str) => RegExpPrototypeExec(debugEnvRegex, str) !== null;
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
        "to '" + StringPrototypeToLowerCase(set) + "' can expose sensitive " +
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
        console.error(
          "%s %s: " + msg,
          set,
          String(Deno.pid),
          ...new SafeArrayIterator(args),
        );
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
    set = StringPrototypeToUpperCase(set);
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

    return ReflectApply(debug, undefined, args);
  };

  let enabled: boolean;
  let test = () => {
    init();
    test = () => enabled;
    return enabled;
  };

  const logger = (...args: unknown[]) => ReflectApply(debug, undefined, args);

  ObjectDefineProperty(logger, "enabled", {
    __proto__: null,
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
  return StringPrototypePadStart(`${value}`, 2, "0");
}

function formatTime(ms: number): string {
  let hours = 0;
  let minutes = 0;
  let seconds = 0;

  if (ms >= kSecond) {
    if (ms >= kMinute) {
      if (ms >= kHour) {
        hours = MathFloor(ms / kHour);
        ms = ms % kHour;
      }
      minutes = MathFloor(ms / kMinute);
      ms = ms % kMinute;
    }
    seconds = ms / kSecond;
  }

  if (hours !== 0 || minutes !== 0) {
    const fixed = StringPrototypeSplit(
      NumberPrototypeToFixed(seconds, 3),
      ".",
    );
    const secondsStr = fixed[0];
    const msStr = fixed[1];
    const res = hours !== 0 ? `${hours}:${pad(minutes)}` : minutes;
    return `${res}:${pad(secondsStr)}.${msStr} (${
      hours !== 0 ? "h:m" : ""
    }m:ss.mmm)`;
  }

  if (seconds !== 0) {
    return `${NumberPrototypeToFixed(seconds, 3)}s`;
  }

  return `${Number(NumberPrototypeToFixed(ms, 3))}ms`;
}

const _defaultExport = { debuglog, formatTime };

return {
  initializeDebugEnv,
  debuglog,
  formatTime,
  default: _defaultExport,
};
})();
