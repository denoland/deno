System.register(
  "$deno$/net.ts",
  [
    "$deno$/errors.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/resources.ts",
    "$deno$/ops/net.ts",
  ],
  function (exports_45, context_45) {
    "use strict";
    let errors_ts_4,
      io_ts_4,
      resources_ts_3,
      netOps,
      ConnImpl,
      ListenerImpl,
      DatagramImpl;
    const __moduleName = context_45 && context_45.id;
    function listen(options) {
      let res;
      if (options.transport === "unix" || options.transport === "unixpacket") {
        res = netOps.listen(options);
      } else {
        res = netOps.listen({
          transport: "tcp",
          hostname: "127.0.0.1",
          ...options,
        });
      }
      if (
        !options.transport ||
        options.transport === "tcp" ||
        options.transport === "unix"
      ) {
        return new ListenerImpl(res.rid, res.localAddr);
      } else {
        return new DatagramImpl(res.rid, res.localAddr);
      }
    }
    exports_45("listen", listen);
    async function connect(options) {
      let res;
      if (options.transport === "unix") {
        res = await netOps.connect(options);
      } else {
        res = await netOps.connect({
          transport: "tcp",
          hostname: "127.0.0.1",
          ...options,
        });
      }
      return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }
    exports_45("connect", connect);
    return {
      setters: [
        function (errors_ts_4_1) {
          errors_ts_4 = errors_ts_4_1;
        },
        function (io_ts_4_1) {
          io_ts_4 = io_ts_4_1;
        },
        function (resources_ts_3_1) {
          resources_ts_3 = resources_ts_3_1;
        },
        function (netOps_1) {
          netOps = netOps_1;
          exports_45({
            ShutdownMode: netOps_1["ShutdownMode"],
            shutdown: netOps_1["shutdown"],
          });
        },
      ],
      execute: function () {
        ConnImpl = class ConnImpl {
          constructor(rid, remoteAddr, localAddr) {
            this.rid = rid;
            this.remoteAddr = remoteAddr;
            this.localAddr = localAddr;
          }
          write(p) {
            return io_ts_4.write(this.rid, p);
          }
          read(p) {
            return io_ts_4.read(this.rid, p);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          closeRead() {
            netOps.shutdown(this.rid, netOps.ShutdownMode.Read);
          }
          closeWrite() {
            netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
          }
        };
        exports_45("ConnImpl", ConnImpl);
        ListenerImpl = class ListenerImpl {
          constructor(rid, addr) {
            this.rid = rid;
            this.addr = addr;
          }
          async accept() {
            const res = await netOps.accept(this.rid, this.addr.transport);
            return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          async *[Symbol.asyncIterator]() {
            while (true) {
              try {
                yield await this.accept();
              } catch (error) {
                if (error instanceof errors_ts_4.errors.BadResource) {
                  break;
                }
                throw error;
              }
            }
          }
        };
        exports_45("ListenerImpl", ListenerImpl);
        DatagramImpl = class DatagramImpl {
          constructor(rid, addr, bufSize = 1024) {
            this.rid = rid;
            this.addr = addr;
            this.bufSize = bufSize;
          }
          async receive(p) {
            const buf = p || new Uint8Array(this.bufSize);
            const { size, remoteAddr } = await netOps.receive(
              this.rid,
              this.addr.transport,
              buf
            );
            const sub = buf.subarray(0, size);
            return [sub, remoteAddr];
          }
          async send(p, addr) {
            const remote = { hostname: "127.0.0.1", transport: "udp", ...addr };
            const args = { ...remote, rid: this.rid };
            await netOps.send(args, p);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          async *[Symbol.asyncIterator]() {
            while (true) {
              try {
                yield await this.receive();
              } catch (error) {
                if (error instanceof errors_ts_4.errors.BadResource) {
                  break;
                }
                throw error;
              }
            }
          }
        };
        exports_45("DatagramImpl", DatagramImpl);
      },
    };
  }
);
