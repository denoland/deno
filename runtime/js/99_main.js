// Copyright 2018-2025 the Deno authors. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import * as internalConsole from "ext:deno_web/01_console.js";
import { core, internals, primordials } from "ext:core/mod.js";
const ops = core.ops;
import {
  op_bootstrap_args,
  op_bootstrap_is_from_unconfigured_runtime,
  op_bootstrap_no_color,
  op_bootstrap_pid,
  op_bootstrap_stderr_no_color,
  op_bootstrap_stdout_no_color,
  op_get_ext_import_meta_proto,
  op_internal_log,
  op_main_module,
  op_ppid,
  op_set_format_exception_callback,
  op_snapshot_options,
  op_worker_close,
  op_worker_get_type,
  op_worker_post_message,
  op_worker_recv_message,
  op_worker_sync_fetch,
} from "ext:core/ops";
const {
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  Error,
  ErrorPrototype,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  PromisePrototypeThen,
  PromiseResolve,
  StringPrototypePadEnd,
  Symbol,
  SymbolDispose,
  SymbolIterator,
  TypeError,
} = primordials;
const {
  isNativeError,
} = core;
import { registerDeclarativeServer } from "ext:deno_http/00_serve.ts";
import * as event from "ext:deno_web/02_event.js";
import * as location from "ext:deno_web/12_location.js";
import * as version from "ext:runtime/01_version.ts";
import * as os from "ext:deno_os/30_os.js";
import * as timers from "ext:deno_web/02_timers.js";
import {
  getConsoleInspectOptions,
  getDefaultInspectOptions,
  getStderrNoColor,
  inspectArgs,
  quoteString,
  setNoColorFns,
} from "ext:deno_web/01_console.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as url from "ext:deno_web/00_url.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import {
  denoNs,
  denoNsUnstableById,
  unstableIds,
} from "ext:runtime/90_deno_ns.js";
import { errors } from "ext:runtime/01_errors.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import {
  unstableForWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
} from "ext:runtime/98_global_scope_shared.js";
import {
  mainRuntimeGlobalProperties,
  memoizeLazy,
} from "ext:runtime/98_global_scope_window.js";
import {
  workerRuntimeGlobalProperties,
} from "ext:runtime/98_global_scope_worker.js";
import { SymbolMetadata } from "ext:deno_web/00_infra.js";
import { bootstrap as bootstrapOtel } from "ext:deno_telemetry/telemetry.ts";
import { nodeGlobals } from "ext:deno_node/00_globals.js";

// deno-lint-ignore prefer-primordials
if (Symbol.metadata) {
  throw "V8 supports Symbol.metadata now, no need to shim it";
}

ObjectDefineProperties(Symbol, {
  dispose: {
    __proto__: null,
    value: SymbolDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
  metadata: {
    __proto__: null,
    value: SymbolMetadata,
    enumerable: false,
    writable: false,
    configurable: false,
  },
});

internals.isFromUnconfiguredRuntime = op_bootstrap_is_from_unconfigured_runtime;

// https://docs.rs/log/latest/log/enum.Level.html
const LOG_LEVELS = {
  error: 1,
  warn: 2,
  info: 3,
  debug: 4,
  trace: 5,
};

op_get_ext_import_meta_proto().log = function internalLog(levelStr, ...args) {
  const level = LOG_LEVELS[levelStr];
  const message = inspectArgs(
    args,
    getConsoleInspectOptions(getStderrNoColor()),
  );
  op_internal_log(this.url, level, message);
};

let windowIsClosing = false;
let globalThis_;

function windowClose() {
  if (!windowIsClosing) {
    windowIsClosing = true;
    // Push a macrotask to exit after a promise resolve.
    // This is not perfect, but should be fine for first pass.
    PromisePrototypeThen(
      PromiseResolve(),
      () =>
        FunctionPrototypeCall(timers.setTimeout, null, () => {
          // This should be fine, since only Window/MainWorker has .close()
          os.exit(0);
        }, 0),
    );
  }
}

function workerClose() {
  if (isClosing) {
    return;
  }

  isClosing = true;
  op_worker_close();
}

function postMessage(message, transferOrOptions = { __proto__: null }) {
  const prefix =
    "Failed to execute 'postMessage' on 'DedicatedWorkerGlobalScope'";
  webidl.requiredArguments(arguments.length, 1, prefix);
  message = webidl.converters.any(message);
  let options;
  if (
    webidl.type(transferOrOptions) === "Object" &&
    transferOrOptions !== undefined &&
    transferOrOptions[SymbolIterator] !== undefined
  ) {
    const transfer = webidl.converters["sequence<object>"](
      transferOrOptions,
      prefix,
      "Argument 2",
    );
    options = { transfer };
  } else {
    options = webidl.converters.StructuredSerializeOptions(
      transferOrOptions,
      prefix,
      "Argument 2",
    );
  }
  const { transfer } = options;
  const data = messagePort.serializeJsMessageData(message, transfer);
  op_worker_post_message(data);
}

let isClosing = false;
let globalDispatchEvent;
let closeOnIdle;

function hasMessageEventListener() {
  // the function name is kind of a misnomer, but we want to behave
  // as if we have message event listeners if a node message port is explicitly
  // refed (and the inverse as well)
  return (event.listenerCount(globalThis, "message") > 0 &&
    !globalThis[messagePort.unrefParentPort]) ||
    messagePort.refedMessagePortsCount > 0;
}

async function pollForMessages() {
  if (!globalDispatchEvent) {
    globalDispatchEvent = FunctionPrototypeBind(
      globalThis.dispatchEvent,
      globalThis,
    );
  }
  while (!isClosing) {
    const recvMessage = op_worker_recv_message();
    // In a Node.js worker, unref() the op promise to prevent it from
    // keeping the event loop alive. This avoids the need to explicitly
    // call self.close() or worker.terminate().
    if (closeOnIdle) {
      core.unrefOpPromise(recvMessage);
    }
    const data = await recvMessage;
    // const data = await op_worker_recv_message();
    if (data === null) break;
    const v = messagePort.deserializeJsMessageData(data);
    const message = v[0];
    const transferables = v[1];

    const msgEvent = new event.MessageEvent("message", {
      cancelable: false,
      data: message,
      ports: ArrayPrototypeFilter(
        transferables,
        (t) =>
          ObjectPrototypeIsPrototypeOf(messagePort.MessagePortPrototype, t),
      ),
    });
    event.setIsTrusted(msgEvent, true);

    try {
      globalDispatchEvent(msgEvent);
    } catch (e) {
      const errorEvent = new event.ErrorEvent("error", {
        cancelable: true,
        message: e.message,
        lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
        colno: e.columnNumber ? e.columnNumber + 1 : undefined,
        filename: e.fileName,
        error: e,
      });

      event.setIsTrusted(errorEvent, true);
      globalDispatchEvent(errorEvent);
      if (!errorEvent.defaultPrevented) {
        throw e;
      }
    }
  }
}

let loadedMainWorkerScript = false;

function importScripts(...urls) {
  if (op_worker_get_type() !== "classic") {
    throw new TypeError("Cannot import scripts in a module worker");
  }

  const baseUrl = location.getLocationHref();
  const parsedUrls = ArrayPrototypeMap(urls, (scriptUrl) => {
    try {
      return new url.URL(scriptUrl, baseUrl ?? undefined).href;
    } catch {
      throw new DOMException(
        `Failed to parse URL: ${scriptUrl}`,
        "SyntaxError",
      );
    }
  });

  // A classic worker's main script has looser MIME type checks than any
  // imported scripts, so we use `loadedMainWorkerScript` to distinguish them.
  // TODO(andreubotella) Refactor worker creation so the main script isn't
  // loaded with `importScripts()`.
  const scripts = op_worker_sync_fetch(
    parsedUrls,
    !loadedMainWorkerScript,
  );
  loadedMainWorkerScript = true;

  for (let i = 0; i < scripts.length; ++i) {
    const { url, script } = scripts[i];
    const err = core.evalContext(script, url)[1];
    if (err !== null) {
      throw err.thrown;
    }
  }
}

const opArgs = memoizeLazy(() => op_bootstrap_args());
const opPid = memoizeLazy(() => op_bootstrap_pid());
setNoColorFns(
  () => op_bootstrap_stdout_no_color(),
  () => op_bootstrap_stderr_no_color(),
);

function formatException(error) {
  if (
    isNativeError(error) ||
    ObjectPrototypeIsPrototypeOf(ErrorPrototype, error)
  ) {
    return null;
  } else if (typeof error == "string") {
    return `Uncaught ${
      inspectArgs([quoteString(error, getDefaultInspectOptions())], {
        colors: !getStderrNoColor(),
      })
    }`;
  } else {
    return `Uncaught ${inspectArgs([error], { colors: !getStderrNoColor() })}`;
  }
}

core.registerErrorClass("NotFound", errors.NotFound);
core.registerErrorClass("ConnectionRefused", errors.ConnectionRefused);
core.registerErrorClass("ConnectionReset", errors.ConnectionReset);
core.registerErrorClass("ConnectionAborted", errors.ConnectionAborted);
core.registerErrorClass("NotConnected", errors.NotConnected);
core.registerErrorClass("AddrInUse", errors.AddrInUse);
core.registerErrorClass("AddrNotAvailable", errors.AddrNotAvailable);
core.registerErrorClass("BrokenPipe", errors.BrokenPipe);
core.registerErrorClass("PermissionDenied", errors.PermissionDenied);
core.registerErrorClass("AlreadyExists", errors.AlreadyExists);
core.registerErrorClass("InvalidData", errors.InvalidData);
core.registerErrorClass("TimedOut", errors.TimedOut);
core.registerErrorClass("WouldBlock", errors.WouldBlock);
core.registerErrorClass("WriteZero", errors.WriteZero);
core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
core.registerErrorClass("Http", errors.Http);
core.registerErrorClass("Busy", errors.Busy);
core.registerErrorClass("NotSupported", errors.NotSupported);
core.registerErrorClass("FilesystemLoop", errors.FilesystemLoop);
core.registerErrorClass("IsADirectory", errors.IsADirectory);
core.registerErrorClass("NetworkUnreachable", errors.NetworkUnreachable);
core.registerErrorClass("NotADirectory", errors.NotADirectory);
core.registerErrorBuilder(
  "DOMExceptionOperationError",
  function DOMExceptionOperationError(msg) {
    return new DOMException(msg, "OperationError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionQuotaExceededError",
  function DOMExceptionQuotaExceededError(msg) {
    return new DOMException(msg, "QuotaExceededError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionNotSupportedError",
  function DOMExceptionNotSupportedError(msg) {
    return new DOMException(msg, "NotSupported");
  },
);
core.registerErrorBuilder(
  "DOMExceptionNetworkError",
  function DOMExceptionNetworkError(msg) {
    return new DOMException(msg, "NetworkError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionAbortError",
  function DOMExceptionAbortError(msg) {
    return new DOMException(msg, "AbortError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionInvalidCharacterError",
  function DOMExceptionInvalidCharacterError(msg) {
    return new DOMException(msg, "InvalidCharacterError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionDataError",
  function DOMExceptionDataError(msg) {
    return new DOMException(msg, "DataError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionInvalidStateError",
  function DOMExceptionInvalidStateError(msg) {
    return new DOMException(msg, "InvalidStateError");
  },
);

function runtimeStart(
  denoVersion,
  v8Version,
  tsVersion,
  target,
) {
  core.setWasmStreamingCallback(fetch.handleWasmStreaming);
  core.setReportExceptionCallback(event.reportException);
  op_set_format_exception_callback(formatException);
  version.setVersions(
    denoVersion,
    v8Version,
    tsVersion,
  );
  core.setBuildInfo(target);
}

core.setUnhandledPromiseRejectionHandler(processUnhandledPromiseRejection);
core.setHandledPromiseRejectionHandler(processRejectionHandled);

// Notification that the core received an unhandled promise rejection that is about to
// terminate the runtime. If we can handle it, attempt to do so.
function processUnhandledPromiseRejection(promise, reason) {
  const rejectionEvent = new event.PromiseRejectionEvent(
    "unhandledrejection",
    {
      cancelable: true,
      promise,
      reason,
    },
  );

  // Note that the handler may throw, causing a recursive "error" event
  globalThis_.dispatchEvent(rejectionEvent);

  // If event was not yet prevented, try handing it off to Node compat layer
  // (if it was initialized)
  if (
    !rejectionEvent.defaultPrevented &&
    typeof internals.nodeProcessUnhandledRejectionCallback !== "undefined"
  ) {
    internals.nodeProcessUnhandledRejectionCallback(rejectionEvent);
  }

  // If event was not prevented (or "unhandledrejection" listeners didn't
  // throw) we will let Rust side handle it.
  if (rejectionEvent.defaultPrevented) {
    return true;
  }

  return false;
}

function processRejectionHandled(promise, reason) {
  const rejectionHandledEvent = new event.PromiseRejectionEvent(
    "rejectionhandled",
    { promise, reason },
  );

  // Note that the handler may throw, causing a recursive "error" event
  globalThis_.dispatchEvent(rejectionHandledEvent);

  if (typeof internals.nodeProcessRejectionHandledCallback !== "undefined") {
    internals.nodeProcessRejectionHandledCallback(rejectionHandledEvent);
  }
}

function dispatchLoadEvent() {
  globalThis_.dispatchEvent(new event.Event("load"));
}

function dispatchBeforeUnloadEvent() {
  return globalThis_.dispatchEvent(
    new event.Event("beforeunload", { cancelable: true }),
  );
}

function dispatchUnloadEvent() {
  globalThis_.dispatchEvent(new event.Event("unload"));
}

let hasBootstrapped = false;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);

// Set up global properties shared by main and worker runtime that are exposed
// by unstable features if those are enabled.
function exposeUnstableFeaturesForWindowOrWorkerGlobalScope(unstableFeatures) {
  const featureIds = ArrayPrototypeMap(
    ObjectKeys(
      unstableForWindowOrWorkerGlobalScope,
    ),
    (k) => k | 0,
  );

  for (let i = 0; i <= featureIds.length; i++) {
    const featureId = featureIds[i];
    if (ArrayPrototypeIncludes(unstableFeatures, featureId)) {
      const props = unstableForWindowOrWorkerGlobalScope[featureId];
      ObjectDefineProperties(globalThis, { ...props });
    }
  }
}

// NOTE(bartlomieju): remove all the ops that have already been imported using
// "virtual op module" (`ext:core/ops`).
const NOT_IMPORTED_OPS = [
  // Related to `Deno.bench()` API
  "op_bench_now",
  "op_dispatch_bench_event",
  "op_register_bench",
  "op_bench_get_origin",

  // Related to `Deno.jupyter` API
  "op_jupyter_broadcast",
  "op_jupyter_input",
  "op_jupyter_create_png_from_texture",
  "op_jupyter_get_buffer",
  // Used in jupyter API
  "op_base64_encode",

  // Used in the lint API
  "op_lint_report",
  "op_lint_get_source",
  "op_lint_create_serialized_ast",
  "op_is_cancelled",

  // Related to `Deno.test()` API
  "op_test_event_step_result_failed",
  "op_test_event_step_result_ignored",
  "op_test_event_step_result_ok",
  "op_test_event_step_wait",
  "op_test_op_sanitizer_collect",
  "op_test_op_sanitizer_finish",
  "op_test_op_sanitizer_get_async_message",
  "op_test_op_sanitizer_report",
  "op_restore_test_permissions",
  "op_register_test_step",
  "op_register_test_hook",
  "op_register_test",
  "op_test_get_origin",
  "op_pledge_test_permissions",

  // TODO(bartlomieju): used in various integration tests - figure out a way
  // to not depend on them.
  "op_set_exit_code",
  "op_napi_open",

  // deno deploy subcommand
  "op_deploy_token_get",
  "op_deploy_token_set",
  "op_deploy_token_delete",
];

function removeImportedOps() {
  const allOpNames = ObjectKeys(ops);
  for (let i = 0; i < allOpNames.length; i++) {
    const opName = allOpNames[i];
    if (!ArrayPrototypeIncludes(NOT_IMPORTED_OPS, opName)) {
      delete ops[opName];
    }
  }
}

// FIXME(bartlomieju): temporarily add whole `Deno.core` to
// `Deno[Deno.internal]` namespace. It should be removed and only necessary
// methods should be left there.
ObjectAssign(internals, { core, nodeGlobals: { ...nodeGlobals } });
const internalSymbol = Symbol("Deno.internal");
const finalDenoNs = {
  internal: internalSymbol,
  [internalSymbol]: internals,
  ...denoNs,
  // Deno.test, Deno.bench, Deno.lint are noops here, but kept for compatibility; so
  // that they don't cause errors when used outside of `deno test`/`deno bench`/`deno lint`
  // contexts.
  test: () => {},
  bench: () => {},
  lint: {
    runPlugin: () => {
      throw new Error(
        "`Deno.lint.runPlugin` is only available in `deno test` subcommand.",
      );
    },
  },
};

ObjectDefineProperties(finalDenoNs, {
  pid: core.propGetterOnly(opPid),
  // `ppid` should not be memoized.
  // https://github.com/denoland/deno/issues/23004
  ppid: core.propGetterOnly(() => op_ppid()),
  noColor: core.propGetterOnly(() => op_bootstrap_no_color()),
  args: core.propGetterOnly(opArgs),
  mainModule: core.propGetterOnly(() => op_main_module()),
  exitCode: {
    __proto__: null,
    get() {
      return os.getExitCode();
    },
    set(value) {
      os.setExitCode(value);
    },
  },
});

const {
  tsVersion,
  v8Version,
  target,
} = op_snapshot_options();

const executionModes = {
  none: 0,
  worker: 1,
  run: 2,
  repl: 3,
  eval: 4,
  test: 5,
  bench: 6,
  serve: 7,
  jupyter: 8,
};

function bootstrapMainRuntime(runtimeOptions, warmup = false) {
  if (!warmup) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }

    const {
      0: denoVersion,
      1: location_,
      2: unstableFeatures,
      3: inspectFlag,
      5: hasNodeModulesDir,
      6: argv0,
      7: nodeDebug,
      8: mode,
      9: servePort,
      10: serveHost,
      11: serveIsMain,
      12: serveWorkerCountOrIndex,
      13: otelConfig,
      15: standalone,
      16: autoServe,
    } = runtimeOptions;

    denoNs.build.standalone = standalone;

    let serveIsMain_ = serveIsMain;
    let serveWorkerCountOrIndex_ = serveWorkerCountOrIndex;
    if (autoServe) {
      serveIsMain_ = true;
      serveWorkerCountOrIndex_ = 0;
    }

    if (mode === executionModes.serve || autoServe) {
      const hasMultipleThreads = serveIsMain_
        ? serveWorkerCountOrIndex_ > 0 // count > 0
        : true;
      if (hasMultipleThreads) {
        const serveLogIndex = serveIsMain_ ? 0 : (serveWorkerCountOrIndex_ + 1);
        const base = `serve-worker-${serveLogIndex}`;
        // 15 = "serve-worker-nn".length, assuming
        // serveWorkerCount < 100
        const prefix = `[${StringPrototypePadEnd(base, 15, " ")}]`;
        // deno-lint-ignore no-global-assign
        console = new internalConsole.Console((msg, level) =>
          core.print(`${prefix} ` + msg, level > 1)
        );
      }
    }

    if (mode === executionModes.run || mode === executionModes.serve) {
      let serve = undefined;
      core.addMainModuleHandler((main) => {
        if (ObjectHasOwn(main, "default")) {
          try {
            serve = registerDeclarativeServer(main.default);
          } catch (e) {
            if (mode === executionModes.serve || autoServe) {
              throw e;
            }
          }
        }

        if (mode === executionModes.serve && !serve) {
          if (serveIsMain_) {
            // Only error if main worker
            import.meta.log(
              "error",
              `%cerror: %cdeno serve requires %cexport default { fetch }%c in the main module, did you mean to run \"deno run\"?`,
              "color: yellow;",
              "color: inherit;",
              "font-weight: bold;",
              "font-weight: normal;",
            );
          }
          return;
        }

        if (serve) {
          if (mode === executionModes.run && !autoServe) {
            import.meta.log(
              "error",
              `%cwarning: %cDetected %cexport default { fetch }%c, did you mean to run \"deno serve\"?`,
              "color: yellow;",
              "color: inherit;",
              "font-weight: bold;",
              "font-weight: normal;",
            );
          }
          if (mode === executionModes.serve || autoServe) {
            serve({
              servePort,
              serveHost,
              workerCountWhenMain: serveIsMain_
                ? serveWorkerCountOrIndex_
                : undefined,
            });
          }
        }
      });
    }

    removeImportedOps();

    performance.setTimeOrigin();
    globalThis_ = globalThis;

    // Remove bootstrapping data from the global scope
    delete globalThis.__bootstrap;
    delete globalThis.bootstrap;
    hasBootstrapped = true;

    // If the `--location` flag isn't set, make `globalThis.location` `undefined` and
    // writable, so that they can mock it themselves if they like. If the flag was
    // set, define `globalThis.location`, using the provided value.
    if (location_ == null) {
      mainRuntimeGlobalProperties.location = {
        writable: true,
        configurable: true,
      };
    } else {
      location.setLocationHref(location_);
    }

    ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
    ObjectDefineProperties(globalThis, {
      // TODO(bartlomieju): in the future we might want to change the
      // behavior of setting `name` to actually update the process name.
      // Empty string matches what browsers do.
      name: core.propWritable(""),
      close: core.propWritable(windowClose),
      closed: core.propGetterOnly(() => windowIsClosing),
    });
    exposeUnstableFeaturesForWindowOrWorkerGlobalScope(unstableFeatures);
    ObjectSetPrototypeOf(globalThis, Window.prototype);

    bootstrapOtel(otelConfig);

    if (inspectFlag) {
      core.wrapConsole(globalThis.console, core.v8Console);
    }

    event.defineEventHandler(globalThis, "error");
    event.defineEventHandler(globalThis, "load");
    event.defineEventHandler(globalThis, "beforeunload");
    event.defineEventHandler(globalThis, "unload");

    runtimeStart(
      denoVersion,
      v8Version,
      tsVersion,
      target,
    );

    // TODO(bartlomieju): this is not ideal, but because we use `ObjectAssign`
    // above any properties that are defined elsewhere using `Object.defineProperty`
    // are lost.
    let jupyterNs = undefined;
    ObjectDefineProperty(finalDenoNs, "jupyter", {
      __proto__: null,
      get() {
        if (jupyterNs) {
          return jupyterNs;
        }
        throw new Error(
          "Deno.jupyter is only available in `deno jupyter` subcommand",
        );
      },
      set(val) {
        jupyterNs = val;
      },
    });

    for (let i = 0; i <= unstableFeatures.length; i++) {
      const id = unstableFeatures[i];
      ObjectAssign(finalDenoNs, denoNsUnstableById[id]);
    }

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.unsafeProto)) {
      // Removes the `__proto__` for security reasons.
      // https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
      delete Object.prototype.__proto__;
    }

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.temporal)) {
      // Removes the `Temporal` API.
      delete globalThis.Temporal;
      delete globalThis.Date.prototype.toTemporalInstant;
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", core.propReadOnly(finalDenoNs));

    if (nodeBootstrap) {
      nodeBootstrap({
        usesLocalNodeModulesDir: hasNodeModulesDir,
        runningOnMainThread: true,
        argv0,
        nodeDebug,
      });
    }
  } else {
    // Warmup
  }
}

function bootstrapWorkerRuntime(
  runtimeOptions,
  name,
  internalName,
  workerId,
  workerType,
  maybeWorkerMetadata,
  warmup = false,
) {
  if (!warmup) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }

    const {
      0: denoVersion,
      1: location_,
      2: unstableFeatures,
      4: enableTestingFeaturesFlag,
      5: hasNodeModulesDir,
      6: argv0,
      7: nodeDebug,
      13: otelConfig,
      15: standalone,
    } = runtimeOptions;

    denoNs.build.standalone = standalone;

    closeOnIdle = runtimeOptions[14];

    performance.setTimeOrigin();
    globalThis_ = globalThis;

    // Remove bootstrapping data from the global scope
    delete globalThis.__bootstrap;
    delete globalThis.bootstrap;
    hasBootstrapped = true;

    if (workerType === "node") {
      delete workerRuntimeGlobalProperties["WorkerGlobalScope"];
      delete workerRuntimeGlobalProperties["self"];
    }
    ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);
    ObjectDefineProperties(globalThis, {
      name: core.propWritable(name),
      // TODO(bartlomieju): should be readonly?
      close: core.propNonEnumerable(workerClose),
      postMessage: core.propWritable(postMessage),
    });
    if (enableTestingFeaturesFlag) {
      ObjectDefineProperty(
        globalThis,
        "importScripts",
        core.propWritable(importScripts),
      );
    }
    exposeUnstableFeaturesForWindowOrWorkerGlobalScope(unstableFeatures);
    ObjectSetPrototypeOf(globalThis, DedicatedWorkerGlobalScope.prototype);

    bootstrapOtel(otelConfig);

    core.wrapConsole(globalThis.console, core.v8Console);

    event.defineEventHandler(globalThis, "message");
    event.defineEventHandler(globalThis, "error", undefined, true);

    // `Deno.exit()` is an alias to `self.close()`. Setting and exit
    // code using an op in worker context is a no-op.
    os.setExitHandler((_exitCode) => {
      workerClose();
    });

    runtimeStart(
      denoVersion,
      v8Version,
      tsVersion,
      target,
      internalName ?? name,
    );

    location.setLocationHref(location_);

    globalThis.pollForMessages = pollForMessages;
    globalThis.hasMessageEventListener = hasMessageEventListener;

    for (let i = 0; i <= unstableFeatures.length; i++) {
      const id = unstableFeatures[i];
      ObjectAssign(finalDenoNs, denoNsUnstableById[id]);
    }

    // Not available in workers
    const moduleSpecifier = finalDenoNs.mainModule;
    delete finalDenoNs.mainModule;

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.unsafeProto)) {
      // Removes the `__proto__` for security reasons.
      // https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
      delete Object.prototype.__proto__;
    }

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.temporal)) {
      // Removes the `Temporal` API.
      delete globalThis.Temporal;
      delete globalThis.Date.prototype.toTemporalInstant;
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", core.propReadOnly(finalDenoNs));

    const workerMetadata = maybeWorkerMetadata
      ? messagePort.deserializeJsMessageData(maybeWorkerMetadata)
      : undefined;

    if (nodeBootstrap) {
      nodeBootstrap({
        usesLocalNodeModulesDir: hasNodeModulesDir,
        runningOnMainThread: false,
        argv0,
        workerId,
        maybeWorkerMetadata: workerMetadata,
        nodeDebug,
        moduleSpecifier: workerType === "node" ? moduleSpecifier : null,
      });
    }
  } else {
    // Warmup
    return;
  }
}

const nodeBootstrap = globalThis.nodeBootstrap;
delete globalThis.nodeBootstrap;
const dispatchProcessExitEvent = internals.dispatchProcessExitEvent;
delete internals.dispatchProcessExitEvent;
const dispatchProcessBeforeExitEvent = internals.dispatchProcessBeforeExitEvent;
delete internals.dispatchProcessBeforeExitEvent;

globalThis.bootstrap = {
  mainRuntime: bootstrapMainRuntime,
  workerRuntime: bootstrapWorkerRuntime,
  dispatchLoadEvent,
  dispatchUnloadEvent,
  dispatchBeforeUnloadEvent,
  dispatchProcessExitEvent,
  dispatchProcessBeforeExitEvent,
};

event.setEventTargetData(globalThis);
event.saveGlobalThisReference(globalThis);
event.defineEventHandler(globalThis, "unhandledrejection");

// Nothing listens to this, but it warms up the code paths for event dispatch
(new event.EventTarget()).dispatchEvent(new event.Event("warmup"));

removeImportedOps();

// Run the warmup path through node and runtime/worker bootstrap functions
bootstrapMainRuntime(undefined, true);
bootstrapWorkerRuntime(
  undefined,
  undefined,
  undefined,
  undefined,
  undefined,
  undefined,
  true,
);
nodeBootstrap({ warmup: true });
