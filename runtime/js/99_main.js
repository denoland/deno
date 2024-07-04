// deno-lint-ignore-file no-deprecated-deno-api
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import { core, internals, primordials } from "ext:core/mod.js";
const ops = core.ops;
import {
  op_bootstrap_args,
  op_bootstrap_is_stderr_tty,
  op_bootstrap_is_stdout_tty,
  op_bootstrap_no_color,
  op_bootstrap_pid,
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
  ArrayPrototypePop,
  ArrayPrototypeShift,
  DateNow,
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
  ObjectValues,
  PromisePrototypeThen,
  PromiseResolve,
  SafeSet,
  StringPrototypeIncludes,
  StringPrototypeSplit,
  StringPrototypeTrim,
  Symbol,
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
import * as os from "ext:runtime/30_os.js";
import * as timers from "ext:deno_web/02_timers.js";
import {
  customInspect,
  getDefaultInspectOptions,
  getStderrNoColor,
  inspectArgs,
  quoteString,
  setNoColorFns,
} from "ext:deno_console/01_console.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as url from "ext:deno_url/00_url.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import {
  denoNs,
  denoNsUnstable,
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
import { SymbolDispose, SymbolMetadata } from "ext:deno_web/00_infra.js";
// deno-lint-ignore prefer-primordials
if (Symbol.metadata) {
  throw "V8 supports Symbol.metadata now, no need to shim it!";
}
ObjectDefineProperties(Symbol, {
  dispose: {
    value: SymbolDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
  metadata: {
    value: SymbolMetadata,
    enumerable: false,
    writable: false,
    configurable: false,
  },
});

let windowIsClosing = false;
let globalThis_;

let verboseDeprecatedApiWarning = false;
let deprecatedApiWarningDisabled = false;
const ALREADY_WARNED_DEPRECATED = new SafeSet();

function warnOnDeprecatedApi(apiName, stack, ...suggestions) {
  if (deprecatedApiWarningDisabled) {
    return;
  }

  if (!verboseDeprecatedApiWarning) {
    if (ALREADY_WARNED_DEPRECATED.has(apiName)) {
      return;
    }
    ALREADY_WARNED_DEPRECATED.add(apiName);
    console.error(
      `%cwarning: %cUse of deprecated "${apiName}" API. This API will be removed in Deno 2. Run again with DENO_VERBOSE_WARNINGS=1 to get more details.`,
      "color: yellow;",
      "font-weight: bold;",
    );
    return;
  }

  if (ALREADY_WARNED_DEPRECATED.has(apiName + stack)) {
    return;
  }

  // If we haven't warned yet, let's do some processing of the stack trace
  // to make it more useful.
  const stackLines = StringPrototypeSplit(stack, "\n");
  ArrayPrototypeShift(stackLines);
  while (stackLines.length > 0) {
    // Filter out internal frames at the top of the stack - they are not useful
    // to the user.
    if (
      StringPrototypeIncludes(stackLines[0], "(ext:") ||
      StringPrototypeIncludes(stackLines[0], "(node:") ||
      StringPrototypeIncludes(stackLines[0], "<anonymous>")
    ) {
      ArrayPrototypeShift(stackLines);
    } else {
      break;
    }
  }
  // Now remove the last frame if it's coming from "ext:core" - this is most likely
  // event loop tick or promise handler calling a user function - again not
  // useful to the user.
  if (
    stackLines.length > 0 &&
    StringPrototypeIncludes(stackLines[stackLines.length - 1], "(ext:core/")
  ) {
    ArrayPrototypePop(stackLines);
  }

  let isFromRemoteDependency = false;
  const firstStackLine = stackLines[0];
  if (firstStackLine && !StringPrototypeIncludes(firstStackLine, "file:")) {
    isFromRemoteDependency = true;
  }

  ALREADY_WARNED_DEPRECATED.add(apiName + stack);
  console.error(
    `%cwarning: %cUse of deprecated "${apiName}" API. This API will be removed in Deno 2.`,
    "color: yellow;",
    "font-weight: bold;",
  );

  console.error();
  console.error(
    "See the Deno 1 to 2 Migration Guide for more information at https://docs.deno.com/runtime/manual/advanced/migrate_deprecations",
  );
  console.error();
  if (stackLines.length > 0) {
    console.error("Stack trace:");
    for (let i = 0; i < stackLines.length; i++) {
      console.error(`  ${StringPrototypeTrim(stackLines[i])}`);
    }
    console.error();
  }

  for (let i = 0; i < suggestions.length; i++) {
    const suggestion = suggestions[i];
    console.error(
      `%chint: ${suggestion}`,
      "font-weight: bold;",
    );
  }

  if (isFromRemoteDependency) {
    console.error(
      `%chint: It appears this API is used by a remote dependency. Try upgrading to the latest version of that dependency.`,
      "font-weight: bold;",
    );
  }
  console.error();
}

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

function hasMessageEventListener() {
  return event.listenerCount(globalThis, "message") > 0 ||
    messagePort.messageEventListenerCount > 0;
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
    if (globalThis[messagePort.unrefPollForMessages] === true) {
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
  if (op_worker_get_type() === "module") {
    throw new TypeError("Can't import scripts in a module worker.");
  }

  const baseUrl = location.getLocationHref();
  const parsedUrls = ArrayPrototypeMap(urls, (scriptUrl) => {
    try {
      return new url.URL(scriptUrl, baseUrl ?? undefined).href;
    } catch {
      throw new DOMException(
        "Failed to parse URL.",
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
  () => op_bootstrap_no_color() || !op_bootstrap_is_stdout_tty(),
  () => op_bootstrap_no_color() || !op_bootstrap_is_stderr_tty(),
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
core.registerErrorClass("PermissionDenied", errors.PermissionDenied);
core.registerErrorClass("ConnectionRefused", errors.ConnectionRefused);
core.registerErrorClass("ConnectionReset", errors.ConnectionReset);
core.registerErrorClass("ConnectionAborted", errors.ConnectionAborted);
core.registerErrorClass("NotConnected", errors.NotConnected);
core.registerErrorClass("AddrInUse", errors.AddrInUse);
core.registerErrorClass("AddrNotAvailable", errors.AddrNotAvailable);
core.registerErrorClass("BrokenPipe", errors.BrokenPipe);
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
  globalThis_.dispatchEvent(new Event("load"));
}

function dispatchBeforeUnloadEvent() {
  return globalThis_.dispatchEvent(
    new Event("beforeunload", { cancelable: true }),
  );
}

function dispatchUnloadEvent() {
  globalThis_.dispatchEvent(new Event("unload"));
}

let hasBootstrapped = false;
// Delete the `console` object that V8 automaticaly adds onto the global wrapper
// object on context creation. We don't want this console object to shadow the
// `console` object exposed by the ext/node globalThis proxy.
delete globalThis.console;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);

// Set up global properties shared by main and worker runtime that are exposed
// by unstable features if those are enabled.
function exposeUnstableFeaturesForWindowOrWorkerGlobalScope(options) {
  const { unstableFlag, unstableFeatures } = options;
  if (unstableFlag) {
    const all = ObjectValues(unstableForWindowOrWorkerGlobalScope);
    for (let i = 0; i <= all.length; i++) {
      const props = all[i];
      ObjectDefineProperties(globalThis, { ...props });
    }
  } else {
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
  "op_jupyter_comm_recv",
  "op_jupyter_comm_open",
  "op_jupyter_input",

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
  "op_register_test",
  "op_test_get_origin",
  "op_pledge_test_permissions",

  // TODO(bartlomieju): used in various integration tests - figure out a way
  // to not depend on them.
  "op_set_exit_code",
  "op_napi_open",
  "op_npm_process_state",
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
ObjectAssign(internals, { core, warnOnDeprecatedApi });
const internalSymbol = Symbol("Deno.internal");
const finalDenoNs = {
  internal: internalSymbol,
  [internalSymbol]: internals,
  resources() {
    internals.warnOnDeprecatedApi("Deno.resources()", new Error().stack);
    return core.resources();
  },
  close(rid) {
    internals.warnOnDeprecatedApi(
      "Deno.close()",
      new Error().stack,
      "Use `closer.close()` instead.",
    );
    core.close(rid);
  },
  ...denoNs,
  // Deno.test and Deno.bench are noops here, but kept for compatibility; so
  // that they don't cause errors when used outside of `deno test`/`deno bench`
  // contexts.
  test: () => {},
  bench: () => {},
};

ObjectDefineProperties(finalDenoNs, {
  pid: core.propGetterOnly(opPid),
  // `ppid` should not be memoized.
  // https://github.com/denoland/deno/issues/23004
  ppid: core.propGetterOnly(() => op_ppid()),
  noColor: core.propGetterOnly(() => op_bootstrap_no_color()),
  args: core.propGetterOnly(opArgs),
  mainModule: core.propGetterOnly(() => op_main_module()),
  // TODO(kt3k): Remove this export at v2
  // See https://github.com/denoland/deno/issues/9294
  customInspect: {
    get() {
      warnOnDeprecatedApi(
        "Deno.customInspect",
        new Error().stack,
        'Use `Symbol.for("Deno.customInspect")` instead.',
      );
      return internals.future ? undefined : customInspect;
    },
  },
  exitCode: {
    get() {
      return os.getExitCode();
    },
    set(value) {
      os.setExitCode(value);
    },
  },
});

const {
  denoVersion,
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
      0: location_,
      1: unstableFlag,
      2: unstableFeatures,
      3: inspectFlag,
      5: hasNodeModulesDir,
      6: argv0,
      7: nodeDebug,
      8: shouldDisableDeprecatedApiWarning,
      9: shouldUseVerboseDeprecatedApiWarning,
      10: future,
      11: mode,
      12: servePort,
      13: serveHost,
    } = runtimeOptions;

    if (mode === executionModes.run || mode === executionModes.serve) {
      let serve = undefined;
      core.addMainModuleHandler((main) => {
        if (ObjectHasOwn(main, "default")) {
          try {
            serve = registerDeclarativeServer(main.default);
          } catch (e) {
            if (mode === executionModes.serve) {
              throw e;
            }
          }
        }

        if (mode === executionModes.serve && !serve) {
          console.error(
            `%cerror: %cdeno serve requires %cexport default { fetch }%c in the main module, did you mean to run \"deno run\"?`,
            "color: yellow;",
            "color: inherit;",
            "font-weight: bold;",
            "font-weight: normal;",
          );
          return;
        }

        if (serve) {
          if (mode === executionModes.run) {
            console.error(
              `%cwarning: %cDetected %cexport default { fetch }%c, did you mean to run \"deno serve\"?`,
              "color: yellow;",
              "color: inherit;",
              "font-weight: bold;",
              "font-weight: normal;",
            );
          }
          if (mode === executionModes.serve) {
            serve({ servePort, serveHost });
          }
        }
      });
    }

    // TODO(iuioiua): remove in Deno v2. This allows us to dynamically delete
    // class properties within constructors for classes that are not defined
    // within the Deno namespace.
    internals.future = future;

    removeImportedOps();

    deprecatedApiWarningDisabled = shouldDisableDeprecatedApiWarning;
    verboseDeprecatedApiWarning = shouldUseVerboseDeprecatedApiWarning;
    performance.setTimeOrigin(DateNow());
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
      };
    } else {
      location.setLocationHref(location_);
    }

    exposeUnstableFeaturesForWindowOrWorkerGlobalScope({
      unstableFlag,
      unstableFeatures,
    });
    ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
    ObjectDefineProperties(globalThis, {
      // TODO(bartlomieju): in the future we might want to change the
      // behavior of setting `name` to actually update the process name.
      // Empty string matches what browsers do.
      name: core.propWritable(""),
      close: core.propWritable(windowClose),
      closed: core.propGetterOnly(() => windowIsClosing),
    });
    ObjectSetPrototypeOf(globalThis, Window.prototype);

    if (inspectFlag) {
      const consoleFromDeno = globalThis.console;
      core.wrapConsole(consoleFromDeno, core.v8Console);
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

    // TODO(bartlomieju): deprecate --unstable
    if (unstableFlag) {
      ObjectAssign(finalDenoNs, denoNsUnstable);
      // TODO(bartlomieju): this is not ideal, but because we use `ObjectAssign`
      // above any properties that are defined elsewhere using `Object.defineProperty`
      // are lost.
      let jupyterNs = undefined;
      ObjectDefineProperty(finalDenoNs, "jupyter", {
        get() {
          if (jupyterNs) {
            return jupyterNs;
          }
          throw new Error(
            "Deno.jupyter is only available in `deno jupyter` subcommand.",
          );
        },
        set(val) {
          jupyterNs = val;
        },
      });
    } else {
      for (let i = 0; i <= unstableFeatures.length; i++) {
        const id = unstableFeatures[i];
        ObjectAssign(finalDenoNs, denoNsUnstableById[id]);
      }
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
    if (future) {
      delete globalThis.window;
      delete Deno.Buffer;
      delete Deno.close;
      delete Deno.copy;
      delete Deno.File;
      delete Deno.fstat;
      delete Deno.fstatSync;
      delete Deno.ftruncate;
      delete Deno.ftruncateSync;
      delete Deno.flock;
      delete Deno.flockSync;
      delete Deno.FsFile.prototype.rid;
      delete Deno.funlock;
      delete Deno.funlockSync;
      delete Deno.iter;
      delete Deno.iterSync;
      delete Deno.metrics;
      delete Deno.readAll;
      delete Deno.readAllSync;
      delete Deno.read;
      delete Deno.readSync;
      delete Deno.resources;
      delete Deno.seek;
      delete Deno.seekSync;
      delete Deno.shutdown;
      delete Deno.writeAll;
      delete Deno.writeAllSync;
      delete Deno.write;
      delete Deno.writeSync;
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
  maybeWorkerMetadata,
  warmup = false,
) {
  if (!warmup) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }

    const {
      0: location_,
      1: unstableFlag,
      2: unstableFeatures,
      4: enableTestingFeaturesFlag,
      5: hasNodeModulesDir,
      6: argv0,
      7: nodeDebug,
      8: shouldDisableDeprecatedApiWarning,
      9: shouldUseVerboseDeprecatedApiWarning,
      10: future,
    } = runtimeOptions;

    // TODO(iuioiua): remove in Deno v2. This allows us to dynamically delete
    // class properties within constructors for classes that are not defined
    // within the Deno namespace.
    internals.future = future;

    deprecatedApiWarningDisabled = shouldDisableDeprecatedApiWarning;
    verboseDeprecatedApiWarning = shouldUseVerboseDeprecatedApiWarning;
    performance.setTimeOrigin(DateNow());
    globalThis_ = globalThis;

    // Remove bootstrapping data from the global scope
    delete globalThis.__bootstrap;
    delete globalThis.bootstrap;
    hasBootstrapped = true;

    exposeUnstableFeaturesForWindowOrWorkerGlobalScope({
      unstableFlag,
      unstableFeatures,
    });
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
    ObjectSetPrototypeOf(globalThis, DedicatedWorkerGlobalScope.prototype);

    const consoleFromDeno = globalThis.console;
    core.wrapConsole(consoleFromDeno, core.v8Console);

    event.defineEventHandler(self, "message");
    event.defineEventHandler(self, "error", undefined, true);

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

    // TODO(bartlomieju): deprecate --unstable
    if (unstableFlag) {
      ObjectAssign(finalDenoNs, denoNsUnstable);
    } else {
      for (let i = 0; i <= unstableFeatures.length; i++) {
        const id = unstableFeatures[i];
        ObjectAssign(finalDenoNs, denoNsUnstableById[id]);
      }
    }

    // Not available in workers
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
      });
    }

    if (future) {
      delete Deno.Buffer;
      delete Deno.close;
      delete Deno.copy;
      delete Deno.File;
      delete Deno.fstat;
      delete Deno.fstatSync;
      delete Deno.ftruncate;
      delete Deno.ftruncateSync;
      delete Deno.flock;
      delete Deno.flockSync;
      delete Deno.FsFile.prototype.rid;
      delete Deno.funlock;
      delete Deno.funlockSync;
      delete Deno.iter;
      delete Deno.iterSync;
      delete Deno.metrics;
      delete Deno.readAll;
      delete Deno.readAllSync;
      delete Deno.read;
      delete Deno.readSync;
      delete Deno.resources;
      delete Deno.seek;
      delete Deno.seekSync;
      delete Deno.shutdown;
      delete Deno.writeAll;
      delete Deno.writeAllSync;
      delete Deno.write;
      delete Deno.writeSync;
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
(new event.EventTarget()).dispatchEvent(new Event("warmup"));

removeImportedOps();

// Run the warmup path through node and runtime/worker bootstrap functions
bootstrapMainRuntime(undefined, true);
bootstrapWorkerRuntime(
  undefined,
  undefined,
  undefined,
  undefined,
  undefined,
  true,
);
nodeBootstrap({ warmup: true });
