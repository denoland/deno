System.register(
  "$deno$/ops/os.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/errors.ts"],
  function (exports_46, context_46) {
    "use strict";
    let dispatch_json_ts_18, errors_ts_5;
    const __moduleName = context_46 && context_46.id;
    function loadavg() {
      return dispatch_json_ts_18.sendSync("op_loadavg");
    }
    exports_46("loadavg", loadavg);
    function hostname() {
      return dispatch_json_ts_18.sendSync("op_hostname");
    }
    exports_46("hostname", hostname);
    function osRelease() {
      return dispatch_json_ts_18.sendSync("op_os_release");
    }
    exports_46("osRelease", osRelease);
    function exit(code = 0) {
      dispatch_json_ts_18.sendSync("op_exit", { code });
      throw new Error("Code not reachable");
    }
    exports_46("exit", exit);
    function setEnv(key, value) {
      dispatch_json_ts_18.sendSync("op_set_env", { key, value });
    }
    function getEnv(key) {
      return dispatch_json_ts_18.sendSync("op_get_env", { key })[0];
    }
    function env(key) {
      if (key) {
        return getEnv(key);
      }
      const env = dispatch_json_ts_18.sendSync("op_env");
      return new Proxy(env, {
        set(obj, prop, value) {
          setEnv(prop, value);
          return Reflect.set(obj, prop, value);
        },
      });
    }
    exports_46("env", env);
    function dir(kind) {
      try {
        return dispatch_json_ts_18.sendSync("op_get_dir", { kind });
      } catch (error) {
        if (error instanceof errors_ts_5.errors.PermissionDenied) {
          throw error;
        }
        return null;
      }
    }
    exports_46("dir", dir);
    function execPath() {
      return dispatch_json_ts_18.sendSync("op_exec_path");
    }
    exports_46("execPath", execPath);
    return {
      setters: [
        function (dispatch_json_ts_18_1) {
          dispatch_json_ts_18 = dispatch_json_ts_18_1;
        },
        function (errors_ts_5_1) {
          errors_ts_5 = errors_ts_5_1;
        },
      ],
      execute: function () {},
    };
  }
);
