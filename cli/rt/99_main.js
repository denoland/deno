// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete Object.prototype.__proto__;

((window) => {
  const core = Deno.core;
  const util = window.__bootstrap.util;
  const eventTarget = window.__bootstrap.eventTarget;
  const dispatchJson = window.__bootstrap.dispatchJson;
  const dispatchMinimal = window.__bootstrap.dispatchMinimal;
  const build = window.__bootstrap.build;
  const version = window.__bootstrap.version;
  const errorStack = window.__bootstrap.errorStack;
  const os = window.__bootstrap.os;
  const timers = window.__bootstrap.timers;
  const replLoop = window.__bootstrap.repl.replLoop;
  const Console = window.__bootstrap.console.Console;
  const worker = window.__bootstrap.worker;
  const signals = window.__bootstrap.signals;
  const { internalSymbol, internalObject } = window.__bootstrap.internals;
  const performance = window.__bootstrap.performance;
  const crypto = window.__bootstrap.crypto;
  const url = window.__bootstrap.url;
  const headers = window.__bootstrap.headers;
  const queuingStrategy = window.__bootstrap.queuingStrategy;
  const streams = window.__bootstrap.streams;
  const blob = window.__bootstrap.blob;
  const domFile = window.__bootstrap.domFile;
  const progressEvent = window.__bootstrap.progressEvent;
  const fileReader = window.__bootstrap.fileReader;
  const formData = window.__bootstrap.formData;
  const request = window.__bootstrap.request;
  const fetch = window.__bootstrap.fetch;
  const denoNs = window.__bootstrap.denoNs;
  const denoNsUnstable = window.__bootstrap.denoNsUnstable;
  const errors = window.__bootstrap.errors.errors;

  let windowIsClosing = false;

  function windowClose() {
    if (!windowIsClosing) {
      windowIsClosing = true;
      // Push a macrotask to exit after a promise resolve.
      // This is not perfect, but should be fine for first pass.
      Promise.resolve().then(() =>
        timers.setTimeout.call(
          null,
          () => {
            // This should be fine, since only Window/MainWorker has .close()
            os.exit(0);
          },
          0,
        )
      );
    }
  }

  const encoder = new TextEncoder();

  function workerClose() {
    if (isClosing) {
      return;
    }

    isClosing = true;
    opCloseWorker();
  }

  // TODO(bartlomieju): remove these funtions
  // Stuff for workers
  const onmessage = () => {};
  const onerror = () => {};

  function postMessage(data) {
    const dataJson = JSON.stringify(data);
    const dataIntArray = encoder.encode(dataJson);
    opPostMessage(dataIntArray);
  }

  let isClosing = false;
  async function workerMessageRecvCallback(data) {
    const msgEvent = new worker.MessageEvent("message", {
      cancelable: false,
      data,
    });

    try {
      if (globalThis["onmessage"]) {
        const result = globalThis.onmessage(msgEvent);
        if (result && "then" in result) {
          await result;
        }
      }
      globalThis.dispatchEvent(msgEvent);
    } catch (e) {
      let handled = false;

      const errorEvent = new ErrorEvent("error", {
        cancelable: true,
        message: e.message,
        lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
        colno: e.columnNumber ? e.columnNumber + 1 : undefined,
        filename: e.fileName,
        error: null,
      });

      if (globalThis["onerror"]) {
        const ret = globalThis.onerror(
          e.message,
          e.fileName,
          e.lineNumber,
          e.columnNumber,
          e,
        );
        handled = ret === true;
      }

      globalThis.dispatchEvent(errorEvent);
      if (errorEvent.defaultPrevented) {
        handled = true;
      }

      if (!handled) {
        throw e;
      }
    }
  }

  function opPostMessage(data) {
    dispatchJson.sendSync("op_worker_post_message", {}, data);
  }

  function opCloseWorker() {
    dispatchJson.sendSync("op_worker_close");
  }

  function opStart() {
    return dispatchJson.sendSync("op_start");
  }

  function opMainModule() {
    return dispatchJson.sendSync("op_main_module");
  }

  function getAsyncHandler(opName) {
    switch (opName) {
      case "op_write":
      case "op_read":
        return dispatchMinimal.asyncMsgFromRust;
      default:
        return dispatchJson.asyncMsgFromRust;
    }
  }

  // TODO(bartlomieju): temporary solution, must be fixed when moving
  // dispatches to separate crates
  function initOps() {
    const opsMap = core.ops();
    for (const [name, opId] of Object.entries(opsMap)) {
      core.setAsyncHandler(opId, getAsyncHandler(name));
    }
    core.setMacrotaskCallback(timers.handleTimerMacrotask);
  }

  function runtimeStart(source) {
    initOps();
    // First we send an empty `Start` message to let the privileged side know we
    // are ready. The response should be a `StartRes` message containing the CLI
    // args and other info.
    const s = opStart();
    version.setVersions(s.denoVersion, s.v8Version, s.tsVersion);
    build.setBuildInfo(s.target);
    util.setLogDebug(s.debugFlag, source);
    errorStack.setPrepareStackTrace(Error);
    return s;
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
    core.registerErrorClass("URIError", URIError);
    core.registerErrorClass("TypeError", TypeError);
    core.registerErrorClass("Other", Error);
    core.registerErrorClass("Busy", errors.Busy);
  }

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScopeMethods = {
    atob: util.writable(atob),
    btoa: util.writable(btoa),
    clearInterval: util.writable(timers.clearInterval),
    clearTimeout: util.writable(timers.clearTimeout),
    fetch: util.writable(fetch.fetch),
    // queueMicrotask is bound in Rust
    setInterval: util.writable(timers.setInterval),
    setTimeout: util.writable(timers.setTimeout),
  };

  // Other properties shared between WindowScope and WorkerGlobalScope
  const windowOrWorkerGlobalScopeProperties = {
    console: util.writable(new Console(core.print)),
    Blob: util.nonEnumerable(blob.Blob),
    ByteLengthQueuingStrategy: util.nonEnumerable(
      queuingStrategy.ByteLengthQueuingStrategy,
    ),
    CountQueuingStrategy: util.nonEnumerable(
      queuingStrategy.CountQueuingStrategy,
    ),
    crypto: util.readOnly(crypto),
    File: util.nonEnumerable(domFile.DomFile),
    FileReader: util.nonEnumerable(fileReader.FileReader),
    CustomEvent: util.nonEnumerable(CustomEvent),
    DOMException: util.nonEnumerable(DOMException),
    ErrorEvent: util.nonEnumerable(ErrorEvent),
    Event: util.nonEnumerable(Event),
    EventTarget: util.nonEnumerable(EventTarget),
    Headers: util.nonEnumerable(headers.Headers),
    FormData: util.nonEnumerable(formData.FormData),
    ReadableStream: util.nonEnumerable(streams.ReadableStream),
    Request: util.nonEnumerable(request.Request),
    Response: util.nonEnumerable(fetch.Response),
    performance: util.writable(new performance.Performance()),
    Performance: util.nonEnumerable(performance.Performance),
    PerformanceEntry: util.nonEnumerable(performance.PerformanceEntry),
    PerformanceMark: util.nonEnumerable(performance.PerformanceMark),
    PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasure),
    ProgressEvent: util.nonEnumerable(progressEvent.ProgressEvent),
    TextDecoder: util.nonEnumerable(TextDecoder),
    TextEncoder: util.nonEnumerable(TextEncoder),
    TransformStream: util.nonEnumerable(streams.TransformStream),
    URL: util.nonEnumerable(url.URL),
    URLSearchParams: util.nonEnumerable(url.URLSearchParams),
    Worker: util.nonEnumerable(worker.Worker),
    WritableStream: util.nonEnumerable(streams.WritableStream),
  };

  const eventTargetProperties = {
    addEventListener: util.readOnly(
      EventTarget.prototype.addEventListener,
    ),
    dispatchEvent: util.readOnly(EventTarget.prototype.dispatchEvent),
    removeEventListener: util.readOnly(
      EventTarget.prototype.removeEventListener,
    ),
  };

  const mainRuntimeGlobalProperties = {
    window: util.readOnly(globalThis),
    self: util.readOnly(globalThis),
    // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
    // it seems those two properties should be available to workers as well
    onload: util.writable(null),
    onunload: util.writable(null),
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
  };

  const workerRuntimeGlobalProperties = {
    self: util.readOnly(globalThis),
    onmessage: util.writable(onmessage),
    onerror: util.writable(onerror),
    // TODO: should be readonly?
    close: util.nonEnumerable(workerClose),
    postMessage: util.writable(postMessage),
    workerMessageRecvCallback: util.nonEnumerable(workerMessageRecvCallback),
  };

  let hasBootstrapped = false;

  function bootstrapMainRuntime() {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    // Remove bootstrapping methods from global scope
    globalThis.__bootstrap = undefined;
    globalThis.bootstrap = undefined;
    util.log("bootstrapMainRuntime");
    hasBootstrapped = true;
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
    Object.defineProperties(globalThis, eventTargetProperties);
    Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
    eventTarget.setEventTargetData(globalThis);
    // Registers the handler for window.onload function.
    globalThis.addEventListener("load", (e) => {
      const { onload } = globalThis;
      if (typeof onload === "function") {
        onload(e);
      }
    });
    // Registers the handler for window.onunload function.
    globalThis.addEventListener("unload", (e) => {
      const { onunload } = globalThis;
      if (typeof onunload === "function") {
        onunload(e);
      }
    });

    const { args, cwd, noColor, pid, ppid, repl, unstableFlag } =
      runtimeStart();

    registerErrors();

    const finalDenoNs = {
      core,
      internal: internalSymbol,
      [internalSymbol]: internalObject,
      ...denoNs,
    };
    Object.defineProperties(finalDenoNs, {
      pid: util.readOnly(pid),
      ppid: util.readOnly(ppid),
      noColor: util.readOnly(noColor),
      args: util.readOnly(Object.freeze(args)),
      mainModule: util.getterOnly(opMainModule),
    });

    if (unstableFlag) {
      Object.assign(finalDenoNs, denoNsUnstable);
    }

    // Setup `Deno` global - we're actually overriding already
    // existing global `Deno` with `Deno` namespace from "./deno.ts".
    util.immutableDefine(globalThis, "Deno", finalDenoNs);
    Object.freeze(globalThis.Deno);
    Object.freeze(globalThis.Deno.core);
    Object.freeze(globalThis.Deno.core.sharedQueue);
    signals.setSignals();

    util.log("cwd", cwd);
    util.log("args", args);

    if (repl) {
      replLoop();
    }
  }

  function bootstrapWorkerRuntime(name, useDenoNamespace, internalName) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    // Remove bootstrapping methods from global scope
    globalThis.__bootstrap = undefined;
    globalThis.bootstrap = undefined;
    util.log("bootstrapWorkerRuntime");
    hasBootstrapped = true;
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
    Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
    Object.defineProperties(globalThis, eventTargetProperties);
    Object.defineProperties(globalThis, { name: util.readOnly(name) });
    eventTarget.setEventTargetData(globalThis);
    const { unstableFlag, pid, noColor, args } = runtimeStart(
      internalName ?? name,
    );

    registerErrors();

    const finalDenoNs = {
      core,
      internal: internalSymbol,
      [internalSymbol]: internalObject,
      ...denoNs,
    };
    if (useDenoNamespace) {
      if (unstableFlag) {
        Object.assign(finalDenoNs, denoNsUnstable);
      }
      Object.defineProperties(finalDenoNs, {
        pid: util.readOnly(pid),
        noColor: util.readOnly(noColor),
        args: util.readOnly(Object.freeze(args)),
      });
      // Setup `Deno` global - we're actually overriding already
      // existing global `Deno` with `Deno` namespace from "./deno.ts".
      util.immutableDefine(globalThis, "Deno", finalDenoNs);
      Object.freeze(globalThis.Deno);
      Object.freeze(globalThis.Deno.core);
      Object.freeze(globalThis.Deno.core.sharedQueue);
      signals.setSignals();
    } else {
      delete globalThis.Deno;
      util.assert(globalThis.Deno === undefined);
    }
  }

  Object.defineProperties(globalThis, {
    bootstrap: {
      value: {
        mainRuntime: bootstrapMainRuntime,
        workerRuntime: bootstrapWorkerRuntime,
      },
      configurable: true,
      writable: true,
    },
  });
})(this);
