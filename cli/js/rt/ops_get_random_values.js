System.register(
  "$deno$/ops/get_random_values.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts"],
  function (exports_72, context_72) {
    "use strict";
    let dispatch_json_ts_34, util_ts_9;
    const __moduleName = context_72 && context_72.id;
    function getRandomValues(typedArray) {
      util_ts_9.assert(typedArray !== null, "Input must not be null");
      util_ts_9.assert(
        typedArray.length <= 65536,
        "Input must not be longer than 65536"
      );
      const ui8 = new Uint8Array(
        typedArray.buffer,
        typedArray.byteOffset,
        typedArray.byteLength
      );
      dispatch_json_ts_34.sendSync("op_get_random_values", {}, ui8);
      return typedArray;
    }
    exports_72("getRandomValues", getRandomValues);
    return {
      setters: [
        function (dispatch_json_ts_34_1) {
          dispatch_json_ts_34 = dispatch_json_ts_34_1;
        },
        function (util_ts_9_1) {
          util_ts_9 = util_ts_9_1;
        },
      ],
      execute: function () {},
    };
  }
);
