System.register(
  "$deno$/plugins.ts",
  ["$deno$/ops/plugins.ts", "$deno$/core.ts"],
  function (exports_50, context_50) {
    "use strict";
    let plugins_ts_1, core_ts_5, PluginOpImpl, PluginImpl;
    const __moduleName = context_50 && context_50.id;
    function openPlugin(filename) {
      const response = plugins_ts_1.openPlugin(filename);
      return new PluginImpl(response.rid, response.ops);
    }
    exports_50("openPlugin", openPlugin);
    return {
      setters: [
        function (plugins_ts_1_1) {
          plugins_ts_1 = plugins_ts_1_1;
        },
        function (core_ts_5_1) {
          core_ts_5 = core_ts_5_1;
        },
      ],
      execute: function () {
        PluginOpImpl = class PluginOpImpl {
          constructor(opId) {
            this.#opId = opId;
          }
          #opId;
          dispatch(control, zeroCopy) {
            return core_ts_5.core.dispatch(this.#opId, control, zeroCopy);
          }
          setAsyncHandler(handler) {
            core_ts_5.core.setAsyncHandler(this.#opId, handler);
          }
        };
        PluginImpl = class PluginImpl {
          constructor(_rid, ops) {
            this.#ops = {};
            for (const op in ops) {
              this.#ops[op] = new PluginOpImpl(ops[op]);
            }
          }
          #ops;
          get ops() {
            return Object.assign({}, this.#ops);
          }
        };
      },
    };
  }
);
