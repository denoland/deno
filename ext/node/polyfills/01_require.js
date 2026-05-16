// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_fs_cwd,
  op_import_sync,
  op_napi_open,
  op_require_as_file_path,
  op_require_break_on_next_statement,
  op_require_can_parse_as_esm,
  op_require_init_paths,
  op_require_is_deno_dir_package,
  op_require_is_maybe_cjs,
  op_require_is_request_relative,
  op_require_node_module_paths,
  op_require_package_imports_resolve,
  op_require_path_basename,
  op_require_path_dirname,
  op_require_path_is_absolute,
  op_require_path_resolve,
  op_require_proxy_path,
  op_require_read_file,
  op_require_read_package_scope,
  op_require_real_path,
  op_require_resolve_deno_dir,
  op_require_resolve_exports,
  op_require_resolve_lookup_paths,
  op_require_stat,
  op_require_try_self,
  op_stream_base_register_state,
} from "ext:core/ops";
const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ArrayPrototypeSort,
  ArrayPrototypeSplice,
  Error,
  JSONParse,
  ObjectCreate,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetPrototypeOf,
  ObjectHasOwn,
  ObjectKeys,
  ObjectPrototype,
  ObjectSetPrototypeOf,
  Proxy,
  ReflectSet,
  RegExpPrototypeExec,
  RegExpPrototypeTest,
  SafeArrayIterator,
  SafeMap,
  SafeSet,
  SafeWeakMap,
  SetPrototypeAdd,
  SetPrototypeHas,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeLastIndexOf,
  StringPrototypeMatch,
  StringPrototypeReplace,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  TypeError,
} = primordials;

import _httpAgent from "node:_http_agent";
import _httpCommon from "node:_http_common";
import _httpOutgoing from "node:_http_outgoing";
import _httpServer from "node:_http_server";
const _streamDuplex = core.loadExtScript(
  "ext:deno_node/internal/streams/duplex.js",
).default;
const _streamPassthrough = core.loadExtScript(
  "ext:deno_node/internal/streams/passthrough.js",
).default;
const _streamReadable = core.loadExtScript(
  "ext:deno_node/internal/streams/readable.js",
).default;
const _streamTransform = core.loadExtScript(
  "ext:deno_node/internal/streams/transform.js",
).default;
const _streamWritable = core.loadExtScript(
  "ext:deno_node/internal/streams/writable.js",
).default;
const _tlsCommon = core.loadExtScript(
  "ext:deno_node/_tls_common.ts",
).default;
const _tlsWrap = core.loadExtScript(
  "ext:deno_node/_tls_wrap.js",
).default;
const { default: assert } = core.loadExtScript("ext:deno_node/assert.ts");
import assertStrict from "node:assert/strict";
const asyncHooks = core.loadExtScript("ext:deno_node/async_hooks.ts").default;
const {
  emitAfter: internalAsyncHooksEmitAfter,
  emitBefore: internalAsyncHooksEmitBefore,
  emitDestroy: internalAsyncHooksEmitDestroy,
  emitInit: internalAsyncHooksEmitInit,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");
const buffer = core.loadExtScript("ext:deno_node/internal/buffer.mjs").default;
const childProcess = core.loadExtScript("ext:deno_node/child_process.ts");
const cluster = core.loadExtScript("ext:deno_node/cluster.ts").default;
import console from "node:console";
const constants = core.loadExtScript("ext:deno_node/constants.ts").default;
const crypto = core.loadExtScript("ext:deno_node/crypto.ts").default;
const dgram = core.loadExtScript("ext:deno_node/dgram.ts").default;
const diagnosticsChannel =
  core.loadExtScript("ext:deno_node/diagnostics_channel.js").default;
const dns = core.loadExtScript("ext:deno_node/dns.ts").default;
const dnsPromises = core.loadExtScript(
  "ext:deno_node/dns/promises.ts",
).default;
const domain = core.loadExtScript("ext:deno_node/domain.ts").default;
const events = core.loadExtScript("ext:deno_node/_events.mjs").default;
const fs = core.loadExtScript("ext:deno_node/fs.ts");
const fsPromises = core.loadExtScript(
  "ext:deno_node/fs/promises.ts",
).fsPromises;
const http = core.loadExtScript("ext:deno_node/http.ts");
const http2 = core.loadExtScript("ext:deno_node/http2.ts");
const https = core.loadExtScript("ext:deno_node/https.ts");
const inspector = core.loadExtScript("ext:deno_node/inspector.js");
const inspectorPromises = core.loadExtScript(
  "ext:deno_node/inspector/promises.js",
);
const internalAssertMyersDiff = core.loadExtScript(
  "ext:deno_node/internal/assert/myers_diff.js",
);
const internalCp = core.loadExtScript(
  "ext:deno_node/internal/child_process.ts",
).default;
const internalCryptoCertificate = core.loadExtScript(
  "ext:deno_node/internal/crypto/certificate.ts",
).default;
const internalCryptoCipher = core.loadExtScript(
  "ext:deno_node/internal/crypto/cipher.ts",
).default;
const internalCryptoDiffiehellman = core.loadExtScript(
  "ext:deno_node/internal/crypto/diffiehellman.ts",
).default;
const internalCryptoHash = core.loadExtScript(
  "ext:deno_node/internal/crypto/hash.ts",
).default;
const internalCryptoHkdf = core.loadExtScript(
  "ext:deno_node/internal/crypto/hkdf.ts",
).default;
const internalCryptoKeygen = core.loadExtScript(
  "ext:deno_node/internal/crypto/keygen.ts",
).default;
const internalCryptoKeys = core.loadExtScript(
  "ext:deno_node/internal/crypto/keys.ts",
).default;
const internalCryptoPbkdf2 = core.loadExtScript(
  "ext:deno_node/internal/crypto/pbkdf2.ts",
).default;
const internalCryptoRandom = core.loadExtScript(
  "ext:deno_node/internal/crypto/random.ts",
).default;
const internalCryptoScrypt = core.loadExtScript(
  "ext:deno_node/internal/crypto/scrypt.ts",
).default;
const internalCryptoSig = core.loadExtScript(
  "ext:deno_node/internal/crypto/sig.ts",
).default;
const internalCryptoUtil = core.loadExtScript(
  "ext:deno_node/internal/crypto/util.ts",
).default;
const internalCryptoX509 = core.loadExtScript(
  "ext:deno_node/internal/crypto/x509.ts",
).default;
const internalDgram = core.loadExtScript(
  "ext:deno_node/internal/dgram.ts",
).default;
const internalUndici = core.loadExtScript(
  "ext:deno_node/internal/deps/undici/undici.js",
);
const internalDnsPromises = core.loadExtScript(
  "ext:deno_node/internal/dns/promises.ts",
).default;
const internalBuffer = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const internalErrors = core.loadExtScript("ext:deno_node/internal/errors.ts");
import internalEventTarget from "ext:deno_node/internal/event_target.mjs";
import internalFsUtils from "ext:deno_node/internal/fs/utils.mjs";
// `internal/fs/promises.ts` evaluates `lazyFs()` at top-level, so triggering
// its load during `setupBuiltinModules()` would re-enter the half-built
// `node:fs` namespace. A Proxy defers evaluation until the first time the
// requiring code reads a property; by then `node:fs` is fully initialized.
const lazyInternalFsPromises = core.createLazyLoader(
  "ext:deno_node/internal/fs/promises.ts",
);
let internalFsPromisesCache;
const internalFsPromisesProxy = new Proxy(ObjectCreate(null), {
  get(_target, prop) {
    return (internalFsPromisesCache ??= lazyInternalFsPromises())[prop];
  },
  has(_target, prop) {
    return prop in (internalFsPromisesCache ??= lazyInternalFsPromises());
  },
});
const internalHttp = core.loadExtScript("ext:deno_node/internal/http.ts");
const internalHttp2Core = core.loadExtScript(
  "ext:deno_node/internal/http2/core.ts",
).default;
const internalHttp2Util = core.loadExtScript(
  "ext:deno_node/internal/http2/util.ts",
).default;
const internalPriorityQueue = core.loadExtScript(
  "ext:deno_node/internal/priority_queue.ts",
);
const internalReadlineUtils = core.loadExtScript(
  "ext:deno_node/internal/readline/utils.mjs",
);
const internalStreamsAddAbortSignal = core.loadExtScript(
  "ext:deno_node/internal/streams/add-abort-signal.js",
).default;
import internalStreamsLazyTransform from "ext:deno_node/internal/streams/lazy_transform.js";
const internalStreamsState =
  core.loadExtScript("ext:deno_node/internal/streams/state.js").default;
const internalSocketAddress = core.loadExtScript(
  "ext:deno_node/internal/socketaddress.js",
);
const internalJsStreamSocket = core.loadExtScript(
  "ext:deno_node/internal/js_stream_socket.js",
).default;
const internalTestBinding = core.loadExtScript(
  "ext:deno_node/internal/test/binding.ts",
);
const internalTimers = core.loadExtScript(
  "ext:deno_node/internal/timers.mjs",
);
import * as internalTty from "ext:deno_node/internal/tty.js";
const internalUrl = core.loadExtScript("ext:deno_node/internal/url.ts");
const internalUtil = core.loadExtScript("ext:deno_node/internal/util.mjs");
const internalUtilDebuglog = core.loadExtScript(
  "ext:deno_node/internal/util/debuglog.ts",
);
const internalUtilInspect = core.loadExtScript(
  "ext:deno_node/internal/util/inspect.mjs",
);
const internalValidators = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const internalConsole = core.loadExtScript(
  "ext:deno_node/internal/console/constructor.mjs",
).default;
const net = core.loadExtScript("ext:deno_node/net.ts").default;
const os = core.loadExtScript("ext:deno_node/os.ts").default;
import pathPosix from "node:path/posix";
import pathWin32 from "node:path/win32";
import path from "node:path";
const perfHooks = core.loadExtScript("ext:deno_node/perf_hooks.js").default;
const punycode = core.loadExtScript("ext:deno_node/punycode.ts").default;
import process from "node:process";
const querystring = core.loadExtScript("ext:deno_node/querystring.js").default;
import readline from "node:readline";
import readlinePromises from "node:readline/promises";
import repl from "node:repl";
import internalRepl from "ext:deno_node/internal/repl.ts";
const sqlite = core.loadExtScript("ext:deno_node/sqlite.ts");
import stream from "node:stream";
const streamConsumers = core.loadExtScript("ext:deno_node/stream/consumers.js");
import streamPromises from "node:stream/promises";
const streamWeb = core.loadExtScript("ext:deno_node/stream/web.js");
const stringDecoder =
  core.loadExtScript("ext:deno_node/string_decoder.ts").default;
const test = core.loadExtScript("ext:deno_node/testing.ts").default;
const timers = core.loadExtScript("ext:deno_node/timers.ts");
const timersPromises = core.loadExtScript(
  "ext:deno_node/timers/promises.ts",
);
import tls from "node:tls";
const traceEvents = core.loadExtScript("ext:deno_node/trace_events.ts").default;
import tty from "node:tty";
const url = core.loadExtScript("ext:deno_node/url.ts");
const utilTypes = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const util = core.loadExtScript("ext:deno_node/util.ts");
const v8 = core.loadExtScript("ext:deno_node/v8.ts");
const vm = core.loadExtScript("ext:deno_node/vm.js").default;
const workerThreads = core.loadExtScript(
  "ext:deno_node/worker_threads.ts",
);
const wasi = core.loadExtScript("ext:deno_node/wasi.ts").default;
const zlib = core.loadExtScript("ext:deno_node/zlib.js");
const internalOptions = core.loadExtScript(
  "ext:deno_node/internal/options.ts",
);
const { getOptionValue } = internalOptions;

const nativeModuleExports = ObjectCreate(null);
const builtinModules = [];

// NOTE(bartlomieju): keep this list in sync with `ext/node/lib.rs`
function setupBuiltinModules() {
  const nodeModules = {
    "_http_agent": _httpAgent,
    "_http_common": _httpCommon,
    "_http_outgoing": _httpOutgoing,
    "_http_server": _httpServer,
    "_stream_duplex": _streamDuplex,
    "_stream_passthrough": _streamPassthrough,
    "_stream_readable": _streamReadable,
    "_stream_transform": _streamTransform,
    "_stream_writable": _streamWritable,
    "_tls_common": _tlsCommon,
    "_tls_wrap": _tlsWrap,
    assert,
    "assert/strict": assertStrict,
    "async_hooks": asyncHooks,
    buffer,
    crypto,
    console,
    constants,
    child_process: childProcess,
    cluster,
    dgram,
    diagnostics_channel: diagnosticsChannel,
    dns,
    "dns/promises": dnsPromises,
    domain,
    events,
    fs,
    "fs/promises": fsPromises,
    http,
    http2,
    https,
    inspector,
    "inspector/promises": inspectorPromises,
    "internal/assert/myers_diff": internalAssertMyersDiff.default,
    "internal/console/constructor": internalConsole,
    "internal/child_process": internalCp,
    "internal/crypto/certificate": internalCryptoCertificate,
    "internal/crypto/cipher": internalCryptoCipher,
    "internal/crypto/diffiehellman": internalCryptoDiffiehellman,
    "internal/crypto/hash": internalCryptoHash,
    "internal/crypto/hkdf": internalCryptoHkdf,
    "internal/crypto/keygen": internalCryptoKeygen,
    "internal/crypto/keys": internalCryptoKeys,
    "internal/crypto/pbkdf2": internalCryptoPbkdf2,
    "internal/crypto/random": internalCryptoRandom,
    "internal/crypto/scrypt": internalCryptoScrypt,
    "internal/crypto/sig": internalCryptoSig,
    "internal/crypto/util": internalCryptoUtil,
    "internal/crypto/x509": internalCryptoX509,
    "internal/dgram": internalDgram,
    "internal/deps/undici/undici": internalUndici.default,
    "internal/dns/promises": internalDnsPromises,
    "internal/buffer": internalBuffer.default,
    "internal/errors": internalErrors,
    "internal/event_target": internalEventTarget,
    "internal/fs/promises": internalFsPromisesProxy,
    "internal/fs/utils": internalFsUtils,
    "internal/http": internalHttp.default,
    "internal/http2/core": internalHttp2Core,
    "internal/http2/util": internalHttp2Util,
    "internal/priority_queue": internalPriorityQueue.default,
    "internal/readline/utils": internalReadlineUtils.default,
    "internal/repl": internalRepl,
    "internal/streams/add-abort-signal": internalStreamsAddAbortSignal,
    "internal/streams/lazy_transform": internalStreamsLazyTransform,
    "internal/streams/state": internalStreamsState,
    "internal/socketaddress": internalSocketAddress,
    "internal/js_stream_socket": internalJsStreamSocket,
    "internal/options": internalOptions,
    "internal/test/binding": internalTestBinding,
    "internal/timers": internalTimers,
    "internal/tty": internalTty,
    "internal/url": internalUrl,
    "internal/util/debuglog": internalUtilDebuglog.default,
    "internal/util/inspect": internalUtilInspect,
    "internal/util": internalUtil,
    "internal/validators": internalValidators,
    net,
    module: Module,
    os,
    "path/posix": pathPosix,
    "path/win32": pathWin32,
    path,
    perf_hooks: perfHooks,
    process,
    get punycode() {
      process.emitWarning(
        "The `punycode` module is deprecated. Please use a userland " +
          "alternative instead.",
        "DeprecationWarning",
        "DEP0040",
      );
      return punycode;
    },
    querystring,
    readline,
    "readline/promises": readlinePromises,
    repl,
    sqlite,
    stream,
    "stream/consumers": streamConsumers,
    "stream/promises": streamPromises,
    "stream/web": streamWeb,
    string_decoder: stringDecoder,
    sys: util,
    test,
    timers,
    "timers/promises": timersPromises,
    tls,
    trace_events: traceEvents,
    tty,
    url,
    util,
    "util/types": utilTypes,
    v8,
    vm,
    wasi,
    worker_threads: workerThreads,
    zlib,
  };
  // Match Node's schemelessBlockList: these modules can only be imported
  // via the `node:` scheme (see lib/internal/bootstrap/realm.js), so they
  // appear in `builtinModules` as `node:<name>` rather than `<name>`.
  const schemelessBlockList = new SafeSet([
    "sqlite",
    "test",
  ]);
  for (const [name, moduleExports] of ObjectEntries(nodeModules)) {
    nativeModuleExports[name] = moduleExports;
    // `internal/*` modules are only exposed under --expose-internals, so
    // they aren't part of the public builtinModules list.
    if (StringPrototypeStartsWith(name, "internal/")) {
      continue;
    }
    if (SetPrototypeHas(schemelessBlockList, name)) {
      ArrayPrototypePush(builtinModules, `node:${name}`);
    } else {
      ArrayPrototypePush(builtinModules, name);
    }
  }
}
setupBuiltinModules();

function pathDirname(filepath) {
  if (filepath == null) {
    throw new Error("Empty filepath.");
  } else if (filepath === "") {
    return ".";
  }
  return op_require_path_dirname(filepath);
}

function pathResolve(...args) {
  return op_require_path_resolve(args);
}

const nativeModulePolyfill = new SafeMap();

const relativeResolveCache = ObjectCreate(null);
let requireDepth = 0;
let statCache = null;
let mainModule = null;
let hasBrokenOnInspectBrk = false;
let hasInspectBrk = false;
// Are we running with --node-modules-dir flag or byonm?
let usesLocalNodeModulesDir = false;
let patched = false;

function stat(filename) {
  if (statCache !== null) {
    const result = statCache.get(filename);
    if (result !== undefined) {
      return result;
    }
  }
  const result = op_require_stat(filename);
  if (statCache !== null && result >= 0) {
    statCache.set(filename, result);
  }

  return result;
}

function updateChildren(parent, child, scan) {
  if (!parent) {
    return;
  }

  const children = parent.children;
  if (children && !(scan && ArrayPrototypeIncludes(children, child))) {
    ArrayPrototypePush(children, child);
  }
}

// Given a path inside a node_modules tree, find the package root by
// locating the last "node_modules" path component and taking the next
// segment (or two for scoped packages). Returns null if no node_modules
// component is found.
function findPackageRootFromNodeModules(filepath) {
  // Find the last occurrence of /node_modules/ or \node_modules\ in the path
  let nmIdx = -1;
  let sep = "/";
  const fwdIdx = StringPrototypeLastIndexOf(filepath, "/node_modules/");
  const bwdIdx = StringPrototypeLastIndexOf(filepath, "\\node_modules\\");
  if (fwdIdx !== -1 && fwdIdx > bwdIdx) {
    nmIdx = fwdIdx;
    sep = "/";
  } else if (bwdIdx !== -1) {
    nmIdx = bwdIdx;
    sep = "\\";
  }
  if (nmIdx === -1) return null;

  const afterNm = nmIdx + sep.length + "node_modules".length + sep.length;
  const rest = StringPrototypeSlice(filepath, afterNm);
  const parts = StringPrototypeSplit(rest, sep);
  if (parts.length === 0 || parts[0] === "") return null;

  if (StringPrototypeStartsWith(parts[0], "@") && parts.length > 1) {
    // Scoped package: @scope/name
    return StringPrototypeSlice(filepath, 0, afterNm) + parts[0] + sep +
      parts[1];
  }
  return StringPrototypeSlice(filepath, 0, afterNm) + parts[0];
}

function tryFile(requestPath, _isMain) {
  const rc = stat(requestPath);
  if (rc !== 0) return;
  return toRealPath(requestPath);
}

function tryPackage(requestPath, exts, isMain, originalPath) {
  const packageJsonPath = pathResolve(
    requestPath,
    "package.json",
  );
  const pkg = op_require_read_package_scope(packageJsonPath)?.main;
  if (!pkg) {
    return tryExtensions(
      pathResolve(requestPath, "index"),
      exts,
      isMain,
    );
  }

  const filename = pathResolve(requestPath, pkg);

  // Find the package root for the path traversal check. For nested
  // package.json files inside node_modules (e.g. pkg/sub/package.json with
  // "main": "../cjs/sub.js"), we allow resolving up to the package root
  // (node_modules/pkg/) rather than restricting to the nested directory.
  const packageRoot = findPackageRootFromNodeModules(requestPath) ??
    requestPath;

  // Ensure the resolved main path doesn't escape the package directory
  // via path traversal (e.g. "main": "../../secret.json")
  if (
    !StringPrototypeStartsWith(filename, packageRoot + "/") &&
    !StringPrototypeStartsWith(filename, packageRoot + "\\") &&
    filename !== packageRoot
  ) {
    const err = new Error(
      `Cannot find module '${filename}'. ` +
        'Please verify that the package.json has a valid "main" entry',
    );
    err.code = "MODULE_NOT_FOUND";
    err.path = pathResolve(requestPath, "package.json");
    err.requestPath = originalPath;
    throw err;
  }
  let actual = tryFile(filename, isMain) ||
    tryExtensions(filename, exts, isMain) ||
    tryExtensions(
      pathResolve(filename, "index"),
      exts,
      isMain,
    );
  if (actual === false) {
    actual = tryExtensions(
      pathResolve(requestPath, "index"),
      exts,
      isMain,
    );
    if (!actual) {
      // eslint-disable-next-line no-restricted-syntax
      const err = new Error(
        `Cannot find module '${filename}'. ` +
          'Please verify that the package.json has a valid "main" entry',
      );
      err.code = "MODULE_NOT_FOUND";
      err.path = pathResolve(
        requestPath,
        "package.json",
      );
      err.requestPath = originalPath;
      throw err;
    } else {
      process.emitWarning(
        `Invalid 'main' field in '${packageJsonPath}' of '${pkg}'. ` +
          "Please either fix that or report it to the module author",
        "DeprecationWarning",
        "DEP0128",
      );
    }
  }
  return actual;
}

const realpathCache = new SafeMap();
function toRealPath(requestPath) {
  const maybeCached = realpathCache.get(requestPath);
  if (maybeCached) {
    return maybeCached;
  }
  const rp = op_require_real_path(requestPath);
  realpathCache.set(requestPath, rp);
  return rp;
}

function tryExtensions(p, exts, isMain) {
  for (let i = 0; i < exts.length; i++) {
    const filename = tryFile(p + exts[i], isMain);

    if (filename) {
      return filename;
    }
  }
  return false;
}

// Find the longest (possibly multi-dot) extension registered in
// Module._extensions
function findLongestRegisteredExtension(filename) {
  const name = op_require_path_basename(filename);
  let currentExtension;
  let index;
  let startIndex = 0;
  while ((index = StringPrototypeIndexOf(name, ".", startIndex)) !== -1) {
    startIndex = index + 1;
    if (index === 0) continue; // Skip dotfiles like .gitignore
    currentExtension = StringPrototypeSlice(name, index);
    if (Module._extensions[currentExtension]) {
      return currentExtension;
    }
  }
  return ".js";
}

function getExportsForCircularRequire(module) {
  if (
    module.exports &&
    // Skip Proxy module.exports so the warning machinery never invokes the
    // user-visible getPrototypeOf / setPrototypeOf traps. Matches the
    // !isProxy(...) guard in Node's lib/internal/modules/cjs/loader.js.
    !core.isProxy(module.exports) &&
    ObjectGetPrototypeOf(module.exports) === ObjectPrototype &&
    // Exclude transpiled ES6 modules / TypeScript code because those may
    // employ unusual patterns for accessing 'module.exports'. That should
    // be okay because ES6 modules have a different approach to circular
    // dependencies anyway.
    !module.exports.__esModule
  ) {
    // This is later unset once the module is done loading.
    ObjectSetPrototypeOf(
      module.exports,
      CircularRequirePrototypeWarningProxy,
    );
  }

  return module.exports;
}

function emitCircularRequireWarning(prop) {
  process.emitWarning(
    `Accessing non-existent property '${String(prop)}' of module exports ` +
      "inside circular dependency",
  );
}

// A Proxy that can be used as the prototype of a module.exports object and
// warns when non-existent properties are accessed.
const CircularRequirePrototypeWarningProxy = new Proxy({}, {
  get(target, prop) {
    // Allow __esModule access in any case because it is used in the output
    // of transpiled code to determine whether something comes from an
    // ES module, and is not used as a regular key of `module.exports`.
    if (prop in target || prop === "__esModule") return target[prop];
    emitCircularRequireWarning(prop);
    return undefined;
  },

  getOwnPropertyDescriptor(target, prop) {
    if (
      ObjectHasOwn(target, prop) || prop === "__esModule"
    ) {
      return ObjectGetOwnPropertyDescriptor(target, prop);
    }
    emitCircularRequireWarning(prop);
    return undefined;
  },
});

const moduleParentCache = new SafeWeakMap();
function Module(id = "", parent) {
  this.id = id;
  this.path = pathDirname(id);
  // Use ObjectDefineProperty so that user-installed Object.prototype.exports
  // setters/getters are not invoked during module construction. Mirrors
  // setOwnProperty() in Node's lib/internal/util.js.
  ObjectDefineProperty(this, "exports", {
    __proto__: null,
    configurable: true,
    enumerable: true,
    value: {},
    writable: true,
  });
  moduleParentCache.set(this, parent);
  updateChildren(parent, this, false);
  this.filename = null;
  this.loaded = false;
  this.children = [];
}

let parentDeprecationEmitted = false;
function emitParentDeprecation() {
  if (parentDeprecationEmitted) return;
  if (!getOptionValue("--pending-deprecation")) return;
  parentDeprecationEmitted = true;
  process.emitWarning(
    "module.parent is deprecated due to accuracy issues. Please use " +
      "require.main to find program entry point instead.",
    "DeprecationWarning",
    "DEP0144",
  );
}

ObjectDefineProperty(Module.prototype, "parent", {
  __proto__: null,
  configurable: true,
  enumerable: true,
  get() {
    emitParentDeprecation();
    return moduleParentCache.get(this);
  },
  set(value) {
    emitParentDeprecation();
    moduleParentCache.set(this, value);
  },
});

Module.builtinModules = builtinModules;

Module._extensions = ObjectCreate(null);
Module._cache = ObjectCreate(null);
Module._pathCache = ObjectCreate(null);
let modulePaths = [];
Module.globalPaths = modulePaths;

const CHAR_FORWARD_SLASH = 47;
const TRAILING_SLASH_REGEX = /(?:^|\/)\.?\.$/;

// This only applies to requests of a specific form:
// 1. name/.*
// 2. @scope/name/.*
const EXPORTS_PATTERN = /^((?:@[^/\\%]+\/)?[^./\\%][^/\\%]*)(\/.*)?$/;
function resolveExports(
  modulesPath,
  request,
  parentPath,
  usesLocalNodeModulesDir,
) {
  // The implementation's behavior is meant to mirror resolution in ESM.
  const [, name, expansion = ""] =
    StringPrototypeMatch(request, EXPORTS_PATTERN) || [];
  if (!name) {
    return;
  }

  return op_require_resolve_exports(
    usesLocalNodeModulesDir,
    modulesPath,
    request,
    name,
    expansion,
    parentPath ?? "",
  ) ?? false;
}

Module._findPath = function (request, paths, isMain, parentPath) {
  const absoluteRequest = op_require_path_is_absolute(request);
  if (absoluteRequest) {
    paths = [""];
  } else if (!paths || paths.length === 0) {
    return false;
  }

  const cacheKey = request + "\x00" + ArrayPrototypeJoin(paths, "\x00");
  const entry = Module._pathCache[cacheKey];
  if (entry) {
    return entry;
  }

  let exts;
  let trailingSlash = request.length > 0 &&
    StringPrototypeCharCodeAt(request, request.length - 1) ===
      CHAR_FORWARD_SLASH;
  if (!trailingSlash) {
    trailingSlash = RegExpPrototypeTest(TRAILING_SLASH_REGEX, request);
  }

  // For each path
  for (let i = 0; i < paths.length; i++) {
    // Don't search further if path doesn't exist
    const curPath = paths[i];
    if (curPath && stat(curPath) < 1) continue;

    if (!absoluteRequest) {
      const exportsResolved = resolveExports(
        curPath,
        request,
        parentPath,
        usesLocalNodeModulesDir,
      );
      if (exportsResolved) {
        return exportsResolved;
      }
    }

    let basePath;

    if (usesLocalNodeModulesDir) {
      basePath = pathResolve(curPath, request);
    } else {
      const isDenoDirPackage = op_require_is_deno_dir_package(
        curPath,
      );
      const isRelative = op_require_is_request_relative(
        request,
      );
      basePath = (isDenoDirPackage && !isRelative)
        ? pathResolve(curPath, packageSpecifierSubPath(request))
        : pathResolve(curPath, request);
    }
    let filename;

    const rc = stat(basePath);
    if (!trailingSlash) {
      if (rc === 0) { // File.
        filename = toRealPath(basePath);
      }

      if (!filename) {
        // Try it with each of the extensions
        if (exts === undefined) {
          exts = ObjectKeys(Module._extensions);
        }
        filename = tryExtensions(basePath, exts, isMain);
      }
    }

    if (!filename && rc === 1) { // Directory.
      // try it with each of the extensions at "index"
      if (exts === undefined) {
        exts = ObjectKeys(Module._extensions);
      }
      filename = tryPackage(basePath, exts, isMain, request);
    }

    if (filename) {
      Module._pathCache[cacheKey] = filename;
      return filename;
    }
  }

  return false;
};

/**
 * Get a list of potential module directories
 * @param {string} fromPath The directory name of the module
 * @returns {string[]} List of module directories
 */
Module._nodeModulePaths = function (fromPath) {
  return op_require_node_module_paths(fromPath);
};

Module._resolveLookupPaths = function (request, parent) {
  if (typeof request !== "string") {
    throw new internalErrors.ERR_INVALID_ARG_TYPE(
      "request",
      "string",
      request,
    );
  }

  // Return null for built-in modules, matching Node.js behavior.
  // Libraries like requizzle rely on this to detect native modules.
  const normalizedRequest = StringPrototypeStartsWith(request, "node:")
    ? StringPrototypeSlice(request, 5)
    : request;
  if (
    isBuiltin(request) ||
    normalizedRequest in nativeModuleExports
  ) {
    return null;
  }

  const paths = [];

  if (op_require_is_request_relative(request)) {
    ArrayPrototypePush(
      paths,
      parent?.filename ? op_require_path_dirname(parent.filename) : ".",
    );
    return paths;
  }

  if (
    !usesLocalNodeModulesDir && parent?.filename && parent.filename.length > 0
  ) {
    const denoDirPath = op_require_resolve_deno_dir(
      request,
      parent.filename,
    );
    if (denoDirPath) {
      ArrayPrototypePush(paths, denoDirPath);
    }
  }
  const lookupPathsResult = op_require_resolve_lookup_paths(
    request,
    parent?.paths,
    parent?.filename ?? "",
  );
  if (lookupPathsResult) {
    ArrayPrototypePush(paths, ...new SafeArrayIterator(lookupPathsResult));
  }
  return paths;
};

Module._load = function (request, parent, isMain) {
  let relResolveCacheIdentifier;
  if (parent) {
    // Fast path for (lazy loaded) modules in the same directory. The indirect
    // caching is required to allow cache invalidation without changing the old
    // cache key names.
    relResolveCacheIdentifier = `${parent.path}\x00${request}`;
    const filename = relativeResolveCache[relResolveCacheIdentifier];
    if (filename !== undefined) {
      const cachedModule = Module._cache[filename];
      if (cachedModule !== undefined) {
        updateChildren(parent, cachedModule, true);
        if (!cachedModule.loaded) {
          return getExportsForCircularRequire(cachedModule);
        }
        return cachedModule.exports;
      }
      delete relativeResolveCache[relResolveCacheIdentifier];
    }
  }

  const filename = Module._resolveFilename(request, parent, isMain);
  if (StringPrototypeStartsWith(filename, "node:")) {
    // Slice 'node:' prefix
    const id = StringPrototypeSlice(filename, 5);

    maybeEmitNativeModuleDeprecation(id);
    const module = loadNativeModule(id, id);
    if (!module) {
      // TODO:
      // throw new ERR_UNKNOWN_BUILTIN_MODULE(filename);
      throw new Error("Unknown built-in module");
    }

    return module.exports;
  }

  const cachedModule = Module._cache[filename];
  if (cachedModule !== undefined) {
    updateChildren(parent, cachedModule, true);
    if (!cachedModule.loaded) {
      return getExportsForCircularRequire(cachedModule);
    }
    return cachedModule.exports;
  }

  maybeEmitNativeModuleDeprecation(filename);
  const mod = loadNativeModule(filename, request);
  if (
    mod
  ) {
    return mod.exports;
  }
  // Don't call updateChildren(), Module constructor already does.
  const module = cachedModule || new Module(filename, parent);

  if (isMain) {
    process.mainModule = module;
    mainModule = module;
    module.id = ".";
  }

  Module._cache[filename] = module;
  if (parent !== undefined) {
    relativeResolveCache[relResolveCacheIdentifier] = filename;
  }

  let threw = true;
  try {
    try {
      module.load(filename);
      threw = false;
    } finally {
      if (threw) {
        delete Module._cache[filename];
        if (parent !== undefined) {
          delete relativeResolveCache[relResolveCacheIdentifier];
          const children = parent?.children;
          if (ArrayIsArray(children)) {
            const index = ArrayPrototypeIndexOf(children, module);
            if (index !== -1) {
              ArrayPrototypeSplice(children, index, 1);
            }
          }
        }
      } else if (
        module.exports &&
        // Skip Proxy module.exports so the cleanup pass after a circular
        // require doesn't invoke user-visible getPrototypeOf traps. Matches
        // Node's lib/internal/modules/cjs/loader.js behavior.
        !core.isProxy(module.exports) &&
        ObjectGetPrototypeOf(module.exports) ===
          CircularRequirePrototypeWarningProxy
      ) {
        ObjectSetPrototypeOf(module.exports, ObjectPrototype);
      }
    }
  } catch (err) {
    // For a top-level CommonJS throw in the entry module, fire
    // 'uncaughtExceptionMonitor' and 'uncaughtException' synchronously with
    // origin === 'uncaughtException', matching Node.js semantics.
    //
    // Without this, the throw bubbles up to the ESM wrapper that loads the
    // main CJS module, becomes a module evaluation rejection, and is routed
    // through Deno's unhandled-rejection path.
    if (
      isMain &&
      parent === null &&
      typeof process !== "undefined" &&
      typeof process._fatalException === "function"
    ) {
      if (process._fatalException(err)) {
        return module.exports;
      }
      if (err !== null && typeof err === "object") {
        const set = internals._dispatchedFatalErrors;
        if (set !== undefined) set.add(err);
      }
    }
    throw err;
  }

  if (isMain && parent === null) {
    core.processTicksAndRejections();
  }

  return module.exports;
};

Module._resolveFilename = function (
  request,
  parent,
  isMain,
  options,
) {
  if (typeof request !== "string") {
    throw new internalErrors.ERR_INVALID_ARG_TYPE(
      "request",
      "string",
      request,
    );
  }

  if (nativeModuleCanBeRequiredByUsers(request)) {
    return request;
  }

  if (StringPrototypeStartsWith(request, "node:")) {
    const id = StringPrototypeSlice(request, 5);
    if (nativeModuleExports[id]) {
      return request;
    }
    const err = new Error(`Cannot find module '${request}'`);
    err.code = "MODULE_NOT_FOUND";
    throw err;
  }

  let paths;

  if (typeof options === "object" && options !== null) {
    if (ArrayIsArray(options.paths)) {
      // Validate all path entries are strings before using them.
      for (let i = 0; i < options.paths.length; i++) {
        if (typeof options.paths[i] !== "string") {
          throw new internalErrors.ERR_INVALID_ARG_TYPE(
            "options.paths",
            "string",
            options.paths[i],
          );
        }
      }

      const isRelative = op_require_is_request_relative(request);

      if (isRelative) {
        // Resolve relative entries to absolute paths so _findPath can
        // stat them correctly.
        paths = [];
        for (let i = 0; i < options.paths.length; i++) {
          ArrayPrototypePush(
            paths,
            pathResolve(process.cwd(), options.paths[i]),
          );
        }
      } else {
        const fakeParent = new Module("", null);
        paths = [];

        for (let i = 0; i < options.paths.length; i++) {
          const path = options.paths[i];
          fakeParent.paths = Module._nodeModulePaths(path);
          const lookupPaths = Module._resolveLookupPaths(request, fakeParent);

          for (let j = 0; j < lookupPaths.length; j++) {
            if (!ArrayPrototypeIncludes(paths, lookupPaths[j])) {
              ArrayPrototypePush(paths, lookupPaths[j]);
            }
          }
        }
      }
    } else if (options.paths === undefined) {
      paths = Module._resolveLookupPaths(request, parent);
    } else {
      throw new internalErrors.ERR_INVALID_ARG_VALUE(
        "options.paths",
        options.paths,
      );
    }
  } else {
    paths = Module._resolveLookupPaths(request, parent);
  }

  if (parent?.filename) {
    if (request[0] === "#") {
      const maybeResolved = op_require_package_imports_resolve(
        parent.filename,
        request,
      );
      if (maybeResolved) {
        return maybeResolved;
      }
    }
  }

  // Try module self resolution first
  const parentPath = trySelfParentPath(parent);
  const selfResolved = parentPath != null
    ? op_require_try_self(parentPath, request)
    : undefined;
  if (selfResolved) {
    const cacheKey = request + "\x00" +
      (paths.length === 1 ? paths[0] : ArrayPrototypeJoin(paths, "\x00"));
    Module._pathCache[cacheKey] = selfResolved;
    return selfResolved;
  }

  // Look up the filename first, since that's the cache key.
  const filename = Module._findPath(
    request,
    paths,
    isMain,
    parentPath,
  );
  if (filename) {
    return op_require_real_path(filename);
  }
  // fallback and attempt to resolve bare specifiers using
  // the global cache when not using --node-modules-dir
  if (
    !usesLocalNodeModulesDir &&
    ArrayIsArray(options?.paths) &&
    request[0] !== "." &&
    request[0] !== "#" &&
    !request.startsWith("file:///") &&
    !op_require_is_request_relative(request) &&
    !op_require_path_is_absolute(request)
  ) {
    try {
      return Module._resolveFilename(request, parent, isMain, {
        ...options,
        paths: undefined,
      });
    } catch {
      // ignore
    }
  }

  if (
    typeof request === "string" &&
    (StringPrototypeEndsWith(request, "$deno$eval.cjs") ||
      StringPrototypeEndsWith(request, "$deno$eval.cts") ||
      StringPrototypeEndsWith(request, "$deno$stdin.cjs") ||
      StringPrototypeEndsWith(request, "$deno$stdin.cts"))
  ) {
    return request;
  }

  const requireStack = [];
  for (let cursor = parent; cursor; cursor = moduleParentCache.get(cursor)) {
    ArrayPrototypePush(requireStack, cursor.filename || cursor.id);
  }
  let message = `Cannot find module '${request}'`;
  if (requireStack.length > 0) {
    message = message + "\nRequire stack:\n- " +
      ArrayPrototypeJoin(requireStack, "\n- ");
  }
  // eslint-disable-next-line no-restricted-syntax
  const err = new Error(message);
  err.code = "MODULE_NOT_FOUND";
  err.requireStack = requireStack;
  throw err;
};

function trySelfParentPath(parent) {
  if (parent == null) {
    return undefined;
  }
  if (typeof parent.filename === "string") {
    return parent.filename;
  }
  if (parent.id === "<repl>" || parent.id === "internal/preload") {
    return op_fs_cwd();
  }
  return undefined;
}

/**
 * Internal CommonJS API to always require modules before requiring the actual
 * one when calling `require("my-module")`. This is used by require hooks such
 * as `ts-node/register`.
 * @param {string[]} requests List of modules to preload
 */
Module._preloadModules = function (requests) {
  if (!ArrayIsArray(requests) || requests.length === 0) {
    return;
  }

  const parent = new Module("internal/preload", null);
  // All requested files must be resolved against cwd
  parent.paths = Module._nodeModulePaths(process.cwd());
  for (let i = 0; i < requests.length; i++) {
    parent.require(requests[i]);
  }
};

Module.prototype.load = function (filename) {
  if (this.loaded) {
    throw new Error("Module already loaded");
  }

  // Canonicalize the path so it's not pointing to the symlinked directory
  // in `node_modules` directory of the referrer.
  this.filename = op_require_real_path(filename);
  this.paths = Module._nodeModulePaths(pathDirname(this.filename));
  const extension = findLongestRegisteredExtension(filename);
  Module._extensions[extension](this, this.filename);
  this.loaded = true;

  // TODO: do caching
};

// Loads a module at the given file path. Returns that module's
// `exports` property.
Module.prototype.require = function (id) {
  if (typeof id !== "string") {
    throw new internalErrors.ERR_INVALID_ARG_TYPE("id", "string", id);
  }

  if (id === "") {
    throw new internalErrors.ERR_INVALID_ARG_VALUE(
      "id",
      id,
      "must be a non-empty string",
    );
  }
  requireDepth++;
  try {
    return Module._load(id, this, /* isMain */ false);
  } finally {
    requireDepth--;
  }
};

const wrapper = [
  "(function (exports, require, module, __filename, __dirname) { ",
  "\n});",
];

export let wrap = function (script) {
  script = script.replace(/^#!.*?\n/, "");
  return `${Module.wrapper[0]}${script}${Module.wrapper[1]}`;
};

let wrapperProxy = new Proxy(wrapper, {
  set(target, property, value, receiver) {
    patched = true;
    return ReflectSet(target, property, value, receiver);
  },

  defineProperty(target, property, descriptor) {
    patched = true;
    return ObjectDefineProperty(target, property, descriptor);
  },
});

ObjectDefineProperty(Module, "wrap", {
  get() {
    return wrap;
  },

  set(value) {
    patched = true;
    wrap = value;
  },
});

ObjectDefineProperty(Module, "wrapper", {
  get() {
    return wrapperProxy;
  },

  set(value) {
    patched = true;
    wrapperProxy = value;
  },
});

function isEsmSyntaxError(error) {
  return error instanceof SyntaxError && (
    StringPrototypeIncludes(
      error.message,
      "Cannot use import statement outside a module",
    ) ||
    StringPrototypeIncludes(error.message, "Unexpected token 'export'")
  );
}

function enrichCJSError(error) {
  if (isEsmSyntaxError(error)) {
    console.error(
      'To load an ES module, set "type": "module" in the package.json or use ' +
        "the .mjs extension.",
    );
  }
}

function wrapSafe(
  filename,
  content,
  cjsModuleInstance,
  format,
) {
  let f;
  let err;

  if (patched) {
    [f, err] = core.evalContext(
      Module.wrap(content),
      url.pathToFileURL(filename).toString(),
      [format !== "module"],
    );
  } else {
    [f, err] = core.compileFunction(
      content,
      url.pathToFileURL(filename).toString(),
      [format !== "module"],
      [
        "exports",
        "require",
        "module",
        "__filename",
        "__dirname",
      ],
    );
  }
  if (err) {
    if (process.mainModule === cjsModuleInstance) {
      enrichCJSError(err.thrown);
    }
    throw err.thrown;
  }
  return f;
}

Module.prototype._compile = function (content, filename, format) {
  if (format === "module") {
    return loadESMFromCJS(this, filename, content);
  }

  let compiledWrapper;
  try {
    compiledWrapper = wrapSafe(filename, content, this, format);
  } catch (err) {
    if (
      format !== "commonjs" && err instanceof SyntaxError &&
      op_require_can_parse_as_esm(content)
    ) {
      return loadESMFromCJS(this, filename, content);
    }
    throw err;
  }

  const dirname = pathDirname(filename);
  const require = makeRequireFunction(this);
  const exports = this.exports;
  const thisValue = exports;
  if (requireDepth === 0) {
    statCache = new SafeMap();
  }

  if (hasInspectBrk && !hasBrokenOnInspectBrk) {
    hasBrokenOnInspectBrk = true;
    op_require_break_on_next_statement();
  }

  const result = compiledWrapper.call(
    thisValue,
    exports,
    require,
    this,
    filename,
    dirname,
  );
  if (requireDepth === 0) {
    statCache = null;
  }
  return result;
};

Module._extensions[".js"] = function (module, filename) {
  // We don't define everything on Module.extensions in
  // order to prevent probing for these files
  if (
    StringPrototypeEndsWith(filename, ".js") ||
    StringPrototypeEndsWith(filename, ".ts") ||
    StringPrototypeEndsWith(filename, ".jsx") ||
    StringPrototypeEndsWith(filename, ".tsx")
  ) {
    return loadMaybeCjs(module, filename);
  } else if (StringPrototypeEndsWith(filename, ".mts")) {
    return loadESMFromCJS(module, filename);
  } else if (StringPrototypeEndsWith(filename, ".cts")) {
    return loadCjs(module, filename);
  } else {
    return loadMaybeCjs(module, filename);
  }
};

Module._extensions[".cjs"] = loadCjs;
Module._extensions[".mjs"] = loadESMFromCJS;
Module._extensions[".wasm"] = loadESMFromCJS;

function loadMaybeCjs(module, filename) {
  const content = op_require_read_file(filename);
  const format = op_require_is_maybe_cjs(filename) ? undefined : "module";
  module._compile(content, filename, format);
}

function loadCjs(module, filename) {
  const content = op_require_read_file(filename);
  module._compile(content, filename, "commonjs");
}

function _throwRequireAsyncModule(specifier, module) {
  // Use moduleParentCache directly to avoid triggering the module.parent
  // deprecation getter when --pending-deprecation is set.
  const parentModule = module ? moduleParentCache.get(module) : undefined;
  const parent = parentModule?.filename ?? "<unknown>";
  throw new internalErrors.ERR_REQUIRE_ASYNC_MODULE(specifier, parent);
}

function loadESMFromCJS(module, filename, code) {
  const specifier = url.pathToFileURL(filename).toString();
  let namespace;
  try {
    namespace = op_import_sync(specifier, code);
  } catch (e) {
    if (
      e instanceof Error &&
      StringPrototypeIncludes(
        e.message,
        "Top-level await is not allowed in synchronous evaluation",
      )
    ) {
      _throwRequireAsyncModule(specifier, module);
    }
    throw e;
  }
  if (ObjectHasOwn(namespace, "module.exports")) {
    module.exports = namespace["module.exports"];
  } else {
    module.exports = namespace;
  }
}

function stripBOM(content) {
  if (StringPrototypeCharCodeAt(content, 0) === 0xfeff) {
    content = StringPrototypeSlice(content, 1);
  }
  return content;
}

// Native extension for .json
Module._extensions[".json"] = function (module, filename) {
  const content = op_require_read_file(filename);

  try {
    module.exports = JSONParse(stripBOM(content));
  } catch (err) {
    err.message = filename + ": " + err.message;
    throw err;
  }
};

// Async hooks wrappers for NAPI - called from Rust via V8 function calls.
function napiAsyncHooksEmitInit(asyncId, type, triggerAsyncId, resource) {
  internalAsyncHooksEmitInit(asyncId, type, triggerAsyncId, resource);
}
function napiAsyncHooksEmitBefore(asyncId) {
  internalAsyncHooksEmitBefore(asyncId);
}
function napiAsyncHooksEmitAfter(asyncId) {
  internalAsyncHooksEmitAfter(asyncId);
}
function napiAsyncHooksEmitDestroy(asyncId) {
  internalAsyncHooksEmitDestroy(asyncId);
}

// Native extension for .node
Module._extensions[".node"] = function (module, filename) {
  if (filename.endsWith("cpufeatures.node")) {
    throw new Error("Using cpu-features module is currently not supported");
  }
  module.exports = op_napi_open(
    filename,
    globalThis,
    buffer.Buffer.from,
    reportError,
    napiAsyncHooksEmitInit,
    napiAsyncHooksEmitBefore,
    napiAsyncHooksEmitAfter,
    napiAsyncHooksEmitDestroy,
  );
};

function createRequireFromPath(filename) {
  const proxyPath = op_require_proxy_path(filename) ?? filename;
  const mod = new Module(proxyPath);
  mod.filename = proxyPath;
  mod.paths = Module._nodeModulePaths(mod.path);
  return makeRequireFunction(mod);
}

function makeRequireFunction(mod) {
  const require = function require(path) {
    return mod.require(path);
  };

  function resolve(request, options) {
    return Module._resolveFilename(request, mod, false, options);
  }

  require.resolve = resolve;

  function paths(request) {
    return Module._resolveLookupPaths(request, mod);
  }

  resolve.paths = paths;
  // Use ObjectDefineProperty so user-installed Object.prototype.main setters
  // are not invoked when require() is constructed. Mirrors setOwnProperty()
  // in Node's lib/internal/modules/helpers.js.
  ObjectDefineProperty(require, "main", {
    __proto__: null,
    configurable: true,
    enumerable: true,
    value: mainModule,
    writable: true,
  });
  // Enable support to add extra extension types.
  require.extensions = Module._extensions;
  require.cache = Module._cache;

  return require;
}

// Matches to:
// - /foo/...
// - \foo\...
// - C:/foo/...
// - C:\foo\...
const RE_START_OF_ABS_PATH = /^([/\\]|[a-zA-Z]:[/\\])/;

function isAbsolute(filenameOrUrl) {
  return RE_START_OF_ABS_PATH.test(filenameOrUrl);
}

// Match Node's error reason (see lib/internal/modules/cjs/loader.js).
const kCreateRequireError =
  "must be a file URL object, file URL string, or absolute path string";

function createRequire(filenameOrUrl) {
  let fileUrlStr;
  if (filenameOrUrl instanceof URL) {
    if (filenameOrUrl.protocol !== "file:") {
      throw new internalErrors.ERR_INVALID_ARG_VALUE(
        "filename",
        filenameOrUrl,
        kCreateRequireError,
      );
    }
    fileUrlStr = filenameOrUrl.toString();
  } else if (typeof filenameOrUrl === "string") {
    if (!filenameOrUrl.startsWith("file:") && !isAbsolute(filenameOrUrl)) {
      throw new internalErrors.ERR_INVALID_ARG_VALUE(
        "filename",
        filenameOrUrl,
        kCreateRequireError,
      );
    }
    fileUrlStr = filenameOrUrl;
  } else {
    throw new internalErrors.ERR_INVALID_ARG_VALUE(
      "filename",
      filenameOrUrl,
      kCreateRequireError,
    );
  }
  const filename = op_require_as_file_path(fileUrlStr) ?? fileUrlStr;
  return createRequireFromPath(filename);
}

function isBuiltin(moduleName) {
  if (typeof moduleName !== "string") {
    return false;
  }

  if (StringPrototypeStartsWith(moduleName, "node:")) {
    moduleName = StringPrototypeSlice(moduleName, 5);
  } else if (moduleName === "test") {
    // test is only a builtin if it has the "node:" scheme
    // see https://github.com/nodejs/node/blob/73025c4dec042e344eeea7912ed39f7b7c4a3991/test/parallel/test-module-isBuiltin.js#L14
    return false;
  }

  return moduleName in nativeModuleExports &&
    !StringPrototypeStartsWith(moduleName, "internal/");
}

function getBuiltinModule(id) {
  if (typeof id !== "string") {
    throw new internalErrors.ERR_INVALID_ARG_TYPE("id", "string", id);
  }
  if (!isBuiltin(id)) {
    return undefined;
  }

  if (StringPrototypeStartsWith(id, "node:")) {
    // Slice 'node:' prefix
    id = StringPrototypeSlice(id, 5);
  }

  const mod = loadNativeModule(id, id);
  if (mod) {
    return mod.exports;
  }

  return undefined;
}

Module.isBuiltin = isBuiltin;

Module.createRequire = createRequire;

Module._initPaths = function () {
  const paths = op_require_init_paths();
  modulePaths = paths;
  Module.globalPaths = ArrayPrototypeSlice(modulePaths);
};

Module.syncBuiltinESMExports = function syncBuiltinESMExports() {
  throw new Error("not implemented");
};

// Mostly used by tools like ts-node.
Module.runMain = function () {
  Module._load(process.argv[1], null, true);
};

Module.Module = Module;

nativeModuleExports.module = Module;

// Modules that emit a deprecation warning the first time they are required via
// the CJS loader (`require('_stream_readable')` etc.). Maps the module name to
// [message, code]. Matches Node's `BuiltinModule#compileForPublicLoader` --
// `process.getBuiltinModule()` does NOT trigger these warnings.
const deprecatedNativeModules = ObjectCreate(null);
deprecatedNativeModules._tls_common = [
  "The _tls_common module is deprecated. Use `node:tls` instead.",
  "DEP0192",
];
deprecatedNativeModules._tls_wrap = [
  "The _tls_wrap module is deprecated. Use `node:tls` instead.",
  "DEP0192",
];
deprecatedNativeModules._stream_duplex = [
  "The _stream_duplex module is deprecated. Use `node:stream` instead.",
  "DEP0193",
];
deprecatedNativeModules._stream_passthrough = [
  "The _stream_passthrough module is deprecated. Use `node:stream` instead.",
  "DEP0193",
];
deprecatedNativeModules._stream_readable = [
  "The _stream_readable module is deprecated. Use `node:stream` instead.",
  "DEP0193",
];
deprecatedNativeModules._stream_transform = [
  "The _stream_transform module is deprecated. Use `node:stream` instead.",
  "DEP0193",
];
deprecatedNativeModules._stream_writable = [
  "The _stream_writable module is deprecated. Use `node:stream` instead.",
  "DEP0193",
];

const emittedNativeModuleDeprecations = new SafeSet();
function maybeEmitNativeModuleDeprecation(request) {
  const deprecation = deprecatedNativeModules[request];
  if (deprecation === undefined) return;
  if (SetPrototypeHas(emittedNativeModuleDeprecations, request)) return;
  SetPrototypeAdd(emittedNativeModuleDeprecations, request);
  process.emitWarning(
    deprecation[0],
    "DeprecationWarning",
    deprecation[1],
  );
}

function loadNativeModule(_id, request) {
  if (nativeModulePolyfill.has(request)) {
    return nativeModulePolyfill.get(request);
  }
  const modExports = nativeModuleExports[request];
  if (modExports) {
    const nodeMod = new Module(request);
    nodeMod.exports = modExports;
    nodeMod.loaded = true;
    nativeModulePolyfill.set(request, nodeMod);
    return nodeMod;
  }
  return undefined;
}

function nativeModuleCanBeRequiredByUsers(request) {
  return !!nativeModuleExports[request];
}

function readPackageScope() {
  throw new Error("not implemented");
}

/** @param specifier {string} */
function packageSpecifierSubPath(specifier) {
  let parts = StringPrototypeSplit(specifier, "/");
  if (StringPrototypeStartsWith(parts[0], "@")) {
    parts = ArrayPrototypeSlice(parts, 2);
  } else {
    parts = ArrayPrototypeSlice(parts, 1);
  }
  return ArrayPrototypeJoin(parts, "/");
}

// This is a temporary namespace, that will be removed when initializing
// in `02_init.js`.
internals.requireImpl = {
  setUsesLocalNodeModulesDir() {
    usesLocalNodeModulesDir = true;
  },
  setInspectBrk() {
    hasInspectBrk = true;
  },
  Module,
  nativeModuleExports,
};

// VLQ Base64 decoding for source maps
const BASE64_CHARS =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_LOOKUP = new Int32Array(128);
for (let i = 0; i < 128; i++) BASE64_LOOKUP[i] = -1;
for (let i = 0; i < BASE64_CHARS.length; i++) {
  BASE64_LOOKUP[StringPrototypeCharCodeAt(BASE64_CHARS, i)] = i;
}

const VLQ_BASE_SHIFT = 5;
const VLQ_BASE = 1 << VLQ_BASE_SHIFT; // 32
const VLQ_BASE_MASK = VLQ_BASE - 1; // 31
const VLQ_CONTINUATION_BIT = VLQ_BASE; // 32

/**
 * Decode a single VLQ value from a string iterator.
 * @param {string} str
 * @param {{ pos: number }} iter
 * @returns {number}
 */
function decodeVLQ(str, iter) {
  let result = 0;
  let shift = 0;
  let digit;
  do {
    if (iter.pos >= str.length) {
      return 0;
    }
    const charCode = StringPrototypeCharCodeAt(str, iter.pos++);
    digit = BASE64_LOOKUP[charCode];
    if (digit === -1) {
      return 0;
    }
    result += (digit & VLQ_BASE_MASK) << shift;
    shift += VLQ_BASE_SHIFT;
  } while (digit & VLQ_CONTINUATION_BIT);

  // The sign is encoded in the least significant bit
  const negative = result & 1;
  // Use unsigned right shift, so that the 32nd bit is properly shifted
  // to the 31st, and the 32nd becomes unset.
  result >>>= 1;
  if (!negative) {
    return result;
  }
  // We need to OR here to ensure the 32nd bit (the sign bit in an Int32) is
  // always set for negative numbers. If `result` were 1, (meaning `negative`
  // is true and all other bits were zeros), `result` would now be 0. But -0
  // doesn't flip the 32nd bit as intended.
  return -result | (1 << 31);
}

/**
 * Parse VLQ-encoded source map mappings string into an array of mapping entries.
 * @param {string} mappings
 * @param {string[]} sources
 * @param {string} sourceRoot
 * @returns {Array<{generatedLine: number, generatedColumn: number, originalSource: string, originalLine: number, originalColumn: number, name?: string}>}
 */
function parseMappings(mappings, sources, names, sourceRoot) {
  const entries = [];
  if (!mappings) return entries;

  let generatedLine = 0;
  let previousGeneratedColumn = 0;
  let previousOriginalLine = 0;
  let previousOriginalColumn = 0;
  let previousSource = 0;
  let previousName = 0;
  const iter = { pos: 0 };

  while (iter.pos < mappings.length) {
    const ch = mappings[iter.pos];
    if (ch === ";") {
      generatedLine++;
      previousGeneratedColumn = 0;
      iter.pos++;
      continue;
    }
    if (ch === ",") {
      iter.pos++;
      continue;
    }

    // Decode segment: generatedColumn, [sourceIndex, originalLine, originalColumn, [nameIndex]]
    const generatedColumn = previousGeneratedColumn + decodeVLQ(mappings, iter);
    previousGeneratedColumn = generatedColumn;

    // Check if there are more fields (source mapping)
    if (iter.pos < mappings.length) {
      const next = mappings[iter.pos];
      if (next !== "," && next !== ";") {
        const sourceIndex = previousSource + decodeVLQ(mappings, iter);
        previousSource = sourceIndex;

        const originalLine = previousOriginalLine +
          decodeVLQ(mappings, iter);
        previousOriginalLine = originalLine;

        const originalColumn = previousOriginalColumn +
          decodeVLQ(mappings, iter);
        previousOriginalColumn = originalColumn;

        let source = sources[sourceIndex] || "";
        if (
          sourceRoot && !StringPrototypeStartsWith(source, "/") &&
          !RegExpPrototypeTest(/^\w+:\/\//, source)
        ) {
          source = sourceRoot + source;
        }

        let name;
        // Check for optional name index
        if (
          iter.pos < mappings.length &&
          mappings[iter.pos] !== "," &&
          mappings[iter.pos] !== ";"
        ) {
          const nameIndex = previousName + decodeVLQ(mappings, iter);
          previousName = nameIndex;
          name = names ? names[nameIndex] : undefined;
        }

        ArrayPrototypePush(entries, {
          generatedLine,
          generatedColumn,
          originalSource: source,
          originalLine,
          originalColumn,
          name,
        });
      }
    }

    // Segments with only generated column (no source mapping) are skipped,
    // matching Node.js behavior - only full mapping entries are included.
  }

  return entries;
}

/**
 * Compare two source map entries for sorting/binary search.
 */
function compareEntries(a, b) {
  if (a.generatedLine !== b.generatedLine) {
    return a.generatedLine - b.generatedLine;
  }
  return a.generatedColumn - b.generatedColumn;
}

/**
 * Binary search for the entry that contains the given generated position.
 */
function findEntryInMappings(entries, line, column) {
  let low = 0;
  let high = entries.length - 1;
  let best = -1;

  while (low <= high) {
    const mid = (low + high) >> 1;
    const entry = entries[mid];
    const cmp = entry.generatedLine - line ||
      entry.generatedColumn - column;

    if (cmp === 0) {
      return entry;
    } else if (cmp < 0) {
      best = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }

  if (best >= 0) {
    const entry = entries[best];
    if (entry.generatedLine === line) {
      return entry;
    }
  }

  return null;
}

/**
 * Deep clone an object (simple JSON-safe objects).
 */
function deepClone(obj) {
  if (obj === null || typeof obj !== "object") return obj;
  if (ArrayIsArray(obj)) return ArrayPrototypeMap(obj, deepClone);
  const clone = {};
  const keys = ObjectKeys(obj);
  for (let i = 0; i < keys.length; i++) {
    clone[keys[i]] = deepClone(obj[keys[i]]);
  }
  return clone;
}

/**
 * SourceMap class implementing Node.js's module.SourceMap API.
 * @see https://nodejs.org/api/module.html#class-modulesourcemap
 */
class SourceMap {
  #payload;
  #lineLengths;
  #mappings;

  /**
   * @param {object} payload - Source Map V3 payload object
   * @param {{ lineLengths?: number[] }} [options]
   */
  constructor(payload, options) {
    if (
      typeof payload !== "object" || payload === null ||
      ArrayIsArray(payload)
    ) {
      let received;
      if (payload === null) {
        received = " Received null";
      } else if (typeof payload === "object") {
        const proto = ObjectGetPrototypeOf(payload);
        const name = proto?.constructor?.name;
        received = name
          ? ` Received an instance of ${name}`
          : ` Received ${typeof payload}`;
      } else {
        let inspected = String(payload);
        if (inspected.length > 28) {
          inspected = StringPrototypeSlice(inspected, 0, 25) + "...";
        }
        received = ` Received type ${typeof payload} (${inspected})`;
      }
      const err = new TypeError(
        `The "payload" argument must be of type object.${received}`,
      );
      err.code = "ERR_INVALID_ARG_TYPE";
      throw err;
    }

    this.#payload = deepClone(payload);
    this.#lineLengths = options?.lineLengths
      ? ArrayPrototypeSlice(options.lineLengths)
      : undefined;

    // Parse mappings - handle both regular and index source maps
    this.#mappings = this.#parseMap(payload);
    // Sort entries by generated position
    ArrayPrototypeSort(this.#mappings, compareEntries);
  }

  /**
   * Parse source map payload into mapping entries.
   * Handles both regular source maps and index source maps (with sections).
   */
  #parseMap(payload) {
    if (payload.sections) {
      // Index Source Map V3
      const entries = [];
      const sections = payload.sections;
      for (let i = 0; i < sections.length; i++) {
        const section = sections[i];
        const offset = section.offset || { line: 0, column: 0 };
        const map = section.map;
        const sectionEntries = parseMappings(
          map.mappings,
          map.sources || [],
          map.names || [],
          map.sourceRoot || "",
        );
        // Apply section offset
        for (let j = 0; j < sectionEntries.length; j++) {
          const entry = sectionEntries[j];
          entry.generatedLine += offset.line;
          if (entry.generatedLine === offset.line) {
            entry.generatedColumn += offset.column;
          }
          ArrayPrototypePush(entries, entry);
        }
      }
      // For index maps, flatten the sources and mappings into the payload clone
      if (!this.#payload.sources) {
        this.#payload.sources = [];
      }
      if (!this.#payload.mappings) {
        this.#payload.mappings = payload.mappings || undefined;
      }
      return entries;
    }

    return parseMappings(
      payload.mappings,
      payload.sources || [],
      payload.names || [],
      payload.sourceRoot || "",
    );
  }

  /**
   * Getter for the payload used to construct the SourceMap instance.
   * Returns a clone of the original payload.
   */
  get payload() {
    return this.#payload;
  }

  /**
   * Getter for line lengths, if provided in the constructor options.
   */
  get lineLengths() {
    return this.#lineLengths;
  }

  /**
   * Given a 0-indexed line offset and column offset in the generated source,
   * returns an object representing the SourceMap range in the original file
   * if found, or an empty object if not.
   *
   * @param {number} lineOffset - Zero-indexed line number in generated source
   * @param {number} columnOffset - Zero-indexed column number in generated source
   * @returns {{ generatedLine: number, generatedColumn: number, originalSource: string, originalLine: number, originalColumn: number } | {}}
   */
  findEntry(lineOffset, columnOffset) {
    if (this.#mappings.length === 0) return {};
    const entry = findEntryInMappings(this.#mappings, lineOffset, columnOffset);
    if (!entry) return {};
    return {
      generatedLine: entry.generatedLine,
      generatedColumn: entry.generatedColumn,
      originalSource: entry.originalSource,
      originalLine: entry.originalLine,
      originalColumn: entry.originalColumn,
    };
  }

  /**
   * Given 1-indexed lineNumber and columnNumber from a call site in the generated
   * source, find the corresponding call site location in the original source.
   *
   * @param {number} lineNumber - 1-indexed line number
   * @param {number} columnNumber - 1-indexed column number
   * @returns {{ name?: string, fileName: string, lineNumber: number, columnNumber: number } | {}}
   */
  findOrigin(lineNumber, columnNumber) {
    const entry = this.findEntry(lineNumber - 1, columnNumber - 1);
    if (
      entry.originalSource === undefined ||
      entry.originalLine === undefined ||
      entry.originalColumn === undefined ||
      entry.generatedLine === undefined ||
      entry.generatedColumn === undefined
    ) {
      return {};
    }
    const lineOffset = lineNumber - entry.generatedLine;
    const columnOffset = columnNumber - entry.generatedColumn;
    const result = {
      fileName: entry.originalSource,
      lineNumber: entry.originalLine + lineOffset,
      columnNumber: entry.originalColumn + columnOffset,
    };
    if (entry.name !== undefined) {
      result.name = entry.name;
    }
    return result;
  }
}

// Cache for findSourceMap: path -> SourceMap | null (null means checked but not found)
const sourceMapCache = new SafeMap();

// Regex to match //# sourceMappingURL=<url> or //@ sourceMappingURL=<url>
const SOURCE_MAP_URL_RE =
  /\/\/[#@]\s*sourceMappingURL\s*=\s*(\S+)\s*(?:\n|\r\n?)?$/;

/**
 * Extract the sourceMappingURL from the last non-empty line of content.
 * @param {string} content
 * @returns {string | null}
 */
function extractSourceMapUrl(content) {
  // Search backwards from the end for the sourceMappingURL comment.
  // The comment must appear in the last non-empty line.
  const match = RegExpPrototypeExec(SOURCE_MAP_URL_RE, content);
  return match ? match[1] : null;
}

/**
 * Resolve a source map from a sourceMappingURL.
 * Handles both inline data URIs and external file references.
 * @param {string} url - The sourceMappingURL value
 * @param {string} filePath - The path of the file containing the reference
 * @returns {object | null} - Parsed source map payload or null
 */
function resolveSourceMapPayload(url, filePath) {
  // Handle inline base64 data URIs
  if (StringPrototypeStartsWith(url, "data:")) {
    const dataUrlMatch = StringPrototypeMatch(
      url,
      /^data:application\/json;(?:charset=utf-?8;)?base64,(.+)$/,
    );
    if (dataUrlMatch) {
      try {
        const decoded = atob(dataUrlMatch[1]);
        return JSONParse(decoded);
      } catch {
        return null;
      }
    }
    return null;
  }

  // Handle external source map files
  try {
    let mapPath;
    if (
      StringPrototypeStartsWith(url, "/") ||
      RegExpPrototypeTest(/^[a-zA-Z]:\\/, url)
    ) {
      // Absolute path
      mapPath = url;
    } else {
      // Relative path - resolve against the source file's directory
      const dir = op_require_path_dirname(filePath);
      mapPath = op_require_path_resolve([dir, url]);
    }
    const mapContent = op_require_read_file(mapPath);
    if (mapContent) {
      return JSONParse(mapContent);
    }
  } catch {
    // File not found or invalid JSON - return null
  }
  return null;
}

/**
 * @param {string} path
 * @returns {SourceMap | undefined}
 */
export function findSourceMap(path) {
  // Normalize the path to avoid duplicate cache entries for equivalent paths
  // (e.g. "/foo/bar.js" vs "/foo/./bar.js")
  path = op_require_path_resolve([path]);

  if (sourceMapCache.has(path)) {
    const cached = sourceMapCache.get(path);
    return cached === null ? undefined : cached;
  }

  try {
    const content = op_require_read_file(path);
    if (!content) {
      sourceMapCache.set(path, null);
      return undefined;
    }

    const url = extractSourceMapUrl(content);
    if (!url) {
      sourceMapCache.set(path, null);
      return undefined;
    }

    const payload = resolveSourceMapPayload(url, path);
    if (!payload) {
      sourceMapCache.set(path, null);
      return undefined;
    }

    // Compute lineLengths from the source file content
    const lines = StringPrototypeSplit(
      StringPrototypeReplace(content, /\n$/, ""),
      "\n",
    );
    const lineLengths = [];
    for (let i = 0; i < lines.length; i++) {
      ArrayPrototypePush(lineLengths, lines[i].length);
    }

    const sourceMap = new SourceMap(payload, { lineLengths });
    sourceMapCache.set(path, sourceMap);
    return sourceMap;
  } catch {
    sourceMapCache.set(path, null);
    return undefined;
  }
}

Module.findSourceMap = findSourceMap;
Module.SourceMap = SourceMap;

let initialized = false;

function initialize(args) {
  const {
    usesLocalNodeModulesDir: usesLocalNodeModulesDirArg,
    argv0,
    runningOnMainThread,
    workerId,
    maybeWorkerMetadata,
    nodeDebug,
    nodeClusterUniqueId,
    nodeClusterSchedPolicy,
    warmup = false,
    moduleSpecifier = null,
  } = args;
  if (!warmup) {
    if (initialized) {
      throw new Error("Node runtime already initialized");
    }
    initialized = true;
    if (usesLocalNodeModulesDirArg) {
      usesLocalNodeModulesDir = true;
    }

    internals.__bootstrapNodeProcess(
      argv0,
      Deno.args,
      Deno.version,
      nodeDebug ?? "",
      false,
      runningOnMainThread,
    );
    internals.__initWorkerThreads(
      runningOnMainThread,
      workerId,
      maybeWorkerMetadata,
      moduleSpecifier,
    );
    internals.__setupChildProcessIpcChannel();
    if (nodeClusterUniqueId) {
      core.loadExtScript("ext:deno_node/cluster.ts");
      internals.__initCluster(nodeClusterUniqueId, nodeClusterSchedPolicy);
    }
    const { streamBaseState } = core.loadExtScript(
      "ext:deno_node/internal_binding/stream_wrap.ts",
    );
    op_stream_base_register_state(streamBaseState);
    nativeModuleExports["internal/console/constructor"].bindStreamsLazy(
      nativeModuleExports["console"],
      nativeModuleExports["process"],
    );
  } else {
    internals.__bootstrapNodeProcess(
      undefined,
      undefined,
      undefined,
      undefined,
      true,
    );
  }
}

globalThis.nodeBootstrap = initialize;

function closeIdleConnections() {
  try {
    const http = nativeModuleExports["http"];
    if (http?.globalAgent) {
      http.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }
  try {
    const https = nativeModuleExports["https"];
    if (https?.globalAgent) {
      https.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }
}

internals.closeIdleConnections = closeIdleConnections;

export {
  builtinModules,
  createRequire,
  getBuiltinModule,
  isBuiltin,
  Module,
  SourceMap,
};
export const _cache = Module._cache;
export const _extensions = Module._extensions;
export const _findPath = Module._findPath;
export const _initPaths = Module._initPaths;
export const _load = Module._load;
export const _nodeModulePaths = Module._nodeModulePaths;
export const _pathCache = Module._pathCache;
export const _preloadModules = Module._preloadModules;
export const _resolveFilename = Module._resolveFilename;
export const _resolveLookupPaths = Module._resolveLookupPaths;
export const globalPaths = Module.globalPaths;

export default Module;
