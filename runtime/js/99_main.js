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
  SymbolFor,
  SymbolIterator,
  PromisePrototypeThen,
  SafeWeakMap,
  TypeError,
  WeakMapPrototypeDelete,
  WeakMapPrototypeGet,
  WeakMapPrototypeSet,
} = primordials;
import * as util from "internal:runtime/js/06_util.js";
import * as event from "internal:deno_web/02_event.js";
import * as location from "internal:deno_web/12_location.js";
import * as build from "internal:runtime/js/01_build.js";
import * as version from "internal:runtime/js/01_version.ts";
import * as os from "internal:runtime/js/30_os.js";
import * as timers from "internal:deno_web/02_timers.js";
import * as colors from "internal:deno_console/01_colors.js";
import * as net from "internal:deno_net/01_net.js";
import {
  inspectArgs,
  quoteString,
  wrapConsole,
} from "internal:deno_console/02_console.js";
import * as performance from "internal:deno_web/15_performance.js";
import * as url from "internal:deno_url/00_url.js";
import * as fetch from "internal:deno_fetch/26_fetch.js";
import * as messagePort from "internal:deno_web/13_message_port.js";
import { denoNs, denoNsUnstable } from "internal:runtime/js/90_deno_ns.js";
import { errors } from "internal:runtime/js/01_errors.js";
import * as webidl from "internal:deno_webidl/00_webidl.js";
import DOMException from "internal:deno_web/01_dom_exception.js";
import * as flash from "internal:deno_flash/01_http.js";
import * as spawn from "internal:runtime/js/40_spawn.js";
import {
  mainRuntimeGlobalProperties,
  setLanguage,
  setNumCpus,
  setUserAgent,
  unstableWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
  workerRuntimeGlobalProperties,
} from "internal:runtime/js/98_global_scope.js";

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

function runtimeStart(runtimeOptions, source) {
  core.setMacrotaskCallback(timers.handleTimerMacrotask);
  core.setMacrotaskCallback(promiseRejectMacrotaskCallback);
  core.setWasmStreamingCallback(fetch.handleWasmStreaming);
  core.setReportExceptionCallback(event.reportException);
  ops.op_set_format_exception_callback(formatException);
  version.setVersions(
    runtimeOptions.denoVersion,
    runtimeOptions.v8Version,
    runtimeOptions.tsVersion,
  );
  build.setBuildInfo(runtimeOptions.target);
  util.setLogDebug(runtimeOptions.debugFlag, source);
  colors.setNoColor(runtimeOptions.noColor || !runtimeOptions.isTty);
  // deno-lint-ignore prefer-primordials
  Error.prepareStackTrace = core.prepareStackTrace;
  registerErrors();
}

function registerErrors() {
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

function bootstrapMainRuntime(runtimeOptions) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  core.initializeAsyncOps();
  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  const consoleFromV8 = globalThis.Deno.core.console;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  util.log("bootstrapMainRuntime");
  hasBootstrapped = true;

  // If the `--location` flag isn't set, make `globalThis.location` `undefined` and
  // writable, so that they can mock it themselves if they like. If the flag was
  // set, define `globalThis.location`, using the provided value.
  if (runtimeOptions.location == null) {
    mainRuntimeGlobalProperties.location = {
      writable: true,
    };
  } else {
    location.setLocationHref(runtimeOptions.location);
  }

  ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);
  if (runtimeOptions.unstableFlag) {
    ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
  }
  ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
  });
  ObjectSetPrototypeOf(globalThis, Window.prototype);

  if (runtimeOptions.inspectFlag) {
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

  const isUnloadDispatched = SymbolFor("isUnloadDispatched");
  // Stores the flag for checking whether unload is dispatched or not.
  // This prevents the recursive dispatches of unload events.
  // See https://github.com/denoland/deno/issues/9201.
  globalThis[isUnloadDispatched] = false;
  globalThis.addEventListener("unload", () => {
    globalThis_[isUnloadDispatched] = true;
  });

  runtimeStart(runtimeOptions);

  setNumCpus(runtimeOptions.cpuCount);
  setUserAgent(runtimeOptions.userAgent);
  setLanguage(runtimeOptions.locale);

  const internalSymbol = Symbol("Deno.internal");

  // These have to initialized here and not in `90_deno_ns.js` because
  // the op function that needs to be passed will be invalidated by creating
  // a snapshot
  ObjectAssign(internals, {
    nodeUnstable: {
      Command: spawn.createCommand(
        spawn.createSpawn(ops.op_node_unstable_spawn_child),
        spawn.createSpawnSync(
          ops.op_node_unstable_spawn_sync,
        ),
        spawn.createSpawnChild(
          ops.op_node_unstable_spawn_child,
        ),
      ),
      serve: flash.createServe(ops.op_node_unstable_flash_serve),
      upgradeHttpRaw: flash.upgradeHttpRaw,
      listenDatagram: net.createListenDatagram(
        ops.op_node_unstable_net_listen_udp,
        ops.op_node_unstable_net_listen_unixpacket,
      ),
      osUptime: os.createOsUptime(ops.op_node_unstable_os_uptime),
    },
  });

  // FIXME(bartlomieju): temporarily add whole `Deno.core` to
  // `Deno[Deno.internal]` namespace. It should be removed and only necessary
  // methods should be left there.
  ObjectAssign(internals, {
    core,
  });

  const finalDenoNs = {
    internal: internalSymbol,
    [internalSymbol]: internals,
    resources: core.resources,
    close: core.close,
    ...denoNs,
  };
  ObjectDefineProperties(finalDenoNs, {
    pid: util.readOnly(runtimeOptions.pid),
    ppid: util.readOnly(runtimeOptions.ppid),
    noColor: util.readOnly(runtimeOptions.noColor),
    args: util.readOnly(ObjectFreeze(runtimeOptions.args)),
    mainModule: util.getterOnly(opMainModule),
  });

  if (runtimeOptions.unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
    // These have to initialized here and not in `90_deno_ns.js` because
    // the op function that needs to be passed will be invalidated by creating
    // a snapshot
    ObjectAssign(finalDenoNs, {
      Command: spawn.createCommand(
        spawn.createSpawn(ops.op_spawn_child),
        spawn.createSpawnSync(ops.op_spawn_sync),
        spawn.createSpawnChild(ops.op_spawn_child),
      ),
      serve: flash.createServe(ops.op_flash_serve),
      listenDatagram: net.createListenDatagram(
        ops.op_net_listen_udp,
        ops.op_net_listen_unixpacket,
      ),
      osUptime: os.createOsUptime(ops.op_os_uptime),
    });
  }

  // Setup `Deno` global - we're actually overriding already existing global
  // `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));

  util.log("args", runtimeOptions.args);
}

function bootstrapWorkerRuntime(
  runtimeOptions,
  name,
  internalName,
) {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }

  core.initializeAsyncOps();
  performance.setTimeOrigin(DateNow());
  globalThis_ = globalThis;

  const consoleFromV8 = globalThis.Deno.core.console;

  // Remove bootstrapping data from the global scope
  delete globalThis.__bootstrap;
  delete globalThis.bootstrap;
  util.log("bootstrapWorkerRuntime");
  hasBootstrapped = true;
  ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);
  if (runtimeOptions.unstableFlag) {
    ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
  }
  ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);
  ObjectDefineProperties(globalThis, {
    name: util.writable(name),
    // TODO(bartlomieju): should be readonly?
    close: util.nonEnumerable(workerClose),
    postMessage: util.writable(postMessage),
  });
  if (runtimeOptions.enableTestingFeaturesFlag) {
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
    runtimeOptions,
    internalName ?? name,
  );

  location.setLocationHref(runtimeOptions.location);

  setNumCpus(runtimeOptions.cpuCount);
  setLanguage(runtimeOptions.locale);

  globalThis.pollForMessages = pollForMessages;

  const internalSymbol = Symbol("Deno.internal");

  // These have to initialized here and not in `90_deno_ns.js` because
  // the op function that needs to be passed will be invalidated by creating
  // a snapshot
  ObjectAssign(internals, {
    nodeUnstable: {
      Command: spawn.createCommand(
        spawn.createSpawn(ops.op_node_unstable_spawn_child),
        spawn.createSpawnSync(
          ops.op_node_unstable_spawn_sync,
        ),
        spawn.createSpawnChild(
          ops.op_node_unstable_spawn_child,
        ),
      ),
      serve: flash.createServe(ops.op_node_unstable_flash_serve),
      upgradeHttpRaw: flash.upgradeHttpRaw,
      listenDatagram: net.createListenDatagram(
        ops.op_node_unstable_net_listen_udp,
        ops.op_node_unstable_net_listen_unixpacket,
      ),
      osUptime: os.createOsUptime(ops.op_node_unstable_os_uptime),
    },
  });

  // FIXME(bartlomieju): temporarily add whole `Deno.core` to
  // `Deno[Deno.internal]` namespace. It should be removed and only necessary
  // methods should be left there.
  ObjectAssign(internals, {
    core,
  });

  const finalDenoNs = {
    internal: internalSymbol,
    [internalSymbol]: internals,
    resources: core.resources,
    close: core.close,
    ...denoNs,
  };
  if (runtimeOptions.unstableFlag) {
    ObjectAssign(finalDenoNs, denoNsUnstable);
    // These have to initialized here and not in `90_deno_ns.js` because
    // the op function that needs to be passed will be invalidated by creating
    // a snapshot
    ObjectAssign(finalDenoNs, {
      Command: spawn.createCommand(
        spawn.createSpawn(ops.op_spawn_child),
        spawn.createSpawnSync(ops.op_spawn_sync),
        spawn.createSpawnChild(ops.op_spawn_child),
      ),
      serve: flash.createServe(ops.op_flash_serve),
      listenDatagram: net.createListenDatagram(
        ops.op_net_listen_udp,
        ops.op_net_listen_unixpacket,
      ),
      osUptime: os.createOsUptime(ops.op_os_uptime),
    });
  }
  ObjectDefineProperties(finalDenoNs, {
    pid: util.readOnly(runtimeOptions.pid),
    noColor: util.readOnly(runtimeOptions.noColor),
    args: util.readOnly(ObjectFreeze(runtimeOptions.args)),
  });
  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));
}

ObjectDefineProperties(globalThis, {
  bootstrap: {
    value: {
      mainRuntime: bootstrapMainRuntime,
      workerRuntime: bootstrapWorkerRuntime,
    },
    configurable: true,
  },
});
