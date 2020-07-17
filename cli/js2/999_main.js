// Removes the `__proto__` for security reasons.  This intentionally makes
// Deno non compliant with ECMA-262 Annex B.2.2.1
//
// eslint-disable-next-line @typescript-eslint/no-explicit-any
delete Object.prototype.__proto__;

((window) => {
  const util = window.__util;
  const eventTarget = window.__eventTarget;
  const dispatchJson = window.__dispatchJson;
  const dispatchMinimal = window.__dispatchMinimal;
  const build = window.__build;
  const version = window.__version;
  const errorStack = window.__errorStack;

  let windowIsClosing = false;

  function windowClose() {
    if (!windowIsClosing) {
      windowIsClosing = true;
      // Push a macrotask to exit after a promise resolve.
      // This is not perfect, but should be fine for first pass.
      Promise.resolve().then(() =>
        setTimeout.call(
          null,
          () => {
            // This should be fine, since only Window/MainWorker has .close()
            // TODO:
            // exit(0);
            throw new Error("close not implemented");
          },
          0,
        )
      );
    }
  }

  const core = Deno.core;

  function opStart() {
    return dispatchJson.sendSync("op_start");
  }

  function opMainModule() {
    return dispatchJson.sendSync("op_main_module");
  }

  function opMetrics() {
    return dispatchJson.sendSync("op_metrics");
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
    // TODO:
    // core.setMacrotaskCallback(handleTimerMacrotask);
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
    Deno.core.print(`startup ${JSON.stringify(s, null, 2)}\n`, true);
    return s;
  }

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScopeMethods = {
    atob: util.writable(atob),
    btoa: util.writable(btoa),
    // clearInterval: util.writable(timers.clearInterval),
    // clearTimeout: util.writable(timers.clearTimeout),
    // fetch: util.writable(fetchTypes.fetch),
    // queueMicrotask is bound in Rust
    // setInterval: util.writable(timers.setInterval),
    // setTimeout: util.writable(timers.setTimeout),
  };

  // Other properties shared between WindowScope and WorkerGlobalScope
  const windowOrWorkerGlobalScopeProperties = {
    // console: util.writable(new consoleTypes.Console(core.print)),
    // AbortController: util.nonEnumerable(abortController.AbortControllerImpl),
    // AbortSignal: util.nonEnumerable(abortSignal.AbortSignalImpl),
    // Blob: util.nonEnumerable(blob.DenoBlob),
    // ByteLengthQueuingStrategy: util.nonEnumerable(
    //   queuingStrategy.ByteLengthQueuingStrategyImpl,
    // ),
    // CountQueuingStrategy: util.nonEnumerable(queuingStrategy.CountQueuingStrategyImpl),
    // crypto: util.readOnly(csprng),
    // File: util.nonEnumerable(domFile.DomFileImpl),
    CustomEvent: util.nonEnumerable(CustomEvent),
    DOMException: util.nonEnumerable(DOMException),
    ErrorEvent: util.nonEnumerable(ErrorEvent),
    Event: util.nonEnumerable(Event),
    EventTarget: util.nonEnumerable(EventTarget),
    // Headers: util.nonEnumerable(headers.HeadersImpl),
    // FormData: util.nonEnumerable(formData.FormDataImpl),
    // ReadableStream: util.nonEnumerable(readableStream.ReadableStreamImpl),
    // Request: util.nonEnumerable(request.Request),
    // Response: util.nonEnumerable(fetchTypes.Response),
    // performance: util.writable(new performance.PerformanceImpl()),
    // Performance: util.nonEnumerable(performance.PerformanceImpl),
    // PerformanceEntry: util.nonEnumerable(performance.PerformanceEntryImpl),
    // PerformanceMark: util.nonEnumerable(performance.PerformanceMarkImpl),
    // PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasureImpl),
    // TextDecoder: util.nonEnumerable(textEncoding.TextDecoder),
    // TextEncoder: util.nonEnumerable(textEncoding.TextEncoder),
    // TransformStream: util.nonEnumerable(transformStream.TransformStreamImpl),
    // URL: util.nonEnumerable(url.URLImpl),
    // URLSearchParams: util.nonEnumerable(urlSearchParams.URLSearchParamsImpl),
    // Worker: util.nonEnumerable(workers.WorkerImpl),
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
      core,
    };
    Object.defineProperties(denoNs, {
      pid: util.readOnly(pid),
      ppid: util.readOnly(ppid),
      noColor: util.readOnly(noColor),
      args: util.readOnly(Object.freeze(args)),
    });

    if (unstableFlag) {
      //   Object.defineProperties(globalThis, unstableMethods);
      //   Object.defineProperties(globalThis, unstableProperties);
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
    // setSignals();

    util.log("cwd", cwd);
    util.log("args", args);

    if (repl) {
      // replLoop();
      throw new Error("repl not implemented");
    }
  }

  function bootstrapWorkerRuntime() {
    Deno.core.print("hello\n");
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
