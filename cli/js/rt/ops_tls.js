System.register("$deno$/ops/tls.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_62,
  context_62
) {
  "use strict";
  let dispatch_json_ts_29;
  const __moduleName = context_62 && context_62.id;
  function connectTLS(args) {
    return dispatch_json_ts_29.sendAsync("op_connect_tls", args);
  }
  exports_62("connectTLS", connectTLS);
  function acceptTLS(rid) {
    return dispatch_json_ts_29.sendAsync("op_accept_tls", { rid });
  }
  exports_62("acceptTLS", acceptTLS);
  function listenTLS(args) {
    return dispatch_json_ts_29.sendSync("op_listen_tls", args);
  }
  exports_62("listenTLS", listenTLS);
  return {
    setters: [
      function (dispatch_json_ts_29_1) {
        dispatch_json_ts_29 = dispatch_json_ts_29_1;
      },
    ],
    execute: function () {},
  };
});
