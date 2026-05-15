// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_fs_cwd,
  op_import_sync,
  op_import_sync_with_source,
  op_module_hooks_poll_load,
  op_module_hooks_poll_resolve,
  op_module_hooks_register,
  op_module_hooks_respond_load,
  op_module_hooks_respond_resolve,
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
const internalTestBinding = core.loadExtScript(
  "ext:deno_node/internal/test/binding.ts",
);
const internalTimers = core.loadExtScript(
  "ext:deno_node/internal/timers.mjs",
);
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
const testReporters = core.loadExtScript("ext:deno_node/test/reporters.ts");
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
    "internal/test/binding": internalTestBinding,
    "internal/timers": internalTimers,
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
    "test/reporters": testReporters,
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
    "test/reporters",
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

// module.registerHooks() infrastructure
const hookEntries = [];
// Pending hook module loads from register(). The ESM hook loops await these
// before processing requests, ensuring hooks are active before subsequent
// imports are resolved.
const pendingHookLoads = [];
let insideResolveHook = false;
let hookResolveConditions = null;
let insideLoadHook = false;
let utf8Decoder;
let esmResolveLoopRunning = false;
let esmLoadLoopRunning = false;
// Formats determined by resolve hooks, keyed by resolved URL.
// Passed as context.format to load hooks per Node.js spec.
const resolvedFormats = new SafeMap();

// module.register() infrastructure - async hooks run in a dedicated worker
// thread to avoid deadlocks when loading the hook module.
let hooksWorker = null;
let asyncHooksHaveResolve = false;
let asyncHooksHaveLoad = false;
let nextHookRequestId = 0;
const pendingHookRequests = new SafeMap();

// Source code for the hooks worker thread. The worker loads hook modules,
// maintains the async hook chain, and processes resolve/load requests.
// deno-lint-ignore prefer-primordials
const HOOKS_WORKER_SOURCE = `
// In Node.js, the hooks thread's process.exit() exits the whole process.
// We intercept it and send a message so the main thread can call process.exit().
if (globalThis.process) {
  globalThis.process.exit = (code) => {
    self.postMessage({ type: "process-exit", code: code ?? 0 });
    // Block the worker so no more code runs after process.exit()
    for (;;) { /* spin until main process terminates us */ }
  };
}

const asyncHookEntries = [];

// Pre-computed Deno default-resolved URL for the current resolve request.
// Set per-request from the message payload so \`defaultResolve()\` (called by
// user hooks via \`next()\`) returns Deno's actual resolution (import map,
// jsr, npm) rather than a naive URL.parse.
let currentRequestSpecifier = null;
let currentRequestDefaultUrl = null;

function defaultResolve(spec, context) {
  if (spec.startsWith("node:")) {
    return { url: spec, shortCircuit: true };
  }
  // When the user hook called next() with the same specifier, use Deno's
  // pre-computed resolution. The hook is expected to then transform that
  // URL (or pass it through).
  if (spec === currentRequestSpecifier && currentRequestDefaultUrl !== null) {
    return { url: currentRequestDefaultUrl, shortCircuit: true };
  }
  const parentURL = context.parentURL;
  if (parentURL) {
    try {
      return { url: new URL(spec, parentURL).href, shortCircuit: true };
    } catch { /* fall through */ }
  }
  return { url: null, shortCircuit: true };
}

function defaultLoad(loadUrl) {
  if (loadUrl.startsWith("node:")) {
    return { source: null, format: "builtin", shortCircuit: true };
  }
  let source = null;
  if (loadUrl.startsWith("file://")) {
    // Read file so hooks calling nextLoad() can inspect/transform the source.
    // Uses Deno.readTextFileSync which respects --allow-read/--deny-read
    // permissions (worker inherits parent permissions).
    try { source = Deno.readTextFileSync(new URL(loadUrl)); }
    catch { /* fall through with null source */ }
  }
  return { source, shortCircuit: true };
}

function runResolveChain(specifier, context) {
  const hooks = [];
  for (let i = asyncHookEntries.length - 1; i >= 0; i--) {
    if (asyncHookEntries[i].resolve !== null) hooks.push(asyncHookEntries[i].resolve);
  }
  if (hooks.length === 0) return null;
  let index = 0;
  let currentContext = context;
  function nextResolve(spec, ctx) {
    if (ctx !== undefined && ctx !== null) currentContext = { ...currentContext, ...ctx };
    if (index >= hooks.length) return defaultResolve(spec, currentContext);
    const hook = hooks[index++];
    let nextCalled = false;
    const wrappedNext = (s, c) => { nextCalled = true; return nextResolve(s, c); };
    const result = hook(spec, currentContext, wrappedNext);
    if (result && typeof result.then === "function") {
      return result.then((r) => {
        if (!nextCalled && !r?.shortCircuit) throw new TypeError("resolve hook must call next or short-circuit");
        return r;
      });
    }
    if (!nextCalled && !result?.shortCircuit) throw new TypeError("resolve hook must call next or short-circuit");
    return result;
  }
  return nextResolve(specifier, context);
}

function runLoadChain(fileUrl, context) {
  const hooks = [];
  for (let i = asyncHookEntries.length - 1; i >= 0; i--) {
    if (asyncHookEntries[i].load !== null) hooks.push(asyncHookEntries[i].load);
  }
  if (hooks.length === 0) return null;
  let index = 0;
  let currentContext = context;
  function nextLoad(loadUrl, ctx) {
    if (ctx !== undefined && ctx !== null) currentContext = { ...currentContext, ...ctx };
    if (index >= hooks.length) return defaultLoad(loadUrl);
    const hook = hooks[index++];
    let nextCalled = false;
    const wrappedNext = (u, c) => { nextCalled = true; return nextLoad(u, c); };
    const result = hook(loadUrl, currentContext, wrappedNext);
    if (result && typeof result.then === "function") {
      return result.then((r) => {
        if (!nextCalled && !r?.shortCircuit) throw new TypeError("load hook must call next or short-circuit");
        return r;
      });
    }
    if (!nextCalled && !result?.shortCircuit) throw new TypeError("load hook must call next or short-circuit");
    return result;
  }
  return nextLoad(fileUrl, context);
}

self.onmessage = (e) => {
  const msg = e.data;
  let promise;
  if (msg.type === "register") {
    promise = (async () => {
      const hookModule = await import(msg.url);
      if (typeof hookModule.initialize === "function") await hookModule.initialize(msg.data);
      const resolve = typeof hookModule.resolve === "function" ? hookModule.resolve : null;
      const load = typeof hookModule.load === "function" ? hookModule.load : null;
      if (resolve !== null || load !== null) asyncHookEntries.push({ resolve, load });
      return { type: "registered", id: msg.id, hasResolve: resolve !== null, hasLoad: load !== null };
    })();
  } else if (msg.type === "resolve") {
    currentRequestSpecifier = msg.specifier;
    currentRequestDefaultUrl = msg.defaultUrl ?? null;
    promise = Promise.resolve(runResolveChain(msg.specifier, msg.context))
      .then((result) => {
        currentRequestSpecifier = null;
        currentRequestDefaultUrl = null;
        return { type: "resolve-result", id: msg.id, result };
      }, (err) => {
        currentRequestSpecifier = null;
        currentRequestDefaultUrl = null;
        throw err;
      });
  } else if (msg.type === "load") {
    promise = Promise.resolve(runLoadChain(msg.url, msg.context))
      .then((result) => ({ type: "load-result", id: msg.id, result }));
  } else {
    return;
  }
  promise.then(
    (response) => self.postMessage(response),
    (err) => {
      // Format like Node.js: use inspect for objects/functions, String for primitives
      let errStr;
      if ((typeof err === "object" && err !== null) || typeof err === "function") {
        errStr = typeof Deno !== "undefined" && Deno.inspect ? Deno.inspect(err) : String(err);
      } else {
        errStr = String(err);
      }
      self.postMessage({ type: "error", id: msg.id, error: errStr });
    },
  );
};
`;

function _ensureHooksWorker() {
  if (hooksWorker !== null) return;
  // deno-lint-ignore prefer-primordials
  const workerUrl = "data:text/javascript," +
    encodeURIComponent(HOOKS_WORKER_SOURCE);
  // deno-lint-ignore prefer-primordials
  hooksWorker = new globalThis.Worker(workerUrl, { type: "module" });
  hooksWorker.onmessage = (e) => {
    const msg = e.data;
    // process.exit() in the hooks worker should exit the main process
    if (msg.type === "process-exit") {
      process.exit(msg.code ?? 1);
      return;
    }
    const entry = pendingHookRequests.get(msg.id);
    if (entry === undefined) return;
    pendingHookRequests.delete(msg.id);
    if (msg.type === "error") {
      entry.reject(new Error(msg.error));
    } else {
      entry.resolve(msg);
    }
  };
  hooksWorker.onerror = (e) => {
    // Uncaught errors in the hooks worker should terminate the main process
    process.stderr.write(e.message + "\n");
    process.exit(1);
  };
  // Unref the worker so it doesn't prevent process exit (like Node.js
  // hooks thread). The Rust module loader's pending futures keep the
  // event loop alive during active imports, so unref'd worker messages
  // still get processed.
  _refHooksWorker(false);
}

function _refHooksWorker(ref) {
  if (!hooksWorker) return;
  const { privateWorkerRef } = core.loadExtScript("ext:runtime/11_workers.js");
  hooksWorker[privateWorkerRef](ref);
}

function _sendToHooksWorker(msg, transferList) {
  const id = nextHookRequestId++;
  msg.id = id;
  const { promise, resolve, reject } = Promise.withResolvers();
  pendingHookRequests.set(id, { resolve, reject });
  if (transferList && transferList.length > 0) {
    hooksWorker.postMessage(msg, transferList);
  } else {
    hooksWorker.postMessage(msg);
  }
  return promise;
}

function executeResolveHookChain(specifier, context, parent, isMain) {
  // Collect resolve hooks from hookEntries in LIFO order
  const resolveHooks = [];
  for (let i = hookEntries.length - 1; i >= 0; i--) {
    if (hookEntries[i].resolve !== null) {
      ArrayPrototypePush(resolveHooks, hookEntries[i].resolve);
    }
  }
  if (resolveHooks.length === 0) return null;

  let index = 0;
  // Running context accumulates changes across the chain
  let currentContext = context;

  function nextResolve(spec, ctx) {
    // If ctx provided, merge into running context
    if (ctx !== undefined && ctx !== null) {
      currentContext = { ...currentContext, ...ctx };
    }

    if (index >= resolveHooks.length) {
      // Default resolve: use Module._resolveFilename
      insideResolveHook = true;
      hookResolveConditions = currentContext.conditions ?? null;
      try {
        // Handle node: builtins
        if (StringPrototypeStartsWith(spec, "node:")) {
          return { url: spec, shortCircuit: true };
        }
        if (nativeModuleCanBeRequiredByUsers(spec)) {
          return { url: "node:" + spec, shortCircuit: true };
        }
        const resolved = Module._resolveFilename(spec, parent, isMain);
        let resolvedUrl;
        if (StringPrototypeStartsWith(resolved, "node:")) {
          resolvedUrl = resolved;
        } else {
          resolvedUrl = url.pathToFileURL(resolved).href;
        }
        return { url: resolvedUrl, shortCircuit: true };
      } finally {
        insideResolveHook = false;
        hookResolveConditions = null;
      }
    }
    const hook = resolveHooks[index++];
    let nextCalled = false;
    const wrappedNext = (s, c) => {
      nextCalled = true;
      return nextResolve(s, c);
    };
    const result = hook(spec, currentContext, wrappedNext);
    if (!nextCalled && !result?.shortCircuit) {
      throw new internalErrors.ERR_INVALID_RETURN_PROPERTY_VALUE(
        "true",
        "resolve",
        "shortCircuit",
        result?.shortCircuit,
      );
    }
    return result;
  }

  return nextResolve(specifier, context);
}

function executeLoadHookChain(fileUrl, context) {
  // Collect load hooks from hookEntries in LIFO order
  const loadHooks = [];
  for (let i = hookEntries.length - 1; i >= 0; i--) {
    if (hookEntries[i].load !== null) {
      ArrayPrototypePush(loadHooks, hookEntries[i].load);
    }
  }
  if (loadHooks.length === 0) return null;

  let index = 0;
  let currentContext = context;

  function nextLoad(loadUrl, ctx) {
    if (ctx !== undefined && ctx !== null) {
      currentContext = { ...currentContext, ...ctx };
    }

    if (index >= loadHooks.length) {
      // Default load: read file from disk
      // For builtins, return null source
      if (StringPrototypeStartsWith(loadUrl, "node:")) {
        return { source: null, format: "builtin", shortCircuit: true };
      }
      const filePath = StringPrototypeStartsWith(loadUrl, "file://")
        ? url.fileURLToPath(loadUrl)
        : loadUrl;
      const source = op_require_read_file(filePath);
      return {
        source,
        format: currentContext?.format ?? undefined,
        shortCircuit: true,
      };
    }
    const hook = loadHooks[index++];
    let nextCalled = false;
    const wrappedNext = (u, c) => {
      nextCalled = true;
      return nextLoad(u, c);
    };
    const result = hook(loadUrl, currentContext, wrappedNext);
    if (!nextCalled && !result?.shortCircuit) {
      throw new internalErrors.ERR_INVALID_RETURN_PROPERTY_VALUE(
        "true",
        "load",
        "shortCircuit",
        result?.shortCircuit,
      );
    }
    return result;
  }

  return nextLoad(fileUrl, context);
}

// ESM resolve hook chain: runs sync hooks (registerHooks) in LIFO order,
// then async hooks (register) in LIFO order.
// Returns { url } if hooks resolved, or null for fallthrough to default.
//
// `defaultUrl` is the URL that Deno's default resolver (import map, jsr,
// npm, etc.) produced for `specifier`. Returned from `defaultResolve()` so
// user hooks calling `next()` see Deno's real resolution -- not a naive
// `new URL(spec, parentURL)`.
async function executeEsmResolveHookChain(specifier, context, defaultUrl) {
  // Run sync hooks (registerHooks) first on the main thread
  const syncResolveHooks = [];
  for (let i = hookEntries.length - 1; i >= 0; i--) {
    if (hookEntries[i].resolve !== null) {
      ArrayPrototypePush(syncResolveHooks, hookEntries[i].resolve);
    }
  }

  if (syncResolveHooks.length > 0) {
    let index = 0;
    let currentContext = context;

    function nextResolve(spec, ctx) {
      if (ctx !== undefined && ctx !== null) {
        currentContext = { ...currentContext, ...ctx };
      }
      if (index >= syncResolveHooks.length) {
        // End of sync chain - if async hooks exist, they will run
        // in the worker below. For now return fallthrough.
        if (asyncHooksHaveResolve) {
          return { url: null, shortCircuit: false, _syncFallthrough: true };
        }
        // Default resolve (no async hooks). If the spec is unchanged we
        // can use Deno's pre-computed default URL.
        if (spec === specifier && defaultUrl != null) {
          return { url: defaultUrl, shortCircuit: true };
        }
        if (StringPrototypeStartsWith(spec, "node:")) {
          return { url: spec, shortCircuit: true };
        }
        if (nativeModuleCanBeRequiredByUsers(spec)) {
          return { url: "node:" + spec, shortCircuit: true };
        }
        const parentURL = currentContext.parentURL;
        if (parentURL) {
          try {
            return {
              url: new URL(spec, parentURL).href,
              shortCircuit: true,
            };
          } catch {
            // Fall through
          }
        }
        try {
          const resolved = Module._resolveFilename(spec, null, false);
          if (StringPrototypeStartsWith(resolved, "node:")) {
            return { url: resolved, shortCircuit: true };
          }
          return {
            url: url.pathToFileURL(resolved).href,
            shortCircuit: true,
          };
        } catch {
          // Could not resolve
        }
        return { url: null, shortCircuit: true };
      }
      const hook = syncResolveHooks[index++];
      let nextCalled = false;
      const wrappedNext = (s, c) => {
        nextCalled = true;
        return nextResolve(s, c);
      };
      const result = hook(spec, currentContext, wrappedNext);
      if (result && typeof result.then === "function") {
        return result.then((r) => {
          if (!nextCalled && !r?.shortCircuit) {
            throw new TypeError(
              "resolve hook must return { shortCircuit: true } or call nextResolve",
            );
          }
          return r;
        });
      }
      if (!nextCalled && !result?.shortCircuit) {
        throw new TypeError(
          "resolve hook must return { shortCircuit: true } or call nextResolve",
        );
      }
      return result;
    }

    const syncResult = await nextResolve(specifier, context);
    if (syncResult && syncResult.shortCircuit && !syncResult._syncFallthrough) {
      return syncResult;
    }
    // Sync hooks fell through; continue to async hooks in the worker
  }

  if (!asyncHooksHaveResolve) {
    return syncResolveHooks.length === 0 ? null : { url: null };
  }

  // Forward to the hooks worker for async hook execution
  const msg = await _sendToHooksWorker({
    type: "resolve",
    specifier,
    context,
    defaultUrl,
  });
  return msg.result;
}

// ESM load hook chain: runs sync hooks (registerHooks) on main thread,
// then async hooks (register) in the worker thread.
// Returns { source } if hooks provided source, or null for fallthrough.
async function executeEsmLoadHookChain(fileUrl, context) {
  // Run sync hooks first on the main thread
  const syncLoadHooks = [];
  for (let i = hookEntries.length - 1; i >= 0; i--) {
    if (hookEntries[i].load !== null) {
      ArrayPrototypePush(syncLoadHooks, hookEntries[i].load);
    }
  }

  if (syncLoadHooks.length > 0) {
    let index = 0;
    let currentContext = context;

    function nextLoad(loadUrl, ctx) {
      if (ctx !== undefined && ctx !== null) {
        currentContext = { ...currentContext, ...ctx };
      }
      if (index >= syncLoadHooks.length) {
        if (asyncHooksHaveLoad) {
          return { source: null, shortCircuit: false, _syncFallthrough: true };
        }
        // Default load (no async hooks)
        if (StringPrototypeStartsWith(loadUrl, "node:")) {
          return { source: null, format: "builtin", shortCircuit: true };
        }
        let source = null;
        if (StringPrototypeStartsWith(loadUrl, "file://")) {
          try {
            source = op_require_read_file(url.fileURLToPath(loadUrl));
          } catch {
            // Fall through with null source
          }
        }
        return { source, shortCircuit: true };
      }
      const hook = syncLoadHooks[index++];
      let nextCalled = false;
      const wrappedNext = (u, c) => {
        nextCalled = true;
        return nextLoad(u, c);
      };
      const result = hook(loadUrl, currentContext, wrappedNext);
      if (result && typeof result.then === "function") {
        return result.then((r) => {
          if (!nextCalled && !r?.shortCircuit) {
            throw new TypeError(
              "load hook must return { shortCircuit: true } or call nextLoad",
            );
          }
          return r;
        });
      }
      if (!nextCalled && !result?.shortCircuit) {
        throw new TypeError(
          "load hook must return { shortCircuit: true } or call nextLoad",
        );
      }
      return result;
    }

    const syncResult = await nextLoad(fileUrl, context);
    if (syncResult && syncResult.shortCircuit && !syncResult._syncFallthrough) {
      return syncResult;
    }
  }

  if (!asyncHooksHaveLoad) {
    return syncLoadHooks.length === 0 ? null : { source: null };
  }

  // Forward to the hooks worker for async hook execution
  const msg = await _sendToHooksWorker({
    type: "load",
    url: fileUrl,
    context,
  });
  return msg.result;
}

function _startEsmResolveLoop() {
  if (esmResolveLoopRunning) return;
  esmResolveLoopRunning = true;
  (async () => {
    while (true) {
      const pollPromise = op_module_hooks_poll_resolve();
      core.unrefOpPromise(pollPromise);
      const req = await pollPromise;
      if (req === null) break;
      // Wait for any pending hook module loads to complete before
      // processing requests. This ensures register() hooks are active
      // before subsequent imports are resolved.
      if (pendingHookLoads.length > 0) {
        await Promise.all(pendingHookLoads);
      }
      const [id, specifier, referrer, defaultUrl] = req;
      const context = {
        conditions: ["node", "import"],
        importAttributes: { __proto__: null },
        parentURL: referrer || undefined,
        importAssertions: { __proto__: null },
      };
      try {
        const result = await executeEsmResolveHookChain(
          specifier,
          context,
          defaultUrl ?? null,
        );
        if (result !== null && result.url != null) {
          if (result.format != null) {
            resolvedFormats.set(result.url, result.format);
          }
          op_module_hooks_respond_resolve(id, result.url, null);
        } else {
          // Fallthrough: tell Rust to use default resolution
          op_module_hooks_respond_resolve(id, null, null);
        }
      } catch (e) {
        op_module_hooks_respond_resolve(id, null, String(e));
      }
    }
  })();
}

function _startEsmLoadLoop() {
  if (esmLoadLoopRunning) return;
  esmLoadLoopRunning = true;
  (async () => {
    while (true) {
      const pollPromise = op_module_hooks_poll_load();
      core.unrefOpPromise(pollPromise);
      const req = await pollPromise;
      if (req === null) break;
      const [id, fileUrl] = req;
      const storedFormat = resolvedFormats.get(fileUrl);
      if (storedFormat !== undefined) resolvedFormats.delete(fileUrl);
      const context = {
        format: storedFormat ?? undefined,
        conditions: ["node", "import"],
        importAttributes: { __proto__: null },
        importAssertions: { __proto__: null },
      };
      try {
        const result = await executeEsmLoadHookChain(fileUrl, context);
        if (result !== null && result.source != null) {
          const source = typeof result.source === "string"
            ? result.source
            : new TextDecoder().decode(result.source);
          const format = result.format || null;
          op_module_hooks_respond_load(id, source, format, null);
        } else {
          // Fallthrough: tell Rust to use default loading
          op_module_hooks_respond_load(id, null, null, null);
        }
      } catch (e) {
        op_module_hooks_respond_load(id, null, null, String(e));
      }
    }
  })();
}

function _activateEsmHooks() {
  let hasResolve = asyncHooksHaveResolve;
  let hasLoad = asyncHooksHaveLoad;
  for (let i = 0; i < hookEntries.length; i++) {
    if (hookEntries[i].resolve !== null) hasResolve = true;
    if (hookEntries[i].load !== null) hasLoad = true;
  }
  op_module_hooks_register(hasResolve, hasLoad);
  if (hasResolve) _startEsmResolveLoop();
  if (hasLoad) _startEsmLoadLoop();
}

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
  this.parent = parent;
  this.children = [];
}

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
    hookResolveConditions,
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
          _throwIfEsmCycle(cachedModule, parent);
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

    // Run load hooks for builtins if registered
    if (hookEntries.length > 0 && !insideLoadHook) {
      let hasLoadHook = false;
      for (let i = 0; i < hookEntries.length; i++) {
        if (hookEntries[i].load !== null) {
          hasLoadHook = true;
          break;
        }
      }
      if (hasLoadHook) {
        const context = {
          format: "builtin",
          conditions: ["node", "require"],
          importAttributes: { __proto__: null },
          importAssertions: { __proto__: null },
        };
        insideLoadHook = true;
        let result;
        try {
          result = executeLoadHookChain(filename, context);
        } finally {
          insideLoadHook = false;
        }
        // If the hook changed the format away from "builtin", use the
        // hook-provided source instead of loading the native module.
        // This matches Node.js behavior where hooks can replace builtins
        // by returning a different format (e.g. "commonjs").
        if (
          result != null && result.format &&
          result.format !== "builtin" && result.source != null
        ) {
          const mod = new Module(filename, parent);
          Module._cache[filename] = mod;
          const source = typeof result.source === "string"
            ? result.source
            : (utf8Decoder ??= new TextDecoder()).decode(result.source);
          if (result.format === "commonjs") {
            mod._compile(source, filename, "commonjs");
          } else if (result.format === "json") {
            mod.exports = JSONParse(stripBOM(source));
          } else {
            mod._compile(source, filename);
          }
          mod.loaded = true;
          return mod.exports;
        }
      }
    }

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
      _throwIfEsmCycle(cachedModule, parent);
      return getExportsForCircularRequire(cachedModule);
    }
    return cachedModule.exports;
  }

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
    // main CJS module (generated by node_resolver::analyze), becomes a
    // module evaluation rejection, and is routed through Deno's unhandled-
    // rejection path -- which emits with origin === 'unhandledRejection'.
    if (
      isMain &&
      parent === null &&
      typeof process !== "undefined" &&
      typeof process._fatalException === "function"
    ) {
      if (process._fatalException(err)) {
        // Handled by a registered 'uncaughtException' listener or an
        // 'uncaughtException' capture callback. Treat the load as complete
        // so the ESM wrapper module evaluation succeeds and the runtime can
        // continue running pending callbacks.
        return module.exports;
      }
      // Not handled. Mark the error so the unhandled-rejection fallback in
      // process.ts skips a redundant second emit of monitor/uncaughtException
      // when the re-thrown error surfaces as a module-evaluation rejection.
      if (err !== null && typeof err === "object") {
        const set = internals._dispatchedFatalErrors;
        if (set !== undefined) set.add(err);
      }
    }
    throw err;
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

  // Run resolve hooks if registered (and not already inside a hook)
  if (hookEntries.length > 0 && !insideResolveHook) {
    const parentURL = parent?.filename
      ? url.pathToFileURL(parent.filename).href
      : undefined;
    const context = {
      conditions: ["node", "require"],
      importAttributes: { __proto__: null },
      parentURL,
      importAssertions: { __proto__: null },
    };
    const result = executeResolveHookChain(request, context, parent, isMain);
    if (result != null && result.url != null) {
      if (StringPrototypeStartsWith(result.url, "file://")) {
        try {
          return url.fileURLToPath(result.url);
        } catch {
          // Virtual file:// URLs may not have valid OS paths (e.g.
          // file:///virtual.js on Windows). Return the URL as-is and
          // let the load hook handle it.
          return result.url;
        }
      }
      // node: and other schemes returned as-is
      return result.url;
    }
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
  // When load hooks are active, the file may not exist on disk (virtual
  // modules), so we fall back to the original filename.
  let hasLoadHooks = false;
  if (hookEntries.length > 0 && !insideLoadHook) {
    for (let i = 0; i < hookEntries.length; i++) {
      if (hookEntries[i].load !== null) {
        hasLoadHooks = true;
        break;
      }
    }
  }
  if (hasLoadHooks) {
    try {
      this.filename = op_require_real_path(filename);
    } catch {
      this.filename = filename;
    }
  } else {
    this.filename = op_require_real_path(filename);
  }
  this.paths = Module._nodeModulePaths(pathDirname(this.filename));

  // Run load hooks if registered
  if (hasLoadHooks) {
    {
      let fileUrl;
      if (StringPrototypeStartsWith(this.filename, "node:")) {
        fileUrl = this.filename;
      } else if (
        StringPrototypeStartsWith(this.filename, "file://") ||
        StringPrototypeIncludes(this.filename, "://")
      ) {
        // Already a URL (e.g. from a resolve hook returning a virtual URL)
        fileUrl = this.filename;
      } else {
        fileUrl = url.pathToFileURL(this.filename).href;
      }
      const context = {
        format: undefined,
        conditions: ["node", "require"],
        importAttributes: { __proto__: null },
        importAssertions: { __proto__: null },
      };
      insideLoadHook = true;
      let result;
      try {
        result = executeLoadHookChain(fileUrl, context);
      } finally {
        insideLoadHook = false;
      }
      // When shortCircuit is set, validate source type strictly
      if (result != null && result.shortCircuit && result.source != null) {
        const src = result.source;
        if (
          typeof src !== "string" &&
          !ArrayBuffer.isView(src) &&
          !(src instanceof ArrayBuffer)
        ) {
          const err = new TypeError(
            `Expected a string, an ArrayBuffer, or a TypedArray to be returned for the "source" from the "load" hook but got ${
              src === null ? "null" : `type ${typeof src}`
            }.`,
          );
          err.code = "ERR_INVALID_RETURN_PROPERTY_VALUE";
          throw err;
        }
      }
      // When shortCircuit is set with null/undefined source, error
      // unless the format is "builtin" (builtins legitimately have no source)
      if (
        result != null && result.shortCircuit &&
        result.format !== "builtin" &&
        (result.source === null || result.source === undefined)
      ) {
        const err = new TypeError(
          `Expected a string, an ArrayBuffer, or a TypedArray to be returned for the "source" from the "load" hook but got ${
            result.source === null ? "null" : "type undefined"
          }.`,
        );
        err.code = "ERR_INVALID_RETURN_PROPERTY_VALUE";
        throw err;
      }
      if (result != null && result.source != null) {
        const format = result.format;
        if (format === "module") {
          loadESMFromCJSWithHookSource(this, this.filename, result.source);
        } else if (format === "commonjs") {
          this._compile(
            typeof result.source === "string"
              ? result.source
              : (utf8Decoder ??= new TextDecoder()).decode(result.source),
            this.filename,
            "commonjs",
          );
        } else if (format === "json") {
          try {
            this.exports = JSONParse(
              stripBOM(
                typeof result.source === "string"
                  ? result.source
                  : (utf8Decoder ??= new TextDecoder()).decode(result.source),
              ),
            );
          } catch (err) {
            err.message = this.filename + ": " + err.message;
            throw err;
          }
        } else {
          // Default: try CJS first, fall back to ESM if the source
          // contains ESM syntax. We handle ESM fallback here (rather
          // than in _compile) so we can use op_import_sync_with_source
          // which bypasses the module cache for hook-provided source.
          const source = typeof result.source === "string"
            ? result.source
            : (utf8Decoder ??= new TextDecoder()).decode(result.source);
          try {
            this._compile(source, this.filename, "commonjs");
          } catch (err) {
            if (
              err instanceof SyntaxError &&
              op_require_can_parse_as_esm(source)
            ) {
              loadESMFromCJSWithHookSource(this, this.filename, source);
            } else {
              throw err;
            }
          }
        }
        this.loaded = true;
        return;
      }
    }
  }

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

function _throwIfEsmCycle(cachedModule, parent) {
  const fn = cachedModule.filename;
  if (
    fn != null &&
    (StringPrototypeEndsWith(fn, ".mjs") ||
      (StringPrototypeEndsWith(fn, ".js") &&
        op_require_is_maybe_cjs(fn) === false))
  ) {
    const parentPath = parent?.filename ?? "<unknown>";
    throw new internalErrors.ERR_REQUIRE_CYCLE_MODULE(fn, parentPath);
  }
}

function _throwRequireAsyncModule(specifier, module) {
  const parent = module?.parent?.filename ?? "<unknown>";
  throw new internalErrors.ERR_REQUIRE_ASYNC_MODULE(specifier, parent);
}

// Like loadESMFromCJS but uses op_import_sync_with_source to compile
// source directly. Used for hook-provided source that must bypass the
// module cache while preserving the correct import.meta.url.
function loadESMFromCJSWithHookSource(module, filename, code) {
  const specifier = url.pathToFileURL(filename).toString();
  const src = typeof code === "string"
    ? code
    : (utf8Decoder ??= new TextDecoder()).decode(code);
  let namespace;
  try {
    namespace = op_import_sync_with_source(specifier, src);
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

function loadESMFromCJS(module, filename, code) {
  const specifier = url.pathToFileURL(filename).toString();
  const codeArg = code !== undefined
    ? (typeof code === "string"
      ? code
      : (utf8Decoder ??= new TextDecoder()).decode(code))
    : undefined;
  let namespace;
  try {
    namespace = op_import_sync(specifier, codeArg);
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

function loadNativeModule(_id, request) {
  if (nativeModulePolyfill.has(request)) {
    return nativeModulePolyfill.get(request);
  }
  const modExports = nativeModuleExports[request];
  if (modExports) {
    if (request === "_tls_common") {
      process.emitWarning(
        "The _tls_common module is deprecated. Use `node:tls` instead.",
        "DeprecationWarning",
        "DEP0192",
      );
    }
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

/**
 * Register synchronous module loader hooks.
 * @param {{ resolve?: Function, load?: Function }} hooks
 * @returns {{ deregister: () => void }}
 */
export function registerHooks(hooks) {
  if (typeof hooks !== "object" || hooks === null) {
    throw new internalErrors.ERR_INVALID_ARG_TYPE("hooks", "object", hooks);
  }
  const resolve = typeof hooks.resolve === "function" ? hooks.resolve : null;
  const load = typeof hooks.load === "function" ? hooks.load : null;
  if (resolve === null && load === null) {
    throw new internalErrors.ERR_INVALID_ARG_VALUE(
      "hooks",
      hooks,
      "must contain at least one of 'resolve' or 'load'",
    );
  }
  const entry = { resolve, load };
  ArrayPrototypePush(hookEntries, entry);

  // Activate ESM hooks in Rust module loader
  _activateEsmHooks();

  return {
    deregister() {
      const idx = ArrayPrototypeIndexOf(hookEntries, entry);
      if (idx !== -1) {
        ArrayPrototypeSplice(hookEntries, idx, 1);
      }
      // Update Rust-side active flags
      _activateEsmHooks();
    },
  };
}

Module.registerHooks = registerHooks;

/**
 * @param {string | URL} specifier
 * @param {string | URL | { parentURL?: string | URL, data?: any, transferList?: any[] }} [parentUrlOrOptions]
 * @param {{ parentURL?: string | URL, data?: any, transferList?: any[] }} [maybeOptions]
 */
export function register(specifier, parentUrlOrOptions, maybeOptions) {
  if (typeof specifier !== "string" && !(specifier instanceof URL)) {
    throw new TypeError("specifier must be a string or URL");
  }

  // Parse overloaded arguments:
  // register(specifier)
  // register(specifier, parentURL)
  // register(specifier, options)
  // register(specifier, parentURL, options)
  let parentURL;
  let options;
  if (
    typeof parentUrlOrOptions === "string" ||
    parentUrlOrOptions instanceof URL
  ) {
    parentURL = String(parentUrlOrOptions);
    options = maybeOptions || {};
  } else if (
    typeof parentUrlOrOptions === "object" && parentUrlOrOptions !== null
  ) {
    options = parentUrlOrOptions;
    parentURL = options.parentURL != null
      ? String(options.parentURL)
      : undefined;
  } else {
    options = {};
  }

  const data = options.data;
  const transferList = options.transferList;

  // Resolve the specifier to a URL
  let resolvedUrl;
  if (
    typeof specifier === "string" && !specifier.startsWith("file://") &&
    !specifier.startsWith("data:") && !specifier.startsWith("node:")
  ) {
    // Relative or bare specifier - resolve against parentURL
    const base = parentURL || "data:";
    try {
      resolvedUrl = new URL(specifier, base).href;
    } catch {
      resolvedUrl = specifier;
    }
  } else {
    resolvedUrl = String(specifier);
  }

  // Load the hook module in the hooks worker thread. This avoids
  // deadlocks because the worker has its own module loader that
  // doesn't go through hooks.
  _ensureHooksWorker();

  const loadPromise = _sendToHooksWorker({
    type: "register",
    url: resolvedUrl,
    data,
  }, transferList).then((msg) => {
    if (msg.hasResolve) asyncHooksHaveResolve = true;
    if (msg.hasLoad) asyncHooksHaveLoad = true;
    _activateEsmHooks();
  });

  ArrayPrototypePush(pendingHookLoads, loadPromise);
  const removePending = () => {
    const idx = ArrayPrototypeIndexOf(pendingHookLoads, loadPromise);
    if (idx !== -1) ArrayPrototypeSplice(pendingHookLoads, idx, 1);
  };
  loadPromise.then(removePending, removePending);

  // Pre-activate hooks so subsequent imports are routed through the
  // bridge and will wait for the hook module to load in the worker.
  op_module_hooks_register(true, true);
  _startEsmResolveLoop();
  _startEsmLoadLoop();

  return undefined;
}

Module.register = register;
Module.SourceMap = SourceMap;

/**
 * Register loader hooks from --experimental-loader CLI flag.
 * Eagerly imports each loader module (so top-level errors crash the process
 * like Node.js), then registers resolve/load hooks via the async hook system.
 * @param {string[]} loaderUrls
 */
export async function _registerCliLoaders(loaderUrls) {
  _ensureHooksWorker();
  // Temporarily ref the worker during registration so run_event_loop
  // stays alive to process messages (called from execute_script context).
  _refHooksWorker(true);
  for (let i = 0; i < loaderUrls.length; i++) {
    const loaderUrl = loaderUrls[i];
    try {
      const msg = await _sendToHooksWorker({
        type: "register",
        url: loaderUrl,
        data: undefined,
      });
      if (msg.hasResolve) asyncHooksHaveResolve = true;
      if (msg.hasLoad) asyncHooksHaveLoad = true;
    } catch (e) {
      // Match Node.js behavior: loader errors crash the process.
      // The error message is already formatted by the worker.
      process.stderr.write((e?.message || String(e)) + "\n");
      process.exit(1);
    }
  }
  _activateEsmHooks();
  // Unref now that registration is done
  _refHooksWorker(false);
}

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

    // FIXME(bartlomieju): not nice to depend on `Deno` namespace here
    // but it's the only way to get `args` and `version` and this point.
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
    // node:cluster worker state is initialized only when NODE_UNIQUE_ID was
    // present in the environment at process startup. The Rust side reads the
    // env var (see `BootstrapOptions::node_cluster_unique_id`) and passes the
    // value through, so plain `deno run` invocations never touch
    // `Deno.permissions`/`Deno.env` here and never load `node:cluster`.
    if (nodeClusterUniqueId) {
      // Force loading the cluster polyfill so it can register
      // `internals.__initCluster`.
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
    // Warm up the process module
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
  // Close all idle connections in Node.js HTTP Agent pools.
  // This is called by the test runner before sanitizer checks to prevent
  // false positive resource leak detection for pooled keepAlive connections.
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
