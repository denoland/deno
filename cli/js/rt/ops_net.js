System.register("$deno$/ops/net.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_44,
  context_44
) {
  "use strict";
  let dispatch_json_ts_17, ShutdownMode;
  const __moduleName = context_44 && context_44.id;
  function shutdown(rid, how) {
    dispatch_json_ts_17.sendSync("op_shutdown", { rid, how });
  }
  exports_44("shutdown", shutdown);
  function accept(rid, transport) {
    return dispatch_json_ts_17.sendAsync("op_accept", { rid, transport });
  }
  exports_44("accept", accept);
  function listen(args) {
    return dispatch_json_ts_17.sendSync("op_listen", args);
  }
  exports_44("listen", listen);
  function connect(args) {
    return dispatch_json_ts_17.sendAsync("op_connect", args);
  }
  exports_44("connect", connect);
  function receive(rid, transport, zeroCopy) {
    return dispatch_json_ts_17.sendAsync(
      "op_receive",
      { rid, transport },
      zeroCopy
    );
  }
  exports_44("receive", receive);
  async function send(args, zeroCopy) {
    await dispatch_json_ts_17.sendAsync("op_send", args, zeroCopy);
  }
  exports_44("send", send);
  return {
    setters: [
      function (dispatch_json_ts_17_1) {
        dispatch_json_ts_17 = dispatch_json_ts_17_1;
      },
    ],
    execute: function () {
      (function (ShutdownMode) {
        // See http://man7.org/linux/man-pages/man2/shutdown.2.html
        // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
        ShutdownMode[(ShutdownMode["Read"] = 0)] = "Read";
        ShutdownMode[(ShutdownMode["Write"] = 1)] = "Write";
        ShutdownMode[(ShutdownMode["ReadWrite"] = 2)] = "ReadWrite";
      })(ShutdownMode || (ShutdownMode = {}));
      exports_44("ShutdownMode", ShutdownMode);
    },
  };
});
