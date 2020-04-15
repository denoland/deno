// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/io.ts",
  ["$deno$/ops/dispatch_minimal.ts", "$deno$/io.ts", "$deno$/runtime.ts"],
  function (exports_29, context_29) {
    "use strict";
    let dispatch_minimal_ts_1, io_ts_2, runtime_ts_3, OP_READ, OP_WRITE;
    const __moduleName = context_29 && context_29.id;
    function readSync(rid, buffer) {
      if (buffer.length == 0) {
        return 0;
      }
      if (OP_READ < 0) {
        OP_READ = runtime_ts_3.OPS_CACHE["op_read"];
      }
      const nread = dispatch_minimal_ts_1.sendSyncMinimal(OP_READ, rid, buffer);
      if (nread < 0) {
        throw new Error("read error");
      } else if (nread == 0) {
        return io_ts_2.EOF;
      } else {
        return nread;
      }
    }
    exports_29("readSync", readSync);
    async function read(rid, buffer) {
      if (buffer.length == 0) {
        return 0;
      }
      if (OP_READ < 0) {
        OP_READ = runtime_ts_3.OPS_CACHE["op_read"];
      }
      const nread = await dispatch_minimal_ts_1.sendAsyncMinimal(
        OP_READ,
        rid,
        buffer
      );
      if (nread < 0) {
        throw new Error("read error");
      } else if (nread == 0) {
        return io_ts_2.EOF;
      } else {
        return nread;
      }
    }
    exports_29("read", read);
    function writeSync(rid, data) {
      if (OP_WRITE < 0) {
        OP_WRITE = runtime_ts_3.OPS_CACHE["op_write"];
      }
      const result = dispatch_minimal_ts_1.sendSyncMinimal(OP_WRITE, rid, data);
      if (result < 0) {
        throw new Error("write error");
      } else {
        return result;
      }
    }
    exports_29("writeSync", writeSync);
    async function write(rid, data) {
      if (OP_WRITE < 0) {
        OP_WRITE = runtime_ts_3.OPS_CACHE["op_write"];
      }
      const result = await dispatch_minimal_ts_1.sendAsyncMinimal(
        OP_WRITE,
        rid,
        data
      );
      if (result < 0) {
        throw new Error("write error");
      } else {
        return result;
      }
    }
    exports_29("write", write);
    return {
      setters: [
        function (dispatch_minimal_ts_1_1) {
          dispatch_minimal_ts_1 = dispatch_minimal_ts_1_1;
        },
        function (io_ts_2_1) {
          io_ts_2 = io_ts_2_1;
        },
        function (runtime_ts_3_1) {
          runtime_ts_3 = runtime_ts_3_1;
        },
      ],
      execute: function () {
        // This is done because read/write are extremely performance sensitive.
        OP_READ = -1;
        OP_WRITE = -1;
      },
    };
  }
);
