// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { Deferred, deferred } from "../../util/async.ts";

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

export class DispatchJsonPluginOp {
  private readonly promiseTable = new Map<number, Deferred<JsonResponse>>();
  private _nextPromiseId = 1;

  constructor(private readonly pluginOp: Deno.PluginOp) {
    pluginOp.setAsyncHandler(resUi8 => this.handleAsync(resUi8));
  }

  private nextPromiseId(): number {
    return this._nextPromiseId++;
  }

  private handleAsync(resUi8: Uint8Array): void {
    const res = decode(resUi8);
    assert(res.promiseId != null);

    const promise = this.promiseTable.get(res.promiseId!);
    assert(promise != null);
    this.promiseTable.delete(res.promiseId!);
    promise.resolve(res);
  }

  dispatchSync(args: object = {}, zeroCopy?: Uint8Array): Ok {
    const argsUi8 = encode(args);
    const resUi8 = this.pluginOp.dispatch(argsUi8, zeroCopy);
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
    const buf = this.pluginOp.dispatch(argsUi8, zeroCopy);
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
