System.register(
  "$deno$/tls.ts",
  ["$deno$/ops/tls.ts", "$deno$/net.ts"],
  function (exports_63, context_63) {
    "use strict";
    let tlsOps, net_ts_1, TLSListenerImpl;
    const __moduleName = context_63 && context_63.id;
    async function connectTLS({
      port,
      hostname = "127.0.0.1",
      transport = "tcp",
      certFile = undefined,
    }) {
      const res = await tlsOps.connectTLS({
        port,
        hostname,
        transport,
        certFile,
      });
      return new net_ts_1.ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }
    exports_63("connectTLS", connectTLS);
    function listenTLS({
      port,
      certFile,
      keyFile,
      hostname = "0.0.0.0",
      transport = "tcp",
    }) {
      const res = tlsOps.listenTLS({
        port,
        certFile,
        keyFile,
        hostname,
        transport,
      });
      return new TLSListenerImpl(res.rid, res.localAddr);
    }
    exports_63("listenTLS", listenTLS);
    return {
      setters: [
        function (tlsOps_1) {
          tlsOps = tlsOps_1;
        },
        function (net_ts_1_1) {
          net_ts_1 = net_ts_1_1;
        },
      ],
      execute: function () {
        TLSListenerImpl = class TLSListenerImpl extends net_ts_1.ListenerImpl {
          async accept() {
            const res = await tlsOps.acceptTLS(this.rid);
            return new net_ts_1.ConnImpl(
              res.rid,
              res.remoteAddr,
              res.localAddr
            );
          }
        };
      },
    };
  }
);
