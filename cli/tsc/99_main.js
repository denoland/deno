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
  const build = window.__bootstrap.build;
  const version = window.__bootstrap.version;
  const errorStack = window.__bootstrap.errorStack;
  const Console = window.__bootstrap.console.Console;
  const worker = window.__bootstrap.worker;
  const { internalSymbol, internalObject } = window.__bootstrap.internals;
  const performance = window.__bootstrap.performance;
  const crypto = window.__bootstrap.crypto;
  const denoNs = window.__bootstrap.denoNs;

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

  // TODO(bartlomieju): temporary solution, must be fixed when moving
  // dispatches to separate crates
  function initOps() {
    const opsMap = core.ops();
    for (const [_name, opId] of Object.entries(opsMap)) {
      core.setAsyncHandler(opId, dispatchJson.asyncMsgFromRust);
    }
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

  // Other properties shared between WindowScope and WorkerGlobalScope
  const windowOrWorkerGlobalScopeProperties = {
    console: util.writable(new Console(core.print)),
    crypto: util.readOnly(crypto),
    CustomEvent: util.nonEnumerable(CustomEvent),
    ErrorEvent: util.nonEnumerable(ErrorEvent),
    Event: util.nonEnumerable(Event),
    EventTarget: util.nonEnumerable(EventTarget),
    performance: util.writable(new performance.Performance()),
    Performance: util.nonEnumerable(performance.Performance),
    PerformanceEntry: util.nonEnumerable(performance.PerformanceEntry),
    PerformanceMark: util.nonEnumerable(performance.PerformanceMark),
    PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasure),
    TextDecoder: util.nonEnumerable(TextDecoder),
    TextEncoder: util.nonEnumerable(TextEncoder),
    Worker: util.nonEnumerable(worker.Worker),
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

  function bootstrapWorkerRuntime(name, useDenoNamespace, internalName) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    // Remove bootstrapping methods from global scope
    globalThis.__bootstrap = undefined;
    globalThis.bootstrap = undefined;
    util.log("bootstrapWorkerRuntime");
    hasBootstrapped = true;
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
    Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
    Object.defineProperties(globalThis, eventTargetProperties);
    Object.defineProperties(globalThis, { name: util.readOnly(name) });
    eventTarget.setEventTargetData(globalThis);
    const { noColor, args } = runtimeStart(
      internalName ?? name,
    );

    const finalDenoNs = {
      core,
      internal: internalSymbol,
      [internalSymbol]: internalObject,
      ...denoNs,
    };
    if (useDenoNamespace) {
      Object.defineProperties(finalDenoNs, {
        noColor: util.readOnly(noColor),
        args: util.readOnly(Object.freeze(args)),
      });
      // Setup `Deno` global - we're actually overriding already
      // existing global `Deno` with `Deno` namespace from "./deno.ts".
      util.immutableDefine(globalThis, "Deno", finalDenoNs);
      Object.freeze(globalThis.Deno);
      Object.freeze(globalThis.Deno.core);
      Object.freeze(globalThis.Deno.core.sharedQueue);
    } else {
      delete globalThis.Deno;
      util.assert(globalThis.Deno === undefined);
    }
  }

  Object.defineProperties(globalThis, {
    bootstrap: {
      value: {
        workerRuntime: bootstrapWorkerRuntime,
      },
      configurable: true,
      writable: true,
    },
  });
})(this);
