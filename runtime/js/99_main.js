// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// Removes the `__proto__` for security reasons.
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
delete Object.prototype.__proto__;

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

const core = globalThis.Deno.core;
const ops = core.ops;
const internals = globalThis.__bootstrap.internals;
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  ArrayPrototypeSplice,
  ArrayPrototypeMap,
  DateNow,
  Error,
  ErrorPrototype,
  FunctionPrototypeCall,
  FunctionPrototypeBind,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectDefineProperties,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  PromiseResolve,
  Symbol,
  SymbolIterator,
  PromisePrototypeThen,
  SafeWeakMap,
  TypeError,
  WeakMapPrototypeDelete,
  WeakMapPrototypeGet,
  WeakMapPrototypeSet,
} = primordials;
import * as util from "ext:runtime/06_util.js";
import * as event from "ext:deno_web/02_event.js";
import * as location from "ext:deno_web/12_location.js";
import * as version from "ext:runtime/01_version.ts";
import * as os from "ext:runtime/30_os.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as colors from "ext:deno_console/01_colors.js";
import {
  inspectArgs,
  quoteString,
  wrapConsole,
} from "ext:deno_console/02_console.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as url from "ext:deno_url/00_url.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import { denoNs, denoNsUnstable } from "ext:runtime/90_deno_ns.js";
import { errors } from "ext:runtime/01_errors.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
import {
  mainRuntimeGlobalProperties,
  setLanguage,
  setNumCpus,
  setUserAgent,
  unstableWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
  workerRuntimeGlobalProperties,
} from "ext:runtime/98_global_scope.js";

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
  webidl.requiredArguments(arguments.length, 1, { prefix });
  message = webidl.converters.any(message);
  let options;
  if (
    webidl.type(transferOrOptions) === "Object" &&
    transferOrOptions !== undefined &&
    transferOrOptions[SymbolIterator] !== undefined
  ) {
    const transfer = webidl.converters["sequence<object>"](
      transferOrOptions,
      { prefix, context: "Argument 2" },
    );
    options = { transfer };
  } else {
    options = webidl.converters.StructuredSerializeOptions(
      transferOrOptions,
      {
        prefix,
        context: "Argument 2",
      },
    );
  }
  const { transfer } = options;
  const data = messagePort.serializeJsMessageData(message, transfer);
  ops.op_worker_post_message(data);
}

let isClosing = false;
let globalDispatchEvent;

async function pollForMessages() {
  if (!globalDispatchEvent) {
    globalDispatchEvent = FunctionPrototypeBind(
      globalThis.dispatchEvent,
      globalThis,
    );
  }
  while (!isClosing) {
    const data = await core.opAsync("op_worker_recv_message");
    if (data === null) break;
    const v = messagePort.deserializeJsMessageData(data);
    const message = v[0];
    const transferables = v[1];

    const msgEvent = new event.MessageEvent("message", {
      cancelable: false,
      data: message,
      ports: transferables.filter((t) =>
        ObjectPrototypeIsPrototypeOf(messagePort.MessagePortPrototype, t)
      ),
    });

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

function formatException(error) {
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, error)) {
    return null;
  } else if (typeof error == "string") {
    return `Uncaught ${
      inspectArgs([quoteString(error)], {
        colors: !colors.getNoColor(),
      })
    }`;
  } else {
    return `Uncaught ${inspectArgs([error], { colors: !colors.getNoColor() })}`;
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
  debugFlag,
  noColor,
  isTty,
  source,
) {
  core.setMacrotaskCallback(timers.handleTimerMacrotask);
  core.setMacrotaskCallback(promiseRejectMacrotaskCallback);
  core.setWasmStreamingCallback(fetch.handleWasmStreaming);
  core.setReportExceptionCallback(event.reportException);
  ops.op_set_format_exception_callback(formatException);
  version.setVersions(
    denoVersion,
    v8Version,
    tsVersion,
  );
  core.setBuildInfo(target);
  util.setLogDebug(debugFlag, source);
  colors.setNoColor(noColor || !isTty);
  // deno-lint-ignore prefer-primordials
  Error.prepareStackTrace = core.prepareStackTrace;
}

const pendingRejections = [];
const pendingRejectionsReasons = new SafeWeakMap();

function promiseRejectCallback(type, promise, reason) {
  switch (type) {
    case 0: {
      ops.op_store_pending_promise_rejection(promise, reason);
      ArrayPrototypePush(pendingRejections, promise);
      WeakMapPrototypeSet(pendingRejectionsReasons, promise, reason);
      break;
    }
    case 1: {
      ops.op_remove_pending_promise_rejection(promise);
      const index = ArrayPrototypeIndexOf(pendingRejections, promise);
      if (index > -1) {
        ArrayPrototypeSplice(pendingRejections, index, 1);
        WeakMapPrototypeDelete(pendingRejectionsReasons, promise);
      }
      break;
    }
    default:
      return false;
  }

  return !!globalThis_.onunhandledrejection ||
    event.listenerCount(globalThis_, "unhandledrejection") > 0;
}

function promiseRejectMacrotaskCallback() {
  while (pendingRejections.length > 0) {
    const promise = ArrayPrototypeShift(pendingRejections);
    const hasPendingException = ops.op_has_pending_promise_rejection(
      promise,
    );
    const reason = WeakMapPrototypeGet(pendingRejectionsReasons, promise);
    WeakMapPrototypeDelete(pendingRejectionsReasons, promise);

    if (!hasPendingException) {
      continue;
    }

    const rejectionEvent = new event.PromiseRejectionEvent(
      "unhandledrejection",
      {
        cancelable: true,
        promise,
        reason,
      },
    );

    const errorEventCb = (event) => {
      if (event.error === reason) {
        ops.op_remove_pending_promise_rejection(promise);
      }
    };
    // Add a callback for "error" event - it will be dispatched
    // if error is thrown during dispatch of "unhandledrejection"
    // event.
    globalThis_.addEventListener("error", errorEventCb);
    globalThis_.dispatchEvent(rejectionEvent);
    globalThis_.removeEventListener("error", errorEventCb);

    // If event was not prevented (or "unhandledrejection" listeners didn't
    // throw) we will let Rust side handle it.
    if (rejectionEvent.defaultPrevented) {
      ops.op_remove_pending_promise_rejection(promise);
    }
  }
  return true;
}

let hasBootstrapped = false;
// Set up global properties shared by main and worker runtime.
ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);
// FIXME(bartlomieju): temporarily add whole `Deno.core` to
// `Deno[Deno.internal]` namespace. It should be removed and only necessary
// methods should be left there.
ObjectAssign(internals, {
  core,
});
const internalSymbol = Symbol("Deno.internal");
const finalDenoNs = {
  internal: internalSymbol,
  [internalSymbol]: internals,
  resources: core.resources,
  close: core.close,
  ...denoNs,
};

function bootstrapMainRuntime(runtimeOptions) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  const {
    0: args,
    1: cpuCount,
    2: debugFlag,
    3: denoVersion,
    4: locale,
    5: location_,
    6: noColor,
    7: isTty,
    8: tsVersion,
    9: unstableFlag,
    10: pid,
    11: ppid,
    12: target,
    13: v8Version,
    14: userAgent,
    15: inspectFlag,
    // 16: enableTestingFeaturesFlag
  } = runtimeOptions;

  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  hasBootstrapped = true;

  // If the `--location` flag isn't set, make `globalThis.location` `undefined` and
  // writable, so that they can mock it themselves if they like. If the flag was
  // set, define `globalThis.location`, using the provided value.
  if (location_ === undefined) {
    mainRuntimeGlobalProperties.location = {
      writable: true,
    };
  } else {
    location.setLocationHref(location_);
  }

  if (unstableFlag) {
    ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
  }
  ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
  });
  ObjectSetPrototypeOf(globalThis, Window.prototype);

  if (inspectFlag) {
    const consoleFromV8 = core.console;
    const consoleFromDeno = globalThis.console;
    wrapConsole(consoleFromDeno, consoleFromV8);
  }

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(globalThis, "error");
  event.defineEventHandler(globalThis, "load");
  event.defineEventHandler(globalThis, "beforeunload");
  event.defineEventHandler(globalThis, "unload");
  event.defineEventHandler(globalThis, "unhandledrejection");

  core.setPromiseRejectCallback(promiseRejectCallback);

  runtimeStart(
    denoVersion,
    v8Version,
    tsVersion,
    target,
    debugFlag,
    noColor,
    isTty,
  );

  setNumCpus(cpuCount);
  setUserAgent(userAgent);
  setLanguage(locale);

  ObjectDefineProperties(finalDenoNs, {
    pid: util.readOnly(pid),
    ppid: util.readOnly(ppid),
    noColor: util.readOnly(noColor),
    args: util.readOnly(ObjectFreeze(args)),
    mainModule: util.getterOnly(opMainModule),
  });

  if (unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
  }

  // Setup `Deno` global - we're actually overriding already existing global
  // `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  util.log("args", args);
}

function bootstrapWorkerRuntime(
  runtimeOptions,
  name,
  internalName,
) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  const {
    0: args,
    1: cpuCount,
    2: debugFlag,
    3: denoVersion,
    4: locale,
    5: location_,
    6: noColor,
    7: isTty,
    8: tsVersion,
    9: unstableFlag,
    10: pid,
    // 11: ppid,
    12: target,
    13: v8Version,
    // 14: userAgent,
    // 15: inspectFlag,
    16: enableTestingFeaturesFlag,
  } = runtimeOptions;

  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  const consoleFromV8 = globalThis.Deno.core.console;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  hasBootstrapped = true;

  if (unstableFlag) {
    ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
  }
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
  wrapConsole(consoleFromDeno, consoleFromV8);

  event.setEventTargetData(globalThis);
  event.saveGlobalThisReference(globalThis);

  event.defineEventHandler(self, "message");
  event.defineEventHandler(self, "error", undefined, true);
  event.defineEventHandler(self, "unhandledrejection");

  core.setPromiseRejectCallback(promiseRejectCallback);

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
    debugFlag,
    noColor,
    isTty,
    internalName ?? name,
  );

  location.setLocationHref(location_);

  setNumCpus(cpuCount);
  setLanguage(locale);

  globalThis.pollForMessages = pollForMessages;

  if (unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
  }
  ObjectDefineProperties(finalDenoNs, {
    pid: util.readOnly(pid),
    noColor: util.readOnly(noColor),
    args: util.readOnly(ObjectFreeze(args)),
  });
  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));
}

globalThis.bootstrap = {
  mainRuntime: bootstrapMainRuntime,
  workerRuntime: bootstrapWorkerRuntime,
};
