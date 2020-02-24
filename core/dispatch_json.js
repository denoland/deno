// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// These utilities are borrowed from std. We don't really
// have a better way to include them here yet.
class AssertionError extends Error {
  constructor(message) {
    super(message);
    this.name = "AssertionError";
  }
}

/** Make an assertion, if not `true`, then throw. */
function assert(expr, msg = "") {
  if (!expr) {
    throw new AssertionError(msg);
  }
}

/** Creates a Promise with the `reject` and `resolve` functions
 * placed as methods on the promise object itself. It allows you to do:
 *
 *     const p = deferred<number>();
 *     // ...
 *     p.resolve(42);
 */
function deferred() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods);
}

function decode(ui8) {
  const s = new TextDecoder().decode(ui8);
  return JSON.parse(s);
}

function encode(args) {
  const s = JSON.stringify(args);
  return new TextEncoder().encode(s);
}

function unwrapResponse(res) {
  if (res.err != null) {
    throw new Error(res.err.message);
  }
  assert(res.ok != null);
  return res.ok;
}

class DispatchJsonOp {
  constructor(dispatch) {
    this.dispatch = dispatch;
    this.promiseTable = new Map();
    this._nextPromiseId = 1;
  }

  nextPromiseId() {
    return this._nextPromiseId++;
  }

  handleAsync(resUi8) {
    const res = decode(resUi8);
    assert(res.promiseId != null);
    const promise = this.promiseTable.get(res.promiseId);
    assert(promise != null);
    this.promiseTable.delete(res.promiseId);
    promise.resolve(res);
  }

  dispatchSync(args = {}, zeroCopy) {
    const argsUi8 = encode(args);
    const resUi8 = this.dispatch(argsUi8, zeroCopy);
    assert(resUi8 != null);
    const res = decode(resUi8);
    assert(res.promiseId == null);
    return unwrapResponse(res);
  }

  async dispatchAsync(args = {}, zeroCopy) {
    const promiseId = this.nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = deferred();
    const argsUi8 = encode(args);
    const buf = this.dispatch(argsUi8, zeroCopy);
    if (buf) {
      // Sync result.
      const res = decode(buf);
      promise.resolve(res);
    } else {
      // Async result.
      this.promiseTable.set(promiseId, promise);
    }
    const res = await promise;
    return unwrapResponse(res);
  }
}

export class DispatchJsonCoreOp extends DispatchJsonOp {
  constructor(opId) {
    super((c, zc) => Deno["core"].dispatch(this.opId, c, zc));
    this.opId = opId;
    Deno["core"].setAsyncHandler(this.opId, resUi8 => this.handleAsync(resUi8));
  }
}

export class DispatchJsonPluginOp extends DispatchJsonOp {
  constructor(pluginOp) {
    super((c, zc) => this.pluginOp.dispatch(c, zc));
    this.pluginOp = pluginOp;
    this.pluginOp.setAsyncHandler(resUi8 => this.handleAsync(resUi8));
  }
}
