// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
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
// Actual dispatch_json code begins here.
// Warning! The values in this enum are duplicated in dispatch_json.rs
// Update carefully!
let InternalErrorKinds;
(function (InternalErrorKinds) {
  InternalErrorKinds[(InternalErrorKinds["JsonIoError"] = 1)] = "JsonIoError";
  InternalErrorKinds[(InternalErrorKinds["JsonSyntaxError"] = 2)] =
    "JsonSyntaxError";
  InternalErrorKinds[(InternalErrorKinds["JsonDataError"] = 3)] =
    "JsonDataError";
  InternalErrorKinds[(InternalErrorKinds["JsonEofError"] = 4)] = "JsonEofError";
})(InternalErrorKinds || (InternalErrorKinds = {}));
export class InternalDispatchJsonError extends Error {
  constructor(kind, msg) {
    super(msg);
    this.name = InternalErrorKinds[kind];
  }
}
const core = Deno["core"];
function decode(ui8) {
  const s = core.decode(ui8);
  return JSON.parse(s);
}
function encode(args) {
  const s = JSON.stringify(args);
  return core.encode(s);
}
/** Json based dispatch wrapper for core ops.
 *
 * Error kind mapping is controlled by errorFactory. Async handler is automatically
 * set during construction.
 *
 *       const opId = Deno.ops()["json_op"];
 *       const jsonOp = new DispatchJsonOp(opId, (kind, msg) => return new CustomError(kind, msg));
 *       const response = jsonOp.dispatchSync({ data });
 *       console.log(response.items[3].name);
 */
export class DispatchJsonOp {
  constructor(opId, errorFactory) {
    this.opId = opId;
    this.errorFactory = errorFactory;
    this.promiseTable = new Map();
    this._nextPromiseId = 1;
  }
  nextPromiseId() {
    return this._nextPromiseId++;
  }
  unwrapResponse(res) {
    if (res.err != null) {
      if (res.err.kind < 0) {
        throw new InternalDispatchJsonError(
          res.err.kind * -1, // Transform kind back to positive
          res.err.message
        );
      } else if (res.err.kind > 0) {
        throw this.errorFactory(res.err.kind, res.err.message);
      } else {
        throw new Error(res.err.message);
      }
    }
    assert(res.ok != null);
    return res.ok;
  }
  handleAsync(resUi8) {
    const res = decode(resUi8);
    assert(res.promiseId != null);
    const promise = this.promiseTable.get(res.promiseId);
    assert(promise != null);
    this.promiseTable.delete(res.promiseId);
    promise.resolve(res);
  }
  dispatch(args = {}, zeroCopy) {
    const argsUi8 = encode(args);
    return core.dispatch(this.opId, argsUi8, zeroCopy);
  }
  // Dispatch this op with a Sync call
  dispatchSync(args = {}, zeroCopy) {
    const resUi8 = this.dispatch(args, zeroCopy);
    assert(resUi8 != null);
    const res = decode(resUi8);
    assert(res.promiseId == null);
    return this.unwrapResponse(res);
  }
  // Dispatch this op with a Async call
  async dispatchAsync(args = {}, zeroCopy) {
    const promiseId = this.nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = deferred();
    const buf = this.dispatch(args, zeroCopy);
    if (buf) {
      // Sync result.
      const res = decode(buf);
      promise.resolve(res);
    } else {
      // Async result.
      this.promiseTable.set(promiseId, promise);
    }
    const res = await promise;
    return this.unwrapResponse(res);
  }
}
