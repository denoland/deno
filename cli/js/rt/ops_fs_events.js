System.register(
  "$deno$/ops/fs_events.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/ops/resources.ts"],
  function (exports_40, context_40) {
    "use strict";
    let dispatch_json_ts_13, resources_ts_2, FsEvents;
    const __moduleName = context_40 && context_40.id;
    function fsEvents(paths, options = { recursive: true }) {
      return new FsEvents(Array.isArray(paths) ? paths : [paths], options);
    }
    exports_40("fsEvents", fsEvents);
    return {
      setters: [
        function (dispatch_json_ts_13_1) {
          dispatch_json_ts_13 = dispatch_json_ts_13_1;
        },
        function (resources_ts_2_1) {
          resources_ts_2 = resources_ts_2_1;
        },
      ],
      execute: function () {
        FsEvents = class FsEvents {
          constructor(paths, options) {
            const { recursive } = options;
            this.rid = dispatch_json_ts_13.sendSync("op_fs_events_open", {
              recursive,
              paths,
            });
          }
          next() {
            return dispatch_json_ts_13.sendAsync("op_fs_events_poll", {
              rid: this.rid,
            });
          }
          return(value) {
            resources_ts_2.close(this.rid);
            return Promise.resolve({ value, done: true });
          }
          [Symbol.asyncIterator]() {
            return this;
          }
        };
      },
    };
  }
);
