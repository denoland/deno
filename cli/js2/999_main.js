// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete Object.prototype.__proto__;

((window) => {
  const core = Deno.core;
  const util = window.__util;
  const eventTarget = window.__eventTarget;
  const dispatchJson = window.__dispatchJson;
  const dispatchMinimal = window.__dispatchMinimal;
  const build = window.__build;
  const version = window.__version;
  const errorStack = window.__errorStack;
  const os = window.__os;
  const timers = window.__timers;
  const replLoop = window.__repl.replLoop;
  const Console = window.__console.Console;
  const worker = window.__worker;
  const signals = window.__signals;
  const { internalSymbol, internalObject } = window.__internals;
  const abortSignal = window.__abortSignal;
  const performance = window.__performance;
  const crypto = window.__crypto;
  const url = window.__url;
  const headers = window.__headers;

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

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScopeMethods = {
    atob: util.writable(atob),
    btoa: util.writable(btoa),
    clearInterval: util.writable(timers.clearInterval),
    clearTimeout: util.writable(timers.clearTimeout),
    // fetch: util.writable(fetchTypes.fetch),
    // queueMicrotask is bound in Rust
    setInterval: util.writable(timers.setInterval),
    setTimeout: util.writable(timers.setTimeout),
  };

  // Other properties shared between WindowScope and WorkerGlobalScope
  const windowOrWorkerGlobalScopeProperties = {
    console: util.writable(new Console(core.print)),
    AbortController: util.nonEnumerable(abortSignal.AbortController),
    AbortSignal: util.nonEnumerable(abortSignal.AbortSignal),
    // Blob: util.nonEnumerable(blob.DenoBlob),
    // ByteLengthQueuingStrategy: util.nonEnumerable(
    //   queuingStrategy.ByteLengthQueuingStrategyImpl,
    // ),
    // CountQueuingStrategy: util.nonEnumerable(queuingStrategy.CountQueuingStrategyImpl),
    crypto: util.readOnly(crypto),
    // File: util.nonEnumerable(domFile.DomFileImpl),
    CustomEvent: util.nonEnumerable(CustomEvent),
    DOMException: util.nonEnumerable(DOMException),
    ErrorEvent: util.nonEnumerable(ErrorEvent),
    Event: util.nonEnumerable(Event),
    EventTarget: util.nonEnumerable(EventTarget),
    Headers: util.nonEnumerable(headers.HeadersImpl),
    // FormData: util.nonEnumerable(formData.FormDataImpl),
    // ReadableStream: util.nonEnumerable(readableStream.ReadableStreamImpl),
    // Request: util.nonEnumerable(request.Request),
    // Response: util.nonEnumerable(fetchTypes.Response),
    performance: util.writable(new performance.Performance()),
    Performance: util.nonEnumerable(performance.Performance),
    PerformanceEntry: util.nonEnumerable(performance.PerformanceEntry),
    PerformanceMark: util.nonEnumerable(performance.PerformanceMark),
    PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasure),
    TextDecoder: util.nonEnumerable(TextDecoder),
    TextEncoder: util.nonEnumerable(TextEncoder),
    // TransformStream: util.nonEnumerable(transformStream.TransformStreamImpl),
    URL: util.nonEnumerable(url.URL),
    URLSearchParams: util.nonEnumerable(url.URLSearchParams),
    Worker: util.nonEnumerable(worker.Worker),
    // WritableStream: util.nonEnumerable(writableStream.WritableStreamImpl),
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

    const denoNs = {
      ...window.Deno,
      core,
      [internalSymbol]: internalObject,
    };
    Object.defineProperties(denoNs, {
      pid: util.readOnly(pid),
      ppid: util.readOnly(ppid),
      noColor: util.readOnly(noColor),
      args: util.readOnly(Object.freeze(args)),
    });

    if (unstableFlag) {
      Object.defineProperty(
        denoNs,
        "mainModule",
        util.getterOnly(opMainModule),
      );
      //   Object.assign(denoNs, denoUnstableNs);
    }

    // Setup `Deno` global - we're actually overriding already
    // existing global `Deno` with `Deno` namespace from "./deno.ts".
    util.immutableDefine(globalThis, "Deno", denoNs);
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

    const denoNs = {
      ...window.Deno,
      core,
      [internalSymbol]: internalObject,
    };
    if (useDenoNamespace) {
      if (unstableFlag) {
        // Object.assign(denoNs, denoUnstableNs);
      }
      Object.defineProperties(denoNs, {
        pid: util.readOnly(pid),
        noColor: util.readOnly(noColor),
        args: util.readOnly(Object.freeze(args)),
      });
      // Setup `Deno` global - we're actually overriding already
      // existing global `Deno` with `Deno` namespace from "./deno.ts".
      util.immutableDefine(globalThis, "Deno", denoNs);
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
