// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

// Removes the `__proto__` for security reasons.
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
delete Object.prototype.__proto__;

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

((window) => {
  const core = Deno.core;
  const ops = core.ops;
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
    SafeArrayIterator,
    SafeWeakMap,
    TypeError,
    WeakMapPrototypeDelete,
    WeakMapPrototypeGet,
    WeakMapPrototypeSet,
  } = window.__bootstrap.primordials;
  const util = window.__bootstrap.util;
  const event = window.__bootstrap.event;
  const eventTarget = window.__bootstrap.eventTarget;
  const location = window.__bootstrap.location;
  const build = window.__bootstrap.build;
  const version = window.__bootstrap.version;
  const os = window.__bootstrap.os;
  const timers = window.__bootstrap.timers;
  const colors = window.__bootstrap.colors;
  const inspectArgs = window.__bootstrap.console.inspectArgs;
  const quoteString = window.__bootstrap.console.quoteString;
  const internals = window.__bootstrap.internals;
  const performance = window.__bootstrap.performance;
  const net = window.__bootstrap.net;
  const url = window.__bootstrap.url;
  const fetch = window.__bootstrap.fetch;
  const messagePort = window.__bootstrap.messagePort;
  const denoNs = window.__bootstrap.denoNs;
  const denoNsUnstable = window.__bootstrap.denoNsUnstable;
  const errors = window.__bootstrap.errors.errors;
  const webidl = window.__bootstrap.webidl;
  const domException = window.__bootstrap.domException;
  const { defineEventHandler, reportException } = window.__bootstrap.event;
  const { deserializeJsMessageData, serializeJsMessageData } =
    window.__bootstrap.messagePort;
  const {
    windowOrWorkerGlobalScope,
    unstableWindowOrWorkerGlobalScope,
    workerRuntimeGlobalProperties,
    mainRuntimeGlobalProperties,
    setNumCpus,
    setUserAgent,
    setLanguage,
  } = window.__bootstrap.globalScope;

  let windowIsClosing = false;

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
    const data = serializeJsMessageData(message, transfer);
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
      const v = deserializeJsMessageData(data);
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
        throw new domException.DOMException(
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

    for (const { url, script } of new SafeArrayIterator(scripts)) {
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
      return `Uncaught ${
        inspectArgs([error], { colors: !colors.getNoColor() })
      }`;
    }
  }

  function runtimeStart(runtimeOptions, source) {
    core.setMacrotaskCallback(timers.handleTimerMacrotask);
    core.setMacrotaskCallback(promiseRejectMacrotaskCallback);
    core.setWasmStreamingCallback(fetch.handleWasmStreaming);
    core.setReportExceptionCallback(reportException);
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
    core.registerErrorClass("WriteZero", errors.WriteZero);
    core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
    core.registerErrorClass("BadResource", errors.BadResource);
    core.registerErrorClass("Http", errors.Http);
    core.registerErrorClass("Busy", errors.Busy);
    core.registerErrorClass("NotSupported", errors.NotSupported);
    core.registerErrorBuilder(
      "DOMExceptionOperationError",
      function DOMExceptionOperationError(msg) {
        return new domException.DOMException(msg, "OperationError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionQuotaExceededError",
      function DOMExceptionQuotaExceededError(msg) {
        return new domException.DOMException(msg, "QuotaExceededError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionNotSupportedError",
      function DOMExceptionNotSupportedError(msg) {
        return new domException.DOMException(msg, "NotSupported");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionNetworkError",
      function DOMExceptionNetworkError(msg) {
        return new domException.DOMException(msg, "NetworkError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionAbortError",
      function DOMExceptionAbortError(msg) {
        return new domException.DOMException(msg, "AbortError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionInvalidCharacterError",
      function DOMExceptionInvalidCharacterError(msg) {
        return new domException.DOMException(msg, "InvalidCharacterError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionDataError",
      function DOMExceptionDataError(msg) {
        return new domException.DOMException(msg, "DataError");
      },
    );
  }

  const pendingRejections = [];
  const pendingRejectionsReasons = new SafeWeakMap();

  function promiseRejectCallback(type, promise, reason) {
    switch (type) {
      case 0: {
        ops.op_store_pending_promise_exception(promise, reason);
        ArrayPrototypePush(pendingRejections, promise);
        WeakMapPrototypeSet(pendingRejectionsReasons, promise, reason);
        break;
      }
      case 1: {
        ops.op_remove_pending_promise_exception(promise);
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

    return !!globalThis.onunhandledrejection ||
      eventTarget.listenerCount(globalThis, "unhandledrejection") > 0;
  }

  function promiseRejectMacrotaskCallback() {
    while (pendingRejections.length > 0) {
      const promise = ArrayPrototypeShift(pendingRejections);
      const hasPendingException = ops.op_has_pending_promise_exception(
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
          ops.op_remove_pending_promise_exception(promise);
        }
      };
      // Add a callback for "error" event - it will be dispatched
      // if error is thrown during dispatch of "unhandledrejection"
      // event.
      globalThis.addEventListener("error", errorEventCb);
      globalThis.dispatchEvent(rejectionEvent);
      globalThis.removeEventListener("error", errorEventCb);

      // If event was not prevented (or "unhandledrejection" listeners didn't
      // throw) we will let Rust side handle it.
      if (rejectionEvent.defaultPrevented) {
        ops.op_remove_pending_promise_exception(promise);
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
    net.setup(runtimeOptions.unstableFlag);

    const consoleFromV8 = window.console;
    const wrapConsole = window.__bootstrap.console.wrapConsole;

    // Remove bootstrapping data from the global scope
    const __bootstrap = globalThis.__bootstrap;
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

    eventTarget.setEventTargetData(globalThis);

    defineEventHandler(window, "error");
    defineEventHandler(window, "load");
    defineEventHandler(window, "beforeunload");
    defineEventHandler(window, "unload");
    defineEventHandler(window, "unhandledrejection");

    core.setPromiseRejectCallback(promiseRejectCallback);

    const isUnloadDispatched = SymbolFor("isUnloadDispatched");
    // Stores the flag for checking whether unload is dispatched or not.
    // This prevents the recursive dispatches of unload events.
    // See https://github.com/denoland/deno/issues/9201.
    window[isUnloadDispatched] = false;
    window.addEventListener("unload", () => {
      window[isUnloadDispatched] = true;
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
        Command: __bootstrap.spawn.createCommand(
          __bootstrap.spawn.createSpawn(ops.op_node_unstable_spawn_child),
          __bootstrap.spawn.createSpawnSync(
            ops.op_node_unstable_spawn_sync,
          ),
          __bootstrap.spawn.createSpawnChild(
            ops.op_node_unstable_spawn_child,
          ),
        ),
        serve: __bootstrap.flash.createServe(ops.op_node_unstable_flash_serve),
        upgradeHttpRaw: __bootstrap.flash.upgradeHttpRaw,
        listenDatagram: __bootstrap.net.createListenDatagram(
          ops.op_node_unstable_net_listen_udp,
          ops.op_node_unstable_net_listen_unixpacket,
        ),
      },
    });

    const finalDenoNs = {
      core,
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
        Command: __bootstrap.spawn.createCommand(
          __bootstrap.spawn.createSpawn(ops.op_spawn_child),
          __bootstrap.spawn.createSpawnSync(ops.op_spawn_sync),
          __bootstrap.spawn.createSpawnChild(ops.op_spawn_child),
        ),
        serve: __bootstrap.flash.createServe(ops.op_flash_serve),
        listenDatagram: __bootstrap.net.createListenDatagram(
          ops.op_net_listen_udp,
          ops.op_net_listen_unixpacket,
        ),
      });
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));
    ObjectFreeze(globalThis.Deno.core);

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
    net.setup(runtimeOptions.unstableFlag);

    const consoleFromV8 = window.console;
    const wrapConsole = window.__bootstrap.console.wrapConsole;

    // Remove bootstrapping data from the global scope
    const __bootstrap = globalThis.__bootstrap;
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

    eventTarget.setEventTargetData(globalThis);

    defineEventHandler(self, "message");
    defineEventHandler(self, "error", undefined, true);
    defineEventHandler(self, "unhandledrejection");

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
        Command: __bootstrap.spawn.createCommand(
          __bootstrap.spawn.createSpawn(ops.op_node_unstable_spawn_child),
          __bootstrap.spawn.createSpawnSync(
            ops.op_node_unstable_spawn_sync,
          ),
          __bootstrap.spawn.createSpawnChild(
            ops.op_node_unstable_spawn_child,
          ),
        ),
        serve: __bootstrap.flash.createServe(ops.op_node_unstable_flash_serve),
        upgradeHttpRaw: __bootstrap.flash.upgradeHttpRaw,
        listenDatagram: __bootstrap.net.createListenDatagram(
          ops.op_node_unstable_net_listen_udp,
          ops.op_node_unstable_net_listen_unixpacket,
        ),
      },
    });

    const finalDenoNs = {
      core,
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
        Command: __bootstrap.spawn.createCommand(
          __bootstrap.spawn.createSpawn(ops.op_spawn_child),
          __bootstrap.spawn.createSpawnSync(ops.op_spawn_sync),
          __bootstrap.spawn.createSpawnChild(ops.op_spawn_child),
        ),
        serve: __bootstrap.flash.createServe(ops.op_flash_serve),
        listenDatagram: __bootstrap.net.createListenDatagram(
          ops.op_net_listen_udp,
          ops.op_net_listen_unixpacket,
        ),
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
    ObjectFreeze(globalThis.Deno.core);
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
})(this);
