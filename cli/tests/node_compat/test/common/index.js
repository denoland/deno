// deno-fmt-ignore-file
// deno-lint-ignore-file

// Copyright Joyent and Node contributors. All rights reserved. MIT license.

/**
 * This file is meant as a replacement for the original common/index.js
 *
 * That file has a lot of node functionality not currently supported, so this is a lite
 * version of that file, which most tests should be able to use
 */
'use strict';
const assert = require("assert");
const path = require("path");
const util = require("util");
const tmpdir = require("./tmpdir");

function platformTimeout(ms) {
  return ms;
}

let localhostIPv4 = null;

let knownGlobals = [
  AbortSignal,
  addEventListener,
  alert,
  atob,
  btoa,
  Buffer,
  caches,
  clearImmediate,
  close,
  closed,
  confirm,
  console,
  crypto,
  Deno,
  dispatchEvent,
  EventSource,
  fetch,
  getParent,
  global,
  global.clearInterval,
  global.clearTimeout,
  global.setInterval,
  global.setTimeout,
  localStorage,
  location,
  name,
  navigator,
  onload,
  onunload,
  process,
  prompt,
  queueMicrotask,
  removeEventListener,
  reportError,
  self,
  sessionStorage,
  setImmediate,
  window,
];

if (global.AbortController)
  knownGlobals.push(global.AbortController);

if (global.gc) {
  knownGlobals.push(global.gc);
}

if (global.performance) {
  knownGlobals.push(global.performance);
}
if (global.PerformanceMark) {
  knownGlobals.push(global.PerformanceMark);
}
if (global.PerformanceMeasure) {
  knownGlobals.push(global.PerformanceMeasure);
}

if (global.structuredClone) {
  knownGlobals.push(global.structuredClone);
}

function allowGlobals(...allowlist) {
  knownGlobals = knownGlobals.concat(allowlist);
}

if (process.env.NODE_TEST_KNOWN_GLOBALS !== '0') {
  if (process.env.NODE_TEST_KNOWN_GLOBALS) {
    const knownFromEnv = process.env.NODE_TEST_KNOWN_GLOBALS.split(',');
    allowGlobals(...knownFromEnv);
  }

  function leakedGlobals() {
    const leaked = [];

    for (const val in global) {
      if (!knownGlobals.includes(global[val])) {
        leaked.push(val);
      }
    }

    return leaked;
  }

  process.on('exit', function() {
    const leaked = leakedGlobals();
    if (leaked.length > 0) {
      assert.fail(`Unexpected global(s) found: ${leaked.join(', ')}`);
    }
  });
}

function _expectWarning(name, expected, code) {
  if (typeof expected === 'string') {
    expected = [[expected, code]];
  } else if (!Array.isArray(expected)) {
    expected = Object.entries(expected).map(([a, b]) => [b, a]);
  } else if (!(Array.isArray(expected[0]))) {
    expected = [[expected[0], expected[1]]];
  }
  // Deprecation codes are mandatory, everything else is not.
  if (name === 'DeprecationWarning') {
    expected.forEach(([_, code]) => assert(code, expected));
  }
  return mustCall((warning) => {
    const [ message, code ] = expected.shift();
    assert.strictEqual(warning.name, name);
    if (typeof message === 'string') {
      assert.strictEqual(warning.message, message);
    } else {
      assert.match(warning.message, message);
    }
    assert.strictEqual(warning.code, code);
  }, expected.length);
}

let catchWarning;

// Accepts a warning name and description or array of descriptions or a map of
// warning names to description(s) ensures a warning is generated for each
// name/description pair.
// The expected messages have to be unique per `expectWarning()` call.
function expectWarning(nameOrMap, expected, code) {
  if (catchWarning === undefined) {
    catchWarning = {};
    process.on('warning', (warning) => {
      if (!catchWarning[warning.name]) {
        throw new TypeError(
          `"${warning.name}" was triggered without being expected.\n` +
          util.inspect(warning)
        );
      }
      catchWarning[warning.name](warning);
    });
  }
  if (typeof nameOrMap === 'string') {
    catchWarning[nameOrMap] = _expectWarning(nameOrMap, expected, code);
  } else {
    Object.keys(nameOrMap).forEach((name) => {
      catchWarning[name] = _expectWarning(name, nameOrMap[name]);
    });
  }
}

/**
 * Useful for testing expected internal/error objects
 *
 * @param {Error} error
 */
function expectsError(validator, exact) {
  /**
   * @param {Error} error
   */
  return mustCall((...args) => {
    if (args.length !== 1) {
      // Do not use `assert.strictEqual()` to prevent `inspect` from
      // always being called.
      assert.fail(`Expected one argument, got ${util.inspect(args)}`);
    }
    const error = args.pop();
    const descriptor = Object.getOwnPropertyDescriptor(error, 'message');
    // The error message should be non-enumerable
    assert.strictEqual(descriptor.enumerable, false);

    assert.throws(() => { throw error; }, validator);
    return true;
  }, exact);
}

const noop = () => {};

/**
 * @param {Function} fn
 * @param {number} exact
 */
function mustCall(fn, exact) {
  return _mustCallInner(fn, exact, "exact");
}

function mustCallAtLeast(fn, minimum) {
  return _mustCallInner(fn, minimum, 'minimum');
}

function mustSucceed(fn, exact) {
  return mustCall(function(err, ...args) {
    assert.ifError(err);
    if (typeof fn === 'function')
      return fn.apply(this, args);
  }, exact);
}

const mustCallChecks = [];
/**
 * @param {number} exitCode
 */
function runCallChecks(exitCode) {
  if (exitCode !== 0) return;

  const failed = mustCallChecks.filter(function (context) {
    if ("minimum" in context) {
      context.messageSegment = `at least ${context.minimum}`;
      return context.actual < context.minimum;
    }
    context.messageSegment = `exactly ${context.exact}`;
    return context.actual !== context.exact;
  });

  failed.forEach(function (context) {
    console.log(
      "Mismatched %s function calls. Expected %s, actual %d.",
      context.name,
      context.messageSegment,
      context.actual,
    );
    console.log(context.stack.split("\n").slice(2).join("\n"));
  });

  if (failed.length) process.exit(1);
}

/**
 * @param {Function} fn
 * @param {"exact" | "minimum"} field
 */
function _mustCallInner(fn, criteria = 1, field) {
  // @ts-ignore
  if (process._exiting) {
    throw new Error("Cannot use common.mustCall*() in process exit handler");
  }
  if (typeof fn === "number") {
    criteria = fn;
    fn = noop;
  } else if (fn === undefined) {
    fn = noop;
  }

  if (typeof criteria !== "number") {
    throw new TypeError(`Invalid ${field} value: ${criteria}`);
  }

  let context;
  if (field === "exact") {
    context = {
      exact: criteria,
      actual: 0,
      stack: util.inspect(new Error()),
      name: fn.name || "<anonymous>",
    };
  } else {
    context = {
      minimum: criteria,
      actual: 0,
      stack: util.inspect(new Error()),
      name: fn.name || "<anonymous>",
    };
  }

  // Add the exit listener only once to avoid listener leak warnings
  if (mustCallChecks.length === 0) process.on("exit", runCallChecks);

  mustCallChecks.push(context);

  return function () {
    context.actual++;
    return fn.apply(this, arguments);
  };
}

/**
 * @param {string=} msg
 */
function mustNotCall(msg) {
  /**
   * @param {any[]} args
   */
  return function mustNotCall(...args) {
    const argsInfo = args.length > 0
      ? `\ncalled with arguments: ${args.map(util.inspect).join(", ")}`
      : "";
    assert.fail(
      `${msg || "function should not have been called"} at unknown` +
        argsInfo,
    );
  };
}

const _mustNotMutateObjectDeepProxies = new WeakMap();

function mustNotMutateObjectDeep(original) {
  // Return primitives and functions directly. Primitives are immutable, and
  // proxied functions are impossible to compare against originals, e.g. with
  // `assert.deepEqual()`.
  if (original === null || typeof original !== 'object') {
    return original;
  }

  const cachedProxy = _mustNotMutateObjectDeepProxies.get(original);
  if (cachedProxy) {
    return cachedProxy;
  }

  const _mustNotMutateObjectDeepHandler = {
    __proto__: null,
    defineProperty(target, property, descriptor) {
      assert.fail(`Expected no side effects, got ${inspect(property)} ` +
                  'defined');
    },
    deleteProperty(target, property) {
      assert.fail(`Expected no side effects, got ${inspect(property)} ` +
                  'deleted');
    },
    get(target, prop, receiver) {
      return mustNotMutateObjectDeep(Reflect.get(target, prop, receiver));
    },
    preventExtensions(target) {
      assert.fail('Expected no side effects, got extensions prevented on ' +
                  inspect(target));
    },
    set(target, property, value, receiver) {
      assert.fail(`Expected no side effects, got ${inspect(value)} ` +
                  `assigned to ${inspect(property)}`);
    },
    setPrototypeOf(target, prototype) {
      assert.fail(`Expected no side effects, got set prototype to ${prototype}`);
    }
  };

  const proxy = new Proxy(original, _mustNotMutateObjectDeepHandler);
  _mustNotMutateObjectDeepProxies.set(original, proxy);
  return proxy;
}

// A helper function to simplify checking for ERR_INVALID_ARG_TYPE output.
function invalidArgTypeHelper(input) {
  if (input == null) {
    return ` Received ${input}`;
  }
  if (typeof input === "function" && input.name) {
    return ` Received function ${input.name}`;
  }
  if (typeof input === "object") {
    if (input.constructor && input.constructor.name) {
      return ` Received an instance of ${input.constructor.name}`;
    }
    return ` Received ${util.inspect(input, { depth: -1 })}`;
  }
  let inspected = util.inspect(input, { colors: false });
  if (inspected.length > 25) {
    inspected = `${inspected.slice(0, 25)}...`;
  }
  return ` Received type ${typeof input} (${inspected})`;
}

const isWindows = process.platform === 'win32';
const isAIX = process.platform === 'aix';
const isSunOS = process.platform === 'sunos';
const isFreeBSD = process.platform === 'freebsd';
const isOpenBSD = process.platform === 'openbsd';
const isLinux = process.platform === 'linux';
const isOSX = process.platform === 'darwin';

const isDumbTerminal = process.env.TERM === 'dumb';

function skipIfDumbTerminal() {
  if (isDumbTerminal) {
    skip('skipping - dumb terminal');
  }
}

function printSkipMessage(msg) {
  console.log(`1..0 # Skipped: ${msg}`);
}

function skip(msg) {
  printSkipMessage(msg);
  process.exit(0);
}

const PIPE = (() => {
  const localRelative = path.relative(process.cwd(), `${tmpdir.path}/`);
  const pipePrefix = isWindows ? "\\\\.\\pipe\\" : localRelative;
  const pipeName = `node-test.${process.pid}.sock`;
  return path.join(pipePrefix, pipeName);
})();

function getArrayBufferViews(buf) {
  const { buffer, byteOffset, byteLength } = buf;

  const out = [];

  const arrayBufferViews = [
    Int8Array,
    Uint8Array,
    Uint8ClampedArray,
    Int16Array,
    Uint16Array,
    Int32Array,
    Uint32Array,
    Float32Array,
    Float64Array,
    DataView,
  ];

  for (const type of arrayBufferViews) {
    const { BYTES_PER_ELEMENT = 1 } = type;
    if (byteLength % BYTES_PER_ELEMENT === 0) {
      out.push(new type(buffer, byteOffset, byteLength / BYTES_PER_ELEMENT));
    }
  }
  return out;
}

function getBufferSources(buf) {
  return [...getArrayBufferViews(buf), new Uint8Array(buf).buffer];
}

const pwdCommand = isWindows ?
  ['cmd.exe', ['/d', '/c', 'cd']] :
  ['pwd', []];

module.exports = {
  allowGlobals,
  expectsError,
  expectWarning,
  getArrayBufferViews,
  getBufferSources,
  hasCrypto: true,
  hasIntl: true,
  hasMultiLocalhost() {
    return false;
  },
  invalidArgTypeHelper,
  mustCall,
  mustCallAtLeast,
  mustNotCall,
  mustNotMutateObjectDeep,
  mustSucceed,
  PIPE,
  platformTimeout,
  printSkipMessage,
  pwdCommand,
  skipIfDumbTerminal,
  isDumbTerminal,
  isWindows,
  isAIX,
  isSunOS,
  isFreeBSD,
  isOpenBSD,
  isLinux,
  isOSX,
  isMainThread: true, // TODO(f3n67u): replace with `worker_thread.isMainThread` when `worker_thread` implemented
  skip,
  get hasIPv6() {
    const iFaces = require('os').networkInterfaces();
    const re = isWindows ? /Loopback Pseudo-Interface/ : /lo/;
    return Object.keys(iFaces).some((name) => {
      return re.test(name) &&
             iFaces[name].some(({ family }) => family === 'IPv6');
    });
  },

  get localhostIPv4() {
    if (localhostIPv4 !== null) return localhostIPv4;
    if (localhostIPv4 === null) localhostIPv4 = '127.0.0.1';

    return localhostIPv4;
  },

  get PORT() {
    return 12346;
  },
};
