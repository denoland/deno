// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const util = window.__bootstrap.util;
  // Using an object without a prototype because `Map` was causing GC problems.
  const promiseTable = Object.create(null);
  let _nextPromiseId = 1;

  function nextPromiseId() {
    return _nextPromiseId++;
  }

  function decode(ui8) {
    return JSON.parse(core.decode(ui8));
  }

  function encode(args) {
    return core.encode(JSON.stringify(args));
  }

  function unwrapResponse(res) {
    if (res.err != null) {
      throw new (core.getErrorClass(res.err.kind))(res.err.message);
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

  function sendSync(
    opName,
    args = {},
    ...zeroCopy
  ) {
    util.log("sendSync", opName);
    const argsUi8 = encode(args);
    const resUi8 = core.dispatchByName(opName, argsUi8, ...zeroCopy);
    util.assert(resUi8 != null);
    const res = decode(resUi8);
    util.assert(res.promiseId == null);
    return unwrapResponse(res);
  }

  async function sendAsync(
    opName,
    args = {},
    ...zeroCopy
  ) {
    util.log("sendAsync", opName);
    const promiseId = nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = util.createResolvable();
    const argsUi8 = encode(args);
    const buf = core.dispatchByName(opName, argsUi8, ...zeroCopy);
    if (buf != null) {
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

  window.__bootstrap.dispatchJson = {
    asyncMsgFromRust,
    sendSync,
    sendAsync,
  };
})(this);
