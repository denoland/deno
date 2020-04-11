// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

function assert(cond) {
  if (!cond) {
    throw Error("assert failed");
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

// eslint-disable-next-line @typescript-eslint/no-unused-vars
function createDispatchJson(core, errorFactory) {
  // Using an object without a prototype because `Map` was causing GC problems.
  const promiseTable = Object.create(null);
  let _nextPromiseId = 1;
  const OPS_CACHE = core.ops();

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

  function asyncMsgFromRust(resUi8) {
    const res = decode(resUi8);
    assert(res.promiseId != null);

    const promise = promiseTable[res.promiseId];
    assert(promise != null);
    delete promiseTable[res.promiseId];
    promise.resolve(res);
  }

  function sendSync(opName, args, zeroCopy) {
    const opId = OPS_CACHE[opName];
    const argsUi8 = encode(args);
    const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
    assert(resUi8 != null);

    const res = decode(resUi8);
    assert(res.promiseId == null);

    if (res.err) {
      return errorFactory(res.err);
    }
    assert(typeof res.ok !== "undefined");
    return res.ok;
  }

  async function sendAsync(opName, args, zeroCopy) {
    const opId = OPS_CACHE[opName];
    const promiseId = nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = createResolvable();

    const argsUi8 = encode(args);
    const buf = core.dispatch(opId, argsUi8, zeroCopy);
    if (buf) {
      // Sync result.
      const res = decode(buf);
      promise.resolve(res);
    } else {
      // Async result.
      promiseTable[promiseId] = promise;
    }
    const res = await promise;
    if (res.err) {
      return errorFactory(res.err);
    }
    assert(typeof res.ok !== "undefined");
    return res.ok;
  }

  return {
    sendSync,
    sendAsync,
    asyncMsgFromRust,
  };
}
