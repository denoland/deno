System.register(
  "$deno$/ops/dispatch_json.ts",
  ["$deno$/util.ts", "$deno$/core.ts", "$deno$/runtime.ts", "$deno$/errors.ts"],
  function (exports_22, context_22) {
    "use strict";
    let util,
      core_ts_4,
      runtime_ts_2,
      errors_ts_3,
      promiseTable,
      _nextPromiseId;
    const __moduleName = context_22 && context_22.id;
    function nextPromiseId() {
      return _nextPromiseId++;
    }
    function decode(ui8) {
      const s = core_ts_4.core.decode(ui8);
      return JSON.parse(s);
    }
    function encode(args) {
      const s = JSON.stringify(args);
      return core_ts_4.core.encode(s);
    }
    function unwrapResponse(res) {
      if (res.err != null) {
        throw new (errors_ts_3.getErrorClass(res.err.kind))(res.err.message);
      }
      util.assert(res.ok != null);
      return res.ok;
    }
    function asyncMsgFromRust(resUi8) {
      const res = decode(resUi8);
      util.assert(res.promiseId != null);
      const promise = promiseTable[res.promiseId];
      util.assert(promise != null);
      delete promiseTable[res.promiseId];
      promise.resolve(res);
    }
    exports_22("asyncMsgFromRust", asyncMsgFromRust);
    function sendSync(opName, args = {}, zeroCopy) {
      const opId = runtime_ts_2.OPS_CACHE[opName];
      util.log("sendSync", opName, opId);
      const argsUi8 = encode(args);
      const resUi8 = core_ts_4.core.dispatch(opId, argsUi8, zeroCopy);
      util.assert(resUi8 != null);
      const res = decode(resUi8);
      util.assert(res.promiseId == null);
      return unwrapResponse(res);
    }
    exports_22("sendSync", sendSync);
    async function sendAsync(opName, args = {}, zeroCopy) {
      const opId = runtime_ts_2.OPS_CACHE[opName];
      util.log("sendAsync", opName, opId);
      const promiseId = nextPromiseId();
      args = Object.assign(args, { promiseId });
      const promise = util.createResolvable();
      const argsUi8 = encode(args);
      const buf = core_ts_4.core.dispatch(opId, argsUi8, zeroCopy);
      if (buf) {
        // Sync result.
        const res = decode(buf);
        promise.resolve(res);
      } else {
        // Async result.
        promiseTable[promiseId] = promise;
      }
      const res = await promise;
      return unwrapResponse(res);
    }
    exports_22("sendAsync", sendAsync);
    return {
      setters: [
        function (util_3) {
          util = util_3;
        },
        function (core_ts_4_1) {
          core_ts_4 = core_ts_4_1;
        },
        function (runtime_ts_2_1) {
          runtime_ts_2 = runtime_ts_2_1;
        },
        function (errors_ts_3_1) {
          errors_ts_3 = errors_ts_3_1;
        },
      ],
      execute: function () {
        // Using an object without a prototype because `Map` was causing GC problems.
        promiseTable = Object.create(null);
        _nextPromiseId = 1;
      },
    };
  }
);
