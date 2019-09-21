// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as util from "./util.ts";
import { TextEncoder, TextDecoder } from "./text_encoding.ts";
import { core } from "./core.ts";
import { ErrorKind, DenoError } from "./errors.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Ok = any;

interface JsonError {
  kind: ErrorKind;
  message: string;
}

interface JsonResponse {
  ok?: Ok;
  err?: JsonError;
  promiseId?: number; // Only present in async messages.
}

core.print("JSON op");
export class JsonOp extends core.Op {
  public opId!: number;

  constructor(public name: string) {
    super(name);
    core.print("registering op " + name + "\n", true);
  }

  static asyncMsgFromRust(opId: number, ui8: Uint8Array): void {
    const res = decode(ui8);
    util.assert(res.promiseId != null);

    const promise = promiseTable.get(res.promiseId!);
    util.assert(promise != null);
    promiseTable.delete(res.promiseId!);
    promise!.resolve(res);
  }

  static sendSync(opId: number, args: object = {}, zeroCopy?: Uint8Array): Ok {
    const argsUi8 = encode(args);
    const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
    util.assert(resUi8 != null);

    const res = decode(resUi8!);
    util.assert(res.promiseId == null);
    return unwrapResponse(res);
  }

  static async sendAsync(
    opId: number,
    args: object = {},
    zeroCopy?: Uint8Array
  ): Promise<Ok> {
    const promiseId = nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = util.createResolvable<Ok>();
    promiseTable.set(promiseId, promise);

    const argsUi8 = encode(args);
    const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
    util.assert(resUi8 == null);

    const res = await promise;
    return unwrapResponse(res);
  }

  sendSync(args: object = {}, zeroCopy?: Uint8Array): Ok {
    return JsonOp.sendSync(this.opId, args, zeroCopy);
  }

  sendAsync(args: object = {}, zeroCopy?: Uint8Array): Promise<Ok> {
    return JsonOp.sendAsync(this.opId, args, zeroCopy);
  }
}

const promiseTable = new Map<number, util.Resolvable<JsonResponse>>();
let _nextPromiseId = 1;

function nextPromiseId(): number {
  return _nextPromiseId++;
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
  core.print("json response" + JSON.stringify(res));
  if (res.err != null) {
    throw new DenoError(res.err!.kind, res.err!.message);
  }
  util.assert(res.ok != null);
  return res.ok!;
}

export function asyncMsgFromRust(opId: number, resUi8: Uint8Array): void {
  const res = decode(resUi8);
  util.assert(res.promiseId != null);

  const promise = promiseTable.get(res.promiseId!);
  util.assert(promise != null);
  promiseTable.delete(res.promiseId!);
  promise!.resolve(res);
}
