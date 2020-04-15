// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/runtime_worker.ts",
  [
    "$deno$/globals.ts",
    "$deno$/ops/web_worker.ts",
    "$deno$/web/location.ts",
    "$deno$/util.ts",
    "$deno$/web/workers.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/runtime.ts",
  ],
  function (exports_109, context_109) {
    "use strict";
    let globals_ts_2,
      webWorkerOps,
      location_ts_2,
      util_ts_23,
      workers_ts_1,
      text_encoding_ts_9,
      runtime,
      encoder,
      onmessage,
      onerror,
      isClosing,
      hasBootstrapped,
      workerRuntimeGlobalProperties;
    const __moduleName = context_109 && context_109.id;
    function postMessage(data) {
      const dataJson = JSON.stringify(data);
      const dataIntArray = encoder.encode(dataJson);
      webWorkerOps.postMessage(dataIntArray);
    }
    exports_109("postMessage", postMessage);
    function close() {
      if (isClosing) {
        return;
      }
      isClosing = true;
      webWorkerOps.close();
    }
    exports_109("close", close);
    async function workerMessageRecvCallback(data) {
      const msgEvent = new workers_ts_1.MessageEvent("message", {
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
        const errorEvent = new workers_ts_1.ErrorEvent("error", {
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
            e
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
    exports_109("workerMessageRecvCallback", workerMessageRecvCallback);
    function bootstrapWorkerRuntime(name, internalName) {
      if (hasBootstrapped) {
        throw new Error("Worker runtime already bootstrapped");
      }
      util_ts_23.log("bootstrapWorkerRuntime");
      hasBootstrapped = true;
      Object.defineProperties(
        globalThis,
        globals_ts_2.windowOrWorkerGlobalScopeMethods
      );
      Object.defineProperties(
        globalThis,
        globals_ts_2.windowOrWorkerGlobalScopeProperties
      );
      Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
      Object.defineProperties(globalThis, globals_ts_2.eventTargetProperties);
      Object.defineProperties(globalThis, {
        name: globals_ts_2.readOnly(name),
      });
      globals_ts_2.setEventTargetData(globalThis);
      const s = runtime.start(internalName ?? name);
      const location = new location_ts_2.LocationImpl(s.location);
      util_ts_23.immutableDefine(globalThis, "location", location);
      Object.freeze(globalThis.location);
      // globalThis.Deno is not available in worker scope
      delete globalThis.Deno;
      util_ts_23.assert(globalThis.Deno === undefined);
    }
    exports_109("bootstrapWorkerRuntime", bootstrapWorkerRuntime);
    return {
      setters: [
        function (globals_ts_2_1) {
          globals_ts_2 = globals_ts_2_1;
        },
        function (webWorkerOps_1) {
          webWorkerOps = webWorkerOps_1;
        },
        function (location_ts_2_1) {
          location_ts_2 = location_ts_2_1;
        },
        function (util_ts_23_1) {
          util_ts_23 = util_ts_23_1;
        },
        function (workers_ts_1_1) {
          workers_ts_1 = workers_ts_1_1;
        },
        function (text_encoding_ts_9_1) {
          text_encoding_ts_9 = text_encoding_ts_9_1;
        },
        function (runtime_2) {
          runtime = runtime_2;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_9.TextEncoder();
        // TODO(bartlomieju): remove these funtions
        // Stuff for workers
        exports_109("onmessage", (onmessage = () => {}));
        exports_109("onerror", (onerror = () => {}));
        isClosing = false;
        hasBootstrapped = false;
        exports_109(
          "workerRuntimeGlobalProperties",
          (workerRuntimeGlobalProperties = {
            self: globals_ts_2.readOnly(globalThis),
            onmessage: globals_ts_2.writable(onmessage),
            onerror: globals_ts_2.writable(onerror),
            // TODO: should be readonly?
            close: globals_ts_2.nonEnumerable(close),
            postMessage: globals_ts_2.writable(postMessage),
            workerMessageRecvCallback: globals_ts_2.nonEnumerable(
              workerMessageRecvCallback
            ),
          })
        );
      },
    };
  }
);
