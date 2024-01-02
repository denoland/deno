// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

import { core, internals, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypeFilter,
  ArrayPrototypeIncludes,
  ArrayPrototypeMap,
  DateNow,
  Error,
  ErrorPrototype,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  ObjectAssign,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  ObjectValues,
  PromisePrototypeThen,
  PromiseResolve,
  Symbol,
  SymbolIterator,
  TypeError,
} = primordials;
import * as util from "ext:runtime/06_util.js";
import * as event from "ext:deno_web/02_event.js";
import * as location from "ext:deno_web/12_location.js";
import * as version from "ext:runtime/01_version.ts";
import * as os from "ext:runtime/30_os.js";
import * as timers from "ext:deno_web/02_timers.js";
import {
  getDefaultInspectOptions,
  getNoColor,
  inspectArgs,
  quoteString,
  setNoColorFn,
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
import DOMException from "ext:deno_web/01_dom_exception.js";
import {
  mainRuntimeGlobalProperties,
  memoizeLazy,
  unstableForWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
  workerRuntimeGlobalProperties,
} from "ext:runtime/98_global_scope.js";
import { SymbolAsyncDispose, SymbolDispose } from "ext:deno_web/00_infra.js";

// deno-lint-ignore prefer-primordials
if (Symbol.dispose) throw "V8 supports Symbol.dispose now, no need to shim it!";
ObjectDefineProperties(Symbol, {
  dispose: {
    value: SymbolDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
  asyncDispose: {
    value: SymbolAsyncDispose,
    enumerable: false,
    writable: false,
    configurable: false,
  },
});

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
  ops.op_worker_close();
}

function postMessage(message, transferOrOptions = {}) {
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
  ops.op_worker_post_message(data);
}

let isClosing = false;
let globalDispatchEvent;

async function pollForMessages() {
  const { op_worker_recv_message } = core.ensureFastOps();

  if (!globalDispatchEvent) {
    globalDispatchEvent = FunctionPrototypeBind(
      globalThis.dispatchEvent,
      globalThis,
    );
  }
  while (!isClosing) {
    const data = await op_worker_recv_message();
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
  if (ops.op_worker_get_type() === "module") {
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
  const scripts = ops.op_worker_sync_fetch(
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

function opMainModule() {
  return ops.op_main_module();
}

const opArgs = memoizeLazy(() => ops.op_bootstrap_args());
const opPid = memoizeLazy(() => ops.op_bootstrap_pid());
const opPpid = memoizeLazy(() => ops.op_ppid());
setNoColorFn(() => ops.op_bootstrap_no_color() || !ops.op_bootstrap_is_tty());

function formatException(error) {
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, error)) {
    return null;
  } else if (typeof error == "string") {
    return `Uncaught ${
      inspectArgs([quoteString(error, getDefaultInspectOptions())], {
        colors: !getNoColor(),
      })
    }`;
  } else {
    return `Uncaught ${inspectArgs([error], { colors: !getNoColor() })}`;
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
core.registerErrorClass("Interrupted", errors.Interrupted);
core.registerErrorClass("WouldBlock", errors.WouldBlock);
core.registerErrorClass("WriteZero", errors.WriteZero);
core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
core.registerErrorClass("BadResource", errors.BadResource);
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
  core.setMacrotaskCallback(timers.handleTimerMacrotask);
  core.setWasmStreamingCallback(fetch.handleWasmStreaming);
  core.setReportExceptionCallback(event.reportException);
  ops.op_set_format_exception_callback(formatException);
  version.setVersions(
    denoVersion,
    v8Version,
    tsVersion,
  );
  core.setBuildInfo(target);
}

core.setUnhandledPromiseRejectionHandler(processUnhandledPromiseRejection);
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

// FIXME(bartlomieju): temporarily add whole `Deno.core` to
// `Deno[Deno.internal]` namespace. It should be removed and only necessary
// methods should be left there.
ObjectAssign(internals, { core });
const internalSymbol = Symbol("Deno.internal");
const finalDenoNs = {
  internal: internalSymbol,
  [internalSymbol]: internals,
  resources: core.resources,
  close: core.close,
  ...denoNs,
  // Deno.test and Deno.bench are noops here, but kept for compatibility; so
  // that they don't cause errors when used outside of `deno test`/`deno bench`
  // contexts.
  test: () => {},
  bench: () => {},
};

const {
  denoVersion,
  tsVersion,
  v8Version,
  target,
} = ops.op_snapshot_options();

function bootstrapMainRuntime(runtimeOptions) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }
  const nodeBootstrap = globalThis.nodeBootstrap;

  const {
    0: location_,
    1: unstableFlag,
    2: unstableFeatures,
    3: inspectFlag,
    5: hasNodeModulesDir,
    6: maybeBinaryNpmCommandName,
  } = runtimeOptions;

  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  delete globalThis.nodeBootstrap;
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
    name: util.writable(""),
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
  });
  ObjectSetPrototypeOf(globalThis, Window.prototype);

  if (inspectFlag) {
    const consoleFromDeno = globalThis.console;
    core.wrapConsole(consoleFromDeno, core.v8Console);
  }

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(globalThis, "error");
  event.defineEventHandler(globalThis, "load");
  event.defineEventHandler(globalThis, "beforeunload");
  event.defineEventHandler(globalThis, "unload");
  event.defineEventHandler(globalThis, "unhandledrejection");

  runtimeStart(
    denoVersion,
    v8Version,
    tsVersion,
    target,
  );

  ObjectDefineProperties(finalDenoNs, {
    pid: util.getterOnly(opPid),
    ppid: util.getterOnly(opPpid),
    noColor: util.getterOnly(() => ops.op_bootstrap_no_color()),
    args: util.getterOnly(opArgs),
    mainModule: util.getterOnly(opMainModule),
  });

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

  // Setup `Deno` global - we're actually overriding already existing global
  // `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  if (nodeBootstrap) {
    nodeBootstrap(hasNodeModulesDir, maybeBinaryNpmCommandName);
  }
}

function bootstrapWorkerRuntime(
  runtimeOptions,
  name,
  internalName,
) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  const nodeBootstrap = globalThis.nodeBootstrap;

  const {
    0: location_,
    1: unstableFlag,
    2: unstableFeatures,
    4: enableTestingFeaturesFlag,
    5: hasNodeModulesDir,
    6: maybeBinaryNpmCommandName,
  } = runtimeOptions;

  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  delete globalThis.nodeBootstrap;
  hasBootstrapped = true;

  exposeUnstableFeaturesForWindowOrWorkerGlobalScope({
    unstableFlag,
    unstableFeatures,
  });
  ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    name: util.writable(name),
    // TODO(bartlomieju): should be readonly?
    close: util.nonEnumerable(workerClose),
    postMessage: util.writable(postMessage),
  });
  if (enableTestingFeaturesFlag) {
    ObjectDefineProperty(
      globalThis,
      "importScripts",
      util.writable(importScripts),
    );
  }
  ObjectSetPrototypeOf(globalThis, DedicatedWorkerGlobalScope.prototype);

  const consoleFromDeno = globalThis.console;
  core.wrapConsole(consoleFromDeno, core.v8Console);

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(self, "message");
  event.defineEventHandler(self, "error", undefined, true);
  event.defineEventHandler(self, "unhandledrejection");

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

  // TODO(bartlomieju): deprecate --unstable
  if (unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
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

  ObjectDefineProperties(finalDenoNs, {
    pid: util.getterOnly(opPid),
    noColor: util.getterOnly(() => ops.op_bootstrap_no_color()),
    args: util.getterOnly(opArgs),
  });
  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  if (nodeBootstrap) {
    nodeBootstrap(hasNodeModulesDir, maybeBinaryNpmCommandName);
  }
}

globalThis.bootstrap = {
  mainRuntime: bootstrapMainRuntime,
  workerRuntime: bootstrapWorkerRuntime,
};
