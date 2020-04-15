// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/runtime_main.ts",
  [
    "$deno$/deno.ts",
    "$deno$/ops/get_random_values.ts",
    "$deno$/ops/os.ts",
    "$deno$/globals.ts",
    "$deno$/internals.ts",
    "$deno$/signals.ts",
    "$deno$/repl.ts",
    "$deno$/web/location.ts",
    "$deno$/web/timers.ts",
    "$deno$/runtime.ts",
    "$deno$/symbols.ts",
    "$deno$/util.ts",
  ],
  function (exports_107, context_107) {
    "use strict";
    let Deno,
      csprng,
      os_ts_4,
      globals_ts_1,
      internals_ts_6,
      signals_ts_2,
      repl_ts_2,
      location_ts_1,
      timers_ts_4,
      runtime,
      symbols_ts_2,
      util_ts_22,
      windowIsClosing,
      mainRuntimeGlobalProperties,
      hasBootstrapped;
    const __moduleName = context_107 && context_107.id;
    function windowClose() {
      if (!windowIsClosing) {
        windowIsClosing = true;
        // Push a macrotask to exit after a promise resolve.
        // This is not perfect, but should be fine for first pass.
        Promise.resolve().then(() =>
          timers_ts_4.setTimeout.call(
            null,
            () => {
              // This should be fine, since only Window/MainWorker has .close()
              os_ts_4.exit(0);
            },
            0
          )
        );
      }
    }
    function bootstrapMainRuntime() {
      if (hasBootstrapped) {
        throw new Error("Worker runtime already bootstrapped");
      }
      util_ts_22.log("bootstrapMainRuntime");
      hasBootstrapped = true;
      Object.defineProperties(
        globalThis,
        globals_ts_1.windowOrWorkerGlobalScopeMethods
      );
      Object.defineProperties(
        globalThis,
        globals_ts_1.windowOrWorkerGlobalScopeProperties
      );
      Object.defineProperties(globalThis, globals_ts_1.eventTargetProperties);
      Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
      globals_ts_1.setEventTargetData(globalThis);
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
      const s = runtime.start();
      const location = new location_ts_1.LocationImpl(s.location);
      util_ts_22.immutableDefine(globalThis, "location", location);
      Object.freeze(globalThis.location);
      Object.defineProperties(Deno, {
        pid: globals_ts_1.readOnly(s.pid),
        noColor: globals_ts_1.readOnly(s.noColor),
        args: globals_ts_1.readOnly(Object.freeze(s.args)),
      });
      // Setup `Deno` global - we're actually overriding already
      // existing global `Deno` with `Deno` namespace from "./deno.ts".
      util_ts_22.immutableDefine(globalThis, "Deno", Deno);
      Object.freeze(globalThis.Deno);
      Object.freeze(globalThis.Deno.core);
      Object.freeze(globalThis.Deno.core.sharedQueue);
      signals_ts_2.setSignals();
      util_ts_22.log("cwd", s.cwd);
      util_ts_22.log("args", Deno.args);
      if (s.repl) {
        repl_ts_2.replLoop();
      }
    }
    exports_107("bootstrapMainRuntime", bootstrapMainRuntime);
    return {
      setters: [
        function (Deno_1) {
          Deno = Deno_1;
        },
        function (csprng_1) {
          csprng = csprng_1;
        },
        function (os_ts_4_1) {
          os_ts_4 = os_ts_4_1;
        },
        function (globals_ts_1_1) {
          globals_ts_1 = globals_ts_1_1;
        },
        function (internals_ts_6_1) {
          internals_ts_6 = internals_ts_6_1;
        },
        function (signals_ts_2_1) {
          signals_ts_2 = signals_ts_2_1;
        },
        function (repl_ts_2_1) {
          repl_ts_2 = repl_ts_2_1;
        },
        function (location_ts_1_1) {
          location_ts_1 = location_ts_1_1;
        },
        function (timers_ts_4_1) {
          timers_ts_4 = timers_ts_4_1;
        },
        function (runtime_1) {
          runtime = runtime_1;
        },
        function (symbols_ts_2_1) {
          symbols_ts_2 = symbols_ts_2_1;
        },
        function (util_ts_22_1) {
          util_ts_22 = util_ts_22_1;
        },
      ],
      execute: function () {
        // TODO: factor out `Deno` global assignment to separate function
        // Add internal object to Deno object.
        // This is not exposed as part of the Deno types.
        // @ts-ignore
        Deno[symbols_ts_2.symbols.internal] = internals_ts_6.internalObject;
        windowIsClosing = false;
        exports_107(
          "mainRuntimeGlobalProperties",
          (mainRuntimeGlobalProperties = {
            window: globals_ts_1.readOnly(globalThis),
            self: globals_ts_1.readOnly(globalThis),
            crypto: globals_ts_1.readOnly(csprng),
            // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
            // it seems those two properties should be available to workers as well
            onload: globals_ts_1.writable(null),
            onunload: globals_ts_1.writable(null),
            close: globals_ts_1.writable(windowClose),
            closed: globals_ts_1.getterOnly(() => windowIsClosing),
          })
        );
        hasBootstrapped = false;
      },
    };
  }
);
