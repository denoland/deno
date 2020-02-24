// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// These utilities are borrowed from std. We don't really
// have a better way to include them here yet.

class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AssertionError";
  }
}

/** Make an assertion, if not `true`, then throw. */
function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}

// TODO(ry) It'd be better to make Deferred a class that inherits from
// Promise, rather than an interface. This is possible in ES2016, however
// typescript produces broken code when targeting ES5 code.
// See https://github.com/Microsoft/TypeScript/issues/15202
// At the time of writing, the github issue is closed but the problem remains.
interface Deferred<T> extends Promise<T> {
  resolve: (value?: T | PromiseLike<T>) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  reject: (reason?: any) => void;
}

/** Creates a Promise with the `reject` and `resolve` functions
 * placed as methods on the promise object itself. It allows you to do:
 *
 *     const p = deferred<number>();
 *     // ...
 *     p.resolve(42);
 */
function deferred<T>(): Deferred<T> {
  let methods;
  const promise = new Promise<T>((resolve, reject): void => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods)! as Deferred<T>;
}

// Actual dispatch_json code begins here.

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Ok = any;

interface JsonError {
  message: string;
}

interface JsonResponse {
  ok?: Ok;
  err?: JsonError;
  promiseId?: number; // Only present in async messages.
}

function decode(ui8: Uint8Array): JsonResponse {
  const s = new TextDecoder().decode(ui8);
  return JSON.parse(s) as JsonResponse;
}

function encode(args: object): Uint8Array {
  const s = JSON.stringify(args);
  return new TextEncoder().encode(s);
}

function unwrapResponse(res: JsonResponse): Ok {
  if (res.err != null) {
    throw new Error(res.err!.message);
  }
  assert(res.ok != null);
  return res.ok;
}

abstract class DispatchJsonOp {
  protected readonly promiseTable = new Map<number, Deferred<JsonResponse>>();
  protected _nextPromiseId = 1;

  constructor(
    private readonly dispatch: (
      control: Uint8Array,
      zeroCopy?: ArrayBufferView | null
    ) => Uint8Array | null
  ) {}

  protected nextPromiseId(): number {
    return this._nextPromiseId++;
  }

  protected handleAsync(resUi8: Uint8Array): void {
    const res = decode(resUi8);
    assert(res.promiseId != null);

    const promise = this.promiseTable.get(res.promiseId!);
    assert(promise != null);
    this.promiseTable.delete(res.promiseId!);
    promise.resolve(res);
  }

  dispatchSync(args: object = {}, zeroCopy?: Uint8Array): Ok {
    const argsUi8 = encode(args);
    const resUi8 = this.dispatch(argsUi8, zeroCopy);
    assert(resUi8 != null);

    const res = decode(resUi8!);
    assert(res.promiseId == null);
    return unwrapResponse(res);
  }

  async dispatchAsync(args: object = {}, zeroCopy?: Uint8Array): Promise<Ok> {
    const promiseId = this.nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = deferred<Ok>();

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
  constructor(private readonly opId: number) {
    super((c, zc) => Deno["core"].dispatch(this.opId, c, zc));
    Deno["core"].setAsyncHandler(this.opId, resUi8 => this.handleAsync(resUi8));
  }
}

export class DispatchJsonPluginOp extends DispatchJsonOp {
  constructor(private readonly pluginOp: Deno.PluginOp) {
    super((c, zc) => this.pluginOp.dispatch(c, zc));
    this.pluginOp.setAsyncHandler(resUi8 => this.handleAsync(resUi8));
  }
}
