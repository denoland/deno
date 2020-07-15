/* eslint-disable */
System.register("internal:///deno_typescript/tests/util.ts", [], function (
  exports_1,
  context_1,
) {
  "use strict";
  var __moduleName = context_1 && context_1.id;
  function log(...s) {
    s;
  }
  exports_1("log", log);
  return {
    setters: [],
    execute: function () {},
  };
});
System.register("internal:///deno_typescript/tests/types.ts", [], function (
  exports_2,
  context_2,
) {
  "use strict";
  var __moduleName = context_2 && context_2.id;
  return {
    setters: [],
    execute: function () {},
  };
});
System.register(
  "internal:///deno_typescript/tests/ops/dispatch.ts",
  [],
  function (exports_3, context_3) {
    "use strict";
    var __moduleName = context_3 && context_3.id;
    function sendSync(_opName, _args) {
      return { ok: new Uint8Array() };
    }
    exports_3("sendSync", sendSync);
    return {
      setters: [],
      execute: function () {},
    };
  },
);
System.register(
  "internal:///deno_typescript/tests/ops/fs.ts",
  [
    "internal:///deno_typescript/tests/util.ts",
    "internal:///deno_typescript/tests/ops/dispatch.ts",
  ],
  function (exports_4, context_4) {
    "use strict";
    var util_ts_1, dispatch;
    var __moduleName = context_4 && context_4.id;
    function read(rid, size) {
      util_ts_1.log("read");
      return dispatch.sendSync("op_read", { rid, size }).ok;
    }
    exports_4("read", read);
    return {
      setters: [
        function (util_ts_1_1) {
          util_ts_1 = util_ts_1_1;
        },
        function (dispatch_1) {
          dispatch = dispatch_1;
        },
      ],
      execute: function () {},
    };
  },
);
System.register(
  "internal:///deno_typescript/tests/main.ts",
  [
    "internal:///deno_typescript/tests/util.ts",
    "internal:///deno_typescript/tests/ops/fs.ts",
  ],
  function (exports_5, context_5) {
    "use strict";
    var util_ts_2, fs;
    var __moduleName = context_5 && context_5.id;
    function main() {
      globalThis.fs = fs;
      util_ts_2.log("hello world");
    }
    return {
      setters: [
        function (util_ts_2_1) {
          util_ts_2 = util_ts_2_1;
        },
        function (fs_1) {
          fs = fs_1;
        },
      ],
      execute: function () {},
    };
  },
);
//# sourceMappingURL=TEST_BUNDLE.js.map
