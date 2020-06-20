// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  // Available on start due to bindings.
  const core = window.Deno.core;

  let errorCallback;

  function assert(cond) {
    if (!cond) {
      throw Error("assert");
    }
  }

  function createResolvable() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    promise.resolve = resolve;
    promise.reject = reject;
    return promise;
  }

  // Using an object without a prototype because `Map` was causing GC problems.
  const promiseTable = Object.create(null);
  let _nextPromiseId = 1;

  function nextPromiseId() {
    return _nextPromiseId++;
  }

  function decode(ui8) {
    const s = core.decode(ui8);
    return JSON.parse(s);
  }

  function encode(args) {
    const s = JSON.stringify(args);
    return core.encode(s);
  }

  function setErrorCb(errorCb) {
    errorCallback = errorCb;
  }

  function unwrapResponse(res) {
    if (res.err != null) {
      throw new (errorCallback(res.err.kind))(res.err.message);
    }
    assert(res.ok != null);
    return res.ok;
  }

  function asyncMsgFromRust(resUi8) {
    const res = decode(resUi8);
    assert(res.promiseId != null);

    const promise = promiseTable[res.promiseId];
    assert(promise != null);
    delete promiseTable[res.promiseId];
    promise.resolve(res);
  }

  function sendSync(opName, args = {}, ...zeroCopy) {
    const argsUi8 = encode(args);
    const resUi8 = core.dispatchByName(opName, argsUi8, ...zeroCopy);
    assert(resUi8 != null);
    const res = decode(resUi8);
    assert(res.promiseId == null);
    return unwrapResponse(res);
  }

  async function sendAsync(opName, args = {}, ...zeroCopy) {
    const promiseId = nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = createResolvable();
    const argsUi8 = encode(args);
    const buf = core.dispatchByName(opName, argsUi8, ...zeroCopy);
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

  Object.assign(window.Deno.core, {
    dispatchJson: {
      sendAsync,
      sendSync,
      asyncMsgFromRust,
      setErrorCb,
    },
  });
})(this);
