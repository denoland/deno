System.register(
  "$deno$/ops/dispatch_minimal.ts",
  [
    "$deno$/util.ts",
    "$deno$/core.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/errors.ts",
  ],
  function (exports_10, context_10) {
    "use strict";
    let util,
      core_ts_2,
      text_encoding_ts_2,
      errors_ts_1,
      promiseTableMin,
      _nextPromiseId,
      decoder,
      scratch32,
      scratchBytes;
    const __moduleName = context_10 && context_10.id;
    function nextPromiseId() {
      return _nextPromiseId++;
    }
    function recordFromBufMinimal(ui8) {
      const header = ui8.subarray(0, 12);
      const buf32 = new Int32Array(
        header.buffer,
        header.byteOffset,
        header.byteLength / 4
      );
      const promiseId = buf32[0];
      const arg = buf32[1];
      const result = buf32[2];
      let err;
      if (arg < 0) {
        const kind = result;
        const message = decoder.decode(ui8.subarray(12));
        err = { kind, message };
      } else if (ui8.length != 12) {
        throw new errors_ts_1.errors.InvalidData("BadMessage");
      }
      return {
        promiseId,
        arg,
        result,
        err,
      };
    }
    exports_10("recordFromBufMinimal", recordFromBufMinimal);
    function unwrapResponse(res) {
      if (res.err != null) {
        throw new (errors_ts_1.getErrorClass(res.err.kind))(res.err.message);
      }
      return res.result;
    }
    function asyncMsgFromRust(ui8) {
      const record = recordFromBufMinimal(ui8);
      const { promiseId } = record;
      const promise = promiseTableMin[promiseId];
      delete promiseTableMin[promiseId];
      util.assert(promise);
      promise.resolve(record);
    }
    exports_10("asyncMsgFromRust", asyncMsgFromRust);
    async function sendAsyncMinimal(opId, arg, zeroCopy) {
      const promiseId = nextPromiseId(); // AKA cmdId
      scratch32[0] = promiseId;
      scratch32[1] = arg;
      scratch32[2] = 0; // result
      const promise = util.createResolvable();
      const buf = core_ts_2.core.dispatch(opId, scratchBytes, zeroCopy);
      if (buf) {
        const record = recordFromBufMinimal(buf);
        // Sync result.
        promise.resolve(record);
      } else {
        // Async result.
        promiseTableMin[promiseId] = promise;
      }
      const res = await promise;
      return unwrapResponse(res);
    }
    exports_10("sendAsyncMinimal", sendAsyncMinimal);
    function sendSyncMinimal(opId, arg, zeroCopy) {
      scratch32[0] = 0; // promiseId 0 indicates sync
      scratch32[1] = arg;
      const res = core_ts_2.core.dispatch(opId, scratchBytes, zeroCopy);
      const resRecord = recordFromBufMinimal(res);
      return unwrapResponse(resRecord);
    }
    exports_10("sendSyncMinimal", sendSyncMinimal);
    return {
      setters: [
        function (util_1) {
          util = util_1;
        },
        function (core_ts_2_1) {
          core_ts_2 = core_ts_2_1;
        },
        function (text_encoding_ts_2_1) {
          text_encoding_ts_2 = text_encoding_ts_2_1;
        },
        function (errors_ts_1_1) {
          errors_ts_1 = errors_ts_1_1;
        },
      ],
      execute: function () {
        // Using an object without a prototype because `Map` was causing GC problems.
        promiseTableMin = Object.create(null);
        // Note it's important that promiseId starts at 1 instead of 0, because sync
        // messages are indicated with promiseId 0. If we ever add wrap around logic for
        // overflows, this should be taken into account.
        _nextPromiseId = 1;
        decoder = new text_encoding_ts_2.TextDecoder();
        scratch32 = new Int32Array(3);
        scratchBytes = new Uint8Array(
          scratch32.buffer,
          scratch32.byteOffset,
          scratch32.byteLength
        );
        util.assert(scratchBytes.byteLength === scratch32.length * 4);
      },
    };
  }
);
