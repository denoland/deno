// Copyright 2018-2026 the Deno authors. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

const internalConsole = core.loadExtScript("ext:deno_web/01_console.js");
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
  op_node_has_child_ipc_pipe,
  op_ppid,
  op_proto_get_attempted,
  op_proto_set_attempted,
  op_set_format_exception_callback,
  op_snapshot_options,
  op_worker_close,
  op_worker_get_type,
  op_worker_maybe_wait_for_debugger,
  op_worker_post_message,
  op_worker_post_message_raw,
  op_worker_recv_message,
  op_worker_recv_message_sync,
  op_worker_sync_fetch,
} from "ext:core/ops";
const {
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  Error,
  ErrorPrototype,
  FunctionPrototypeBind,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectGetOwnPropertyDescriptors,
  ObjectHasOwn,
  ObjectIsExtensible,
  ObjectKeys,
  ObjectPrototype,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  queueMicrotask,
  ReflectApply,
  StringPrototypePadEnd,
  Symbol,
  SymbolDispose,
  SymbolIterator,
  TypeError,
} = primordials;
const {
  isNativeError,
} = core;
// Deno.serve (00_serve.ts) chains through 23_request/23_response/22_body
// into the 208 KB web-streams polyfill. Only loaded if `deno serve` / a
// declarative server export is actually used.
let _serveMod;
const lazyServeMod = () =>
  _serveMod ??
    (_serveMod = core.loadExtScript("ext:deno_http/00_serve.ts"));
const event = core.loadExtScript("ext:deno_web/02_event.js");
const location = core.loadExtScript("ext:deno_web/12_location.js");
const version = core.loadExtScript("ext:runtime/01_version.ts");
const os = core.loadExtScript("ext:deno_os/30_os.js");
const {
  getConsoleInspectOptions,
  getDefaultInspectOptions,
  getStderrNoColor,
  inspectArgs,
  quoteString,
  setNoColorFns,
} = core.loadExtScript("ext:deno_web/01_console.js");
const performance = core.loadExtScript("ext:deno_web/15_performance.js");
const url = core.loadExtScript("ext:deno_web/00_url.js");
// 26_fetch pulls 22_body -> 06_streams (208 KB). The only thing 99_main
// needs from it at bootstrap is the wasm-streaming callback registration -
// wrap that so the actual module loads on first WebAssembly streaming use.
let _fetchMod;
const lazyFetchMod = () =>
  _fetchMod ??
    (_fetchMod = core.loadExtScript("ext:deno_fetch/26_fetch.js"));
const messagePort = core.loadExtScript("ext:deno_web/13_message_port.js");
import {
  denoNs,
  denoNsUnstableById,
  unstableIds,
} from "ext:runtime/90_deno_ns.js";
const { errors } = core.loadExtScript("ext:runtime/01_errors.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  DOMException,
  QuotaExceededError,
} = core.loadExtScript("ext:deno_web/01_dom_exception.js");
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
const { SymbolMetadata } = core.loadExtScript("ext:deno_web/00_infra.js");
// Telemetry (~2000 LOC, ~70 KB of bytecode + V8 heap) is only needed when an
// OTEL config flag is set, but it was being loaded unconditionally at snapshot
// build time. Skip the load on the cold-start path; only enter the module
// when the bootstrap config actually asks for telemetry.
//
// Layout of `otelConfig` (see OtelConfig::as_v8 in ext/telemetry/lib.rs):
//   [0]  tracing_enabled (0/1)
//   [1]  metrics_enabled (0/1)
//   [2]  console mode (OtelConsoleConfig: 0 = Ignore, 1 = Capture, 2 = Replace)
//   [3..] zero or more propagator ids; if no propagator is configured the
//         array ends at length 3.
function bootstrapOtel(otelConfig) {
  if (
    otelConfig[0] === 0 &&
    otelConfig[1] === 0 &&
    otelConfig[2] === 0 &&
    otelConfig.length <= 3
  ) {
    return;
  }
  const { bootstrap } = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
  bootstrap(otelConfig);
}

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

// Equivalent of import.meta.log for use in lazy-loaded (IIFE) scripts that
// lack access to import.meta.
internals.log = function internalLog(levelStr, ...args) {
  const level = LOG_LEVELS[levelStr];
  const message = inspectArgs(
    args,
    getConsoleInspectOptions(getStderrNoColor()),
  );
  op_internal_log("ext:runtime", level, message);
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
        core.createSystemTimer(
          () => {
            // This should be fine, since only Window/MainWorker has .close()
            os.exit(0);
          },
          0,
          true,
        ),
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
  // Fast path: no transferables
  if (
    transferOrOptions === undefined ||
    transferOrOptions === null ||
    (arguments.length <= 1)
  ) {
    op_worker_post_message_raw(
      messagePort.serializeMessageData(message, (err) => {
        throw new DOMException(err, "DataCloneError");
      }),
    );
    return;
  }
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

function dispatchWorkerMessage(data) {
  let message, transferables;
  try {
    const v = messagePort.deserializeJsMessageData(data);
    message = v[0];
    transferables = v[1];
  } catch (err) {
    const errorEvent = new event.MessageEvent("messageerror", {
      cancelable: false,
      data: err,
    });
    event.setIsTrusted(errorEvent, true);
    globalDispatchEvent(errorEvent);
    return;
  }

  const msgEvent = new event.MessageEvent("message", {
    cancelable: false,
    data: message,
    // Skip the transferables filter for the common no-transferables case.
    // Passing `undefined` lets the MessageEvent constructor take its cheap
    // `ports == null` branch (a single frozen empty array, no iterator
    // validation) instead of allocating a filtered array per message.
    ports: transferables.length === 0 ? undefined : ArrayPrototypeFilter(
      transferables,
      (t) => ObjectPrototypeIsPrototypeOf(messagePort.MessagePortPrototype, t),
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
    if (data === null) break;
    op_worker_maybe_wait_for_debugger();
    dispatchWorkerMessage(data);
    // Drain messages already queued on the host side instead of taking the
    // async op + Promise path for each. The whole burst is processed within
    // this event-loop turn; the batch limit prevents starving the event loop
    // under a sustained flood.
    for (let i = 0; i < 1000 && !isClosing; i++) {
      const syncData = op_worker_recv_message_sync();
      if (syncData === null) break;
      // Each message dispatch is its own task. Yield a microtask before
      // delivering this already-dequeued message so a handler that re-armed
      // itself in a microtask after the previous dispatch (e.g. reassigning
      // `onmessage` inside a `.then`) is installed first -- otherwise the
      // message reaches the stale handler and is lost. A synchronous
      // checkpoint can't help: V8 won't run microtasks reentrantly while we
      // are already inside one.
      await new Promise((resolve) => queueMicrotask(() => resolve()));
      if (isClosing) break;
      op_worker_maybe_wait_for_debugger();
      dispatchWorkerMessage(syncData);
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
    return new QuotaExceededError(msg);
  },
);
core.registerErrorBuilder(
  "DOMExceptionNotSupportedError",
  function DOMExceptionNotSupportedError(msg) {
    return new DOMException(msg, "NotSupportedError");
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
core.registerErrorBuilder(
  "DOMExceptionSyntaxError",
  function DOMExceptionSyntaxError(msg) {
    return new DOMException(msg, "SyntaxError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionIndexSizeError",
  function DOMExceptionIndexSizeError(msg) {
    return new DOMException(msg, "IndexSizeError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionTypeMismatchError",
  function DOMExceptionTypeMismatchError(msg) {
    return new DOMException(msg, "TypeMismatchError");
  },
);
core.registerErrorBuilder(
  "DOMExceptionInvalidAccessError",
  function DOMExceptionInvalidAccessError(msg) {
    return new DOMException(msg, "InvalidAccessError");
  },
);

function runtimeStart(
  denoVersion,
  v8Version,
  tsVersion,
  target,
) {
  core.setWasmStreamingCallback(function wasmStreamingCallback(source, rid) {
    const handleWasmStreaming = lazyFetchMod().handleWasmStreaming;
    return handleWasmStreaming(source, rid);
  });
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
core.defineGlobalProperties(globalThis, windowOrWorkerGlobalScope);

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
      core.defineGlobalProperties(globalThis, { ...props });
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

  // Related to `Deno.jupyter` REPL API
  "op_jupyter_broadcast",
  "op_jupyter_input",
  "op_jupyter_create_png_from_texture",
  "op_jupyter_get_buffer",
  // Related to the Jupyter ZMQ kernel worker
  "op_jupyter_get_connection_info",
  "op_jupyter_repl_evaluate",
  "op_jupyter_repl_get_properties",
  "op_jupyter_repl_global_lexical_scope_names",
  "op_jupyter_repl_call_function_on_args",
  "op_jupyter_repl_call_function_on",
  "op_jupyter_repl_interrupt",
  "op_jupyter_repl_cancel_interrupt",
  "op_jupyter_recv_iopub",
  "op_jupyter_recv_input",
  "op_jupyter_send_input_reply",
  "op_jupyter_deno_version",
  "op_jupyter_typescript_version",
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
  "op_test_event_exit",
  "op_test_isolate_exit",
  "op_pledge_test_permissions",
  "op_test_snapshot_in_update_mode",
  "op_test_snapshot_read",
  "op_test_snapshot_write",
  "op_test_event_snapshot_summary",

  // TODO(bartlomieju): used in various integration tests - figure out a way
  // to not depend on them.
  "op_set_exit_code",
  "op_napi_open",

  // Related to `Deno.desktop` API (deno compile --desktop)
  "BrowserWindow",
  "Dock",
  "Tray",
  "Notification",
  "op_desktop_apply_patch",
  "op_desktop_confirm_update",
  "op_desktop_init",
  "op_desktop_recv_event",
  "op_desktop_resolve_bind_call",
  "op_desktop_reject_bind_call",
  "op_desktop_alert",
  "op_desktop_confirm",
  "op_desktop_prompt",
  "op_desktop_send_error_report",
  "op_desktop_request_notification_permission",
  "op_desktop_query_notification_permission",

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

// `Deno[Deno.internal]` is reachable from user code. Keep extension-loading
// capabilities on the core object imported through `ext:core/mod.js`, and only
// expose the subset needed by internal tooling and tests here.
const userVisibleCore = ObjectFreeze({
  __proto__: null,
  callConsole: core.callConsole,
  encodeBinaryString: core.encodeBinaryString,
  evalContext: core.evalContext,
  ops,
  print: core.print,
  propNonEnumerable: core.propNonEnumerable,
  propReadOnly: core.propReadOnly,
  propWritable: core.propWritable,
  resources: core.resources,
  setLeakTracingEnabled: core.setLeakTracingEnabled,
  setPromiseHooks: core.setPromiseHooks,
  unrefOpPromise: core.unrefOpPromise,
});
ObjectAssign(internals, { core: userVisibleCore });
const internalSymbol = Symbol("Deno.internal");
// `Deno.test` and its sub-methods are no-ops outside of `deno test`, kept for
// compatibility so they don't error under `deno run`. Mirrors the surface of
// the real `Deno.test` defined in cli/js/40_test.js.
function noopTest() {}
noopTest.ignore = () => {};
noopTest.only = () => {};
noopTest.beforeAll = () => {};
noopTest.beforeEach = () => {};
noopTest.afterEach = () => {};
noopTest.afterAll = () => {};
noopTest.sanitizer = () => {};
const noopTestEach = () => () => {};
noopTest.each = noopTestEach;
noopTest.only.each = noopTestEach;
noopTest.ignore.each = noopTestEach;
// Build finalDenoNs without spreading denoNs: spread invokes every getter,
// including the lazy ones (Deno.serve / Deno.run / etc.) that intentionally
// avoid loading 06_streams / 22_body / 40_process at snapshot time. Use
// ObjectDefineProperties + getOwnPropertyDescriptors to preserve the lazy
// descriptors.
const finalDenoNs = ObjectDefineProperties(
  {
    internal: internalSymbol,
    [internalSymbol]: internals,
    // Deno.test, Deno.bench, Deno.lint are noops here, but kept for
    // compatibility; so that they don't cause errors when used outside of
    // `deno test`/`deno bench`/`deno lint` contexts.
    test: noopTest,
    bench: () => {},
    lint: {
      runPlugin: () => {
        throw new Error(
          "`Deno.lint.runPlugin` is only available in `deno test` subcommand.",
        );
      },
    },
  },
  ObjectGetOwnPropertyDescriptors(denoNs),
);

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

let protoSetRecorded = false;
let protoGetRecorded = false;

// By default Deno disables the `Object.prototype.__proto__` accessor for
// security reasons (it enables prototype pollution), see
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
//
// Historically this was done with `delete Object.prototype.__proto__`, which
// makes `obj.__proto__` reads return `undefined` and writes silently create a
// useless own property. Making the accessor *throw* instead would surface those
// silent bugs, but it broke real packages such as Playwright (see
// denoland/deno#34730 / #34772), so we keep the silent behavior.
//
// Instead of deleting we install an accessor that reproduces the deleted
// semantics exactly (read -> `undefined`, write -> own data property, prototype
// unchanged) but additionally records the first read and the first write. When
// the program later crashes, the uncaught-error formatter (runtime/fmt_errors.rs)
// reads those flags and suggests `--unstable-unsafe-proto`. A write surfaces
// downstream so any later crash triggers the nudge; a read crashes at the
// access site, so the formatter additionally requires `__proto__` on the
// failing line before nudging. The `__proto__` key in object literals (e.g.
// `{ __proto__: null }`) is separate syntax and is unaffected.
function disableProtoAccessor() {
  ObjectDefineProperty(ObjectPrototype, "__proto__", {
    __proto__: null,
    configurable: true,
    enumerable: false,
    // Distinct getter/setter function objects: the native accessor uses
    // separate functions and WPT asserts accessor get/set are unique.
    get: function __proto__() {
      if (!protoGetRecorded) {
        protoGetRecorded = true;
        op_proto_get_attempted();
      }
      return undefined;
    },
    set: function __proto__(value) {
      if (!protoSetRecorded) {
        protoSetRecorded = true;
        op_proto_set_attempted();
      }
      // Reproduce the previous `delete` behavior: a bare assignment created a
      // normal own data property. Skip non-extensible receivers, where that
      // assignment was a silent no-op in sloppy mode (and a throw in strict
      // mode); keeping it silent here matches the "stay silent" goal above.
      if (ObjectIsExtensible(this)) {
        ObjectDefineProperty(this, "__proto__", {
          __proto__: null,
          value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      }
    },
  });
}

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
      17: nodeClusterUniqueId,
      18: nodeClusterSchedPolicy,
      19: disableOffscreenCanvas,
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
            serve = lazyServeMod().registerDeclarativeServer(main.default);
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

    // Remove bootstrapping data from the global scope. Lazy-loaded IIFE
    // scripts (`ext:.../*.js`) and the synthetic_esm backing-script path
    // both read `globalThis.__bootstrap.core.ops` at module body; the
    // Rust `load_ext_script` reinstalls a captured snapshot view of
    // `__bootstrap` for the duration of each script's evaluation (see
    // `BootstrapInstallGuard` in `libs/core/modules/map.rs`).
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

    // Use defineGlobalProperties so the lazy-loaded descriptors (alert,
    // confirm, prompt, Storage) get their `lazyNameSym` stamped with the
    // property name. Without this, the setter calls
    // `Object.defineProperty(this, undefined, ...)` which fails because the
    // global `undefined` is non-configurable on `globalThis`.
    core.defineGlobalProperties(globalThis, mainRuntimeGlobalProperties);
    ObjectDefineProperties(globalThis, {
      // TODO(bartlomieju): in the future we might want to change the
      // behavior of setting `name` to actually update the process name.
      // Empty string matches what browsers do.
      name: core.propWritable(""),
      close: core.propWritable(windowClose),
      closed: core.propGetterOnly(() => windowIsClosing),
    });
    if (disableOffscreenCanvas) {
      delete globalThis.OffscreenCanvas;
    }
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
      const unstable = denoNsUnstableById[id];
      if (unstable) {
        ObjectDefineProperties(
          finalDenoNs,
          ObjectGetOwnPropertyDescriptors(unstable),
        );
      }
    }

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.unsafeProto)) {
      disableProtoAccessor();
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", core.propReadOnly(finalDenoNs));

    const nodeBootstrapArgs = {
      usesLocalNodeModulesDir: hasNodeModulesDir,
      runningOnMainThread: true,
      argv0,
      nodeDebug,
      nodeClusterUniqueId,
      nodeClusterSchedPolicy,
      // Stashed so process.ts's self-trigger can call __bootstrapNodeProcess
      // without reading Deno.* (the no-deno-api-in-polyfills lint counts
      // Deno.* references in node polyfills; passing them in keeps process.ts
      // at zero new violations).
      denoArgs: Deno.args,
      denoVersion: Deno.version,
    };
    if (nodeBootstrap) {
      nodeBootstrap(nodeBootstrapArgs);
    } else if (op_node_has_child_ipc_pipe()) {
      // node-defer: this main process is a forked child with an IPC pipe. It
      // needs the IPC channel (set up in the full `initialize`) ready
      // synchronously before its main module calls process.send /
      // process.on("message"). Run the full node bootstrap EAGERLY (like
      // workers) -- a forked IPC child is a node process, so the deser win
      // doesn't apply. node:process first (fully evaluates), then node:module
      // (its closure captures a finished node:process, so no cold-bootstrap
      // TDZ), then `initialize`.
      core.createLazyLoader("node:process")();
      core.createLazyLoader("node:module")();
      globalThis.nodeBootstrap(nodeBootstrapArgs);
    } else {
      // node-defer: node:module (01_require.js) is `lazy_loaded_esm`, so its
      // top-level `globalThis.nodeBootstrap = initialize` hasn't run yet and
      // `nodeBootstrap` is undefined here. Stash the args so node:process and
      // 01_require.js self-bootstrap from them when first lazily loaded (on
      // the first node:* use), so non-node programs never pay node bootstrap.
      internals.__nodeBootstrapArgs = nodeBootstrapArgs;
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
      17: nodeClusterUniqueId,
      18: nodeClusterSchedPolicy,
      19: disableOffscreenCanvas,
    } = runtimeOptions;

    denoNs.build.standalone = standalone;

    closeOnIdle = runtimeOptions[14];

    performance.setTimeOrigin();
    globalThis_ = globalThis;

    // Remove bootstrapping data from the global scope. Lazy-loaded IIFE
    // scripts (`ext:.../*.js`) and the synthetic_esm backing-script path
    // both read `globalThis.__bootstrap.core.ops` at module body; the
    // Rust `load_ext_script` reinstalls a captured snapshot view of
    // `__bootstrap` for the duration of each script's evaluation (see
    // `BootstrapInstallGuard` in `libs/core/modules/map.rs`).
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
    if (disableOffscreenCanvas) {
      delete globalThis.OffscreenCanvas;
    }
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

    // `Deno.exit()` closes the worker using the internal worker close
    // operation. Setting an exit code using an op in worker context is a no-op.
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
      const unstable = denoNsUnstableById[id];
      if (unstable) {
        ObjectDefineProperties(
          finalDenoNs,
          ObjectGetOwnPropertyDescriptors(unstable),
        );
      }
    }

    // Not available in workers
    const moduleSpecifier = finalDenoNs.mainModule;
    delete finalDenoNs.mainModule;

    if (!ArrayPrototypeIncludes(unstableFeatures, unstableIds.unsafeProto)) {
      disableProtoAccessor();
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", core.propReadOnly(finalDenoNs));

    const workerMetadata = maybeWorkerMetadata
      ? messagePort.deserializeJsMessageData(maybeWorkerMetadata)
      : undefined;

    const nodeBootstrapArgs = {
      usesLocalNodeModulesDir: hasNodeModulesDir,
      runningOnMainThread: false,
      argv0,
      workerId,
      maybeWorkerMetadata: workerMetadata,
      nodeDebug,
      nodeClusterUniqueId,
      nodeClusterSchedPolicy,
      moduleSpecifier: workerType === "node" ? moduleSpecifier : null,
      // Stashed so process.ts's self-trigger can call __bootstrapNodeProcess
      // without reading Deno.* (see the main-thread branch above).
      denoArgs: Deno.args,
      denoVersion: Deno.version,
    };
    if (nodeBootstrap) {
      nodeBootstrap(nodeBootstrapArgs);
    } else if (workerType === "node") {
      // node-defer: node worker_threads need the FULL node bootstrap eagerly:
      // require, workerData, SharedArrayBuffer, etc. must be ready before the
      // worker's first line runs. Web workers stay lazy like the main thread
      // so non-node workers never pay node bootstrap.
      // Load node:process FIRST (fully evaluates + runs the process bootstrap),
      // THEN node:module (its closure now captures a fully-evaluated
      // node:process, avoiding the cold-bootstrap TDZ), then run `initialize`.
      core.createLazyLoader("node:process")();
      core.createLazyLoader("node:module")();
      globalThis.nodeBootstrap(nodeBootstrapArgs);
    } else {
      internals.__nodeBootstrapArgs = nodeBootstrapArgs;
    }
  } else {
    // Warmup
    return;
  }
}

const nodeBootstrap = globalThis.nodeBootstrap;
delete globalThis.nodeBootstrap;
// node-defer: node:process sets internals.dispatchProcess{Exit,BeforeExit}Event
// during its bootstrap, which is now deferred to first node:* use -- i.e. AFTER
// this module evaluates. Capturing the values here would freeze the no-op
// (node not yet bootstrapped). Instead dispatch dynamically: look up the
// current internals handler at call time. No-op when node was never
// bootstrapped (a non-node program has no process exit listeners).
const dispatchProcessExitEvent = (...args) =>
  internals.dispatchProcessExitEvent
    ? ReflectApply(internals.dispatchProcessExitEvent, internals, args)
    : undefined;
const dispatchProcessBeforeExitEvent = (...args) =>
  internals.dispatchProcessBeforeExitEvent
    ? ReflectApply(internals.dispatchProcessBeforeExitEvent, internals, args)
    : false;

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
// Skip warmup. The warmup branch creates placeholder stdin/stdout/stderr
// streams that the runtime bootstrap (__bootstrapNodeProcess(warmup=false))
// then unconditionally overwrites with fresh TTYWriteStream instances, so
// the only observable effect of warmup was pulling node:stream and friends
// into the snapshot via createWritableStdioStream/initStdin.
// nodeBootstrap({ warmup: true });
