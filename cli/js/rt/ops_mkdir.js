System.register(
  "$deno$/ops/fs/mkdir.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_43, context_43) {
    "use strict";
    let dispatch_json_ts_16;
    const __moduleName = context_43 && context_43.id;
    function mkdirArgs(path, options) {
      const args = { path, recursive: false };
      if (options) {
        if (typeof options.recursive == "boolean") {
          args.recursive = options.recursive;
        }
        if (options.mode) {
          args.mode = options.mode;
        }
      }
      return args;
    }
    function mkdirSync(path, options) {
      dispatch_json_ts_16.sendSync("op_mkdir", mkdirArgs(path, options));
    }
    exports_43("mkdirSync", mkdirSync);
    async function mkdir(path, options) {
      await dispatch_json_ts_16.sendAsync("op_mkdir", mkdirArgs(path, options));
    }
    exports_43("mkdir", mkdir);
    return {
      setters: [
        function (dispatch_json_ts_16_1) {
          dispatch_json_ts_16 = dispatch_json_ts_16_1;
        },
      ],
      execute: function () {},
    };
  }
);
